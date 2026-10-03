use super::MAX_ENTRIES;
use diagnostics::{Category, EmuError};
use std::io::Cursor;
use zip::ZipArchive;

const ZIP32_EOCD_SIGNATURE: [u8; 4] = 0x0605_4b50_u32.to_le_bytes();
const ZIP64_EOCD_SIGNATURE: [u8; 4] = 0x0606_4b50_u32.to_le_bytes();
const ZIP64_LOCATOR_SIGNATURE: [u8; 4] = 0x0706_4b50_u32.to_le_bytes();
const CENTRAL_HEADER_SIGNATURE: [u8; 4] = 0x0201_4b50_u32.to_le_bytes();
const ZIP32_EOCD_BYTES: usize = 22;
const ZIP64_EOCD_MIN_BYTES: usize = 56;
const ZIP64_LOCATOR_BYTES: usize = 20;
const CENTRAL_HEADER_BYTES: usize = 46;
const PROBE_COMMENT_PREFIX: &[u8] = b"~J2PLAY-ZIP-PROBE-";
const PROBE_NONCE_HEX_BYTES: usize = 16;
const PROBE_COMMENT_BYTES: usize = PROBE_COMMENT_PREFIX.len() + PROBE_NONCE_HEX_BYTES;

struct RawCentralCandidate {
    archive_offset: u64,
    directory_start: Option<u64>,
    entry_count: u64,
    disk_number: u32,
    central_disk: u32,
    entries_on_disk_valid: bool,
}

enum RawSemanticOutcome {
    Accept,
    RetryOlder,
    TooMany,
    Duplicate,
}

pub(super) fn raw_zip_preflight(bytes: &[u8]) -> Result<(), EmuError> {
    if bytes.len() < ZIP32_EOCD_BYTES {
        return Err(invalid_zip_preflight());
    }

    let work_limit = bytes.len().saturating_mul(12).saturating_add(2048);
    let mut work = 0_usize;
    charge_raw_work(&mut work, bytes.len(), work_limit).map_err(invalid_zip_work)?;
    for eocd_offset in (0..=bytes.len() - ZIP32_EOCD_BYTES).rev() {
        if bytes.get(eocd_offset..eocd_offset + 4) != Some(ZIP32_EOCD_SIGNATURE.as_slice()) {
            continue;
        }
        let Some(candidate) = find_raw_central_candidate(bytes, eocd_offset, &mut work, work_limit)
            .map_err(invalid_zip_work)?
        else {
            continue;
        };
        match evaluate_raw_candidate(bytes, &candidate, &mut work, work_limit)
            .map_err(invalid_zip_work)?
        {
            RawSemanticOutcome::Accept => return Ok(()),
            RawSemanticOutcome::RetryOlder => {}
            RawSemanticOutcome::TooMany => {
                return Err(EmuError::new(
                    Category::Jar,
                    "too-many-entries",
                    format!(
                        "archive has {} entries; limit is {MAX_ENTRIES}",
                        candidate.entry_count
                    ),
                ));
            }
            RawSemanticOutcome::Duplicate => {
                return Err(EmuError::new(
                    Category::Jar,
                    "duplicate-entry",
                    "archive contains duplicate decoded entry names",
                ));
            }
        }
    }

    Err(invalid_zip_preflight())
}

