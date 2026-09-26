//! Shared type/field names are host metadata, separate from CLDC object sizes.

use super::HeapError;
use std::collections::BTreeSet;
use std::sync::Arc;

const MAX_NAME_BYTES: usize = 64 * 1024 * 1024;
const NAME_OVERHEAD: usize = 64;

#[derive(Debug)]
pub(super) struct SharedNames {
    // Ordered comparisons avoid hashing every byte of long Java names on
    // each allocation. Linked field names already share the program's Arc.
    values: BTreeSet<Arc<str>>,
    bytes: usize,
    limit: usize,
}

impl Default for SharedNames {
    fn default() -> Self {
        Self {
            values: BTreeSet::new(),
            bytes: 0,
            limit: MAX_NAME_BYTES,
        }
    }
}

impl SharedNames {
    pub(super) fn intern(&mut self, name: &str) -> Result<Arc<str>, HeapError> {
        if let Some(shared) = self.values.get(name) {
            return Ok(Arc::clone(shared));
        }
        let bytes = name
            .len()
            .checked_add(NAME_OVERHEAD)
            .and_then(|bytes| self.bytes.checked_add(bytes))
            .filter(|bytes| *bytes <= self.limit)
            .ok_or(HeapError::MetadataLimitExceeded)?;
        let shared: Arc<str> = name.into();
        self.values.insert(Arc::clone(&shared));
        self.bytes = bytes;
        Ok(shared)
    }

    pub(super) fn sweep(&mut self) {
        self.values.retain(|name| {
            if Arc::strong_count(name) > 1 {
                true
            } else {
                self.bytes -= name.len() + NAME_OVERHEAD;
                false
            }
        });
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/heap/names.rs"]
mod tests;
