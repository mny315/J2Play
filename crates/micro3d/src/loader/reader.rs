//! Checked little-endian byte and packed-bit access.

use super::{EmuError, loader_error};

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(super) fn with_position(bytes: &'a [u8], position: usize) -> Result<Self, EmuError> {
        if position > bytes.len() {
            return Err(loader_error(
                "resource-offset",
                "cursor offset is outside resource",
            ));
        }
        Ok(Self { bytes, position })
    }

    pub(super) const fn position(&self) -> usize {
        self.position
    }

    pub(super) fn set_position(&mut self, position: usize) -> Result<(), EmuError> {
        if position > self.bytes.len() {
            return Err(loader_error(
                "resource-offset",
                "cursor offset is outside resource",
            ));
        }
        self.position = position;
        Ok(())
    }

    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], EmuError> {
        let output = self
            .position
            .checked_add(count)
            .and_then(|end| self.bytes.get(self.position..end))
            .ok_or_else(|| {
                loader_error(
                    "resource-truncated",
                    format!(
                        "Micro3D resource is truncated at byte {} while reading {count} bytes",
                        self.position
                    ),
                )
            })?;
        self.position += count;
        Ok(output)
    }

    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N], EmuError> {
        self.take(N)?.try_into().map_err(|_| {
            loader_error(
                "resource-truncated",
                "Micro3D resource field has an invalid fixed width",
            )
        })
    }

    pub(super) fn skip(&mut self, count: usize) -> Result<(), EmuError> {
        self.take(count).map(|_| ())
    }

    pub(super) fn u8(&mut self) -> Result<u8, EmuError> {
        let [value] = self.array()?;
        Ok(value)
    }

    pub(super) fn u16(&mut self) -> Result<u16, EmuError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(super) fn i16(&mut self) -> Result<i16, EmuError> {
        Ok(i16::from_le_bytes(self.array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, EmuError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(super) fn i32(&mut self) -> Result<i32, EmuError> {
        Ok(i32::from_le_bytes(self.array()?))
    }
}

pub(super) struct BitReader<'a> {
    bytes: &'a [u8],
    bit: usize,
}

impl<'a> BitReader<'a> {
    pub(super) fn new(bytes: &'a [u8], byte_offset: usize) -> Result<Self, EmuError> {
        let bit = byte_offset
            .checked_mul(8)
            .filter(|bit| *bit <= bytes.len().saturating_mul(8))
            .ok_or_else(|| loader_error("resource-offset", "bit offset is outside resource"))?;
        Ok(Self { bytes, bit })
    }

    pub(super) fn unsigned(&mut self, bits: usize) -> Result<u64, EmuError> {
        if bits > 64 {
            return Err(loader_error(
                "resource-bit-width",
                "bit field exceeds 64 bits",
            ));
        }
        let end = self
            .bit
            .checked_add(bits)
            .filter(|end| *end <= self.bytes.len().saturating_mul(8))
            .ok_or_else(|| {
                loader_error("resource-truncated", "packed Micro3D field is truncated")
            })?;
        let mut value = 0_u64;
        let mut source = self.bit;
        let mut shift = 0;
        while source < end {
            let offset = source % 8;
            let take = (8 - offset).min(end - source);
            let mask = u8::MAX >> (8 - take);
            let chunk = (self.bytes[source / 8] >> offset) & mask;
            value |= u64::from(chunk) << shift;
            source += take;
            shift += take;
        }
        self.bit = end;
        Ok(value)
    }

    pub(super) fn signed(&mut self, bits: usize) -> Result<i32, EmuError> {
        if bits == 0 || bits > 32 {
            return Err(loader_error(
                "resource-bit-width",
                "signed bit field width is invalid",
            ));
        }
        let value = self.unsigned(bits)? as u32;
        let shift = 32 - bits;
        Ok((value << shift).cast_signed() >> shift)
    }

    pub(super) const fn aligned_position(&self) -> usize {
        self.bit.div_ceil(8)
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/micro3d/loader/reader.rs"]
mod tests;
