//! Admission of saved GL resources against the restored Java heap and profile.

use super::super::{Allocation, ArrayKind, EmuError, HeapState, HeapValue, Limits, vm_error};
use super::{Jsr239FloatPointer, Jsr239State};

impl Jsr239State {
    pub(in crate::machine) fn validate_checkpoint(
        &self,
        heap: &HeapState,
        limits: &Limits,
    ) -> Result<(), EmuError> {
        if !matches!(self.matrix_mode, 0x1700 | 0x1701)
            || self.model_view_stack.len() > 64
            || self.textures.len() > limits.m3g_arena.objects
            || self.textures.len() > u32::MAX as usize
            || (self.bound_texture != 0 && !self.textures.contains_key(&self.bound_texture))
        {
            return Err(invalid_checkpoint());
        }
        let mut bytes = 0_usize;
        for texture in self.textures.values() {
            bytes = bytes
                .checked_add(texture.validate_checkpoint(limits)?)
                .filter(|bytes| *bytes <= limits.m3g_arena.bytes)
                .ok_or_else(invalid_checkpoint)?;
        }
        if bytes != self.texture_bytes {
            return Err(invalid_checkpoint());
        }
        for pointer in [&self.vertex_pointer, &self.texture_pointer]
            .into_iter()
            .flatten()
        {
            pointer.validate_checkpoint(heap)?;
        }
        Ok(())
    }
}

impl Jsr239FloatPointer {
    fn validate_checkpoint(&self, heap: &HeapState) -> Result<(), EmuError> {
        if !matches!(heap.managed.get(self.buffer), Ok(Allocation::Object { class, .. })
            if class.as_ref() == "java/nio/FloatBuffer")
        {
            return Err(invalid_checkpoint());
        }
        let integer_field = |name| match heap.managed.field(self.buffer, name) {
            Ok(HeapValue::Int(value)) => usize::try_from(value).map_err(|_| invalid_checkpoint()),
            _ => Err(invalid_checkpoint()),
        };
        let offset = integer_field("java/nio/Buffer.byteOffset:I")?;
        let capacity = integer_field("java/nio/Buffer.capacity:I")?;
        let end = capacity
            .checked_mul(4)
            .and_then(|bytes| offset.checked_add(bytes));
        let Ok(HeapValue::Reference(Some(array))) =
            heap.managed.field(self.buffer, "java/nio/Buffer.bytes:[B")
        else {
            return Err(invalid_checkpoint());
        };
        let Ok(Allocation::Array {
            kind: ArrayKind::Byte,
            elements,
        }) = heap.managed.get(array)
        else {
            return Err(invalid_checkpoint());
        };
        // A FloatBuffer can start at any byte offset. GL retains the position at
        // binding time, independently of later changes to Buffer.position.
        if self.bytes.start < offset
            || self.bytes.start > self.bytes.end
            || Some(self.bytes.end) != end
            || self.bytes.end > elements.len()
            || !self.bytes.len().is_multiple_of(4)
        {
            return Err(invalid_checkpoint());
        }
        Ok(())
    }
}

pub(super) fn invalid_checkpoint() -> EmuError {
    vm_error(
        "checkpoint-jsr239",
        "The checkpoint contains invalid OpenGL ES resources.",
    )
}
