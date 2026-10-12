//! Checked byte access and the shared decoded-class allocation budget.

use diagnostics::EmuError;

use crate::{CLASS_MATERIALIZATION_RATIO, MAX_CLASS_MATERIALIZED_BYTES, parse_error};

pub(super) struct MaterializationBudget {
    remaining: usize,
}

impl MaterializationBudget {
    pub(super) fn for_input(input_bytes: usize) -> Self {
        Self {
            remaining: input_bytes
                .saturating_mul(CLASS_MATERIALIZATION_RATIO)
                .min(MAX_CLASS_MATERIALIZED_BYTES),
        }
    }

    pub(super) fn with_ceiling(mut self, bytes: usize) -> Self {
        self.remaining = self.remaining.min(bytes);
        self
    }

    pub(super) const fn remaining(&self) -> usize {
        self.remaining
    }

    pub(super) fn charge(&mut self, bytes: usize, offset: usize) -> Result<(), EmuError> {
        self.remaining = self.remaining.checked_sub(bytes).ok_or_else(|| {
            parse_error(
                "class-materialization-limit",
                offset,
                "decoded class structure exceeds the materialization budget",
            )
        })?;
        Ok(())
    }

    pub(super) fn charge_items<T>(&mut self, count: usize, offset: usize) -> Result<(), EmuError> {
        let bytes = count.checked_mul(std::mem::size_of::<T>()).ok_or_else(|| {
            parse_error(
                "class-materialization-limit",
                offset,
                "decoded class structure size overflowed",
            )
        })?;
        self.charge(bytes, offset)
    }
}

pub(super) fn check_count(
    count: u16,
    limit: u16,
    offset: usize,
    kind: &str,
) -> Result<(), EmuError> {
    if count > limit {
        return Err(parse_error(
            "structure-limit",
            offset,
            format!("{kind} count {count} exceeds limit {limit}"),
        ));
    }
    Ok(())
}

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    base_offset: usize,
}

impl<'a> Reader<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self::with_base(bytes, 0)
    }
    pub(super) const fn with_base(bytes: &'a [u8], base_offset: usize) -> Self {
        Self {
            bytes,
            position: 0,
            base_offset,
        }
    }
    pub(super) fn at(bytes: &'a [u8], offset: usize) -> Result<Self, EmuError> {
        let tail = bytes
            .get(offset..)
            .ok_or_else(|| parse_error("truncated", offset, "parser offset is outside input"))?;
        Ok(Self::with_base(tail, offset))
    }
    pub(super) const fn offset(&self) -> usize {
        self.base_offset + self.position
    }
    pub(super) const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
    pub(super) fn bytes(&mut self, length: usize) -> Result<&'a [u8], EmuError> {
        let start = self.position;
        let absolute_start = self.offset();
        let end = start.checked_add(length).ok_or_else(|| {
            parse_error(
                "truncated",
                absolute_start,
                "requested length overflows address space",
            )
        })?;
        let value = self.bytes.get(start..end).ok_or_else(|| {
            parse_error(
                "truncated",
                absolute_start,
                format!(
                    "need {length} bytes, only {} remain",
                    self.bytes.len().saturating_sub(start)
                ),
            )
        })?;
        self.position = end;
        Ok(value)
    }
    pub(super) fn u1(&mut self) -> Result<u8, EmuError> {
        Ok(self.bytes(1)?[0])
    }
    pub(super) fn u2(&mut self) -> Result<u16, EmuError> {
        let bytes = self.bytes(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }
    pub(super) fn u4(&mut self) -> Result<u32, EmuError> {
        let bytes = self.bytes(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    pub(super) fn u8(&mut self) -> Result<u64, EmuError> {
        let bytes = self.bytes(8)?;
        Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/classfile/reader.rs"]
mod tests;
