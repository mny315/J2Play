//! Per-suite opaque handles, balanced opens and guest-facing operations.

use diagnostics::EmuError;
use natives::RmsMetadataField;
use std::collections::{HashMap, hash_map::Entry};
use std::path::PathBuf;

use crate::{Limits, PersistentStore, Repository, SuiteId, rms_error, validate_store_name};

mod checkpoint;

#[derive(Debug)]
struct OpenStore {
    references: u32,
    store: PersistentStore,
}

/// Per-running-suite RMS handle registry.
///
/// Duplicate opens share a handle and are reference counted. After the final
/// close, the handle is permanently stale; reopening allocates a new handle so
/// old Java objects and enumerations cannot become valid again accidentally.
#[derive(Debug)]
pub struct Runtime {
    repository: Repository,
    suite: SuiteId,
    next_handle: u64,
    by_name: HashMap<String, u64>,
    stores: HashMap<u64, OpenStore>,
}

impl Runtime {
    /// Creates a per-suite handle registry over one repository root.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, suite: SuiteId, limits: Limits) -> Self {
        Self {
            repository: Repository::new(root, limits),
            suite,
            next_handle: 1,
            by_name: HashMap::new(),
            stores: HashMap::new(),
        }
    }

    /// Opens or creates one store, balancing duplicate opens through a shared handle.
    ///
    /// # Errors
    /// Propagates validation, persistence, corruption, and quota diagnostics.
    pub fn open(&mut self, name: &str, create: bool, now_millis: i64) -> Result<u64, EmuError> {
        validate_store_name(name)?;
        if let Some(&handle) = self.by_name.get(name) {
            let open = self.stores.get_mut(&handle).ok_or_else(|| {
                rms_error("store-not-open", "RMS handle registry is inconsistent")
            })?;
            open.references = open
                .references
                .checked_add(1)
                .ok_or_else(|| rms_error("open-limit", "record store open count exhausted"))?;
            return Ok(handle);
        }
        let handle = self.next_handle;
        let next_handle = self
            .next_handle
            .checked_add(1)
            .filter(|value| i64::try_from(*value).is_ok())
            .ok_or_else(|| rms_error("open-limit", "RMS handle space exhausted"))?;
        let store = self
            .repository
            .open(&self.suite, name, create, now_millis)?;
        self.next_handle = next_handle;
        self.by_name.insert(name.to_owned(), handle);
        self.stores.insert(
            handle,
            OpenStore {
                references: 1,
                store,
            },
        );
        Ok(handle)
    }

    /// Opens the active suite through the owner-qualified MIDP overload.
    ///
    /// Cross-suite sharing remains deny-by-default: no permission grant exposes
    /// another suite's persistent records.
    ///
    /// # Errors
    /// Returns `security-exception` for a different owner.
    pub fn open_owned(
        &mut self,
        name: &str,
        vendor: &str,
        suite: &str,
        now_millis: i64,
    ) -> Result<u64, EmuError> {
        if self.suite.vendor != vendor || self.suite.name != suite {
            return Err(rms_error(
                "security-exception",
                "cross-suite RMS access is denied",
            ));
        }
        self.open(name, false, now_millis)
    }

    /// Balances one open and invalidates the handle after the final close.
    ///
    /// # Errors
    /// Returns `store-not-open` for a stale or unknown handle.
    pub fn close(&mut self, handle: u64) -> Result<(), EmuError> {
        let Entry::Occupied(mut entry) = self.stores.entry(handle) else {
            return Err(store_not_open());
        };
        let open = entry.get_mut();
        open.references -= 1;
        if open.references == 0 {
            self.by_name.remove(entry.remove().store.name());
        }
        Ok(())
    }

    /// Deletes one closed store owned by this suite.
    ///
    /// # Errors
    /// Returns `store-open` if any balanced open remains.
    pub fn delete_store(&mut self, name: &str) -> Result<(), EmuError> {
        if self.by_name.contains_key(name) {
            return Err(rms_error("store-open", "record store is open"));
        }
        self.repository.delete(&self.suite, name)
    }

    /// Lists committed store names in deterministic order.
    ///
    /// # Errors
    /// Propagates bounded decode and host I/O diagnostics.
    pub fn list_stores(&self) -> Result<Vec<String>, EmuError> {
        self.repository.list(&self.suite)
    }

    /// Returns the newest guest wall-clock timestamp stored in this suite's RMS snapshots.
    ///
    /// # Errors
    /// Propagates bounded decode and host I/O diagnostics.
    pub fn latest_modified(&self) -> Result<Option<i64>, EmuError> {
        self.repository.latest_modified(&self.suite)
    }

    /// Returns one current metadata value for an open handle.
    /// Only `SizeAvailable` measures the suite directory; other values use the open store.
    ///
    /// # Errors
    /// Returns `store-not-open`, an encoding limit, or host I/O diagnostic.
    pub fn metadata(&self, handle: u64, field: RmsMetadataField) -> Result<i64, EmuError> {
        let store = &self.opened(handle)?.store;
        Ok(i64::from(match field {
            RmsMetadataField::Version => bounded_i32(store.version()),
            RmsMetadataField::RecordCount => bounded_i32(store.record_count()),
            RmsMetadataField::Size => bounded_i32(store.size()?),
            RmsMetadataField::SizeAvailable => bounded_i32(store.size_available()?),
            RmsMetadataField::NextRecordId => bounded_i32(store.next_record_id()),
            RmsMetadataField::LastModified => return Ok(store.last_modified()),
        }))
    }

    /// Returns current record IDs in ascending order.
    ///
    /// # Errors
    /// Returns `store-not-open` for a stale handle.
    pub fn record_ids(&self, handle: u64) -> Result<Vec<i32>, EmuError> {
        self.opened(handle)?
            .store
            .record_ids()
            .into_iter()
            .map(|id| {
                i32::try_from(id)
                    .map_err(|_| rms_error("corrupt-store", "record ID exceeds Java int"))
            })
            .collect()
    }

    /// Borrows one record's bytes from the open store.
    ///
    /// # Errors
    /// Returns `invalid-record-id` or `store-not-open`.
    pub fn get(&self, handle: u64, record_id: i32) -> Result<&[u8], EmuError> {
        let id = positive_record_id(record_id)?;
        self.opened(handle)?
            .store
            .get(id)
            .ok_or_else(invalid_record_id)
    }

    /// Adds and commits a record.
    ///
    /// # Errors
    /// Propagates handle, quota, ID, and persistence diagnostics.
    pub fn add(&mut self, handle: u64, data: &[u8], now_millis: i64) -> Result<i32, EmuError> {
        let id = self.open_mut(handle)?.store.add(data, now_millis)?;
        i32::try_from(id).map_err(|_| rms_error("record-id-exhausted", "record ID exhausted"))
    }

    /// Replaces and commits a record.
    ///
    /// # Errors
    /// Propagates handle, record, quota, and persistence diagnostics.
    pub fn set(
        &mut self,
        handle: u64,
        record_id: i32,
        data: &[u8],
        now_millis: i64,
    ) -> Result<(), EmuError> {
        let id = positive_record_id(record_id)?;
        self.open_mut(handle)?.store.set(id, data, now_millis)
    }

    /// Deletes and commits a record.
    ///
    /// # Errors
    /// Propagates handle, record, and persistence diagnostics.
    pub fn delete(&mut self, handle: u64, record_id: i32, now_millis: i64) -> Result<(), EmuError> {
        let id = positive_record_id(record_id)?;
        self.open_mut(handle)?.store.delete(id, now_millis)
    }

    fn opened(&self, handle: u64) -> Result<&OpenStore, EmuError> {
        self.stores.get(&handle).ok_or_else(store_not_open)
    }

    fn open_mut(&mut self, handle: u64) -> Result<&mut OpenStore, EmuError> {
        self.stores.get_mut(&handle).ok_or_else(store_not_open)
    }
}

fn bounded_i32(value: impl TryInto<i32>) -> i32 {
    value.try_into().unwrap_or(i32::MAX)
}

fn positive_record_id(record_id: i32) -> Result<u32, EmuError> {
    u32::try_from(record_id)
        .ok()
        .filter(|id| *id != 0)
        .ok_or_else(invalid_record_id)
}

fn invalid_record_id() -> EmuError {
    rms_error("invalid-record-id", "record does not exist")
}

pub(super) fn store_not_open() -> EmuError {
    rms_error("store-not-open", "record store is not open")
}

#[cfg(test)]
#[path = "../../../tests/unit/rms/runtime.rs"]
mod tests;
