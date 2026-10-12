//! Versioned RMS encoding and bounded snapshot validation.

use diagnostics::EmuError;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{
    CHECKSUM_BYTES, FORMAT_VERSION, Limits, MAGIC, MAX_RECORD_ID, rms_error, validate_record,
    validate_store_name,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StoreState {
    pub(super) suite_digest: [u8; 32],
    pub(super) name: String,
    pub(super) next_record_id: u32,
    pub(super) version: u32,
    pub(super) last_modified: i64,
    pub(super) records: BTreeMap<u32, Arc<[u8]>>,
}

impl StoreState {
    pub(super) fn empty(suite_digest: [u8; 32], name: &str, now_millis: i64) -> Self {
        Self {
            suite_digest,
            name: name.to_owned(),
            next_record_id: 1,
            version: 0,
            last_modified: now_millis,
            records: BTreeMap::new(),
        }
    }

    pub(super) fn encoded_len(&self, limits: Limits) -> Result<usize, EmuError> {
        let name = self.name.as_bytes();
        u16::try_from(name.len())
            .map_err(|_| rms_error("store-name", "encoded store name is too long"))?;
        u32::try_from(self.records.len())
            .map_err(|_| rms_error("record-limit", "too many records"))?;
        let mut length = MAGIC
            .len()
            .checked_add(2 + 32 + 2)
            .and_then(|length| length.checked_add(name.len()))
            .and_then(|length| length.checked_add(4 + 4 + 8 + 4))
            .ok_or_else(|| rms_error("store-full", "record store size overflow"))?;
        for data in self.records.values() {
            validate_record(data, limits)?;
            u32::try_from(data.len())
                .map_err(|_| rms_error("record-limit", "record is too large"))?;
            length = length
                .checked_add(4 + 4)
                .and_then(|length| length.checked_add(data.len()))
                .ok_or_else(|| rms_error("store-full", "record store size overflow"))?;
        }
        length
            .checked_add(CHECKSUM_BYTES)
            .filter(|length| *length <= limits.max_store_bytes)
            .ok_or_else(|| rms_error("store-full", "record store byte limit reached"))
    }

    pub(super) fn encode(&self, limits: Limits) -> Result<Vec<u8>, EmuError> {
        let encoded_len = self.encoded_len(limits)?;
        let name = self.name.as_bytes();
        let name_length = u16::try_from(name.len())
            .map_err(|_| rms_error("store-name", "encoded store name is too long"))?;
        let record_count = u32::try_from(self.records.len())
            .map_err(|_| rms_error("record-limit", "too many records"))?;
        let mut output = Vec::with_capacity(encoded_len);
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
        output.extend_from_slice(&self.suite_digest);
        output.extend_from_slice(&name_length.to_be_bytes());
        output.extend_from_slice(name);
        output.extend_from_slice(&self.next_record_id.to_be_bytes());
        output.extend_from_slice(&self.version.to_be_bytes());
        output.extend_from_slice(&self.last_modified.to_be_bytes());
        output.extend_from_slice(&record_count.to_be_bytes());
        for (&id, data) in &self.records {
            let length = u32::try_from(data.len())
                .map_err(|_| rms_error("record-limit", "record is too large"))?;
            output.extend_from_slice(&id.to_be_bytes());
            output.extend_from_slice(&length.to_be_bytes());
            output.extend_from_slice(data);
        }
        let digest = Sha256::digest(&output);
        output.extend_from_slice(&digest);
        debug_assert_eq!(output.len(), encoded_len);
        Ok(output)
    }
}

pub(super) fn decode(
    bytes: &[u8],
    limits: Limits,
    suite_digest: [u8; 32],
) -> Result<StoreState, EmuError> {
    let mut records = BTreeMap::new();
    let header = inspect_snapshot(bytes, limits, suite_digest, |id, data| {
        records.insert(id, Arc::from(data));
    })?;
    Ok(StoreState {
        suite_digest,
        name: header.name,
        next_record_id: header.next_record_id,
        version: header.version,
        last_modified: header.last_modified,
        records,
    })
}

#[derive(Debug)]
pub(super) struct StoreHeader {
    pub(super) name: String,
    pub(super) next_record_id: u32,
    pub(super) version: u32,
    pub(super) last_modified: i64,
}

pub(super) fn inspect_snapshot(
    bytes: &[u8],
    limits: Limits,
    suite_digest: [u8; 32],
    mut accept_record: impl FnMut(u32, &[u8]),
) -> Result<StoreHeader, EmuError> {
    if bytes.len() < MAGIC.len() + 2 + 32 + 2 + 4 + 4 + 8 + 4 + CHECKSUM_BYTES
        || bytes.len() > limits.max_store_bytes
    {
        return Err(rms_error("corrupt-store", "invalid record store length"));
    }
    let payload_length = bytes.len() - CHECKSUM_BYTES;
    if Sha256::digest(&bytes[..payload_length]).as_slice() != &bytes[payload_length..] {
        return Err(rms_error("corrupt-store", "record store checksum mismatch"));
    }
    let mut cursor = Cursor::new(&bytes[..payload_length]);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(rms_error("corrupt-store", "invalid record store magic"));
    }
    if cursor.u16()? != FORMAT_VERSION {
        return Err(rms_error(
            "unsupported-format",
            "unsupported RMS format version",
        ));
    }
    let stored_suite: [u8; 32] = cursor.array()?;
    if stored_suite != suite_digest {
        return Err(rms_error("corrupt-store", "record store suite mismatch"));
    }
    let name_length = usize::from(cursor.u16()?);
    let name = std::str::from_utf8(cursor.take(name_length)?)
        .map_err(|_| rms_error("corrupt-store", "store name is not UTF-8"))?;
    validate_store_name(name).map_err(|_| rms_error("corrupt-store", "invalid store name"))?;
    let next_record_id = cursor.u32()?;
    let version = cursor.u32()?;
    let last_modified = cursor.i64()?;
    let record_count = usize::try_from(cursor.u32()?)
        .map_err(|_| rms_error("corrupt-store", "invalid record count"))?;
    let minimum_record_bytes = record_count
        .checked_mul(8)
        .ok_or_else(|| rms_error("corrupt-store", "record count overflow"))?;
    if minimum_record_bytes > cursor.remaining() {
        return Err(rms_error("corrupt-store", "record table is truncated"));
    }
    let mut previous_id = 0;
    for _ in 0..record_count {
        let id = cursor.u32()?;
        let data_length = usize::try_from(cursor.u32()?)
            .map_err(|_| rms_error("corrupt-store", "invalid record length"))?;
        if id == 0
            || id > MAX_RECORD_ID
            || id <= previous_id
            || data_length > limits.max_record_bytes
        {
            return Err(rms_error("corrupt-store", "invalid record entry"));
        }
        accept_record(id, cursor.take(data_length)?);
        previous_id = id;
    }
    if cursor.remaining() != 0
        || next_record_id == 0
        || next_record_id > MAX_RECORD_ID + 1
        || next_record_id <= previous_id
    {
        return Err(rms_error("corrupt-store", "invalid record store metadata"));
    }
    Ok(StoreHeader {
        name: name.to_owned(),
        next_record_id,
        version,
        last_modified,
    })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], EmuError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| rms_error("corrupt-store", "record store is truncated"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| rms_error("corrupt-store", "record store is truncated"))?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], EmuError> {
        self.take(N)?
            .try_into()
            .map_err(|_| rms_error("corrupt-store", "record store field has an invalid length"))
    }

    fn u16(&mut self) -> Result<u16, EmuError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, EmuError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn i64(&mut self) -> Result<i64, EmuError> {
        Ok(i64::from_be_bytes(self.array()?))
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/rms/snapshot.rs"]
mod tests;
