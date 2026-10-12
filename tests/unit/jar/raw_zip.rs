use super::*;
use crate::tests::fixture;

fn append_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn append_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn append_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn append_raw_central_header(bytes: &mut Vec<u8>, name: &[u8]) {
    append_custom_central_header(bytes, name, 0, 0, &[]);
}

fn append_custom_central_header(
    bytes: &mut Vec<u8>,
    name: &[u8],
    flags: u16,
    compression_method: u16,
    extra: &[u8],
) {
    append_custom_central_header_with_comment(bytes, name, flags, compression_method, extra, &[]);
}

fn append_custom_central_header_with_comment(
    bytes: &mut Vec<u8>,
    name: &[u8],
    flags: u16,
    compression_method: u16,
    extra: &[u8],
    comment: &[u8],
) {
    bytes.extend_from_slice(&CENTRAL_HEADER_SIGNATURE);
    append_u16(bytes, 20);
    append_u16(bytes, 20);
    append_u16(bytes, flags);
    append_u16(bytes, compression_method);
    append_u16(bytes, 0);
    append_u16(bytes, 0);
    append_u32(bytes, 0);
    append_u32(bytes, 0);
    append_u32(bytes, 0);
    append_u16(bytes, u16::try_from(name.len()).unwrap());
    append_u16(bytes, u16::try_from(extra.len()).unwrap());
    append_u16(bytes, u16::try_from(comment.len()).unwrap());
    append_u16(bytes, 0);
    append_u16(bytes, 0);
    append_u32(bytes, 0);
    append_u32(bytes, 0);
    bytes.extend_from_slice(name);
    bytes.extend_from_slice(extra);
    bytes.extend_from_slice(comment);
}

fn append_zip32_eocd(bytes: &mut Vec<u8>, entries: u16, central_size: u32, central_offset: u32) {
    append_zip32_eocd_counts(bytes, entries, entries, central_size, central_offset);
}

fn append_zip32_eocd_counts(
    bytes: &mut Vec<u8>,
    entries_on_disk: u16,
    total_entries: u16,
    central_size: u32,
    central_offset: u32,
) {
    bytes.extend_from_slice(&ZIP32_EOCD_SIGNATURE);
    append_u16(bytes, 0);
    append_u16(bytes, 0);
    append_u16(bytes, entries_on_disk);
    append_u16(bytes, total_entries);
    append_u32(bytes, central_size);
    append_u32(bytes, central_offset);
    append_u16(bytes, 0);
}

fn finish_raw_zip32_candidate(bytes: &mut Vec<u8>, central_start: usize, entries: u16) {
    let central_size = bytes.len() - central_start;
    append_zip32_eocd(
        bytes,
        entries,
        u32::try_from(central_size).unwrap(),
        u32::try_from(central_start).unwrap(),
    );
}

fn test_crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn unicode_path_extra(raw_name: &[u8], decoded_name: &[u8]) -> Vec<u8> {
    let mut extra = Vec::new();
    append_u16(&mut extra, 0x7075);
    append_u16(&mut extra, u16::try_from(5 + decoded_name.len()).unwrap());
    extra.push(1);
    append_u32(&mut extra, test_crc32(raw_name));
    extra.extend_from_slice(decoded_name);
    extra
}

fn append_zip64_record(
    bytes: &mut Vec<u8>,
    record_size: u64,
    disk_number: u32,
    central_disk: u32,
    entries_on_disk: u64,
    total_entries: u64,
    central_offset: u64,
) {
    bytes.extend_from_slice(&ZIP64_EOCD_SIGNATURE);
    append_u64(bytes, record_size);
    append_u16(bytes, 45);
    append_u16(bytes, 45);
    append_u32(bytes, disk_number);
    append_u32(bytes, central_disk);
    append_u64(bytes, entries_on_disk);
    append_u64(bytes, total_entries);
    append_u64(bytes, 0);
    append_u64(bytes, central_offset);
}

