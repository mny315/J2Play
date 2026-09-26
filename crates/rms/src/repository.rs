//! Filesystem snapshots, suite quotas and durable atomic replacement.

use diagnostics::{Category, EmuError};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::{Arc, Mutex};

use crate::snapshot::{StoreHeader, StoreState, decode, inspect_snapshot};
use crate::{Limits, PersistentStore, SuiteId, hex, rms_error, store_digest, validate_store_name};

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CommitPhase {
    BeforeTemporaryOpen,
    AfterTemporaryWrite,
    AfterTemporarySync,
    AfterRename,
    AfterDirectorySync,
}

pub(super) struct CommitFailure {
    error: EmuError,
    pub(super) replacement_visible: bool,
}

impl CommitFailure {
    fn before_rename(error: EmuError) -> Self {
        Self {
            error,
            replacement_visible: false,
        }
    }

    fn after_rename(error: EmuError) -> Self {
        Self {
            error,
            replacement_visible: true,
        }
    }

    pub(super) fn into_error(self) -> EmuError {
        self.error
    }
}

/// Filesystem-backed RMS repository. Paths contain only hashes, never raw
/// suite or store names.
#[derive(Clone, Debug)]
pub struct Repository {
    root: PathBuf,
    pub(super) limits: Limits,
    #[cfg(test)]
    failure: Arc<Mutex<Option<CommitPhase>>>,
}

