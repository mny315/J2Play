//! Bounded safe wrapper around the `OpenCORE` AMR-NB decoder.

use std::fmt;

mod ffi;

/// AMR-NB storage-file signature from RFC 4867.
pub const FILE_MAGIC: &[u8; 6] = b"#!AMR\n";
/// AMR-NB always produces 8 kHz mono PCM.
pub const SAMPLE_RATE: u32 = 8_000;
const SAMPLES_PER_FRAME: usize = 160;
const FRAME_BYTES: [usize; 16] = [13, 14, 16, 18, 20, 21, 27, 32, 6, 0, 0, 0, 0, 0, 0, 1];

/// Controlled failures reported while parsing or decoding AMR-NB input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    InvalidHeader,
    InvalidFrameHeader,
    ReservedFrameType(u8),
    TruncatedFrame,
    SampleLimit,
    Cancelled,
    DecoderInitialization,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => formatter.write_str("invalid AMR-NB storage header"),
            Self::InvalidFrameHeader => formatter.write_str("invalid AMR-NB frame header"),
            Self::ReservedFrameType(frame_type) => {
                write!(formatter, "reserved AMR-NB frame type: {frame_type}")
            }
            Self::TruncatedFrame => formatter.write_str("truncated AMR-NB frame"),
            Self::SampleLimit => formatter.write_str("decoded AMR-NB sample limit reached"),
            Self::Cancelled => formatter.write_str("AMR-NB decoding was cancelled by the host"),
            Self::DecoderInitialization => {
                formatter.write_str("cannot initialize the AMR-NB decoder")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Decodes an RFC 4867 AMR-NB storage file to 8 kHz mono signed PCM.
///
/// The complete stream is validated before native codec state is allocated.
/// `max_samples` is checked before the output buffer grows or a frame is
/// passed across the native boundary.
///
/// # Errors
/// Returns a controlled error for malformed input, reserved frame types,
/// decoder initialization failure, or output exceeding `max_samples`.
pub fn decode_storage_file(data: &[u8], max_samples: usize) -> Result<Vec<i16>, DecodeError> {
    decode_storage_file_with_cancellation(data, max_samples, &|| false)
}

/// Decodes a storage file with cooperative cancellation between complete frames.
///
/// # Errors
/// Returns the same validation and codec errors as [`decode_storage_file`], or
/// [`DecodeError::Cancelled`] before parsing or decoding another frame.
pub fn decode_storage_file_with_cancellation(
    data: &[u8],
    max_samples: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<i16>, DecodeError> {
    let sample_count = validated_sample_count(data, max_samples, cancelled)?;
    let mut decoder = ffi::Decoder::new()?;
    let mut samples = Vec::with_capacity(sample_count);
    let mut remaining = &data[FILE_MAGIC.len()..];
    while let Some(&header) = remaining.first() {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        // The first pass validated every frame boundary in this immutable input.
        let frame_length = FRAME_BYTES[usize::from((header >> 3) & 0x0f)];
        let (frame, tail) = remaining.split_at(frame_length);
        let mut pcm = [0_i16; SAMPLES_PER_FRAME];
        decoder.decode(frame, &mut pcm, frame_is_bad(frame));
        samples.extend_from_slice(&pcm);
        remaining = tail;
    }
    Ok(samples)
}

fn frame_is_bad(frame: &[u8]) -> bool {
    frame.first().is_none_or(|header| header & 0x04 == 0)
}

fn validated_sample_count(
    data: &[u8],
    max_samples: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, DecodeError> {
    let mut remaining = data
        .strip_prefix(FILE_MAGIC)
        .ok_or(DecodeError::InvalidHeader)?;
    let mut sample_count = 0_usize;
    while let Some((&header, tail)) = remaining.split_first() {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        if header & 0x83 != 0 {
            return Err(DecodeError::InvalidFrameHeader);
        }
        let frame_type = (header >> 3) & 0x0f;
        let frame_length = FRAME_BYTES[usize::from(frame_type)];
        if frame_length == 0 {
            return Err(DecodeError::ReservedFrameType(frame_type));
        }
        let payload_length = frame_length - 1;
        if tail.len() < payload_length {
            return Err(DecodeError::TruncatedFrame);
        }
        sample_count = sample_count
            .checked_add(SAMPLES_PER_FRAME)
            .filter(|&count| count <= max_samples)
            .ok_or(DecodeError::SampleLimit)?;
        remaining = &tail[payload_length..];
    }
    Ok(sample_count)
}

#[cfg(test)]
#[path = "../../../tests/unit/amr-nb/mod.rs"]
mod tests;
