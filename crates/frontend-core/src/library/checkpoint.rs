//! Atomic automatic checkpoints and isolated restored storage roots.

use super::{
    LibraryEntry, LibraryRepository, atomic_write, io_error, library_error, read_bounded_file,
    remove_optional_file, remove_optional_tree,
};
use diagnostics::EmuError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

mod compression;
mod storage;
pub(crate) use storage::{StorageEntry, capture_storage};

const MAGIC: &[u8; 8] = b"J2PSAVE\0";
// Version 3 requires VM format 7, including the updated bootstrap state.
const FORMAT: u32 = 3;
const HEADER_BYTES: usize = 8 + 4 + 8 + 8 + 32;
const MAX_FILE_BYTES: u64 = save_state::MAX_COMPONENT_BYTES as u64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct CheckpointIdentity {
    pub(crate) jar_sha256: String,
    pub(crate) midlet_class: String,
    pub(crate) profile_catalog: String,
    pub(crate) persona: String,
    pub(crate) runtime_host: String,
    pub(crate) canvas: (u32, u32),
    pub(crate) properties: [u8; 32],
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SavedFrame {
    pub(crate) canvas_region: [u32; 4],
    pub(crate) pixels: std::sync::Arc<[u32]>,
}

#[derive(Serialize, Deserialize)]
#[serde(bound(
    serialize = "V: AsRef<[u8]>, D: AsRef<[u8]>",
    deserialize = "V: From<Vec<u8>>, D: From<Vec<u8>>"
))]
pub(crate) struct Checkpoint<V = Vec<u8>, D = Vec<u8>> {
    pub(crate) identity: CheckpointIdentity,
    pub(crate) saved_at_millis: i64,
    pub(crate) monotonic_millis: i64,
    pub(crate) wall_clock_millis: i64,
    pub(crate) active_canvas: (u32, u32),
    pub(crate) frame: Option<SavedFrame>,
    #[serde(
        serialize_with = "save_state::serialize_bytes",
        deserialize_with = "save_state::deserialize_bytes"
    )]
    pub(crate) vm: V,
    #[serde(
        serialize_with = "save_state::serialize_bytes",
        deserialize_with = "save_state::deserialize_bytes"
    )]
    pub(crate) driver: D,
    #[serde(
        serialize_with = "save_state::serialize_bytes",
        deserialize_with = "save_state::deserialize_bytes"
    )]
    pub(crate) ams: Vec<u8>,
    #[serde(
        serialize_with = "save_state::serialize_bytes",
        deserialize_with = "save_state::deserialize_bytes"
    )]
    pub(crate) rms: Vec<u8>,
    #[serde(
        serialize_with = "save_state::serialize_bytes",
        deserialize_with = "save_state::deserialize_bytes"
    )]
    pub(crate) mmapi: Vec<u8>,
    pub(crate) files: Vec<StorageEntry>,
}

#[derive(Clone)]
pub(crate) struct RuntimeStorage {
    pub(crate) rms: PathBuf,
    pub(crate) files: PathBuf,
    pub(crate) generation: Option<u8>,
}

impl LibraryRepository {
    fn checkpoint_root(&self, entry: &LibraryEntry) -> PathBuf {
        self.root.join("runtime").join("resume").join(entry.id())
    }

    pub(crate) fn checkpoint_epoch(&self, entry: &LibraryEntry) -> Result<u64, EmuError> {
        let path = self.checkpoint_root(entry).join("epoch");
        if !path.try_exists().map_err(storage_error)? {
            return Ok(0);
        }
        let bytes = read_bounded_file(&path, 8, "checkpoint-epoch")?;
        Ok(u64::from_be_bytes(bytes.try_into().map_err(|_| {
            invalid("The checkpoint session marker is damaged.")
        })?))
    }

    pub(crate) fn begin_checkpoint_session(&self, entry: &LibraryEntry) -> Result<u64, EmuError> {
        let epoch = self
            .checkpoint_epoch(entry)?
            .checked_add(1)
            .ok_or_else(|| invalid("The checkpoint session counter is exhausted."))?;
        atomic_write(
            &self.checkpoint_root(entry).join("epoch"),
            &epoch.to_be_bytes(),
        )?;
        Ok(epoch)
    }

    pub(crate) fn invalidate_checkpoint(
        &self,
        entry: &LibraryEntry,
        epoch: u64,
    ) -> Result<(), EmuError> {
        if self.checkpoint_epoch(entry)? == epoch {
            self.begin_checkpoint_session(entry)?;
        }
        Ok(())
    }