impl Repository {
    /// Creates a repository rooted at the supplied host directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, limits: Limits) -> Self {
        Self {
            root: root.into(),
            limits,
            #[cfg(test)]
            failure: Arc::new(Mutex::new(None)),
        }
    }

    /// Opens one store, optionally creating and durably committing it.
    ///
    /// # Errors
    /// Returns a categorized storage error for invalid names, missing stores,
    /// corrupt snapshots, exhausted limits, or host I/O failures.
    pub fn open(
        &self,
        suite: &SuiteId,
        name: &str,
        create: bool,
        now_millis: i64,
    ) -> Result<PersistentStore, EmuError> {
        validate_store_name(name)?;
        let suite_digest = suite.digest();
        let path = self.store_path(suite_digest, name);
        validate_suite_directory(
            &self.suite_path(suite_digest),
            if create {
                "create-suite"
            } else {
                "corrupt-store"
            },
        )?;
        let state = match decode_file(&path, self.limits, suite_digest, name) {
            Ok(state) => state,
            Err(error) if create && error.code() == "store-not-found" => {
                let state = StoreState::empty(suite_digest, name, now_millis);
                self.commit(&state).map_err(CommitFailure::into_error)?;
                state
            }
            Err(error) => return Err(error),
        };
        Ok(PersistentStore::new(self.clone(), state))
    }

    /// Lists committed stores owned by one suite. Incomplete temporary files
    /// are deliberately ignored.
    ///
    /// # Errors
    /// Returns a categorized error for corrupt snapshots or host I/O failures.
    pub fn list(&self, suite: &SuiteId) -> Result<Vec<String>, EmuError> {
        let suite_digest = suite.digest();
        let directory = self.suite_path(suite_digest);
        let mut names = Vec::new();
        for path in self.store_paths(&directory)? {
            names.push(inspect_file(&path, self.limits, suite_digest)?.name);
        }
        names.sort();
        Ok(names)
    }

    pub(super) fn latest_modified(&self, suite: &SuiteId) -> Result<Option<i64>, EmuError> {
        let suite_digest = suite.digest();
        let directory = self.suite_path(suite_digest);
        let mut latest = None;
        for path in self.store_paths(&directory)? {
            let modified = inspect_file(&path, self.limits, suite_digest)?.last_modified;
            latest = Some(latest.map_or(modified, |current: i64| current.max(modified)));
        }
        Ok(latest)
    }

    pub(super) fn checkpoint_stores(
        &self,
        suite: &SuiteId,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Vec<u8>>, EmuError> {
        let check_cancelled = || {
            if cancelled() {
                Err(save_state::error(
                    "checkpoint-cancelled",
                    "Automatic saving was interrupted.",
                ))
            } else {
                Ok(())
            }
        };
        check_cancelled()?;
        let suite_digest = suite.digest();
        let mut paths = self.store_paths(&self.suite_path(suite_digest))?;
        paths.sort();
        let mut remaining = self
            .limits
            .max_suite_bytes
            .min(save_state::MAX_COMPONENT_BYTES);
        let mut stores = Vec::with_capacity(paths.len());
        for path in paths {
            check_cancelled()?;
            let limits = Limits {
                max_store_bytes: self.limits.max_store_bytes.min(remaining),
                ..self.limits
            };
            // Committed snapshots already use the checkpoint's RMS format.
            // Validate the exact bytes retained, without rebuilding record maps.
            let bytes = read_store_file(&path, limits)?;
            let header = inspect_snapshot(&bytes, limits, suite_digest, |_, _| {})?;
            validate_store_identity(&path, &header.name)?;
            remaining -= bytes.len();
            stores.push(bytes);
        }
        Ok(stores)
    }

    /// Deletes a committed store and synchronizes its suite directory.
    ///
    /// Open-store exclusion is enforced by [`crate::Runtime`], which owns handles.
    ///
    /// # Errors
    /// Returns `store-not-found` or a categorized host I/O error.
    pub fn delete(&self, suite: &SuiteId, name: &str) -> Result<(), EmuError> {
        validate_store_name(name)?;
        let suite_digest = suite.digest();
        let directory = self.suite_path(suite_digest);
        let final_path = self.store_path(suite_digest, name);
        validate_suite_directory(&directory, "corrupt-store")?;
        // Validate identity before deleting a path selected by its digest.
        let header = inspect_file(&final_path, self.limits, suite_digest)?;
        if header.name != name {
            return Err(rms_error("corrupt-store", "record store identity mismatch"));
        }
        let temporary = final_path.with_extension("tmp");
        if let Err(error) = fs::remove_file(temporary)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(io_error("delete-temporary", error));
        }
        fs::remove_file(&final_path).map_err(|error| io_error("delete-store", error))?;
        File::open(&directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| io_error("sync-suite", error))
    }

    pub(super) fn suite_path(&self, suite_digest: [u8; 32]) -> PathBuf {
        self.root.join(hex(&suite_digest))
    }

    pub(super) fn store_path(&self, suite_digest: [u8; 32], name: &str) -> PathBuf {
        self.suite_path(suite_digest)
            .join(format!("{}.rms", hex(&store_digest(name))))
    }

    pub(super) fn commit(&self, state: &StoreState) -> Result<(), CommitFailure> {
        let encoded = state
            .encode(self.limits)
            .map_err(CommitFailure::before_rename)?;
        let directory = self.suite_path(state.suite_digest);
        create_dir_all_durable(&directory).map_err(CommitFailure::before_rename)?;
        validate_suite_directory(&directory, "create-suite")
            .map_err(CommitFailure::before_rename)?;
        let final_path = self.store_path(state.suite_digest, &state.name);
        self.enforce_suite_limit(&directory, &final_path, encoded.len())
            .map_err(CommitFailure::before_rename)?;
        let temporary = final_path.with_extension("tmp");
        if let Err(error) = fs::remove_file(&temporary)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(CommitFailure::before_rename(io_error(
                "write-temporary",
                error,
            )));
        }
        #[cfg(test)]
        self.fail_at(CommitPhase::BeforeTemporaryOpen)
            .map_err(CommitFailure::before_rename)?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| CommitFailure::before_rename(io_error("write-temporary", error)))?;
        file.write_all(&encoded)
            .map_err(|error| CommitFailure::before_rename(io_error("write-temporary", error)))?;
        #[cfg(test)]
        self.fail_at(CommitPhase::AfterTemporaryWrite)
            .map_err(CommitFailure::before_rename)?;
        file.sync_all()
            .map_err(|error| CommitFailure::before_rename(io_error("sync-temporary", error)))?;
        #[cfg(test)]
        self.fail_at(CommitPhase::AfterTemporarySync)
            .map_err(CommitFailure::before_rename)?;
        fs::rename(&temporary, &final_path)
            .map_err(|error| CommitFailure::before_rename(io_error("commit-store", error)))?;
        #[cfg(test)]
        self.fail_at(CommitPhase::AfterRename)
            .map_err(CommitFailure::after_rename)?;
        File::open(&directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| CommitFailure::after_rename(io_error("sync-suite", error)))?;
        #[cfg(test)]
        self.fail_at(CommitPhase::AfterDirectorySync)
            .map_err(CommitFailure::after_rename)?;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn inject_failure(&self, phase: CommitPhase) {
        *self.failure.lock().unwrap() = Some(phase);
    }

    #[cfg(test)]
    fn fail_at(&self, phase: CommitPhase) -> Result<(), EmuError> {
        let mut failure = self.failure.lock().unwrap();
        if *failure == Some(phase) {
            *failure = None;
            return Err(rms_error(
                "injected-commit-failure",
                "injected RMS commit failure",
            ));
        }
        Ok(())
    }

    fn enforce_suite_limit(
        &self,
        directory: &Path,
        target: &Path,
        replacement_bytes: usize,
    ) -> Result<(), EmuError> {
        let mut total = 0usize;
        let paths = self.store_paths(directory)?;
        let stores = paths.len();
        for path in paths {
            if path != target {
                let length = path
                    .metadata()
                    .map_err(|error| io_error("suite-size", error))?
                    .len();
                total = total.saturating_add(usize::try_from(length).unwrap_or(usize::MAX));
            }
        }
        if !target.exists() && stores >= self.limits.max_stores_per_suite {
            return Err(rms_error("store-limit", "suite store limit reached"));
        }
        if total.saturating_add(replacement_bytes) > self.limits.max_suite_bytes {
            return Err(rms_error("store-full", "suite RMS byte limit reached"));
        }
        Ok(())
    }

    pub(super) fn suite_committed_bytes(&self, directory: &Path) -> Result<usize, EmuError> {
        let mut total = 0usize;
        for path in self.store_paths(directory)? {
            let length = path
                .metadata()
                .map_err(|error| io_error("suite-size", error))?
                .len();
            total = total.saturating_add(usize::try_from(length).unwrap_or(usize::MAX));
        }
        Ok(total)
    }

    fn store_paths(&self, directory: &Path) -> Result<Vec<PathBuf>, EmuError> {
        validate_suite_directory(directory, "corrupt-store")?;
        let mut paths = Vec::new();
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(paths),
            Err(error) => return Err(io_error("list-stores", error)),
        };
        // Include ignored crash leftovers in the work budget. Limiting only
        // recognized stores permits an unbounded directory scan on every save.
        let scan_limit = self
            .limits
            .max_stores_per_suite
            .saturating_mul(2)
            .saturating_add(1);
        for (index, entry) in entries.enumerate() {
            if index >= scan_limit {
                return Err(rms_error(
                    "store-limit",
                    "suite directory scan limit exceeded",
                ));
            }
            let entry = entry.map_err(|error| io_error("list-stores", error))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("rms") {
                continue;
            }
            if paths.len() >= self.limits.max_stores_per_suite {
                return Err(rms_error("store-limit", "suite contains too many stores"));
            }
            if !entry
                .file_type()
                .map_err(|error| io_error("list-stores", error))?
                .is_file()
            {
                return Err(rms_error(
                    "corrupt-store",
                    "record store is not a regular file",
                ));
            }
            paths.push(path);
        }
        Ok(paths)
    }
}

