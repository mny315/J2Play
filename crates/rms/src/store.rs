//! Transactional record mutations and metadata for one opened store.

use diagnostics::EmuError;
use std::sync::Arc;

use crate::snapshot::StoreState;
use crate::{MAX_RECORD_ID, Repository, rms_error, validate_record};

/// An opened store whose mutations are persisted atomically before success is
/// returned to the caller.
#[derive(Debug)]
pub struct PersistentStore {
    repository: Repository,
    state: StoreState,
}

impl PersistentStore {
    pub(super) fn new(repository: Repository, state: StoreState) -> Self {
        Self { repository, state }
    }

    /// Returns the original case-sensitive store name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.state.name
    }

    /// Returns the format-visible mutation version.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.state.version
    }

    /// Returns the wall-clock timestamp of the last committed mutation.
    #[must_use]
    pub fn last_modified(&self) -> i64 {
        self.state.last_modified
    }

    /// Returns the ID that the next successful add will allocate.
    #[must_use]
    pub fn next_record_id(&self) -> u32 {
        self.state.next_record_id
    }

    /// Returns the number of live records.
    #[must_use]
    pub fn record_count(&self) -> usize {
        self.state.records.len()
    }

    /// Borrows one live record payload, or returns `None` for an absent ID.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&[u8]> {
        self.state.records.get(&id).map(AsRef::as_ref)
    }

    /// Returns live record IDs in ascending order.
    #[must_use]
    pub fn record_ids(&self) -> Vec<u32> {
        self.state.records.keys().copied().collect()
    }

    /// Returns the complete encoded size, including format overhead.
    ///
    /// # Errors
    /// Returns a controlled limit error if the in-memory state cannot encode.
    pub fn size(&self) -> Result<usize, EmuError> {
        self.state.encoded_len(self.repository.limits)
    }

    /// Returns the maximum additional encoded bytes allowed by store and suite quotas.
    ///
    /// # Errors
    /// Returns a categorized host I/O error while measuring suite usage.
    pub fn size_available(&self) -> Result<usize, EmuError> {
        let current = self.size()?;
        let directory = self.repository.suite_path(self.state.suite_digest);
        let suite_bytes = self.repository.suite_committed_bytes(&directory)?;
        let store_room = self
            .repository
            .limits
            .max_store_bytes
            .saturating_sub(current);
        let suite_room = self
            .repository
            .limits
            .max_suite_bytes
            .saturating_sub(suite_bytes);
        Ok(store_room.min(suite_room))
    }

    /// Adds and durably commits a record. Deleted IDs are never reused.
    ///
    /// # Errors
    /// Returns `record-limit`, `record-id-exhausted`, `store-full`, or a host
    /// persistence error. A failure after atomic replacement retains the
    /// snapshot visible to subsequent opens even when durability was not confirmed.
    pub fn add(&mut self, data: &[u8], now_millis: i64) -> Result<u32, EmuError> {
        validate_record(data, self.repository.limits)?;
        let id = self.state.next_record_id;
        if id == 0 || id > MAX_RECORD_ID {
            return Err(rms_error(
                "record-id-exhausted",
                "record ID space exhausted",
            ));
        }
        self.mutate(now_millis, |candidate| {
            candidate.records.insert(id, Arc::from(data));
            candidate.next_record_id = id + 1;
            id
        })
    }

    /// Replaces and durably commits an existing record.
    ///
    /// # Errors
    /// Returns `invalid-record-id`, a storage limit error, or a host I/O error.
    pub fn set(&mut self, id: u32, data: &[u8], now_millis: i64) -> Result<(), EmuError> {
        validate_record(data, self.repository.limits)?;
        self.require_record(id)?;
        self.mutate(now_millis, |candidate| {
            candidate.records.insert(id, Arc::from(data));
        })
    }

    /// Deletes and durably commits an existing record.
    ///
    /// # Errors
    /// Returns `invalid-record-id` or a host persistence error.
    pub fn delete(&mut self, id: u32, now_millis: i64) -> Result<(), EmuError> {
        self.require_record(id)?;
        self.mutate(now_millis, |candidate| {
            candidate.records.remove(&id);
        })
    }

    fn require_record(&self, id: u32) -> Result<(), EmuError> {
        if self.state.records.contains_key(&id) {
            Ok(())
        } else {
            Err(rms_error("invalid-record-id", "record does not exist"))
        }
    }

    fn mutate<T>(
        &mut self,
        now_millis: i64,
        operation: impl FnOnce(&mut StoreState) -> T,
    ) -> Result<T, EmuError> {
        let next_version = self
            .state
            .version
            .checked_add(1)
            .ok_or_else(|| rms_error("version-exhausted", "record store version exhausted"))?;
        let mut candidate = self.state.clone();
        let result = operation(&mut candidate);
        candidate.version = next_version;
        candidate.last_modified = now_millis;
        match self.repository.commit(&candidate) {
            Ok(()) => {
                self.state = candidate;
                Ok(result)
            }
            Err(failure) => {
                // Once rename succeeded, the replacement is already visible to
                // reopen. Keep this handle on the same snapshot even though the
                // durability confirmation failed and the Java operation throws.
                if failure.replacement_visible {
                    self.state = candidate;
                }
                Err(failure.into_error())
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/rms/store.rs"]
mod tests;
