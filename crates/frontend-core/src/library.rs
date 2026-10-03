use crate::{
    GameSettings, ImportSource, LaunchPlan, PreparedImport, ProfileChoice, inspect_import,
};
use diagnostics::{Category, EmuError, bounded_text};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

mod app_settings;
pub(crate) mod checkpoint;
mod entry;
mod folders;
mod game_info;
mod import_draft;
mod storage;
pub use app_settings::{
    AccentColor, AppSettings, AppTheme, LibraryView, MAX_UI_SCALE_PERCENT, MIN_UI_SCALE_PERCENT,
};
pub use entry::LibraryEntry;
use entry::{
    CachedAutomaticProfile, ENTRY_SCHEMA_VERSION, MAX_ENTRY_METADATA_BYTES, descriptor_slot_name,
    entry_id, validate_entry_id,
};
pub use folders::{LibraryFolder, LibraryFolders, MAX_FOLDER_NAME_CHARS, MAX_LIBRARY_FOLDERS};
use game_info::GameInfo;
pub use game_info::RecoveredGameInfo;
use storage::{atomic_write, read_bounded_file, remove_optional_file, remove_optional_tree};

const MAX_PRIVATE_JAR_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PRIVATE_JAD_BYTES: u64 = 1024 * 1024;
const MAX_WARNING_BYTES: usize = 512;
const MAX_ICON_COMPRESSED_BYTES: u64 = 1024 * 1024;
const MAX_FRONTEND_STATE_BYTES: u64 = 128;
const FULLSCREEN_HELP_ACKNOWLEDGEMENT: &[u8] = b"schema-version=1\nfullscreen-help=acknowledged\n";
pub const MAX_LIBRARY_ENTRIES: usize = 4_096;
const MAX_LIBRARY_DIRECTORY_SCAN: usize = MAX_LIBRARY_ENTRIES * 2 + 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryWarning {
    pub record: String,
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LibraryLoad {
    pub entries: Vec<LibraryEntry>,
    pub warnings: Vec<LibraryWarning>,
}

/// App-private filesystem repository. Each entry is a separate bounded JSON
/// record so one damaged record never prevents the rest of the library loading.
#[derive(Clone, Debug)]
pub struct LibraryRepository {
    root: PathBuf,
    #[cfg(any(feature = "aot", test))]
    cache_root: PathBuf,
    instance_lock: Option<std::sync::Arc<fs::File>>,
    default_virtual_controls_visible: bool,
}

impl LibraryRepository {
    /// Retains the shell's locked file until the last repository owner exits,
    /// including a worker whose shutdown returned a deadline error.
    #[must_use]
    pub fn with_instance_lock(mut self, lock: fs::File) -> Self {
        self.instance_lock = Some(std::sync::Arc::new(lock));
        self
    }

    /// Opens or creates a repository below the app-private directory selected
    /// by the platform shell.
    ///
    /// # Errors
    /// Returns a platform diagnostic when required directories cannot be created.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, EmuError> {
        let root = root.into();
        let cache_root = root.join("native-code-cache");
        Self::open_with_cache(root, cache_root)
    }

    /// Opens persistent data separately from the shell's disposable AOT cache.
    /// No existing data or caches are moved or deleted.
    ///
    /// # Errors
    /// Returns a platform diagnostic when required data directories cannot be created.
    pub fn open_with_cache(
        root: impl Into<PathBuf>,
        cache_root: impl Into<PathBuf>,
    ) -> Result<Self, EmuError> {
        #[cfg(not(any(feature = "aot", test)))]
        let _ = cache_root;
        let repository = Self {
            root: root.into(),
            #[cfg(any(feature = "aot", test))]
            cache_root: cache_root.into(),
            instance_lock: None,
            default_virtual_controls_visible: true,
        };
        fs::create_dir_all(repository.entries_dir()).map_err(|error| {
            io_error(
                "library-create-entries",
                "cannot create library metadata directory",
                error,
            )
        })?;
        fs::create_dir_all(repository.archives_dir()).map_err(|error| {
            io_error(
                "library-create-archives",
                "cannot create private archive directory",
                error,
            )
        })?;
        Ok(repository)
    }

    /// Loads independently validated entries in stable title/id order.
    /// Corrupt records are reported as bounded warnings and skipped.
    ///
    /// # Errors
    /// Returns a platform diagnostic when the metadata directory itself is unreadable.
    pub fn load(&self) -> Result<LibraryLoad, EmuError> {
        let profiles = launch::builtin_device_profiles()?;
        let catalog_fingerprint = crate::catalog_fingerprint(profiles);
        let directory = fs::read_dir(self.entries_dir()).map_err(|error| {
            io_error(
                "library-read",
                "cannot read library metadata directory",
                error,
            )
        })?;
        let mut loaded = LibraryLoad::default();
        let mut paths = Vec::with_capacity(MAX_LIBRARY_ENTRIES);
        let mut capacity_reached = false;
        for (index, result) in directory.enumerate() {
            if index >= MAX_LIBRARY_DIRECTORY_SCAN {
                capacity_reached = true;
                break;
            }
            match result {
                Ok(entry)
                    if entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "json") =>
                {
                    paths.push(entry.path());
                }
                Ok(_) => {}
                Err(error) => loaded.warnings.push(LibraryWarning {
                    record: "<directory-entry>".to_owned(),
                    code: "library-read-entry".to_owned(),
                    detail: bounded_text(&error.to_string(), MAX_WARNING_BYTES),
                }),
            }
        }
        paths.sort();
        for path in paths {
            // Corrupt metadata consumes scan capacity, not a recoverable-entry
            // slot. Otherwise a sorted prefix of broken files hides valid games.
            if loaded.entries.len() == MAX_LIBRARY_ENTRIES {
                capacity_reached = true;
                break;
            }
            match Self::load_entry_path(&path, profiles, &catalog_fingerprint) {
                Ok(entry) => loaded.entries.push(entry),
                Err(error) => loaded.warnings.push(LibraryWarning {
                    record: path.file_name().map_or_else(
                        || "<invalid>".to_owned(),
                        |name| name.to_string_lossy().into_owned(),
                    ),
                    code: error.code().to_owned(),
                    detail: bounded_text(error.message(), MAX_WARNING_BYTES),
                }),
            }
        }
        if capacity_reached {
            loaded.warnings.push(LibraryWarning {
                record: "<library>".to_owned(),
                code: "library-entry-capacity".to_owned(),
                detail: format!(
                    "library scan is limited to {MAX_LIBRARY_ENTRIES} recoverable entries"
                ),
            });
        }
        loaded
            .entries
            .sort_by_cached_key(|entry| (entry.title.to_lowercase(), entry.id.clone()));
        Ok(loaded)
    }

    /// Reads one current record for launch without loading or sorting other games.
    pub(crate) fn load_entry(&self, id: &str) -> Result<LibraryEntry, EmuError> {
        validate_entry_id(id)?;
        let profiles = launch::builtin_device_profiles()?;
        Self::load_entry_path(
            &self.entries_dir().join(format!("{id}.json")),
            profiles,
            &crate::catalog_fingerprint(profiles),
        )
    }

    /// Atomically stores the exact validated picker bytes and then publishes
    /// one metadata record. No source URI or source path is persisted.
    ///
    /// # Errors
    /// Returns a controlled settings, serialization, or app-private I/O diagnostic.
    pub fn commit_import(
        &self,
        prepared: &PreparedImport,
        settings: GameSettings,
    ) -> Result<LibraryEntry, EmuError> {
        let profiles = launch::builtin_device_profiles()?;
        let catalog_fingerprint = crate::catalog_fingerprint(profiles);
        let id = entry_id(prepared.jar_sha256(), prepared.midlet().index);
        let metadata_path = self.entries_dir().join(format!("{id}.json"));
        let destination_exists = match fs::symlink_metadata(&metadata_path) {
            Ok(metadata) if metadata.file_type().is_file() => true,
            Ok(_) => {
                return Err(library_error(
                    "library-entry-target",
                    "library metadata destination is not a regular file",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(io_error(
                    "library-entry-target",
                    "cannot inspect library metadata destination",
                    error,
                ));
            }
        };
        if !destination_exists {
            self.ensure_new_entry_capacity()?;
        }
        let previous_entry = destination_exists
            .then(|| Self::load_entry_path(&metadata_path, profiles, &catalog_fingerprint).ok())
            .flatten();
        let folder_id = previous_entry.as_ref().and_then(|entry| entry.folder_id);
        let previous_descriptor = previous_entry.and_then(|entry| entry.private_jad);
        let private_archive = format!("{id}.jar");
        let private_descriptor = prepared.jad_bytes().map(|_| {
            let slot =
                usize::from(previous_descriptor.as_deref() == Some(&descriptor_slot_name(&id, 0)));
            descriptor_slot_name(&id, slot)
        });
        let descriptor_digest = prepared
            .jad_bytes()
            .map(|bytes| format!("{:x}", Sha256::digest(bytes)));
        let entry = LibraryEntry {
            schema_version: ENTRY_SCHEMA_VERSION,
            id,
            title: prepared.title().to_owned(),
            folder_id,
            game_info: Some(GameInfo::from_properties(&prepared.suite().properties)),
            midlet_index: prepared.midlet().index,
            midlet_class: prepared.midlet().class_name.clone(),
            icon_resource: prepared.midlet().icon.clone(),
            jar_sha256: prepared.jar_sha256().to_owned(),
            archive_leaf_name: prepared.archive_leaf_name().map(str::to_owned),
            private_jar: private_archive,
            private_jad: private_descriptor,
            jad_sha256: descriptor_digest,
            settings,
            cached_automatic_profile: CachedAutomaticProfile::from_summary(
                prepared.profile_catalog_fingerprint().to_owned(),
                prepared.automatic_profile_summary(),
            ),
        };
        entry.validate_with_profiles(profiles, &catalog_fingerprint)?;
        let metadata = entry.encode()?;
        atomic_write(
            &self.archives_dir().join(&entry.private_jar),
            prepared.jar_bytes(),
        )?;
        if let (Some(name), Some(bytes)) = (entry.private_jad.as_deref(), prepared.jad_bytes()) {
            atomic_write(&self.archives_dir().join(name), bytes)?;
        }
        atomic_write(&metadata_path, &metadata)?;
        Ok(entry)
    }

    /// Re-inspects the app-private bytes and recreates an authoritative launch
    /// preparation. Cached automatic metadata is never used here.
    ///
    /// # Errors
    /// Returns a controlled metadata, archive, or profile diagnostic.
    pub fn prepare_launch(&self, entry: &LibraryEntry) -> Result<PreparedImport, EmuError> {
        self.prepare_launch_cancellable(entry, || false)
    }

    /// Re-inspects private bytes with cancellation between bounded preparation
    /// stages and while reading the archive.
    ///
    /// # Errors
    /// Returns the preparation diagnostic or an explicit cancellation error.
    pub fn prepare_launch_cancellable(
        &self,
        entry: &LibraryEntry,
        cancelled: impl Fn() -> bool,
    ) -> Result<PreparedImport, EmuError> {
        let check_cancelled = || {
            if cancelled() {
                Err(library_error(
                    "preparation-cancelled",
                    "Game preparation was cancelled",
                ))
            } else {
                Ok(())
            }
        };
        check_cancelled()?;
        entry.validate()?;
        let archive_bytes = storage::read_bounded_file_cancellable(
            &self.archives_dir().join(&entry.private_jar),
            MAX_PRIVATE_JAR_BYTES,
            "library-private-jar",
            &cancelled,
        )?;
        check_cancelled()?;
        let descriptor_bytes = self.read_private_jad(entry)?;
        check_cancelled()?;
        let inspected = inspect_import(ImportSource::new(
            entry.archive_leaf_name.clone(),
            archive_bytes,
            descriptor_bytes,
        ))?;
        check_cancelled()?;
        if inspected.jar_sha256() != entry.jar_sha256 {
            return Err(library_error(
                "library-jar-integrity",
                "private JAR digest no longer matches library metadata",
            ));
        }
        let prepared = inspected.select_midlet(entry.midlet_index)?;
        check_cancelled()?;
        if prepared.midlet().class_name != entry.midlet_class {
            return Err(library_error(
                "library-midlet-integrity",
                "selected MIDlet class no longer matches library metadata",
            ));
        }
        Ok(prepared)
    }

    /// Atomically replaces per-game settings in the current metadata record,
    /// preserving imports, folder moves and caches newer than the caller's copy.
    ///
    /// # Errors
    /// Returns a controlled settings, serialization, or app-private I/O diagnostic.
    pub fn save_settings(
        &self,
        entry: &mut LibraryEntry,
        settings: GameSettings,
    ) -> Result<(), EmuError> {
        let mut updated = self.load_entry(entry.id())?;
        updated.settings = settings;
        updated.validate()?;
        self.write_entry(&updated)?;
        *entry = updated;
        Ok(())
    }

    /// Atomically refreshes display-only automatic resolution metadata after
    /// an authoritative launch preparation. Preserves newer metadata and ignores
    /// results after descriptor/name changes or a switch to a manual profile.
    ///
    /// # Errors
    /// Returns a controlled serialization or app-private I/O diagnostic.
    pub fn cache_launch_resolution(
        &self,
        entry: &mut LibraryEntry,
        plan: &LaunchPlan,
    ) -> Result<(), EmuError> {
        if !matches!(entry.settings.device_profile, ProfileChoice::Automatic) {
            return Ok(());
        }
        let cached = CachedAutomaticProfile::from_summary(
            plan.profile_catalog.clone(),
            plan.profile_summary(),
        );
        if entry.cached_automatic_profile == cached {
            return Ok(());
        }
        let mut updated = self.load_entry(entry.id())?;
        if updated.jad_sha256 == entry.jad_sha256
            && updated.archive_leaf_name == entry.archive_leaf_name
            && matches!(updated.settings.device_profile, ProfileChoice::Automatic)
            && updated.cached_automatic_profile != cached
        {
            updated.cached_automatic_profile = cached;
            updated.validate()?;
            self.write_entry(&updated)?;
        }
        *entry = updated;
        Ok(())
    }

    /// Removes only the library metadata record. The private JAR/JAD and all
    /// runtime data remain owned by the application until separately removed.
    ///
    /// # Errors
    /// Returns a controlled app-private I/O diagnostic when the record cannot
    /// be removed. A record that is already absent is treated as removed.
    pub fn delete_entry_record(&self, entry: &LibraryEntry) -> Result<(), EmuError> {
        entry.validate()?;
        remove_optional_file(
            &self.entries_dir().join(format!("{}.json", entry.id())),
            "library-delete-entry",
            "cannot remove library metadata record",
        )
    }

    /// Removes only the app-private archive copy (JAR and optional JAD). The
    /// library record and RMS/FileConnection data remain untouched.
    ///
    /// # Errors
    /// Returns a controlled app-private I/O diagnostic when an archive cannot
    /// be removed. Archives that are already absent are treated as removed.
    pub fn delete_private_archives(&self, entry: &LibraryEntry) -> Result<(), EmuError> {
        entry.validate()?;
        remove_optional_file(
            &self.archives_dir().join(&entry.private_jar),
            "library-delete-jar",
            "cannot remove private JAR copy",
        )?;
        let possible_descriptors = [
            format!("{}.jad", entry.id()),
            descriptor_slot_name(entry.id(), 0),
            descriptor_slot_name(entry.id(), 1),
        ];
        for descriptor in possible_descriptors {
            remove_optional_file(
                &self.archives_dir().join(descriptor),
                "library-delete-jad",
                "cannot remove private JAD copy",
            )?;
        }
        Ok(())
    }

    /// Removes RMS, `FileConnection` data and resume snapshots for this entry. The
    /// library record and private archives remain untouched.
    ///
    /// # Errors
    /// Returns a controlled app-private I/O diagnostic when runtime data cannot
    /// be removed. Data that is already absent is treated as removed.
    pub fn delete_runtime_data(&self, entry: &LibraryEntry) -> Result<(), EmuError> {
        entry.validate()?;
        remove_optional_tree(
            &self.rms_root(entry),
            "library-delete-rms",
            "cannot remove RMS data",
        )?;
        remove_optional_tree(
            &self.file_root(entry),
            "library-delete-files",
            "cannot remove FileConnection data",
        )?;
        self.delete_checkpoints(entry)
    }

    /// Removes a game, its app-private archives, settings and all saved data.
    /// The caller must first stop any session using this entry. Source archives
    /// outside the library are never touched. The listing is removed last so
    /// interrupted cleanup can be retried from the library.
    ///
    /// # Errors
    /// Returns the first cleanup error. Already removed data stays removed;
    /// retrying is safe even when some files are already absent.
    pub fn delete_game(&self, entry: &LibraryEntry) -> Result<(), EmuError> {
        self.delete_runtime_data(entry)?;
        self.delete_private_archives(entry)?;
        self.delete_entry_record(entry)
    }

    #[must_use]
    pub fn private_jar_path(&self, entry: &LibraryEntry) -> PathBuf {
        self.archives_dir().join(&entry.private_jar)
    }

    pub(crate) fn rms_root(&self, entry: &LibraryEntry) -> PathBuf {
        self.root.join("runtime").join("rms").join(entry.id())
    }

    pub(crate) fn file_root(&self, entry: &LibraryEntry) -> PathBuf {
        self.root.join("runtime").join("files").join(entry.id())
    }

    #[cfg(any(feature = "aot", test))]
    pub(crate) fn native_code_cache_root(&self) -> PathBuf {
        self.cache_root.clone()
    }

    /// Reads the declared icon under a strict compressed/decompressed resource
    /// ceiling. Decode limits remain the responsibility of the shared UI.
    ///
    /// # Errors
    /// Returns a controlled archive diagnostic for invalid or oversized icon data.
    pub fn read_icon_bytes(
        &self,
        entry: &LibraryEntry,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Option<Vec<u8>>, EmuError> {
        let Some(resource) = entry.icon_resource() else {
            return Ok(None);
        };
        let bytes = storage::read_bounded_file_cancellable(
            &self.private_jar_path(entry),
            MAX_PRIVATE_JAR_BYTES,
            "library-icon-read",
            &mut cancelled,
        )?;
        let archive = jar::ResourceArchive::from_owned_bytes(bytes)?;
        if cancelled() {
            return Err(library_error(
                "library-icon-read",
                "Icon reading was interrupted",
            ));
        }
        archive.read_with_limit(resource, MAX_ICON_COMPRESSED_BYTES)
    }

    /// Reports whether the one-time fullscreen gesture help was acknowledged.
    /// Missing state is the explicit first-run default.
    ///
    /// # Errors
    /// Returns a controlled diagnostic for malformed or unreadable app-private state.
    pub fn fullscreen_help_acknowledged(&self) -> Result<bool, EmuError> {
        let path = self.fullscreen_help_state_path();
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                let bytes = read_bounded_file(
                    &path,
                    MAX_FRONTEND_STATE_BYTES,
                    "frontend-state-fullscreen-help",
                )?;
                if bytes == FULLSCREEN_HELP_ACKNOWLEDGEMENT {
                    Ok(true)
                } else {
                    Err(library_error(
                        "frontend-state-fullscreen-help",
                        "fullscreen help state has an unsupported schema or value",
                    ))
                }
            }
            Ok(_) => Err(library_error(
                "frontend-state-fullscreen-help",
                "fullscreen help state is not a regular file",
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(io_error(
                "frontend-state-fullscreen-help",
                "cannot inspect fullscreen help state",
                error,
            )),
        }
    }

    /// Atomically acknowledges the one-time fullscreen gesture help.
    ///
    /// # Errors
    /// Returns a controlled diagnostic when app-private state cannot be persisted.
    pub fn acknowledge_fullscreen_help(&self) -> Result<(), EmuError> {
        atomic_write(
            &self.fullscreen_help_state_path(),
            FULLSCREEN_HELP_ACKNOWLEDGEMENT,
        )
    }

    fn entries_dir(&self) -> PathBuf {
        self.root.join("entries")
    }

    fn archives_dir(&self) -> PathBuf {
        self.root.join("archives")
    }

    fn read_private_jad(&self, entry: &LibraryEntry) -> Result<Option<Vec<u8>>, EmuError> {
        entry
            .private_jad
            .as_deref()
            .map(|name| {
                let bytes = read_bounded_file(
                    &self.archives_dir().join(name),
                    MAX_PRIVATE_JAD_BYTES,
                    "library-private-jad",
                )?;
                if entry.jad_sha256.as_ref().is_some_and(|expected| {
                    !format!("{:x}", Sha256::digest(&bytes)).eq_ignore_ascii_case(expected)
                }) {
                    return Err(library_error(
                        "library-jad-integrity",
                        "private JAD digest no longer matches library metadata",
                    ));
                }
                Ok(bytes)
            })
            .transpose()
    }

    fn fullscreen_help_state_path(&self) -> PathBuf {
        self.root.join("frontend-state").join("fullscreen-help-v1")
    }

    fn ensure_new_entry_capacity(&self) -> Result<(), EmuError> {
        let directory = fs::read_dir(self.entries_dir()).map_err(|error| {
            io_error(
                "library-entry-capacity",
                "cannot inspect library metadata capacity",
                error,
            )
        })?;
        let mut records = 0_usize;
        for (index, result) in directory.enumerate() {
            if index >= MAX_LIBRARY_DIRECTORY_SCAN {
                return Err(library_error(
                    "library-entry-capacity",
                    "library metadata directory exceeds its bounded scan capacity",
                ));
            }
            let entry = result.map_err(|error| {
                io_error(
                    "library-entry-capacity",
                    "cannot inspect a library metadata record",
                    error,
                )
            })?;
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                records += 1;
                if records >= MAX_LIBRARY_ENTRIES {
                    return Err(library_error(
                        "library-entry-capacity",
                        format!("library is limited to {MAX_LIBRARY_ENTRIES} entries"),
                    ));
                }
            }
        }
        Ok(())
    }

    fn load_entry_path(
        path: &Path,
        profiles: &[device_profile::DeviceProfile],
        catalog_fingerprint: &str,
    ) -> Result<LibraryEntry, EmuError> {
        let metadata_stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| {
                library_error(
                    "library-entry-name",
                    "library metadata filename is invalid Unicode",
                )
            })?;
        let bytes = read_bounded_file(path, MAX_ENTRY_METADATA_BYTES, "library-entry-read")?;
        let mut entry: LibraryEntry = serde_json::from_slice(&bytes).map_err(|error| {
            EmuError::with_source(
                Category::Platform,
                "library-entry-json",
                "invalid library entry JSON",
                error,
            )
        })?;
        entry.settings.migrate_to_current()?;
        if entry.id != metadata_stem {
            return Err(library_error(
                "library-entry-name",
                "library entry identifier does not match its metadata filename",
            ));
        }
        entry.validate_with_profiles(profiles, catalog_fingerprint)?;
        Ok(entry)
    }

    fn write_entry(&self, entry: &LibraryEntry) -> Result<(), EmuError> {
        atomic_write(
            &self.entries_dir().join(format!("{}.json", entry.id)),
            &entry.encode()?,
        )
    }
}

fn library_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Platform, code, message)
}

fn io_error(code: &'static str, message: &'static str, source: std::io::Error) -> EmuError {
    EmuError::with_source(Category::Platform, code, message, source)
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-core/library/mod.rs"]
mod tests;
