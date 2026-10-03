use super::super::storage::read_bounded_file_cancellable;
use super::{Deserialize, EmuError, Serialize, atomic_write, fs, invalid, storage_error};
use std::path::{Component, Path};

const MAX_STORAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_STORAGE_ENTRIES: usize = 4096;

#[derive(Serialize, Deserialize)]
pub(crate) struct StorageEntry {
    pub(super) path: String,
    #[serde(
        serialize_with = "save_state::serialize_optional_bytes",
        deserialize_with = "save_state::deserialize_optional_bytes"
    )]
    pub(super) bytes: Option<Vec<u8>>,
}

pub(crate) fn capture_storage(
    root: &Path,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Vec<StorageEntry>, EmuError> {
    if cancelled() {
        return Err(invalid("Automatic saving was interrupted."));
    }
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Err(invalid("Game storage is not a directory.")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(storage_error(error)),
    }
    let mut pending = vec![root.to_owned()];
    let mut entries = Vec::new();
    let mut total = 0_usize;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(storage_error)? {
            if cancelled() {
                return Err(invalid("Automatic saving was interrupted."));
            }
            if entries.len() >= MAX_STORAGE_ENTRIES {
                return Err(invalid("Too many files for an automatic save."));
            }
            let entry = entry.map_err(storage_error)?;
            let metadata = entry.file_type().map_err(storage_error)?;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| invalid("Invalid game storage path."))?
                .to_str()
                .ok_or_else(|| invalid("Game storage contains an unsupported filename."))?
                .to_owned();
            validate_storage_path(&relative)?;
            let bytes = if metadata.is_dir() {
                pending.push(path);
                None
            } else if metadata.is_file() {
                let bytes = read_bounded_file_cancellable(
                    &path,
                    MAX_STORAGE_BYTES.saturating_sub(total) as u64,
                    "checkpoint-files",
                    &mut cancelled,
                )?;
                total += bytes.len();
                Some(bytes)
            } else {
                return Err(invalid("Game storage contains an unsupported file type."));
            };
            entries.push(StorageEntry {
                path: relative,
                bytes,
            });
        }
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

pub(super) fn restore_storage(root: &Path, entries: &[StorageEntry]) -> Result<(), EmuError> {
    for entry in entries {
        let path = root.join(&entry.path);
        if let Some(bytes) = &entry.bytes {
            atomic_write(&path, bytes)?;
        } else {
            fs::create_dir_all(path).map_err(storage_error)?;
        }
    }
    Ok(())
}

pub(super) fn validate_storage(entries: &[StorageEntry]) -> Result<(), EmuError> {
    let mut paths = std::collections::BTreeMap::new();
    let mut total = 0_usize;
    if entries.len() > MAX_STORAGE_ENTRIES {
        return Err(invalid("Too many saved storage entries."));
    }
    for entry in entries {
        validate_storage_path(&entry.path)?;
        total = total
            .checked_add(entry.bytes.as_ref().map_or(0, Vec::len))
            .filter(|bytes| *bytes <= MAX_STORAGE_BYTES)
            .ok_or_else(|| invalid("Saved game storage exceeds its byte limit."))?;
        if paths
            .insert(entry.path.as_str(), entry.bytes.is_some())
            .is_some()
        {
            return Err(invalid("Saved game storage contains duplicate paths."));
        }
    }
    // Check the complete path set so validation is independent of entry order.
    // Missing parents are valid: restoration creates them as directories.
    for path in paths.keys() {
        for (end, _) in path.match_indices('/') {
            if paths.get(&path[..end]) == Some(&true) {
                return Err(invalid("Saved game storage uses a file as a directory."));
            }
        }
    }
    Ok(())
}

fn validate_storage_path(path: &str) -> Result<(), EmuError> {
    if path.is_empty()
        || path.len() > 4096
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path.contains('\0')
        || Path::new(path)
            .components()
            .enumerate()
            .any(|(index, component)| index >= 32 || !matches!(component, Component::Normal(_)))
    {
        return Err(invalid("Saved game storage contains an invalid path."));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/unit/frontend-core/library/checkpoint/storage.rs"]
mod tests;
