//! Section checksums, bounded decompression and object record framing.

use super::{
    Category, EmuError, LoaderLimits, ObjectType, ParsedObject, Section, at_error, loader_error,
    read_u32,
};
use flate2::{Decompress, FlushDecompress, Status};
use std::borrow::Cow;

pub(super) fn parse_section(
    file: &[u8],
    offset: usize,
    limits: LoaderLimits,
    decompressed_total: &mut usize,
    prior_objects: usize,
) -> Result<Section, EmuError> {
    let header_end = offset
        .checked_add(9)
        .ok_or_else(|| loader_error("length-overflow", "section header overflow"))?;
    if header_end > file.len() {
        return Err(at_error(
            "truncated-section",
            offset,
            "incomplete section header",
        ));
    }
    let compression = file[offset];
    if compression > 1 {
        return Err(at_error(
            "unsupported-compression",
            offset,
            "reserved M3G compression scheme",
        ));
    }
    let total_length = read_u32(file, offset + 1)?;
    let uncompressed_length = read_u32(file, offset + 5)?;
    if total_length < 13 {
        return Err(at_error(
            "invalid-section-length",
            offset,
            "M3G section is shorter than its fixed fields",
        ));
    }
    if total_length as usize > limits.file_bytes {
        return Err(at_error(
            "resource-limit",
            offset,
            "M3G section exceeds configured file byte budget",
        ));
    }
    let section_end = offset
        .checked_add(total_length as usize)
        .ok_or_else(|| loader_error("length-overflow", "section length overflow"))?;
    if section_end > file.len() {
        return Err(at_error(
            "truncated-section",
            offset,
            "declared section extends past end of file",
        ));
    }
    let payload_end = section_end - 4;
    let expected_checksum = read_u32(file, payload_end)?;
    let actual_checksum = adler32(&file[offset..payload_end]);
    if expected_checksum != actual_checksum {
        return Err(at_error(
            "checksum-mismatch",
            offset,
            "M3G section Adler-32 checksum mismatch",
        ));
    }
    let requested = uncompressed_length as usize;
    let next_total = decompressed_total
        .checked_add(requested)
        .ok_or_else(|| loader_error("length-overflow", "decompressed size overflow"))?;
    if next_total > limits.decompressed_bytes {
        return Err(at_error(
            "resource-limit",
            offset,
            "M3G decompressed byte budget exceeded",
        ));
    }
    let serialized = &file[header_end..payload_end];
    let payload = if compression == 0 {
        if serialized.len() != requested {
            return Err(at_error(
                "length-mismatch",
                offset,
                "uncompressed section length does not match payload",
            ));
        }
        Cow::Borrowed(serialized)
    } else {
        Cow::Owned(inflate_exact(serialized, requested, offset)?)
    };
    *decompressed_total = next_total;
    let objects = parse_objects(&payload, offset, prior_objects, limits)?;
    Ok(Section {
        offset,
        compressed: compression == 1,
        total_length,
        uncompressed_length,
        objects,
    })
}

fn inflate_exact(input: &[u8], expected: usize, offset: usize) -> Result<Vec<u8>, EmuError> {
    let mut output = Vec::new();
    output.try_reserve_exact(expected).map_err(|_| {
        at_error(
            "resource-limit",
            offset,
            "cannot reserve M3G decompression buffer",
        )
    })?;
    // StreamEnd also verifies the zlib trailer. Read-based decoding can reach
    // EOF after producing all expected bytes while that trailer is truncated.
    let mut decoder = Decompress::new(true);
    let status = decoder
        .decompress_vec(input, &mut output, FlushDecompress::Finish)
        .map_err(|source| {
            EmuError::with_source(
                Category::M3g,
                "invalid-compressed-section",
                format!("at section offset {offset}: zlib stream is invalid"),
                source,
            )
        })?;
    if status != Status::StreamEnd || decoder.total_in() != input.len() as u64 {
        return Err(at_error(
            "invalid-compressed-section",
            offset,
            "section must contain one complete zlib stream of the declared size",
        ));
    }
    if output.len() != expected {
        return Err(at_error(
            "length-mismatch",
            offset,
            "inflated section length differs from declaration",
        ));
    }
    Ok(output)
}

pub(super) fn parse_objects(
    payload: &[u8],
    section_offset: usize,
    prior_objects: usize,
    limits: LoaderLimits,
) -> Result<Vec<ParsedObject>, EmuError> {
    let mut objects = Vec::new();
    let mut cursor = 0_usize;
    while cursor < payload.len() {
        if objects.len().saturating_add(prior_objects) >= limits.objects {
            return Err(at_error(
                "resource-limit",
                section_offset,
                "M3G object count exceeds configured budget",
            ));
        }
        if payload.len() - cursor < 5 {
            return Err(at_error(
                "truncated-object",
                section_offset + 9 + cursor,
                "incomplete M3G object record header",
            ));
        }
        let object_type = ObjectType::try_from(payload[cursor])?;
        let length = read_u32(payload, cursor + 1)? as usize;
        if length > limits.object_bytes {
            return Err(at_error(
                "resource-limit",
                section_offset + 9 + cursor,
                "M3G object exceeds configured byte budget",
            ));
        }
        let data_start = cursor + 5;
        let data_end = data_start
            .checked_add(length)
            .ok_or_else(|| loader_error("length-overflow", "object length overflow"))?;
        if data_end > payload.len() {
            return Err(at_error(
                "truncated-object",
                section_offset + 9 + cursor,
                "declared object extends past section payload",
            ));
        }
        let ordinal = prior_objects
            .checked_add(objects.len())
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| loader_error("length-overflow", "object index overflow"))?;
        objects.push(ParsedObject {
            index: u32::try_from(ordinal)
                .map_err(|_| loader_error("resource-limit", "M3G object index exceeds UInt32"))?,
            object_type,
            data: payload[data_start..data_end].into(),
            section_offset,
        });
        cursor = data_end;
    }
    Ok(objects)
}

pub(super) fn adler32(bytes: &[u8]) -> u32 {
    const MODULUS: u32 = 65_521;
    let mut first = 1_u32;
    let mut second = 0_u32;
    for chunk in bytes.chunks(5_552) {
        for byte in chunk {
            first += u32::from(*byte);
            second += first;
        }
        first %= MODULUS;
        second %= MODULUS;
    }
    (second << 16) | first
}
