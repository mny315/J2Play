//! Validation and relinking of a decoded managed heap.

use super::{
    Allocation, ArrayKind, FieldToken, Handle, HashSet, Heap, HeapError, HeapValue, object_bytes,
    reference,
};

impl Heap {
    /// Validates a checkpoint's allocator metadata, references and profile budget,
    /// then reconnects field identities to the freshly linked program.
    ///
    /// # Errors
    /// Returns an error for an invalid allocator, allocation, reference or field.
    pub fn validate_checkpoint(
        &mut self,
        limit: usize,
        mut field_token: impl FnMut(&str, &str, super::ValueKind) -> Option<FieldToken>,
    ) -> Result<(), HeapError> {
        if self.limit != limit || self.slots.len() > limit / 8 + 1 {
            return Err(HeapError::LimitExceeded);
        }
        if self.free.len() > self.slots.len() {
            return Err(HeapError::InvalidHandle);
        }
        let mut free = vec![false; self.slots.len()];
        for index in &self.free {
            let listed = free
                .get_mut(*index as usize)
                .ok_or(HeapError::InvalidHandle)?;
            if std::mem::replace(listed, true) {
                return Err(HeapError::InvalidHandle);
            }
        }
        let mut bytes = 0_usize;
        let mut objects = 0_usize;
        for (slot, free) in self.slots.iter_mut().zip(free) {
            let Some(allocation) = &mut slot.allocation else {
                if slot.bytes != 0
                    || slot.external_bytes != 0
                    || slot.generation == 0
                    || (!free && slot.generation != u32::MAX)
                {
                    return Err(HeapError::InvalidHandle);
                }
                continue;
            };
            if free {
                return Err(HeapError::InvalidHandle);
            }
            let base = match allocation {
                Allocation::Object { class, fields } => {
                    *class = self.names.intern(class)?;
                    let mut names = HashSet::new();
                    for (name, token, value) in &mut fields.entries {
                        *name = self.names.intern(name)?;
                        if !names.insert(name.clone()) {
                            return Err(HeapError::TypeMismatch);
                        }
                        *token = field_token(class, name, value.kind())
                            .ok_or(HeapError::TypeMismatch)?;
                    }
                    object_bytes(fields.values())?
                }
                Allocation::Array { kind, elements } => {
                    if let ArrayKind::Reference(component) = kind {
                        *component = self.names.intern(component)?;
                    }
                    for value in elements.iter() {
                        // Integer storage must already obey the component width.
                        // Float values retain every bit pattern, including NaN.
                        if let HeapValue::Int(normalized) =
                            super::normalize(kind, *value).ok_or(HeapError::TypeMismatch)?
                            && *value != HeapValue::Int(normalized)
                        {
                            return Err(HeapError::TypeMismatch);
                        }
                    }
                    kind.allocation_bytes(elements.len())?
                }
            };
            if base.checked_add(slot.external_bytes) != Some(slot.bytes) {
                return Err(HeapError::TypeMismatch);
            }
            objects += 1;
            bytes = bytes
                .checked_add(slot.bytes)
                .ok_or(HeapError::LimitExceeded)?;
        }
        if bytes != self.bytes
            || bytes > limit
            || objects != self.objects
            || self.peak_bytes < bytes
            || self.peak_bytes > limit
            || self.peak_objects < objects
            || self.peak_objects > self.slots.len()
        {
            return Err(HeapError::LimitExceeded);
        }
        for slot in &self.slots {
            match &slot.allocation {
                Some(Allocation::Object { fields, .. }) => {
                    for handle in fields.values().filter_map(reference) {
                        self.get(handle)?;
                    }
                }
                Some(Allocation::Array {
                    kind: ArrayKind::Reference(_),
                    elements,
                }) => {
                    for handle in elements.iter().filter_map(reference) {
                        self.get(handle)?;
                    }
                }
                Some(Allocation::Array { .. }) | None => {}
            }
        }
        Ok(())
    }

    /// Validates an external root against the restored allocator.
    ///
    /// # Errors
    /// Returns an error for a stale or missing reference.
    pub fn validate_checkpoint_root(&self, handle: Handle) -> Result<(), HeapError> {
        self.get(handle).map(|_| ())
    }

    /// Checks that a restored native payload has exactly its recorded charge.
    ///
    /// # Errors
    /// Returns an error for a stale handle or inconsistent external storage.
    pub fn validate_checkpoint_external_bytes(
        &self,
        handle: Handle,
        expected: usize,
    ) -> Result<(), HeapError> {
        self.get(handle)?;
        if self.slots[handle.slot as usize].external_bytes != expected {
            return Err(HeapError::TypeMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/heap/checkpoint.rs"]
mod tests;