fn find_raw_central_candidate(
    bytes: &[u8],
    eocd_offset: usize,
    work: &mut usize,
    work_limit: usize,
) -> Result<Option<RawCentralCandidate>, ()> {
    let Some(eocd_end) = eocd_offset.checked_add(ZIP32_EOCD_BYTES) else {
        return Ok(None);
    };
    let Some(eocd) = bytes.get(eocd_offset..eocd_end) else {
        return Ok(None);
    };
    let Some(comment_len) = read_u16(eocd, 20).map(usize::from) else {
        return Ok(None);
    };
    let Some(comment_end) = eocd_end.checked_add(comment_len) else {
        return Ok(None);
    };
    if comment_end > bytes.len() {
        return Ok(None);
    }

    let Some(disk_number) = read_u16(eocd, 4) else {
        return Ok(None);
    };
    let Some(central_disk) = read_u16(eocd, 6) else {
        return Ok(None);
    };
    let Some(entries_on_disk) = read_u16(eocd, 8) else {
        return Ok(None);
    };
    let Some(total_entries) = read_u16(eocd, 10) else {
        return Ok(None);
    };
    let Some(central_offset) = read_u32(eocd, 16) else {
        return Ok(None);
    };
    let may_be_zip64 = total_entries == u16::MAX || central_offset == u32::MAX;

    if may_be_zip64 && has_zip64_locator(bytes, eocd_offset) {
        return find_raw_zip64_candidate(bytes, eocd_offset, work, work_limit);
    }

    let relative_start = u64::from(central_offset);
    let entry_count = u64::from(entries_on_disk);
    if total_entries == 0 {
        let Ok(eocd_offset) = u64::try_from(eocd_offset) else {
            return Ok(None);
        };
        let archive_offset = eocd_offset.saturating_sub(relative_start);
        return Ok(Some(RawCentralCandidate {
            archive_offset,
            directory_start: relative_start.checked_add(archive_offset),
            entry_count,
            disk_number: u32::from(disk_number),
            central_disk: u32::from(central_disk),
            entries_on_disk_valid: true,
        }));
    }
    let Ok(relative_start_usize) = usize::try_from(relative_start) else {
        return Ok(None);
    };
    if relative_start_usize >= eocd_offset {
        return Ok(None);
    }
    let Some(directory_start) = find_signature(
        bytes,
        relative_start_usize,
        eocd_offset,
        CENTRAL_HEADER_SIGNATURE,
        work,
        work_limit,
    )?
    else {
        return Ok(None);
    };
    let archive_offset = u64::try_from(directory_start)
        .ok()
        .and_then(|start| start.checked_sub(relative_start));
    let Some(archive_offset) = archive_offset else {
        return Ok(None);
    };
    Ok(Some(RawCentralCandidate {
        archive_offset,
        directory_start: u64::try_from(directory_start).ok(),
        entry_count,
        disk_number: u32::from(disk_number),
        central_disk: u32::from(central_disk),
        entries_on_disk_valid: true,
    }))
}

fn has_zip64_locator(bytes: &[u8], eocd_offset: usize) -> bool {
    eocd_offset
        .checked_sub(ZIP64_LOCATOR_BYTES)
        .and_then(|offset| bytes.get(offset..offset + 4))
        == Some(ZIP64_LOCATOR_SIGNATURE.as_slice())
}

fn find_raw_zip64_candidate(
    bytes: &[u8],
    eocd_offset: usize,
    work: &mut usize,
    work_limit: usize,
) -> Result<Option<RawCentralCandidate>, ()> {
    let Some(locator_offset) = eocd_offset.checked_sub(ZIP64_LOCATOR_BYTES) else {
        return Ok(None);
    };
    let Some(locator) = bytes.get(locator_offset..eocd_offset) else {
        return Ok(None);
    };
    let Some(locator_disk) = read_u32(locator, 4) else {
        return Ok(None);
    };
    let Some(relative_zip64_offset) =
        read_u64(locator, 8).and_then(|offset| usize::try_from(offset).ok())
    else {
        return Ok(None);
    };
    let Some(number_of_disks) = read_u32(locator, 16) else {
        return Ok(None);
    };
    if relative_zip64_offset >= locator_offset || number_of_disks > 1 {
        return Ok(None);
    }

    charge_raw_work(
        work,
        locator_offset.saturating_sub(relative_zip64_offset),
        work_limit,
    )?;
    let Some(search) = bytes.get(relative_zip64_offset..locator_offset) else {
        return Ok(None);
    };
    for (relative, window) in search.windows(ZIP64_EOCD_SIGNATURE.len()).enumerate() {
        if window != ZIP64_EOCD_SIGNATURE.as_slice() {
            continue;
        }
        let Some(zip64_offset) = relative_zip64_offset.checked_add(relative) else {
            return Ok(None);
        };
        let Some(candidate) = find_raw_zip64_record(
            bytes,
            zip64_offset,
            locator_offset,
            locator_disk,
            relative_zip64_offset,
        ) else {
            continue;
        };
        return Ok(Some(candidate));
    }
    Ok(None)
}

