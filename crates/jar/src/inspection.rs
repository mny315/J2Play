//! Launch metadata and archive timestamps without resource extraction.

use super::archive::open_validated_archive;
use super::bounded_io::{read_bounded_entry, read_regular_file_bounded};
use super::properties::{parse_manifest, parse_midlets};
use super::{JarInfo, MAX_ARCHIVE_BYTES, MAX_MANIFEST_BYTES, ResourceInfo};
use diagnostics::{Category, EmuError};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Reads metadata from a JAR without extracting it.
///
/// # Errors
///
/// Returns a categorized error for inaccessible, oversized or malformed input.
pub fn inspect_path(path: impl AsRef<Path>) -> Result<JarInfo, EmuError> {
    let path = path.as_ref();
    let bytes = read_regular_file_bounded(
        path,
        MAX_ARCHIVE_BYTES,
        "open",
        "read",
        "archive-too-large",
        "archive",
    )?;
    inspect_bytes(&bytes)
}

/// Inspects an in-memory JAR using the same bounded policy as file input.
///
/// # Errors
///
/// Returns a categorized error for oversized or malformed input.
pub fn inspect_bytes(bytes: &[u8]) -> Result<JarInfo, EmuError> {
    let archive_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut archive = open_validated_archive(bytes)?;
    let sha256 = format!("{:x}", Sha256::digest(bytes));

    let mut resources = Vec::with_capacity(archive.len());
    let mut manifest = None;
    let mut manifest_timestamp = None;
    let mut newest_timestamp = None;
    for index in 0..archive.len() {
        let is_manifest = archive
            .name_for_index(index)
            .is_some_and(|name| name.eq_ignore_ascii_case("META-INF/MANIFEST.MF"));
        // Admission already checked every entry. Metadata reads do not need
        // another decompressor for resources whose bytes are never consumed.
        let entry = if is_manifest {
            archive.by_index(index)
        } else {
            archive.by_index_raw(index)
        };
        let mut entry = entry.map_err(|error| {
            EmuError::with_source(
                Category::Jar,
                "invalid-entry",
                format!("cannot read entry {index}"),
                error,
            )
        })?;
        let name = entry.name().to_owned();
        if let Some(timestamp) = entry.last_modified() {
            newest_timestamp = Some(
                newest_timestamp.map_or(timestamp, |current: zip::DateTime| current.max(timestamp)),
            );
            if is_manifest {
                manifest_timestamp = Some(timestamp);
            }
        }
        if is_manifest {
            if manifest.is_some() {
                return Err(EmuError::new(
                    Category::Jar,
                    "duplicate-manifest",
                    "archive contains more than one manifest",
                ));
            }
            if entry.size() > MAX_MANIFEST_BYTES {
                return Err(EmuError::new(
                    Category::Jar,
                    "manifest-too-large",
                    "manifest exceeds 1 MiB",
                ));
            }
            let declared = entry.size();
            let bytes =
                read_bounded_entry(&mut entry, declared, MAX_MANIFEST_BYTES, "read-manifest")?;
            manifest = Some(parse_manifest(&bytes)?);
        }
        resources.push(ResourceInfo {
            name,
            compressed_bytes: entry.compressed_size(),
            uncompressed_bytes: entry.size(),
        });
    }
    let manifest = manifest.ok_or_else(|| {
        EmuError::new(
            Category::Jar,
            "missing-manifest",
            "META-INF/MANIFEST.MF is missing",
        )
    })?;
    let midlets = parse_midlets(&manifest.main)?;
    let archive_timestamp_millis = manifest_timestamp
        .or(newest_timestamp)
        .map(zip_date_time_epoch_millis);
    Ok(JarInfo {
        sha256,
        archive_bytes,
        archive_timestamp_millis,
        manifest: manifest.main,
        manifest_sections: manifest.sections,
        midlets,
        resources,
    })
}

fn zip_date_time_epoch_millis(value: zip::DateTime) -> i64 {
    // ZIP admission already validated the calendar date and the 1980..=2107 range.
    let year = i64::from(value.year());
    let month = i64::from(value.month());
    let day = i64::from(value.day());
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let unix_days = era * 146_097 + day_of_era - 719_468;
    let day_millis = unix_days * 86_400_000;
    let time_millis = i64::from(value.hour()) * 3_600_000
        + i64::from(value.minute()) * 60_000
        + i64::from(value.second()) * 1_000;
    day_millis + time_millis
}

#[cfg(test)]
#[path = "../../../tests/unit/jar/inspection.rs"]
mod tests;
