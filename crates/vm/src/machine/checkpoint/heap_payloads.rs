//! Validates VM-owned payloads against the restored managed heap.

use super::super::{
    Allocation, ArrayKind, EmuError, HeapState, ImmutableImagePixels, Program, vm_error,
};

impl HeapState {
    pub(in crate::machine) fn validate_checkpoint_payloads(
        &self,
        program: &Program,
    ) -> Result<(), EmuError> {
        let strings_valid = self.string_values.validate_checkpoint(|handle, units| {
            let copies = if self.interned_strings.get(units) == Some(&handle) { 2 } else { 1 };
            matches!(self.managed.get(handle), Ok(Allocation::Object { class, .. }) if class.as_ref() == "java/lang/String")
                && units.len().checked_mul(std::mem::size_of::<u16>() * copies)
                    .is_some_and(|bytes| self.managed.validate_checkpoint_external_bytes(handle, bytes).is_ok())
        });
        if !strings_valid
            || self
                .interned_strings
                .iter()
                .any(|(units, handle)| self.string_values.get(handle) != Some(units))
        {
            return Err(invalid_payload());
        }
        for (&handle, pixels) in &self.immutable_image_pixels {
            let Ok(Allocation::Array {
                kind: ArrayKind::Int,
                elements,
            }) = self.managed.get(handle)
            else {
                return Err(invalid_payload());
            };
            // Native images use an empty token array; cached Java images keep
            // their original int[] with a matching number of pixels.
            if !elements.is_empty() && elements.len() != pixels.len() {
                return Err(invalid_payload());
            }
            if let ImmutableImagePixels::Indexed8 { palette, indices } = pixels.as_ref()
                && (palette.len() > 256
                    || indices
                        .iter()
                        .any(|&index| usize::from(index) >= palette.len()))
            {
                return Err(invalid_payload());
            }
            let bytes = pixels.storage_bytes().ok_or_else(invalid_payload)?;
            self.managed
                .validate_checkpoint_external_bytes(handle, bytes)
                .map_err(|_| invalid_payload())?;
        }
        for (&handle, frames) in &self.throwable_traces {
            self.validate_throwable(program, handle)
                .map_err(|_| invalid_payload())?;
            self.managed
                .validate_checkpoint_external_bytes(
                    handle,
                    std::mem::size_of_val(frames.as_slice()),
                )
                .map_err(|_| invalid_payload())?;
        }
        // These tables are weak metadata during GC, so append_roots purposely
        // does not keep their owners alive. At restore their references must
        // still name allocations from the saved heap.
        for handle in self
            .weak_references
            .iter()
            .flat_map(|(owner, referent)| std::iter::once(owner).chain(referent.as_ref()))
            .chain(&self.managed_heap_limit_throwables)
        {
            self.managed.get(*handle).map_err(|_| invalid_payload())?;
        }
        Ok(())
    }
}

fn invalid_payload() -> EmuError {
    vm_error(
        "checkpoint-heap-payload",
        "The checkpoint contains invalid native Java heap data.",
    )
}
