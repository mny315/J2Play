//! Reusable suite resource lookup and bounded eager class loading.

use super::archive::{open_validated_archive, safe_entry_name};
use super::bounded_io::{read_bounded_entry, read_regular_file_bounded};
use super::{ClassResource, MAX_ARCHIVE_BYTES, MAX_ENTRY_BYTES};
use diagnostics::{Category, EmuError};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use zip::ZipArchive;

/// Validated, reusable reader for resources in one suite archive.
pub struct ResourceArchive<B = Vec<u8>> {
    archive: Mutex<ZipArchive<Cursor<B>>>,
    folded_names: OnceLock<BTreeMap<String, Option<usize>>>,
}

impl ResourceArchive {
    /// Opens and validates a suite archive once for repeated resource reads.
    ///
    /// # Errors
    /// Returns a categorized error for an inaccessible, oversized or unsafe archive.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, EmuError> {
        let path = path.as_ref();
        let bytes = read_regular_file_bounded(
            path,
            MAX_ARCHIVE_BYTES,
            "open",
            "read",
            "archive-too-large",
            "archive",
        )?;
        Self::from_owned_bytes(bytes)
    }

    /// Borrows and validates in-memory bytes for repeated resource reads without
    /// duplicating the archive. The caller retains ownership of the bytes.
    ///
    /// # Errors
    /// Returns a categorized error for oversized, malformed, or unsafe input.
    pub fn from_bytes(bytes: &[u8]) -> Result<ResourceArchive<&[u8]>, EmuError> {
        ResourceArchive::from_storage(bytes)
    }

    /// Opens and validates owned in-memory bytes without duplicating the archive.
    ///
    /// # Errors
    /// Returns a categorized error for oversized, malformed, or unsafe input.
    pub fn from_owned_bytes(bytes: Vec<u8>) -> Result<Self, EmuError> {
        Self::from_storage(bytes)
    }
}

impl<B: AsRef<[u8]>> ResourceArchive<B> {
    fn from_storage(bytes: B) -> Result<Self, EmuError> {
        let archive = open_validated_archive(bytes)?;
        Ok(Self {
            archive: Mutex::new(archive),
            folded_names: OnceLock::new(),
        })
    }