fn empty_zip64_archive() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&ZIP64_EOCD_SIGNATURE);
    append_u64(&mut bytes, 44);
    append_u16(&mut bytes, 45);
    append_u16(&mut bytes, 45);
    append_u32(&mut bytes, 0);
    append_u32(&mut bytes, 0);
    append_u64(&mut bytes, 0);
    append_u64(&mut bytes, 0);
    append_u64(&mut bytes, 0);
    append_u64(&mut bytes, 0);
    bytes.extend_from_slice(&ZIP64_LOCATOR_SIGNATURE);
    append_u32(&mut bytes, 0);
    append_u64(&mut bytes, 0);
    append_u32(&mut bytes, 1);
    bytes.extend_from_slice(&ZIP32_EOCD_SIGNATURE);
    append_u16(&mut bytes, 0);
    append_u16(&mut bytes, 0);
    append_u16(&mut bytes, u16::MAX);
    append_u16(&mut bytes, u16::MAX);
    append_u32(&mut bytes, u32::MAX);
    append_u32(&mut bytes, u32::MAX);
    append_u16(&mut bytes, 0);
    bytes
}

#[test]
fn raw_preflight_rejects_duplicate_central_names() {
    let mut bytes = Vec::new();
    append_raw_central_header(&mut bytes, b"same.class");
    append_raw_central_header(&mut bytes, b"same.class");
    let central_size = u32::try_from(bytes.len()).unwrap();
    append_zip32_eocd(&mut bytes, 2, central_size, 0);

    assert_eq!(
        raw_zip_preflight(&bytes).unwrap_err().code(),
        "duplicate-entry"
    );
    assert_eq!(
        crate::inspect_bytes(&bytes).unwrap_err().code(),
        "duplicate-entry"
    );
    assert_eq!(
        crate::read_class_entries_bytes(&bytes).unwrap_err().code(),
        "duplicate-entry"
    );
    assert_eq!(
        crate::ResourceArchive::from_bytes(&bytes)
            .err()
            .unwrap()
            .code(),
        "duplicate-entry"
    );
}

#[test]
fn raw_preflight_rejects_entry_count_before_zip_allocation() {
    let entry_count = MAX_ENTRIES + 1;
    let mut bytes = Vec::with_capacity(entry_count * CENTRAL_HEADER_BYTES + ZIP32_EOCD_BYTES);
    for _ in 0..entry_count {
        append_raw_central_header(&mut bytes, b"");
    }
    let central_size = u32::try_from(bytes.len()).unwrap();
    append_zip32_eocd(
        &mut bytes,
        u16::try_from(entry_count).unwrap(),
        central_size,
        0,
    );

    assert_eq!(
        raw_zip_preflight(&bytes).unwrap_err().code(),
        "too-many-entries"
    );
}

#[test]
fn raw_preflight_checks_earlier_footer_after_late_semantic_failure() {
    let entry_count = MAX_ENTRIES + 1;
    let mut bytes = Vec::with_capacity(
        entry_count * CENTRAL_HEADER_BYTES + CENTRAL_HEADER_BYTES + 2 * ZIP32_EOCD_BYTES,
    );
    for _ in 0..entry_count {
        append_raw_central_header(&mut bytes, b"");
    }
    let oversized_central_size = u32::try_from(bytes.len()).unwrap();
    append_zip32_eocd(
        &mut bytes,
        u16::try_from(entry_count).unwrap(),
        oversized_central_size,
        0,
    );

    let late_central_offset = bytes.len();
    append_raw_central_header(&mut bytes, b"late");
    bytes[late_central_offset + 10..late_central_offset + 12]
        .copy_from_slice(&99_u16.to_le_bytes());
    append_zip32_eocd(
        &mut bytes,
        1,
        u32::try_from(CENTRAL_HEADER_BYTES + 4).unwrap(),
        u32::try_from(late_central_offset).unwrap(),
    );

    assert_eq!(
        raw_zip_preflight(&bytes).unwrap_err().code(),
        "too-many-entries"
    );
}

#[test]
fn raw_preflight_stops_after_valid_newest_candidate() {
    let entry_count = MAX_ENTRIES + 1;
    let mut bytes = Vec::new();
    let nested_start = bytes.len();
    for _ in 0..entry_count {
        append_raw_central_header(&mut bytes, b"");
    }
    finish_raw_zip32_candidate(
        &mut bytes,
        nested_start,
        u16::try_from(entry_count).unwrap(),
    );

    let outer_start = bytes.len();
    append_raw_central_header(&mut bytes, b"outer");
    finish_raw_zip32_candidate(&mut bytes, outer_start, 1);

    raw_zip_preflight(&bytes).unwrap();
}

