use super::storage::{atomic_write_parts, open_bounded_file, remove_optional_file};
use super::{
    ImportSource, LibraryRepository, MAX_PRIVATE_JAD_BYTES, MAX_PRIVATE_JAR_BYTES, library_error,
};
use crate::preparation::normalized_leaf_name;
use diagnostics::EmuError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;

const MAX_HEADER_BYTES: usize = 4096;
const DRAFT_NAME: &str = "pending-import.bin";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftHeader {
    schema_version: u32,
    archive_leaf_name: Option<String>,
    jar_length: usize,
    jad_length: Option<usize>,
}

impl LibraryRepository {
    /// Atomically retains an unfinished import across platform recreation.
    /// The record holds only bounded private bytes and a normalized leaf name.
    ///
    /// # Errors
    /// Returns a bounded-input or app-private storage diagnostic.
    pub fn stage_import(&self, source: &ImportSource) -> Result<(), EmuError> {
        let header = DraftHeader {
            schema_version: 1,
            archive_leaf_name: normalized_leaf_name(source.archive_leaf_name.as_deref())?,
            jar_length: source.jar_bytes.len(),
            jad_length: source.jad_bytes.as_ref().map(Vec::len),
        };
        validate_header(&header)?;
        let header = serde_json::to_vec(&header)
            .map_err(|_| draft_error("Cannot encode the pending import header"))?;
        if header.len() > MAX_HEADER_BYTES {
            return Err(draft_error("The pending import header is oversized"));
        }
        let header_length = u32::try_from(header.len())
            .map_err(|_| draft_error("The pending import header is oversized"))?
            .to_le_bytes();
        atomic_write_parts(
            &self.root.join(DRAFT_NAME),
            &[
                &header_length,
                &header,
                &source.jar_bytes,
                source.jad_bytes.as_deref().unwrap_or_default(),
            ],
        )
    }

    /// Loads the last complete import draft. A partial temporary write is never used.
    ///
    /// # Errors
    /// Returns a diagnostic for an unreadable, malformed or oversized draft.
    pub fn restore_import(&self) -> Result<Option<ImportSource>, EmuError> {
        let path = self.root.join(DRAFT_NAME);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Ok(metadata) if metadata.file_type().is_file() => {}
            _ => {
                return Err(draft_error(
                    "The pending import is not a readable regular file",
                ));
            }
        }
        let (mut file, length) = open_bounded_file(
            &path,
            MAX_PRIVATE_JAR_BYTES + MAX_PRIVATE_JAD_BYTES + MAX_HEADER_BYTES as u64 + 4,
            "import-draft-read",
        )?;
        read_draft(&mut file, length).map(Some)
    }

    /// Removes temporary import state after confirmation or cancellation.
    ///
    /// # Errors
    /// Returns a diagnostic if the private draft cannot be removed durably.
    pub fn discard_import_draft(&self) -> Result<(), EmuError> {
        remove_optional_file(
            &self.root.join(DRAFT_NAME),
            "import-draft-remove",
            "Cannot discard the pending import",
        )
    }
}

fn read_draft(reader: &mut impl Read, length: u64) -> Result<ImportSource, EmuError> {
    let mut prefix = [0; 4];
    reader
        .read_exact(&mut prefix)
        .map_err(|_| draft_error("The pending import header is truncated"))?;
    let header_length = usize::try_from(u32::from_le_bytes(prefix))
        .map_err(|_| draft_error("The pending import header is oversized"))?;
    if header_length > MAX_HEADER_BYTES {
        return Err(draft_error("The pending import header is oversized"));
    }
    let header_bytes = read_draft_bytes(reader, header_length)?;
    let header: DraftHeader = serde_json::from_slice(&header_bytes)
        .map_err(|_| draft_error("The pending import header is invalid"))?;
    validate_header(&header)?;
    let expected_length =
        4 + header_length + header.jar_length + header.jad_length.unwrap_or_default();
    if length != expected_length as u64 {
        return Err(draft_error(
            "The pending import payload has an invalid length",
        ));
    }
    let jar = read_draft_bytes(reader, header.jar_length)?;
    let jad = header
        .jad_length
        .map(|length| read_draft_bytes(reader, length))
        .transpose()?;
    // read_exact retries interrupted system calls, including this EOF probe.
    match reader.read_exact(&mut [0]) {
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            Ok(ImportSource::new(header.archive_leaf_name, jar, jad))
        }
        Ok(()) => Err(draft_error(
            "The pending import payload has an invalid length",
        )),
        Err(_) => Err(draft_error("Cannot read the pending import payload")),
    }
}

fn read_draft_bytes(reader: &mut impl Read, length: usize) -> Result<Vec<u8>, EmuError> {
    let mut bytes = Vec::with_capacity(length);
    reader
        .take(length as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| draft_error("Cannot read the pending import data"))?;
    if bytes.len() != length {
        return Err(draft_error("The pending import data is truncated"));
    }
    Ok(bytes)
}

fn validate_header(header: &DraftHeader) -> Result<(), EmuError> {
    if header.schema_version != 1
        || header.jar_length as u64 > MAX_PRIVATE_JAR_BYTES
        || header
            .jad_length
            .is_some_and(|length| length as u64 > MAX_PRIVATE_JAD_BYTES)
        || normalized_leaf_name(header.archive_leaf_name.as_deref())? != header.archive_leaf_name
    {
        return Err(draft_error(
            "The pending import has an unsupported schema or invalid bounds",
        ));
    }
    Ok(())
}

fn draft_error(message: &str) -> EmuError {
    library_error("import-draft", message)
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-core/library/import_draft/mod.rs"]
mod tests;