    /// Reads all class resources from these already validated archive bytes.
    ///
    /// # Errors
    /// Returns a categorized diagnostic for a poisoned lock or invalid class entry.
    pub fn class_entries(&self) -> Result<Vec<ClassResource>, EmuError> {
        let mut archive = self.lock()?;
        let mut classes = Vec::new();
        for index in 0..archive.len() {
            let name = archive.name_for_index(index).ok_or_else(|| {
                EmuError::new(
                    Category::Jar,
                    "invalid-entry",
                    format!("cannot index class entry {index}"),
                )
            })?;
            if !Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("class"))
            {
                continue;
            }
            let name = name.to_owned();
            let mut entry = archive.by_index(index).map_err(|error| {
                EmuError::with_source(
                    Category::Jar,
                    "invalid-entry",
                    format!("cannot read entry {index}"),
                    error,
                )
            })?;
            let declared = entry.size();
            let bytes = read_bounded_entry(&mut entry, declared, MAX_ENTRY_BYTES, "read-class")?;
            if !bytes.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) {
                // Ignore non-class resources with a .class suffix. A reference to
                // one still fails normal class resolution.
                continue;
            }
            classes.push(ClassResource { name, bytes });
        }
        Ok(classes)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ZipArchive<Cursor<B>>>, EmuError> {
        self.archive.lock().map_err(|_| {
            EmuError::new(
                Category::Jar,
                "resource-lock",
                "suite resource archive lock is poisoned",
            )
        })
    }

    /// Reads one bounded resource without re-opening or re-validating the archive.
    ///
    /// # Errors
    /// Returns a categorized error for an unsafe name, failed lock or ZIP read error.
    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>, EmuError> {
        self.read_with_limit(name, MAX_ENTRY_BYTES)
    }

    /// Reads one resource with an additional caller-provided byte ceiling.
    /// The normal archive entry limit remains authoritative when it is lower.
    ///
    /// # Errors
    /// Returns a categorized error for an unsafe name, failed lock, oversized
    /// resource, or ZIP read error.
    pub fn read_with_limit(
        &self,
        name: &str,
        maximum_bytes: u64,
    ) -> Result<Option<Vec<u8>>, EmuError> {
        let normalized = name.strip_prefix('/').unwrap_or(name);
        if !safe_entry_name(normalized) {
            return Err(EmuError::new(
                Category::Jar,
                "unsafe-resource",
                format!("unsafe resource name: {name}"),
            ));
        }
        let mut archive = self.lock()?;
        // Sony Ericsson's deployed JAR resource lookup was case-insensitive.
        // Preserve exact ZIP semantics first, then accept one unambiguous ASCII
        // case-only match for old games whose source and archive disagree.
        // Exact reads reuse the ZIP's existing index. Copy its names only
        // when a fallback lookup actually needs the additional folded index.
        let index = archive.index_for_name(normalized).or_else(|| {
            self.folded_names
                .get_or_init(|| {
                    let mut folded_names = BTreeMap::new();
                    for (index, name) in archive.file_names().enumerate() {
                        folded_names
                            .entry(name.to_ascii_lowercase())
                            .and_modify(|slot| *slot = None)
                            .or_insert(Some(index));
                    }
                    folded_names
                })
                .get(&normalized.to_ascii_lowercase())
                .copied()
                .flatten()
        });
        let Some(index) = index else {
            return Ok(None);
        };
        let mut entry = archive.by_index(index).map_err(|error| {
            EmuError::with_source(
                Category::Jar,
                "read-resource",
                format!("cannot open resource {normalized:?}"),
                error,
            )
        })?;
        let declared = entry.size();
        read_bounded_entry(
            &mut entry,
            declared,
            maximum_bytes.min(MAX_ENTRY_BYTES),
            "read-resource",
        )
        .map(Some)
    }
}

/// Reads one resource without extracting it, under the normal archive limits.
///
/// # Errors
/// Returns a categorized error for an inaccessible or unsafe archive/resource.
pub fn read_resource_path(path: impl AsRef<Path>, name: &str) -> Result<Option<Vec<u8>>, EmuError> {
    ResourceArchive::open(path)?.read(name)
}

/// Reads one resource with a caller-provided byte ceiling.
///
/// # Errors
/// Returns a categorized error for inaccessible, unsafe, malformed, or oversized input.
pub fn read_resource_path_with_limit(
    path: impl AsRef<Path>,
    name: &str,
    maximum_bytes: u64,
) -> Result<Option<Vec<u8>>, EmuError> {
    ResourceArchive::open(path)?.read_with_limit(name, maximum_bytes)
}

/// Reads one resource from already opened archive bytes under the normal limits.
///
/// # Errors
/// Returns a categorized error for oversized, malformed, unsafe archive/resource input.
pub fn read_resource_bytes(bytes: &[u8], name: &str) -> Result<Option<Vec<u8>>, EmuError> {
    ResourceArchive::from_bytes(bytes)?.read(name)
}

/// Reads all `.class` resources using the same archive safety policy as inspection.
///
/// # Errors
///
/// Returns an error for inaccessible, malformed, unsafe, or oversized archives.
pub fn read_class_entries_path(path: impl AsRef<Path>) -> Result<Vec<ClassResource>, EmuError> {
    ResourceArchive::open(path)?.class_entries()
}

/// Reads all `.class` resources from an in-memory JAR using the archive safety policy.
///
/// # Errors
///
/// Returns an error for malformed, unsafe, or oversized archives.
pub fn read_class_entries_bytes(bytes: &[u8]) -> Result<Vec<ClassResource>, EmuError> {
    ResourceArchive::from_bytes(bytes)?.class_entries()
}

#[cfg(test)]
#[path = "../../../tests/unit/jar/resources.rs"]
mod tests;