fn find_raw_zip64_record(
    bytes: &[u8],
    zip64_offset: usize,
    locator_offset: usize,
    locator_disk: u32,
    relative_zip64_offset: usize,
) -> Option<RawCentralCandidate> {
    let fixed = bytes.get(zip64_offset..zip64_offset.checked_add(ZIP64_EOCD_MIN_BYTES)?)?;
    let record_size = usize::try_from(read_u64(fixed, 4)?).ok()?;
    if record_size < 44 || zip64_offset.checked_add(12)?.checked_add(record_size)? != locator_offset
    {
        return None;
    }
    let disk_number = read_u32(fixed, 16)?;
    let central_disk = read_u32(fixed, 20)?;
    let entries_on_disk = read_u64(fixed, 24)?;
    let entry_count = read_u64(fixed, 32)?;
    let relative_central_start = read_u64(fixed, 48)?;
    let zip64_offset_u64 = u64::try_from(zip64_offset).ok()?;
    if central_disk != locator_disk
        || zip64_offset_u64
            < entry_count
                .saturating_mul(u64::try_from(CENTRAL_HEADER_BYTES).ok()?)
                .saturating_add(relative_central_start)
    {
        return None;
    }
    let archive_offset = zip64_offset.checked_sub(relative_zip64_offset)?;
    let archive_offset = u64::try_from(archive_offset).ok()?;
    Some(RawCentralCandidate {
        archive_offset,
        directory_start: relative_central_start.checked_add(archive_offset),
        entry_count,
        disk_number,
        central_disk,
        entries_on_disk_valid: entries_on_disk <= entry_count,
    })
}

fn evaluate_raw_candidate(
    bytes: &[u8],
    candidate: &RawCentralCandidate,
    work: &mut usize,
    work_limit: usize,
) -> Result<RawSemanticOutcome, ()> {
    if !candidate.entries_on_disk_valid || candidate.disk_number != candidate.central_disk {
        return Ok(RawSemanticOutcome::RetryOlder);
    }
    let Some(directory_start) = candidate.directory_start else {
        return Ok(RawSemanticOutcome::RetryOlder);
    };
    if candidate.entry_count > MAX_ENTRIES as u64 {
        if candidate.entry_count <= directory_start {
            return Ok(RawSemanticOutcome::TooMany);
        }
        let probe_count = MAX_ENTRIES + 1;
        return Ok(
            match probe_raw_central_directory(bytes, candidate, probe_count, work, work_limit)? {
                Some(_) => RawSemanticOutcome::TooMany,
                None => RawSemanticOutcome::RetryOlder,
            },
        );
    }

    let Ok(probe_count) = usize::try_from(candidate.entry_count) else {
        return Ok(RawSemanticOutcome::RetryOlder);
    };
    Ok(
        match probe_raw_central_directory(bytes, candidate, probe_count, work, work_limit)? {
            Some(unique_count) if unique_count == probe_count => RawSemanticOutcome::Accept,
            Some(unique_count) if unique_count < probe_count => RawSemanticOutcome::Duplicate,
            Some(_) | None => RawSemanticOutcome::RetryOlder,
        },
    )
}

fn probe_raw_central_directory(
    bytes: &[u8],
    candidate: &RawCentralCandidate,
    probe_count: usize,
    work: &mut usize,
    work_limit: usize,
) -> Result<Option<usize>, ()> {
    let central = if probe_count == 0 {
        &[]
    } else {
        let Some(directory_start) = candidate
            .directory_start
            .and_then(|start| usize::try_from(start).ok())
        else {
            return Ok(None);
        };
        let Some(central_end) =
            raw_central_prefix_end(bytes, directory_start, probe_count, work, work_limit)?
        else {
            return Ok(None);
        };
        let Some(central) = bytes.get(directory_start..central_end) else {
            return Ok(None);
        };
        central
    };
    let Ok(archive_offset) = usize::try_from(candidate.archive_offset) else {
        return Ok(None);
    };
    let placeholder_comment = probe_comment(0);
    let Some(canonical_eocd_offset) = archive_offset.checked_add(central.len()) else {
        return Ok(None);
    };
    let Some(synthetic_size) = canonical_eocd_offset
        .checked_add(ZIP32_EOCD_BYTES)
        .and_then(|size| size.checked_add(placeholder_comment.len()))
    else {
        return Ok(None);
    };
    charge_raw_work(work, synthetic_size, work_limit)?;
    let mut synthetic = Vec::new();
    if synthetic.try_reserve_exact(synthetic_size).is_err() {
        return Err(());
    }
    synthetic.resize(archive_offset, 0);
    synthetic.extend_from_slice(central);
    append_probe_eocd(
        &mut synthetic,
        probe_count,
        central.len(),
        &placeholder_comment,
    )?;
    let Some(comment_start) = canonical_eocd_offset.checked_add(ZIP32_EOCD_BYTES) else {
        return Ok(None);
    };
    let sentinel = choose_unique_probe_comment(
        &mut synthetic,
        canonical_eocd_offset,
        comment_start,
        work,
        work_limit,
    )?;

    let Ok(archive) = ZipArchive::new(Cursor::new(synthetic)) else {
        return Ok(None);
    };
    if archive.offset() != candidate.archive_offset
        || archive.central_directory_start() != candidate.archive_offset
        || archive.comment() != sentinel
    {
        return Ok(None);
    }
    Ok(Some(archive.len()))
}

