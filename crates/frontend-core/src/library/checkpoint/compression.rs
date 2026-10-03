//! Bounded zlib checkpoints, with fast encoding and exact stream validation.

use super::{EmuError, HEADER_BYTES, invalid, storage_error};
use flate2::{Decompress, FlushDecompress, Status};
use miniz_oxide::deflate::core::{
    CompressorOxide, TDEFLFlush, TDEFLStatus, compress_to_output, create_comp_flags_from_zip_params,
};
use std::io::{self, BufWriter, Write};

pub(super) fn decode(input: &[u8], expected: u64) -> Result<Vec<u8>, EmuError> {
    let expected = usize::try_from(expected)
        .ok()
        .filter(|length| *length <= save_state::MAX_COMPONENT_BYTES)
        .ok_or_else(|| invalid("The automatic save is too large."))?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(expected)
        .map_err(|_| invalid("Could not allocate the automatic save buffer."))?;
    // A Read EOF can hide a truncated zlib trailer after the whole payload.
    // Decode into fixed capacity and require the checksum-verified stream end.
    let mut decoder = Decompress::new(true);
    let status = decoder
        .decompress_vec(input, &mut output, FlushDecompress::Finish)
        .map_err(|_| invalid("The automatic save is incomplete."))?;
    if status != Status::StreamEnd
        || output.len() != expected
        || decoder.total_in() != input.len() as u64
    {
        return Err(invalid("The automatic save is incomplete."));
    }
    Ok(output)
}

pub(super) fn encode<T: serde::Serialize>(
    value: &T,
    cancelled: &dyn Fn() -> bool,
) -> Result<(usize, Vec<u8>), EmuError> {
    // One-probe miniz compression is substantially faster for decoded PCM
    // than the zlib-rs backend selected by the JAR reader's Cargo features.
    // Both produce the existing zlib wire format; old checkpoints still load.
    let encoder = Encoder {
        compressor: Box::new(CompressorOxide::new(create_comp_flags_from_zip_params(
            1, 15, 0,
        ))),
        output: Vec::new(),
        cancelled,
    };
    let mut buffered = BufWriter::with_capacity(64 * 1024, encoder);
    let length = save_state::write(
        value,
        &mut buffered,
        save_state::MAX_COMPONENT_BYTES,
        cancelled,
    )?;
    let mut encoder = buffered
        .into_inner()
        .map_err(|error| storage_error(error.into_error()))?;
    encoder
        .compress(&[], TDEFLFlush::Finish)
        .map_err(storage_error)?;
    Ok((length, encoder.output))
}

struct Encoder<'a> {
    compressor: Box<CompressorOxide>,
    output: Vec<u8>,
    cancelled: &'a dyn Fn() -> bool,
}

impl Encoder<'_> {
    fn compress(&mut self, bytes: &[u8], flush: TDEFLFlush) -> io::Result<()> {
        if (self.cancelled)() {
            return Err(io::Error::other("checkpoint compression cancelled"));
        }
        let (status, consumed) = compress_to_output(&mut self.compressor, bytes, flush, |block| {
            let limit = save_state::MAX_COMPONENT_BYTES - HEADER_BYTES;
            if block.len() > limit.saturating_sub(self.output.len()) || (self.cancelled)() {
                return false;
            }
            self.output.extend_from_slice(block);
            true
        });
        let expected = if flush == TDEFLFlush::Finish {
            TDEFLStatus::Done
        } else {
            TDEFLStatus::Okay
        };
        if status != expected || consumed != bytes.len() {
            return Err(io::Error::other(
                "checkpoint compression interrupted or exceeded its limit",
            ));
        }
        Ok(())
    }
}

impl Write for Encoder<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for chunk in bytes.chunks(64 * 1024) {
            self.compress(chunk, TDEFLFlush::None)?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/frontend-core/library/checkpoint/compression.rs"]
mod tests;
