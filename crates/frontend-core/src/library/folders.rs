//! Library organization only: folder names never become filesystem paths.

use super::{
    LibraryEntry, LibraryRepository, atomic_write, io_error, library_error, read_bounded_file,
};
use diagnostics::EmuError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_LIBRARY_FOLDERS: usize = 256;
pub const MAX_FOLDER_NAME_CHARS: usize = 64;
const MAX_FOLDERS_BYTES: u64 = 128 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryFolder {
    id: u64,
    name: String,
}

impl LibraryFolder {
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryFolders {
    schema_version: u32,
    next_id: u64,
    folders: Vec<LibraryFolder>,
}

impl Default for LibraryFolders {
    fn default() -> Self {
        Self {
            schema_version: 1,
            next_id: 1,
            folders: Vec::new(),
        }
    }
}

impl LibraryFolders {
    #[must_use]
    pub fn folders(&self) -> &[LibraryFolder] {
        &self.folders
    }

    #[must_use]
    pub fn get(&self, id: u64) -> Option<&LibraryFolder> {
        self.folders.iter().find(|folder| folder.id == id)
    }

    /// Deleted or unavailable folders leave their games visible without a folder.
    #[must_use]
    pub fn folder_for(&self, entry: &LibraryEntry) -> Option<u64> {
        entry.folder_id.filter(|id| self.get(*id).is_some())
    }

    /// # Errors
    /// Rejects blank, oversized, control-containing or duplicate names.
    pub fn validate_name(&self, name: &str, except: Option<u64>) -> Result<(), EmuError> {
        let name = name.trim();
        validate_name(name)?;
        let normalized = name.to_lowercase();
        if self
            .folders
            .iter()
            .any(|folder| Some(folder.id) != except && folder.name.to_lowercase() == normalized)
        {
            return Err(library_error(
                "library-folder-duplicate",
                "a folder with this name already exists",
            ));
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), EmuError> {
        if self.schema_version != 1 || self.next_id == 0 || self.folders.len() > MAX_LIBRARY_FOLDERS
        {
            return Err(library_error(
                "library-folders-schema",
                "invalid folder schema, identity sequence or capacity",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for folder in &self.folders {
            validate_name(&folder.name)?;
            if folder.id == 0
                || folder.id >= self.next_id
                || !ids.insert(folder.id)
                || !names.insert(folder.name.to_lowercase())
                || folder.name.trim() != folder.name
            {
                return Err(library_error(
                    "library-folder-record",
                    "invalid or duplicate folder record",
                ));
            }
        }
        Ok(())
    }

    fn sort(&mut self) {
        self.folders
            .sort_by_cached_key(|folder| (folder.name.to_lowercase(), folder.id));
    }
}

fn validate_name(name: &str) -> Result<(), EmuError> {
    if name.trim().is_empty()
        || name.chars().count() > MAX_FOLDER_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(library_error(
            "library-folder-name",
            "folder name must contain 1–64 characters without control characters",
        ));
    }
    Ok(())
}

impl LibraryRepository {
    /// Missing metadata means an older library with no folders. Corruption is
    /// reported separately, so callers can keep games available and disable
    /// folder edits without overwriting an unreadable catalog.
    ///
    /// # Errors
    /// Returns bounded storage or folder validation diagnostics.
    pub fn load_folders(&self) -> Result<LibraryFolders, EmuError> {
        let path = self.root.join("folders.json");
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LibraryFolders::default());
            }
            Err(error) => {
                return Err(io_error(
                    "library-folders-read",
                    "cannot inspect library folders",
                    error,
                ));
            }
            Ok(_) => {}
        }
        let bytes = read_bounded_file(&path, MAX_FOLDERS_BYTES, "library-folders-read")?;
        let mut folders: LibraryFolders = serde_json::from_slice(&bytes).map_err(|error| {
            diagnostics::EmuError::with_source(
                diagnostics::Category::Platform,
                "library-folders-json",
                "cannot decode library folders",
                error,
            )
        })?;
        folders.validate()?;
        folders.sort();
        Ok(folders)
    }

    /// Creates a folder and publishes its catalog atomically.
    ///
    /// # Errors
    /// Returns name, capacity or storage diagnostics without changing the catalog.
    pub fn create_folder(&self, name: &str) -> Result<(LibraryFolders, u64), EmuError> {
        let mut folders = self.load_folders()?;
        folders.validate_name(name, None)?;
        if folders.folders.len() >= MAX_LIBRARY_FOLDERS {
            return Err(library_error(
                "library-folder-capacity",
                "library folder limit reached",
            ));
        }
        let id = folders.next_id;
        folders.next_id = id.checked_add(1).ok_or_else(|| {
            library_error(
                "library-folder-capacity",
                "folder identity sequence exhausted",
            )
        })?;
        folders.folders.push(LibraryFolder {
            id,
            name: name.trim().to_owned(),
        });
        self.write_folders(&mut folders)?;
        Ok((folders, id))
    }

    /// # Errors
    /// Returns name, missing-folder or storage diagnostics.
    pub fn rename_folder(&self, id: u64, name: &str) -> Result<LibraryFolders, EmuError> {
        let mut folders = self.load_folders()?;
        folders.validate_name(name, Some(id))?;
        let folder = folders
            .folders
            .iter_mut()
            .find(|folder| folder.id == id)
            .ok_or_else(missing_folder)?;
        name.trim().clone_into(&mut folder.name);
        self.write_folders(&mut folders)?;
        Ok(folders)
    }

    /// Removes organization metadata only. Entries, archives and runtime data
    /// stay intact. IDs are never reused, so orphaned memberships cannot reappear.
    ///
    /// # Errors
    /// Returns missing-folder or storage diagnostics.
    pub fn delete_folder(&self, id: u64) -> Result<LibraryFolders, EmuError> {
        let mut folders = self.load_folders()?;
        if folders.get(id).is_none() {
            return Err(missing_folder());
        }
        folders.folders.retain(|folder| folder.id != id);
        self.write_folders(&mut folders)?;
        Ok(folders)
    }

    /// Atomically changes organization independently of game settings.
    ///
    /// # Errors
    /// Returns missing-folder, validation or storage diagnostics.
    pub fn move_to_folder(
        &self,
        entry: &mut LibraryEntry,
        folder_id: Option<u64>,
    ) -> Result<(), EmuError> {
        let folders = self.load_folders()?;
        if folder_id.is_some_and(|id| folders.get(id).is_none()) {
            return Err(missing_folder());
        }
        let mut updated = self.load_entry(entry.id())?;
        updated.folder_id = folder_id;
        updated.validate()?;
        self.write_entry(&updated)?;
        *entry = updated;
        Ok(())
    }

    fn write_folders(&self, folders: &mut LibraryFolders) -> Result<(), EmuError> {
        folders.validate()?;
        folders.sort();
        let bytes = serde_json::to_vec_pretty(folders).map_err(|error| {
            diagnostics::EmuError::with_source(
                diagnostics::Category::Platform,
                "library-folders-json",
                "cannot encode library folders",
                error,
            )
        })?;
        if bytes.len() as u64 > MAX_FOLDERS_BYTES {
            return Err(library_error(
                "library-folders-size",
                "folder metadata exceeds its size limit",
            ));
        }
        atomic_write(&self.root.join("folders.json"), &bytes)
    }
}

fn missing_folder() -> EmuError {
    library_error("library-folder-missing", "library folder no longer exists")
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-core/library/folders.rs"]
mod tests;