fn raw_central_prefix_end(
    bytes: &[u8],
    start: usize,
    entry_count: usize,
    work: &mut usize,
    work_limit: usize,
) -> Result<Option<usize>, ()> {
    let mut offset = start;
    for _ in 0..entry_count {
        charge_raw_work(work, CENTRAL_HEADER_BYTES, work_limit)?;
        let Some(fixed_end) = offset.checked_add(CENTRAL_HEADER_BYTES) else {
            return Ok(None);
        };
        let Some(fixed) = bytes.get(offset..fixed_end) else {
            return Ok(None);
        };
        if fixed.get(..4) != Some(CENTRAL_HEADER_SIGNATURE.as_slice()) {
            return Ok(None);
        }
        let variable_bytes = [28, 30, 32].into_iter().try_fold(0_usize, |total, field| {
            total.checked_add(usize::from(read_u16(fixed, field)?))
        });
        let Some(variable_bytes) = variable_bytes else {
            return Ok(None);
        };
        charge_raw_work(work, variable_bytes, work_limit)?;
        let Some(entry_end) = fixed_end.checked_add(variable_bytes) else {
            return Ok(None);
        };
        if entry_end > bytes.len() {
            return Ok(None);
        }
        offset = entry_end;
    }
    Ok(Some(offset))
}

fn append_probe_eocd(
    output: &mut Vec<u8>,
    entry_count: usize,
    central_size: usize,
    comment: &[u8],
) -> Result<(), ()> {
    let entry_count = u16::try_from(entry_count).map_err(|_| ())?;
    let central_size = u32::try_from(central_size).map_err(|_| ())?;
    let comment_size = u16::try_from(comment.len()).map_err(|_| ())?;
    output.extend_from_slice(&ZIP32_EOCD_SIGNATURE);
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&entry_count.to_le_bytes());
    output.extend_from_slice(&entry_count.to_le_bytes());
    output.extend_from_slice(&central_size.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&comment_size.to_le_bytes());
    output.extend_from_slice(comment);
    Ok(())
}