fn decode_file(
    path: &Path,
    limits: Limits,
    suite_digest: [u8; 32],
    name: &str,
) -> Result<StoreState, EmuError> {
    let state = decode(&read_store_file(path, limits)?, limits, suite_digest)?;
    if state.name != name {
        return Err(rms_error("corrupt-store", "record store identity mismatch"));
    }
    validate_store_identity(path, &state.name)?;
    Ok(state)
}

fn inspect_file(
    path: &Path,
    limits: Limits,
    suite_digest: [u8; 32],
) -> Result<StoreHeader, EmuError> {
    let header = inspect_snapshot(
        &read_store_file(path, limits)?,
        limits,
        suite_digest,
        |_, _| {},
    )?;
    validate_store_identity(path, &header.name)?;
    Ok(header)
}

fn validate_store_identity(path: &Path, name: &str) -> Result<(), EmuError> {
    if path.file_stem().and_then(|value| value.to_str()) != Some(&hex(&store_digest(name))) {
        return Err(rms_error("corrupt-store", "record store identity mismatch"));
    }
    Ok(())
}

fn validate_suite_directory(path: &Path, code: &'static str) -> Result<(), EmuError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(rms_error(
            code,
            "RMS suite directory must not be a symbolic link",
        )),
        Ok(metadata) if !metadata.is_dir() => {
            Err(rms_error(code, "RMS suite path is not a directory"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(code, error)),
    }
}

fn read_store_file(path: &Path, limits: Limits) -> Result<Vec<u8>, EmuError> {
    if !fs::symlink_metadata(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                rms_error("store-not-found", "record store does not exist")
            } else {
                io_error("open-store", error)
            }
        })?
        .is_file()
    {
        return Err(rms_error(
            "corrupt-store",
            "record store is not a regular file",
        ));
    }
    let file = File::open(path).map_err(|error| io_error("open-store", error))?;
    let length = file
        .metadata()
        .map_err(|error| io_error("open-store", error))?
        .len();
    if length > limits.max_store_bytes as u64 {
        return Err(rms_error(
            "corrupt-store",
            "record store exceeds byte limit",
        ));
    }
    read_store_bytes(file, limits.max_store_bytes)
}

fn read_store_bytes(file: impl Read, max_store_bytes: usize) -> Result<Vec<u8>, EmuError> {
    let read_limit = u64::try_from(max_store_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error("read-store", error))?;
    if bytes.len() > max_store_bytes {
        return Err(rms_error(
            "corrupt-store",
            "record store exceeds byte limit",
        ));
    }
    Ok(bytes)
}

fn create_dir_all_durable(path: &Path) -> Result<(), EmuError> {
    let mut missing = Vec::new();
    let mut current = path;
    loop {
        if current.as_os_str().is_empty() {
            current = Path::new(".");
        }
        match fs::metadata(current) {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => return Err(rms_error("create-suite", "RMS path is not a directory")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(current.to_path_buf());
                current = current
                    .parent()
                    .ok_or_else(|| rms_error("create-suite", "RMS path has no existing parent"))?;
            }
            Err(error) => return Err(io_error("create-suite", error)),
        }
    }
    for directory in missing.into_iter().rev() {
        match fs::create_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if !directory.is_dir() {
                    return Err(rms_error("create-suite", "RMS path is not a directory"));
                }
            }
            Err(error) => return Err(io_error("create-suite", error)),
        }
        let parent = directory.parent().unwrap_or_else(|| Path::new("."));
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| io_error("sync-suite", error))?;
    }
    Ok(())
}

fn io_error(code: &'static str, error: std::io::Error) -> EmuError {
    EmuError::with_source(Category::Api, code, error.to_string(), error)
}

#[cfg(test)]
#[path = "../../../tests/unit/rms/repository.rs"]
mod tests;
