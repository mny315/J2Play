use super::media_decode::resampled_frame_count;
use super::{Clip, EmuError, Limits, check_decode_cancellation, media_error, resample_mono};

pub(super) fn decode_wav(
    bytes: &[u8],
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(media_error("media-malformed", "invalid RIFF/WAVE header"));
    }
    let declared = usize::try_from(read_u32_le(bytes, 4)?)
        .map_err(|_| media_error("media-malformed", "WAV size overflow"))?;
    if declared.checked_add(8) != Some(bytes.len()) {
        return Err(media_error("media-malformed", "WAV RIFF size mismatch"));
    }
    let mut offset = 12;
    let mut format = None;
    let mut data = None;
    while offset < bytes.len() {
        check_decode_cancellation(cancelled)?;
        if bytes.len() - offset < 8 {
            return Err(media_error("media-malformed", "truncated WAV chunk"));
        }
        let id = &bytes[offset..offset + 4];
        let length = usize::try_from(read_u32_le(bytes, offset + 4)?)
            .map_err(|_| media_error("media-malformed", "WAV chunk size overflow"))?;
        offset += 8;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| media_error("media-malformed", "WAV chunk overflow"))?;
        if end > bytes.len() {
            return Err(media_error(
                "media-malformed",
                "truncated WAV chunk payload",
            ));
        }
        if id == b"fmt " {
            if format.is_some() || length < 16 {
                return Err(media_error(
                    "media-malformed",
                    "invalid duplicate WAV fmt chunk",
                ));
            }
            format = Some(parse_wav_format(&bytes[offset..end])?);
        } else if id == b"data" {
            if data.is_some() {
                return Err(media_error("media-malformed", "duplicate WAV data chunk"));
            }
            data = Some(&bytes[offset..end]);
        }
        if length & 1 == 1 {
            if end >= bytes.len() {
                return Err(media_error("media-malformed", "missing WAV chunk padding"));
            }
            offset = end + 1;
        } else {
            offset = end;
        }
    }
    let format = format.ok_or_else(|| media_error("media-malformed", "missing WAV fmt chunk"))?;
    let data = data.ok_or_else(|| media_error("media-malformed", "missing WAV data chunk"))?;
    let frames = source_frame_count(data.len(), format)?;
    resampled_frame_count(frames, format.sample_rate, limits.max_pcm_frames)?;
    let source_samples = match format.encoding {
        WavEncoding::Pcm => decode_pcm_wav_samples(data, format, frames, cancelled)?,
        WavEncoding::ImaAdpcm => decode_ima_adpcm_wav_samples(data, format, frames, cancelled)?,
    };
    resample_mono(source_samples, format.sample_rate, limits, cancelled)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WavEncoding {
    Pcm,
    ImaAdpcm,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct WavFormat {
    encoding: WavEncoding,
    channels: u16,
    sample_rate: u32,
    block_align: u16,
    bits: u16,
    samples_per_block: u16,
}

pub(super) fn parse_wav_format(bytes: &[u8]) -> Result<WavFormat, EmuError> {
    let encoding = read_u16_le(bytes, 0)?;
    let channels = read_u16_le(bytes, 2)?;
    let sample_rate = read_u32_le(bytes, 4)?;
    let byte_rate = read_u32_le(bytes, 8)?;
    let block_align = read_u16_le(bytes, 12)?;
    let bits = read_u16_le(bytes, 14)?;
    if !(4_000..=48_000).contains(&sample_rate) || !matches!(channels, 1 | 2) {
        return Err(media_error(
            "media-unsupported-format",
            "unsupported WAV sample rate or channel count",
        ));
    }
    match encoding {
        1 => {
            if !matches!(bits, 8 | 16) {
                return Err(media_error(
                    "media-unsupported-format",
                    "PCM WAV must use 8-bit or 16-bit samples",
                ));
            }
            let expected_align = channels.saturating_mul(bits / 8);
            let expected_rate = sample_rate.saturating_mul(u32::from(expected_align));
            if block_align != expected_align || byte_rate != expected_rate {
                return Err(media_error(
                    "media-malformed",
                    "inconsistent PCM WAV format rates",
                ));
            }
            Ok(WavFormat {
                encoding: WavEncoding::Pcm,
                channels,
                sample_rate,
                block_align,
                bits,
                samples_per_block: 1,
            })
        }
        0x11 => {
            // Microsoft/DVI IMA ADPCM as carried in Java ME WAV resources.
            if bits != 4 || channels != 1 || block_align < 5 || bytes.len() < 20 {
                return Err(media_error(
                    "media-unsupported-format",
                    "IMA ADPCM WAV currently requires 4-bit mono blocks",
                ));
            }
            let extra_size = read_u16_le(bytes, 16)?;
            let samples_per_block = read_u16_le(bytes, 18)?;
            let expected_samples =
                1_u32.saturating_add(u32::from(block_align.saturating_sub(4)).saturating_mul(2));
            if extra_size < 2 || u32::from(samples_per_block) != expected_samples {
                return Err(media_error(
                    "media-malformed",
                    "invalid IMA ADPCM samples-per-block",
                ));
            }
            // Old feature-phone encoders commonly wrote the nominal 4-bit
            // payload rate (sample_rate / 2) here and omitted the per-block
            // predictor overhead. Decode boundaries come from block_align and
            // samples_per_block, so the advisory average rate is not needed.
            Ok(WavFormat {
                encoding: WavEncoding::ImaAdpcm,
                channels,
                sample_rate,
                block_align,
                bits,
                samples_per_block,
            })
        }
        _ => Err(media_error(
            "media-unsupported-format",
            format!("unsupported WAV codec 0x{encoding:04x}"),
        )),
    }
}

fn source_frame_count(bytes: usize, format: WavFormat) -> Result<usize, EmuError> {
    let align = usize::from(format.block_align);
    if align == 0 || !bytes.is_multiple_of(align) {
        return Err(media_error(
            "media-malformed",
            "partial WAV sample frame or block",
        ));
    }
    let blocks = bytes / align;
    let frames_per_block = match format.encoding {
        WavEncoding::Pcm => 1,
        WavEncoding::ImaAdpcm => usize::from(format.samples_per_block),
    };
    blocks
        .checked_mul(frames_per_block)
        .ok_or_else(|| media_error("media-limit", "WAV sample count overflow"))
}

fn decode_pcm_wav_samples(
    data: &[u8],
    format: WavFormat,
    frames: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<i16>, EmuError> {
    let align = usize::from(format.block_align);
    let mut samples = Vec::with_capacity(frames);
    // Format and frame alignment are validated before entering the sample loop.
    // Poll on the same frame boundaries while decoding each format in bulk.
    for block in data.chunks(align * 4_096) {
        check_decode_cancellation(cancelled)?;
        match (format.bits, format.channels) {
            (8, 1) => samples.extend(block.iter().map(|&byte| (i16::from(byte) - 128) << 8)),
            (8, 2) => {
                samples.extend(block.as_chunks::<2>().0.iter().map(|&[left, right]| {
                    let left = (i16::from(left) - 128) << 8;
                    let right = (i16::from(right) - 128) << 8;
                    i16::midpoint(left, right)
                }));
            }
            (16, 1) => samples.extend(
                block
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .copied()
                    .map(i16::from_le_bytes),
            ),
            (16, 2) => {
                samples.extend(block.as_chunks::<4>().0.iter().map(|&[l0, l1, r0, r1]| {
                    i16::midpoint(i16::from_le_bytes([l0, l1]), i16::from_le_bytes([r0, r1]))
                }));
            }
            _ => unreachable!(),
        }
    }
    Ok(samples)
}

fn decode_ima_adpcm_wav_samples(
    data: &[u8],
    format: WavFormat,
    frames: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<i16>, EmuError> {
    const STEP_TABLE: [i32; 89] = [
        7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60,
        66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371,
        408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878,
        2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845,
        8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086,
        29794, 32767,
    ];
    const INDEX_TABLE: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];
    let block_align = usize::from(format.block_align);
    let mut output = Vec::with_capacity(frames);
    for block in data.chunks_exact(block_align) {
        check_decode_cancellation(cancelled)?;
        let mut predictor = i32::from(i16::from_le_bytes([block[0], block[1]]));
        let mut index = i32::from(block[2]);
        if !(0..=88).contains(&index) || block[3] != 0 {
            return Err(media_error(
                "media-malformed",
                "invalid IMA ADPCM block header",
            ));
        }
        output.push(i16::try_from(predictor).expect("initial predictor is an i16"));
        for &byte in &block[4..] {
            for nibble in [byte & 0x0f, byte >> 4] {
                let step = STEP_TABLE[usize::try_from(index).unwrap_or(0)];
                // Keep the division until after the weighted sum. Splitting it
                // into step>>3 + step>>2 ... changes rounding and audibly drifts
                // from Microsoft's IMA ADPCM decoder on small step sizes.
                let delta = i32::from((nibble & 7).saturating_mul(2).saturating_add(1))
                    .saturating_mul(step)
                    / 8;
                if nibble & 8 != 0 {
                    predictor -= delta;
                } else {
                    predictor += delta;
                }
                predictor = predictor.clamp(i32::from(i16::MIN), i32::from(i16::MAX));
                index = (index + INDEX_TABLE[usize::from(nibble)]).clamp(0, 88);
                output.push(i16::try_from(predictor).expect("predictor is clamped to i16"));
            }
        }
    }
    Ok(output)
}

pub(super) fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16, EmuError> {
    bytes
        .get(offset..offset + 2)
        .map(|value| u16::from_le_bytes([value[0], value[1]]))
        .ok_or_else(|| media_error("media-malformed", "truncated little-endian value"))
}

pub(super) fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, EmuError> {
    bytes
        .get(offset..offset + 4)
        .map(|value| u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
        .ok_or_else(|| media_error("media-malformed", "truncated little-endian value"))
}

#[cfg(test)]
#[path = "../../../tests/unit/mmapi/wav.rs"]
mod tests;
