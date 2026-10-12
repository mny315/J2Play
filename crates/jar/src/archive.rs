//! Shared ZIP admission policy for inspection, class loading, and resource reads.

use super::raw_zip::raw_zip_preflight;
use super::{
    MAX_ARCHIVE_BYTES, MAX_COMPRESSION_RATIO, MAX_ENTRIES, MAX_ENTRY_BYTES,
    MAX_TOTAL_UNCOMPRESSED_BYTES,
};
use diagnostics::{Category, EmuError};
use std::io::{Cursor, Read, Seek};
use zip::ZipArchive;

pub(super) fn open_validated_archive<B: AsRef<[u8]>>(
    bytes: B,
) -> Result<ZipArchive<Cursor<B>>, EmuError> {
    check_archive_size(bytes.as_ref().len())?;
    // ZipArchive indexes entries by decoded name, hiding duplicates. Check
    // the original directory before constructing that index.
    raw_zip_preflight(bytes.as_ref())?;
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        EmuError::with_source(
            Category::Jar,
            "invalid-zip",
            "invalid ZIP/JAR structure",
            error,
        )
    })?;
    validate_archive_entries(&mut archive)?;
    Ok(archive)
}

pub(super) fn check_archive_size(bytes: usize) -> Result<(), EmuError> {
    if u64::try_from(bytes).unwrap_or(u64::MAX) > MAX_ARCHIVE_BYTES {
        return Err(EmuError::new(
            Category::Jar,
            "archive-too-large",
            "archive exceeds size limit",
        ));
    }
    Ok(())
}

fn validate_archive_entries<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<(), EmuError> {
    if archive.len() > MAX_ENTRIES {
        return Err(EmuError::new(
            Category::Jar,
            "too-many-entries",
            format!(
                "archive has {} entries; limit is {MAX_ENTRIES}",
                archive.len()
            ),
        ));
    }
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            EmuError::with_source(
                Category::Jar,
                "invalid-entry",
                format!("cannot read entry {index}"),
                error,
            )
        })?;
        let name = entry.name();
        if name == "/" && entry.is_dir() && entry.size() == 0 {
            continue;
        }
        if !safe_entry_name(name) {
            return Err(EmuError::new(
                Category::Jar,
                "unsafe-entry-name",
                format!("unsafe archive entry: {name:?}"),
            ));
        }
        if entry.size() > MAX_ENTRY_BYTES {
            return Err(EmuError::new(
                Category::Jar,
                "entry-too-large",
                format!("entry {name:?} exceeds {MAX_ENTRY_BYTES} bytes"),
            ));
        }
        total = total.checked_add(entry.size()).ok_or_else(|| {
            EmuError::new(
                Category::Jar,
                "archive-expanded-too-large",
                "expanded size overflows",
            )
        })?;
        if total > MAX_TOTAL_UNCOMPRESSED_BYTES {
            return Err(EmuError::new(
                Category::Jar,
                "archive-expanded-too-large",
                format!("expanded archive exceeds {MAX_TOTAL_UNCOMPRESSED_BYTES} bytes"),
            ));
        }
        let compressed_limit = entry
            .compressed_size()
            .saturating_mul(MAX_COMPRESSION_RATIO);
        if entry.size() > compressed_limit && entry.size() > 1024 * 1024 {
            return Err(EmuError::new(
                Category::Jar,
                "compression-ratio",
                format!("entry {name:?} exceeds compression ratio limit"),
            ));
        }
    }
    Ok(())
}

pub(super) fn safe_entry_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && !name.split('/').any(|component| component == "..")
        && !name
            .as_bytes()
            .get(1)
            .is_some_and(|byte| *byte == b':' && name.as_bytes()[0].is_ascii_alphabetic())
}
