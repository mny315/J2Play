//! Authoritative UTF-16 payloads, indexed by bounded managed-heap slots.
use super::{Handle, HashMap};

// At most 1 MiB of dense entries on 64-bit hosts. Sparse/high slots keep the
// existing hash table representation; guest handles cannot grow this Vec
// without bound. Entries retain the complete generation-bearing identity.
const DENSE_SLOTS: usize = 32_768;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct StringValues {
    dense: Vec<Option<(Handle, Vec<u16>)>>,
    overflow: HashMap<Handle, Vec<u16>>,
}

impl StringValues {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn validate_checkpoint(
        &self,
        mut validate: impl FnMut(Handle, &[u16]) -> bool,
    ) -> bool {
        if self.dense.len() > DENSE_SLOTS {
            return false;
        }
        for (slot, entry) in self.dense.iter().enumerate() {
            if let Some((handle, units)) = entry
                && ((handle.to_raw() >> 32) as usize != slot || !validate(*handle, units))
            {
                return false;
            }
        }
        self.overflow.iter().all(|(handle, units)| {
            let slot = (handle.to_raw() >> 32) as usize;
            !self
                .dense
                .get(slot)
                .and_then(Option::as_ref)
                .is_some_and(|(stored, _)| stored == handle)
                && validate(*handle, units)
        })
    }

    // Retain the borrowed-key API of the authoritative map this replaces.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[inline]
    pub(super) fn get(&self, handle: &Handle) -> Option<&Vec<u16>> {
        let slot = (handle.to_raw() >> 32) as usize;
        if let Some(Some((stored, units))) = self.dense.get(slot)
            && stored == handle
        {
            return Some(units);
        }
        self.overflow.get(handle)
    }

    pub(super) fn insert(&mut self, handle: Handle, units: Vec<u16>) -> Option<Vec<u16>> {
        let slot = (handle.to_raw() >> 32) as usize;
        if slot >= DENSE_SLOTS {
            return self.overflow.insert(handle, units);
        }
        if self.dense.len() <= slot {
            // Power-of-two growth keeps Vec's geometric capacity within the
            // dense ceiling even when the first String occupies a high slot.
            self.dense
                .reserve_exact((slot + 1).next_power_of_two() - self.dense.len());
            self.dense.resize_with(slot + 1, || None);
        }
        match &mut self.dense[slot] {
            Some((stored, previous)) if *stored == handle => {
                Some(std::mem::replace(previous, units))
            }
            // Preserve map semantics even for synthetic callers holding two
            // generations of one slot. Normal GC removes the old one first.
            Some(_) => self.overflow.insert(handle, units),
            empty @ None => {
                let previous = self.overflow.remove(&handle);
                *empty = Some((handle, units));
                previous
            }
        }
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(&Handle, &mut Vec<u16>) -> bool) {
        for entry in &mut self.dense {
            if entry
                .as_mut()
                .is_some_and(|(handle, units)| !keep(handle, units))
            {
                *entry = None;
            }
        }
        self.overflow.retain(keep);
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.dense.iter().filter(|entry| entry.is_some()).count() + self.overflow.len()
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[cfg(test)]
    #[allow(clippy::trivially_copy_pass_by_ref)] // Same borrowed-key map API.
    pub(super) fn contains_key(&self, handle: &Handle) -> bool {
        self.get(handle).is_some()
    }
}

#[cfg(test)]
impl<const N: usize> From<[(Handle, Vec<u16>); N]> for StringValues {
    fn from(entries: [(Handle, Vec<u16>); N]) -> Self {
        let mut values = Self::new();
        for (handle, units) in entries {
            values.insert(handle, units);
        }
        values
    }
}

#[cfg(test)]
impl std::ops::Index<&Handle> for StringValues {
    type Output = Vec<u16>;

    fn index(&self, handle: &Handle) -> &Self::Output {
        self.get(handle).expect("test String payload")
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/string_values/mod.rs"]
mod tests;