    pub(crate) fn write_checkpoint<V: AsRef<[u8]>, D: AsRef<[u8]>>(
        &self,
        entry: &LibraryEntry,
        epoch: u64,
        checkpoint: &Checkpoint<V, D>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EmuError> {
        if self.checkpoint_epoch(entry)? != epoch {
            return Err(invalid("The checkpoint belongs to an earlier launch."));
        }
        let (length, compressed) = compression::encode(checkpoint, cancelled)?;
        let mut header = Vec::with_capacity(HEADER_BYTES);
        header.extend_from_slice(MAGIC);
        header.extend_from_slice(&FORMAT.to_be_bytes());
        header.extend_from_slice(&epoch.to_be_bytes());
        header.extend_from_slice(&(length as u64).to_be_bytes());
        let mut digest = Sha256::new();
        digest.update(&header);
        digest.update(&compressed);
        header.extend_from_slice(&digest.finalize());
        if cancelled() {
            return Err(invalid("Automatic saving was interrupted."));
        }
        super::storage::atomic_write_parts(
            &self.checkpoint_root(entry).join("automatic.save"),
            &[&header, &compressed],
        )
    }

    pub(crate) fn load_checkpoint(
        &self,
        entry: &LibraryEntry,
        identity: &CheckpointIdentity,
    ) -> Result<Option<Checkpoint>, EmuError> {
        let path = self.checkpoint_root(entry).join("automatic.save");
        if !path.try_exists().map_err(storage_error)? {
            return Ok(None);
        }
        let bytes = read_bounded_file(&path, MAX_FILE_BYTES, "checkpoint-read")?;
        if bytes.len() < HEADER_BYTES
            || &bytes[..8] != MAGIC
            || bytes[8..12] != FORMAT.to_be_bytes()
        {
            return Err(invalid("This automatic save uses an unsupported format."));
        }
        let number = |start: usize| -> Result<u64, EmuError> {
            Ok(u64::from_be_bytes(
                bytes[start..start + 8]
                    .try_into()
                    .map_err(|_| invalid("The automatic save header is damaged."))?,
            ))
        };
        if number(12)? != self.checkpoint_epoch(entry)? {
            return Ok(None);
        }
        let length = number(20)?;
        let mut digest = Sha256::new();
        digest.update(&bytes[..28]);
        digest.update(&bytes[HEADER_BYTES..]);
        if digest.finalize().as_slice() != &bytes[28..HEADER_BYTES] {
            return Err(invalid(
                "The automatic save is damaged. Your in-game saves are still available.",
            ));
        }
        let payload = compression::decode(&bytes[HEADER_BYTES..], length)?;
        let checkpoint: Checkpoint = save_state::decode(&payload)?;
        if &checkpoint.identity != identity {
            return Err(invalid(
                "The game or device profile has changed since this automatic save.",
            ));
        }
        storage::validate_storage(&checkpoint.files)?;
        Ok(Some(checkpoint))
    }

    pub(crate) fn runtime_storage(&self, entry: &LibraryEntry) -> Result<RuntimeStorage, EmuError> {
        let marker = self.checkpoint_root(entry).join("active-storage");
        if !marker.try_exists().map_err(storage_error)? {
            return Ok(RuntimeStorage {
                rms: self.rms_root(entry),
                files: self.file_root(entry),
                generation: None,
            });
        }
        let bytes = read_bounded_file(&marker, 1, "checkpoint-storage")?;
        let generation = match bytes.as_slice() {
            [0] => 0,
            [1] => 1,
            _ => return Err(invalid("The restored storage marker is damaged.")),
        };
        Ok(self.restored_storage(entry, generation))
    }

    fn restored_storage(&self, entry: &LibraryEntry, generation: u8) -> RuntimeStorage {
        let root = self
            .checkpoint_root(entry)
            .join(format!("storage-{generation}"));
        RuntimeStorage {
            rms: root.join("rms"),
            files: root.join("files"),
            generation: Some(generation),
        }
    }

    pub(crate) fn prepare_restored_storage(
        &self,
        entry: &LibraryEntry,
        files: &[StorageEntry],
    ) -> Result<RuntimeStorage, EmuError> {
        storage::validate_storage(files)?;
        let active = self.runtime_storage(entry)?;
        let generation = u8::from(active.generation == Some(0));
        let storage = self.restored_storage(entry, generation);
        // Recycle only an inactive slot created by automatic recovery. Keep
        // the active generation and the original pre-feature storage intact.
        let root = self
            .checkpoint_root(entry)
            .join(format!("storage-{generation}"));
        remove_optional_tree(
            &root,
            "checkpoint-storage",
            "cannot prepare restored storage",
        )?;
        fs::create_dir_all(&storage.rms).map_err(storage_error)?;
        fs::create_dir_all(&storage.files).map_err(storage_error)?;
        storage::restore_storage(&storage.files, files)?;
        Ok(storage)
    }

    pub(crate) fn publish_restored_storage(
        &self,
        entry: &LibraryEntry,
        storage: &RuntimeStorage,
    ) -> Result<(), EmuError> {
        let generation = storage
            .generation
            .ok_or_else(|| invalid("No restored storage was prepared."))?;
        atomic_write(
            &self.checkpoint_root(entry).join("active-storage"),
            &[generation],
        )
    }

    /// Removes the automatic resume snapshot while preserving current RMS and
    /// `FileConnection` data, including storage published by an earlier resume.
    /// The caller must first stop any session using this entry.
    ///
    /// # Errors
    /// Returns a controlled diagnostic if the entry is invalid or the snapshot
    /// cannot be removed. An absent snapshot is already cleared.
    pub fn delete_resume_save(&self, entry: &LibraryEntry) -> Result<(), EmuError> {
        entry.validate()?;
        remove_optional_file(
            &self.checkpoint_root(entry).join("automatic.save"),
            "checkpoint-delete",
            "cannot remove automatic resume save",
        )
    }

    pub(crate) fn delete_checkpoints(&self, entry: &LibraryEntry) -> Result<(), EmuError> {
        remove_optional_tree(
            &self.checkpoint_root(entry),
            "checkpoint-delete",
            "cannot remove automatic saves",
        )
    }
}

fn invalid(message: &str) -> EmuError {
    library_error("checkpoint-invalid", message)
}
fn storage_error(error: std::io::Error) -> EmuError {
    io_error(
        "checkpoint-storage",
        "Could not read or write the automatic game save",
        error,
    )
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-core/library/checkpoint.rs"]
mod tests;
