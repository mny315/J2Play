//! A suite's committed records and open-handle identities form one checkpoint.

use super::{
    EmuError, HashMap, Limits, OpenStore, PathBuf, PersistentStore, Runtime, SuiteId, rms_error,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Checkpoint {
    next_handle: u64,
    #[serde(
        serialize_with = "serialize_stores",
        deserialize_with = "deserialize_stores"
    )]
    stores: Vec<Vec<u8>>,
    open: Vec<(String, u64, u32)>,
}

fn serialize_stores<S: serde::Serializer>(
    stores: &[Vec<u8>],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Bytes<'a>(#[serde(serialize_with = "save_state::serialize_bytes")] &'a [u8]);

    // Postcard encodes byte strings and u8 sequences identically. Write each
    // committed buffer as a block without per-byte serializer/writer calls.
    serializer.collect_seq(stores.iter().map(|bytes| Bytes(bytes)))
}

fn deserialize_stores<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Vec<u8>>, D::Error> {
    #[derive(Deserialize)]
    struct Bytes(#[serde(deserialize_with = "save_state::deserialize_bytes")] Vec<u8>);

    Vec::<Bytes>::deserialize(deserializer)
        .map(|stores| stores.into_iter().map(|bytes| bytes.0).collect())
}

impl Runtime {
    /// Captures all committed records and balanced open handles for this suite.
    ///
    /// # Errors
    /// Returns a storage, corruption or checkpoint size error.
    pub fn encode_checkpoint(&self) -> Result<Vec<u8>, EmuError> {
        self.encode_checkpoint_cancellable(&|| false)
    }

    /// Captures stores with a host deadline or cancellation signal.
    ///
    /// # Errors
    /// Returns a storage, size or cancellation error.
    pub fn encode_checkpoint_cancellable(
        &self,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EmuError> {
        let stores = self.repository.checkpoint_stores(&self.suite, cancelled)?;
        save_state::encode_cancellable(
            &Checkpoint {
                next_handle: self.next_handle,
                stores,
                open: self
                    .stores
                    .iter()
                    .map(|(handle, open)| (open.store.name().to_owned(), *handle, open.references))
                    .collect(),
            },
            save_state::MAX_COMPONENT_BYTES,
            cancelled,
        )
    }

    /// Restores records into a fresh, unpublished repository root. The caller
    /// publishes that root only after the complete game state has been validated.
    ///
    /// # Errors
    /// Returns a validation, quota or persistence error, preserving the active root.
    pub fn restore_checkpoint(
        root: impl Into<PathBuf>,
        suite: SuiteId,
        limits: Limits,
        bytes: &[u8],
    ) -> Result<Self, EmuError> {
        let saved: Checkpoint = save_state::decode(bytes)?;
        if saved.stores.len() > limits.max_stores_per_suite
            || saved.open.len() > saved.stores.len()
            || saved.next_handle == 0
            || saved.next_handle > i64::MAX as u64
            || saved
                .stores
                .iter()
                .try_fold(0_usize, |sum, bytes| sum.checked_add(bytes.len()))
                .is_none_or(|size| size > limits.max_suite_bytes)
        {
            return Err(invalid());
        }
        let mut stores = HashMap::new();
        let suite_digest = suite.digest();
        // Release each encoded buffer once decoded instead of retaining a
        // second complete copy of all record payloads until restore finishes.
        for bytes in saved.stores {
            let state = crate::snapshot::decode(&bytes, limits, suite_digest)?;
            if stores.insert(state.name.clone(), state).is_some() {
                return Err(invalid());
            }
        }
        let mut runtime = Self::new(root, suite, limits);
        runtime.next_handle = saved.next_handle;
        for (name, handle, references) in saved.open {
            if handle == 0
                || handle >= saved.next_handle
                || references == 0
                || runtime.stores.contains_key(&handle)
                || runtime.by_name.contains_key(&name)
            {
                return Err(invalid());
            }
            let state = stores.get(&name).ok_or_else(invalid)?;
            runtime.by_name.insert(name, handle);
            runtime.stores.insert(
                handle,
                OpenStore {
                    references,
                    store: PersistentStore::new(runtime.repository.clone(), state.clone()),
                },
            );
        }
        for state in stores.values() {
            runtime
                .repository
                .commit(state)
                .map_err(crate::repository::CommitFailure::into_error)?;
        }
        Ok(runtime)
    }
}

fn invalid() -> EmuError {
    rms_error(
        "checkpoint-rms",
        "The checkpoint contains invalid saved records or open handles",
    )
}

#[cfg(test)]
#[path = "../../../../tests/unit/rms/checkpoint.rs"]
mod tests;