#[test]
fn raw_preflight_distinguishes_probe_from_embedded_eocd_fallback() {
    let mut embedded_eocd = Vec::new();
    append_zip32_eocd(&mut embedded_eocd, 1, 0, 0);

    let mut bytes = Vec::new();
    append_custom_central_header_with_comment(&mut bytes, b"first", 0, 0, &[], &embedded_eocd);
    append_custom_central_header(&mut bytes, b"late", 0, 99, &[]);
    let central_size = u32::try_from(bytes.len()).unwrap();
    append_zip32_eocd(&mut bytes, 2, central_size, 0);

    let archive = ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
    assert_eq!(archive.len(), 1);
    assert!(archive.comment().is_empty());
    raw_zip_preflight(&bytes).unwrap();
}

#[test]
fn probe_comment_cannot_match_embedded_footer_comments() {
    let mut bytes = Vec::new();
    for nonce in (0..130).chain(std::iter::once(usize::MAX)) {
        append_probe_eocd(&mut bytes, 0, 0, &probe_comment(nonce)).unwrap();
    }
    let canonical = bytes.len();
    append_probe_eocd(&mut bytes, 0, 0, &probe_comment(0)).unwrap();
    let limit = bytes.len() * 12 + 2048;
    let marker = choose_unique_probe_comment(
        &mut bytes,
        canonical,
        canonical + ZIP32_EOCD_BYTES,
        &mut 0,
        limit,
    )
    .unwrap();
    for offset in 0..canonical {
        if let Some((start, end)) = raw_eocd_comment_range(&bytes, offset) {
            assert_ne!(bytes[start..end], marker);
        }
    }
    assert_eq!(bytes[canonical + ZIP32_EOCD_BYTES..], marker);
}

#[test]
fn raw_preflight_uses_total_count_only_for_zip32_empty_find_stage() {
    let entries_on_disk = u16::MAX - 1;
    let mut bytes = vec![0; usize::from(entries_on_disk)];
    append_zip32_eocd_counts(&mut bytes, entries_on_disk, 0, 0, 0);

    assert_eq!(
        raw_zip_preflight(&bytes).unwrap_err().code(),
        "too-many-entries"
    );
}

#[test]
fn raw_preflight_allows_fallback_when_large_candidate_has_one_header() {
    let mut bytes = Vec::new();
    let older_start = bytes.len();
    append_raw_central_header(&mut bytes, b"older");
    finish_raw_zip32_candidate(&mut bytes, older_start, 1);

    let newest_start = bytes.len();
    append_raw_central_header(&mut bytes, b"incomplete");
    finish_raw_zip32_candidate(
        &mut bytes,
        newest_start,
        u16::try_from(MAX_ENTRIES + 1).unwrap(),
    );

    raw_zip_preflight(&bytes).unwrap();
}

#[test]
fn raw_preflight_detects_utf8_lossy_name_collision() {
    let mut bytes = Vec::new();
    append_custom_central_header(&mut bytes, &[0xff], 1 << 11, 0, &[]);
    append_custom_central_header(&mut bytes, &[0xfe], 1 << 11, 0, &[]);
    let central_size = u32::try_from(bytes.len()).unwrap();
    append_zip32_eocd(&mut bytes, 2, central_size, 0);

    assert_eq!(
        raw_zip_preflight(&bytes).unwrap_err().code(),
        "duplicate-entry"
    );
}

#[test]
fn raw_preflight_detects_cp437_utf8_name_collision() {
    let mut bytes = Vec::new();
    append_custom_central_header(&mut bytes, &[0x82], 0, 0, &[]);
    append_custom_central_header(&mut bytes, "é".as_bytes(), 1 << 11, 0, &[]);
    let central_size = u32::try_from(bytes.len()).unwrap();
    append_zip32_eocd(&mut bytes, 2, central_size, 0);

    assert_eq!(
        raw_zip_preflight(&bytes).unwrap_err().code(),
        "duplicate-entry"
    );
}

#[test]
fn raw_preflight_detects_unicode_path_extra_collision() {
    let first_name = b"first";
    let second_name = b"second";
    let first_extra = unicode_path_extra(first_name, b"same");
    let second_extra = unicode_path_extra(second_name, b"same");
    let mut bytes = Vec::new();
    append_custom_central_header(&mut bytes, first_name, 0, 0, &first_extra);
    append_custom_central_header(&mut bytes, second_name, 0, 0, &second_extra);
    let central_size = u32::try_from(bytes.len()).unwrap();
    append_zip32_eocd(&mut bytes, 2, central_size, 0);

    assert_eq!(
        raw_zip_preflight(&bytes).unwrap_err().code(),
        "duplicate-entry"
    );
}