fn choose_unique_probe_comment(
    synthetic: &mut [u8],
    canonical_eocd_offset: usize,
    comment_start: usize,
    work: &mut usize,
    work_limit: usize,
) -> Result<[u8; PROBE_COMMENT_BYTES], ()> {
    let marker_len = PROBE_COMMENT_BYTES;
    let comment_end = comment_start.checked_add(marker_len).ok_or(())?;
    if comment_end != synthetic.len() {
        return Err(());
    }

    let nonce_count = synthetic.len().checked_add(1).ok_or(())?;
    charge_raw_work(work, nonce_count, work_limit)?;
    // One bit per possible nonce keeps adversarial archive comments from
    // requiring another byte buffer as large as the synthetic directory.
    let word_count = nonce_count.div_ceil(64);
    let mut used_nonces = Vec::new();
    used_nonces.try_reserve_exact(word_count).map_err(|_| ())?;
    used_nonces.resize(word_count, 0_u64);

    charge_raw_work(work, synthetic.len(), work_limit)?;
    for eocd_offset in 0..=synthetic.len().saturating_sub(ZIP32_EOCD_BYTES) {
        if eocd_offset == canonical_eocd_offset {
            continue;
        }
        let Some((start, end)) = raw_eocd_comment_range(synthetic, eocd_offset) else {
            continue;
        };
        if end.saturating_sub(start) != marker_len {
            continue;
        }
        charge_raw_work(work, marker_len, work_limit)?;
        let Some(nonce) = parse_probe_comment(&synthetic[start..end]) else {
            continue;
        };
        if let Some(used) = used_nonces.get_mut(nonce / 64) {
            *used |= 1_u64 << (nonce % 64);
        }
    }

    charge_raw_work(work, nonce_count, work_limit)?;
    let (block, used) = used_nonces
        .iter()
        .enumerate()
        .find(|(_, used)| **used != u64::MAX)
        .ok_or(())?;
    let nonce = block * 64 + used.trailing_ones() as usize;
    let sentinel = probe_comment(nonce);
    synthetic
        .get_mut(comment_start..comment_end)
        .ok_or(())?
        .copy_from_slice(&sentinel);

    charge_raw_work(work, synthetic.len(), work_limit)?;
    for eocd_offset in 0..=synthetic.len().saturating_sub(ZIP32_EOCD_BYTES) {
        if eocd_offset == canonical_eocd_offset {
            continue;
        }
        let Some((start, end)) = raw_eocd_comment_range(synthetic, eocd_offset) else {
            continue;
        };
        if end.saturating_sub(start) != marker_len {
            continue;
        }
        charge_raw_work(work, marker_len, work_limit)?;
        if synthetic[start..end] == sentinel {
            return Err(());
        }
    }
    Ok(sentinel)
}

fn probe_comment(mut nonce: usize) -> [u8; PROBE_COMMENT_BYTES] {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut comment = [b'0'; PROBE_COMMENT_BYTES];
    let digit_start = PROBE_COMMENT_PREFIX.len();
    comment[..digit_start].copy_from_slice(PROBE_COMMENT_PREFIX);
    for digit in comment[digit_start..].iter_mut().rev() {
        *digit = HEX[nonce & 0x0f];
        nonce >>= 4;
    }
    comment
}

fn parse_probe_comment(comment: &[u8]) -> Option<usize> {
    let digits = comment.strip_prefix(PROBE_COMMENT_PREFIX)?;
    if digits.len() != PROBE_NONCE_HEX_BYTES {
        return None;
    }
    digits.iter().try_fold(0_usize, |nonce, byte| {
        let digit = match byte {
            b'0'..=b'9' => usize::from(*byte - b'0'),
            b'a'..=b'f' => usize::from(*byte - b'a' + 10),
            _ => return None,
        };
        nonce.checked_mul(16)?.checked_add(digit)
    })
}

fn raw_eocd_comment_range(bytes: &[u8], eocd_offset: usize) -> Option<(usize, usize)> {
    let fixed_end = eocd_offset.checked_add(ZIP32_EOCD_BYTES)?;
    let fixed = bytes.get(eocd_offset..fixed_end)?;
    if fixed.get(..4) != Some(ZIP32_EOCD_SIGNATURE.as_slice()) {
        return None;
    }
    let comment_len = usize::from(read_u16(fixed, 20)?);
    let comment_end = fixed_end.checked_add(comment_len)?;
    bytes.get(fixed_end..comment_end)?;
    Some((fixed_end, comment_end))
}

fn find_signature(
    bytes: &[u8],
    start: usize,
    end: usize,
    signature: [u8; 4],
    work: &mut usize,
    work_limit: usize,
) -> Result<Option<usize>, ()> {
    let Some(search) = bytes.get(start..end) else {
        return Ok(None);
    };
    charge_raw_work(work, search.len(), work_limit)?;
    Ok(search
        .windows(signature.len())
        .position(|window| window == signature.as_slice())
        .and_then(|relative| start.checked_add(relative)))
}

fn charge_raw_work(work: &mut usize, amount: usize, work_limit: usize) -> Result<(), ()> {
    *work = work.checked_add(amount).ok_or(())?;
    if *work > work_limit {
        return Err(());
    }
    Ok(())
}

fn invalid_zip_work((): ()) -> EmuError {
    invalid_zip_preflight()
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

fn invalid_zip_preflight() -> EmuError {
    EmuError::new(
        Category::Jar,
        "invalid-zip",
        "invalid ZIP/JAR central directory",
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/jar/raw_zip.rs"]
mod tests;
