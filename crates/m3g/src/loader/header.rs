//! Versioned file header and its authoring metadata.

use super::{
    Category, EmuError, FileHeader, ObjectType, Section, at_error, loader_error, read_u32,
};

pub(super) fn parse_header_section(section: &Section) -> Result<FileHeader, EmuError> {
    if section.compressed
        || section.objects.len() != 1
        || section.objects[0].object_type != ObjectType::Header
    {
        return Err(at_error(
            "invalid-header-section",
            section.offset,
            "first section must be uncompressed and contain only the header object",
        ));
    }
    parse_file_header(&section.objects[0].data)
}

fn parse_file_header(data: &[u8]) -> Result<FileHeader, EmuError> {
    if data.len() < 12 {
        return Err(loader_error(
            "truncated-header",
            "M3G header object is too short",
        ));
    }
    let major_version = data[0];
    let minor_version = data[1];
    if (major_version, minor_version) != (1, 0) {
        return Err(loader_error(
            "unsupported-file-version",
            format!("unsupported M3G file version {major_version}.{minor_version}"),
        ));
    }
    let has_external_references = parse_boolean(data[2], "header external-reference flag")?;
    let total_file_size = read_u32(data, 3)?;
    let approximate_content_size = read_u32(data, 7)?;
    let author = &data[11..];
    let Some(terminator) = author.iter().position(|byte| *byte == 0) else {
        return Err(loader_error(
            "unterminated-string",
            "M3G header authoring string is not NUL-terminated",
        ));
    };
    if terminator + 1 != author.len() {
        return Err(loader_error(
            "extra-object-data",
            "M3G header contains bytes after its authoring string",
        ));
    }
    let authoring_field = std::str::from_utf8(&author[..terminator])
        .map_err(|source| {
            EmuError::with_source(
                Category::M3g,
                "invalid-utf8",
                "M3G header authoring string is not UTF-8",
                source,
            )
        })?
        .to_owned();
    Ok(FileHeader {
        major_version,
        minor_version,
        has_external_references,
        total_file_size,
        approximate_content_size,
        authoring_field,
    })
}

fn parse_boolean(value: u8, field: &'static str) -> Result<bool, EmuError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(loader_error(
            "invalid-boolean",
            format!("{field} must be encoded as 0 or 1"),
        )),
    }
}