#[test]
fn raw_preflight_does_not_retry_zip64_record_after_semantic_failure() {
    let entry_count = MAX_ENTRIES + 1;
    let mut bytes = Vec::new();
    let oversized_start = bytes.len();
    for _ in 0..entry_count {
        append_raw_central_header(&mut bytes, b"");
    }
    finish_raw_zip32_candidate(
        &mut bytes,
        oversized_start,
        u16::try_from(entry_count).unwrap(),
    );

    let first_zip64_offset = bytes.len();
    append_zip64_record(&mut bytes, 0, 1, 0, 0, 0, 0);
    append_zip64_record(&mut bytes, 44, 0, 0, 0, 0, 0);
    let locator_offset = bytes.len();
    let first_record_size = u64::try_from(locator_offset - first_zip64_offset - 12).unwrap();
    bytes[first_zip64_offset + 4..first_zip64_offset + 12]
        .copy_from_slice(&first_record_size.to_le_bytes());
    bytes.extend_from_slice(&ZIP64_LOCATOR_SIGNATURE);
    append_u32(&mut bytes, 0);
    append_u64(&mut bytes, u64::try_from(first_zip64_offset).unwrap());
    append_u32(&mut bytes, 1);
    append_zip32_eocd(&mut bytes, u16::MAX, u32::MAX, u32::MAX);

    assert_eq!(
        raw_zip_preflight(&bytes).unwrap_err().code(),
        "too-many-entries"
    );
}

#[test]
fn raw_preflight_accepts_zip64_and_rejects_malformed_footers() {
    let valid = empty_zip64_archive();
    raw_zip_preflight(&valid).unwrap();

    let mut malformed_eocd = Vec::new();
    append_zip32_eocd(&mut malformed_eocd, 0, 0, 0);
    let comment_length_offset = malformed_eocd.len() - 2;
    malformed_eocd[comment_length_offset..].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        raw_zip_preflight(&malformed_eocd).unwrap_err().code(),
        "invalid-zip"
    );

    let mut missing_locator = Vec::new();
    append_zip32_eocd(&mut missing_locator, u16::MAX, u32::MAX, u32::MAX);
    assert_eq!(
        raw_zip_preflight(&missing_locator).unwrap_err().code(),
        "invalid-zip"
    );

    let mut malformed_zip64 = valid;
    malformed_zip64[4..12].copy_from_slice(&43_u64.to_le_bytes());
    assert_eq!(
        raw_zip_preflight(&malformed_zip64).unwrap_err().code(),
        "invalid-zip"
    );

    let mut bad_locator_offset = empty_zip64_archive();
    bad_locator_offset[64..72].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        raw_zip_preflight(&bad_locator_offset).unwrap_err().code(),
        "invalid-zip"
    );
}

#[test]
fn raw_preflight_handles_false_footers_prefixes_and_trailing_data() {
    let canonical = fixture(b"Manifest-Version: 1.0\nMIDlet-1: Demo, , demo.Main\n").into_inner();

    let mut false_footer_in_comment = canonical.clone();
    let eocd_offset = false_footer_in_comment.len() - ZIP32_EOCD_BYTES;
    let mut fake = Vec::new();
    fake.extend_from_slice(&ZIP32_EOCD_SIGNATURE);
    append_u16(&mut fake, 1);
    fake.extend_from_slice(&[0; ZIP32_EOCD_BYTES - 6]);
    false_footer_in_comment[eocd_offset + 20..eocd_offset + 22]
        .copy_from_slice(&u16::try_from(fake.len()).unwrap().to_le_bytes());
    false_footer_in_comment.extend_from_slice(&fake);
    raw_zip_preflight(&false_footer_in_comment).unwrap();

    let mut prefixed = b"#!/usr/bin/env j2play\n".to_vec();
    prefixed.extend_from_slice(&canonical);
    raw_zip_preflight(&prefixed).unwrap();

    let mut trailing = canonical;
    let trailing_len = trailing.len() + usize::from(u16::MAX) + 1;
    trailing.resize(trailing_len, 0);
    raw_zip_preflight(&trailing).unwrap();

    let mut prefixed_zip64 = b"launcher-prefix".to_vec();
    prefixed_zip64.extend_from_slice(&empty_zip64_archive());
    raw_zip_preflight(&prefixed_zip64).unwrap();
}
