//! Stream-PCM wave banks, Yamaha ADPCM decoding and bounded score mixing.

use super::{DecodeState, micros_to_frames, next_chunk};
use crate::media_decode::{resample_mono, resampled_frame_count};
use crate::midi::PcmMix;
use crate::{Clip, Limits, check_decode_cancellation, media_error};
use diagnostics::EmuError;

pub(super) fn parse_wave_bank(
    bytes: &[u8],
    track_index: usize,
    state: &mut DecodeState,
    wave_ids: &mut Vec<u8>,
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EmuError> {
    let mut offset = 0;
    while offset < bytes.len() {
        check_decode_cancellation(cancelled)?;
        let chunk = next_chunk(bytes, &mut offset)?;
        if &chunk.id[..3] != b"Mwa" {
            continue;
        }
        let id = chunk.id[3];
        if !(1..=0x3e).contains(&id) {
            return Err(media_error(
                "media-malformed",
                "invalid SMAF stream-wave id",
            ));
        }
        let key = (track_index, id);
        if state.waves.contains_key(&key) {
            return Err(media_error("media-malformed", "duplicate SMAF wave id"));
        }
        let wave_limits = Limits {
            max_pcm_frames: limits.max_pcm_frames.min(
                limits
                    .max_total_pcm_frames
                    .saturating_sub(state.wave_frames),
            ),
            ..limits
        };
        let clip = decode_wave(chunk.body, wave_limits, cancelled)?;
        state.wave_frames = state
            .wave_frames
            .checked_add(clip.samples.len())
            .filter(|frames| *frames <= limits.max_total_pcm_frames)
            .ok_or_else(|| media_error("media-limit", "SMAF wave-bank limit reached"))?;
        state.waves.insert(key, clip);
        wave_ids.push(id);
    }
    Ok(())
}

fn decode_wave(
    bytes: &[u8],
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    let header = bytes
        .get(..3)
        .ok_or_else(|| media_error("media-malformed", "truncated SMAF wave header"))?;
    let wave_type = header[0];
    let stereo = wave_type & 0x80 != 0;
    let encoding = (wave_type >> 4) & 7;
    let base_bits = wave_type & 0x0f;
    let sample_rate = u32::from(u16::from_be_bytes([header[1], header[2]]));
    if encoding != 2 || base_bits != 0 {
        return Err(media_error(
            "media-unsupported-format",
            "SMAF wave is not 4-bit Yamaha ADPCM",
        ));
    }
    if !(4_000..=48_000).contains(&sample_rate) || bytes.len() == 3 {
        return Err(media_error(
            "media-malformed",
            "invalid SMAF wave sample rate or empty payload",
        ));
    }
    let source_frames = if stereo {
        bytes.len() - 3
    } else {
        (bytes.len() - 3)
            .checked_mul(2)
            .ok_or_else(|| media_error("media-limit", "SMAF wave size overflow"))?
    };
    resampled_frame_count(source_frames, sample_rate, limits.max_pcm_frames)?;

    let mut samples = Vec::with_capacity(source_frames);
    let mut left = YamahaAdpcm::default();
    let mut right = YamahaAdpcm::default();
    for (index, &byte) in bytes[3..].iter().enumerate() {
        if index.is_multiple_of(4096) {
            check_decode_cancellation(cancelled)?;
        }
        let low = byte & 0x0f;
        let high = byte >> 4;
        if stereo {
            samples.push(i16::midpoint(left.decode(low), right.decode(high)));
        } else {
            samples.push(left.decode(low));
            samples.push(left.decode(high));
        }
    }
    resample_mono(samples, sample_rate, limits, cancelled)
}

struct YamahaAdpcm {
    predictor: i32,
    step: i32,
}

impl Default for YamahaAdpcm {
    fn default() -> Self {
        Self {
            predictor: 0,
            step: 127,
        }
    }
}

impl YamahaAdpcm {
    fn decode(&mut self, nibble: u8) -> i16 {
        const DIFFERENCE: [i32; 16] =
            [1, 3, 5, 7, 9, 11, 13, 15, -1, -3, -5, -7, -9, -11, -13, -15];
        const SCALE: [i32; 16] = [
            230, 230, 230, 230, 307, 409, 512, 614, 230, 230, 230, 230, 307, 409, 512, 614,
        ];
        let index = usize::from(nibble & 0x0f);
        // step is always 127..=24576 and predictor is an i16 value. Both
        // products and the predictor sum fit in i32 before codec clamping.
        self.predictor = (self.predictor + self.step * DIFFERENCE[index] / 8)
            .clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        self.step = ((self.step * SCALE[index]) >> 8).clamp(127, 24_576);
        i16::try_from(self.predictor).expect("SMAF ADPCM predictor is clamped to i16")
    }
}

pub(super) fn mix_stream_pcm(
    mut mixed: PcmMix,
    state: &DecodeState,
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    if state.triggers.is_empty() {
        return Ok(mixed.into_clip());
    }
    let target_frames = mixed.samples.len().max(micros_to_frames(state.end_micros));
    if target_frames > limits.max_pcm_frames {
        return Err(media_error(
            "media-limit",
            "decoded SMAF exceeds duration limit",
        ));
    }
    mixed.samples.resize(target_frames, 0);
    mixed.duration_micros = mixed
        .duration_micros
        .max(i64::try_from(state.end_micros).unwrap_or(i64::MAX));
    let mut work = 0_usize;
    for trigger in &state.triggers {
        let wave = state
            .waves
            .get(&trigger.key)
            .ok_or_else(|| media_error("media-malformed", "missing decoded SMAF wave"))?;
        let start = micros_to_frames(trigger.start_micros).min(mixed.samples.len());
        let gate_frames = micros_to_frames(trigger.gate_micros);
        let count = wave
            .samples
            .len()
            .min(gate_frames)
            .min(mixed.samples.len().saturating_sub(start));
        work = work
            .checked_add(count)
            .filter(|frames| *frames <= limits.max_midi_voice_frames)
            .ok_or_else(|| media_error("media-work-limit", "SMAF mixing-work limit reached"))?;
        for (output, samples) in mixed.samples[start..start + count]
            .chunks_mut(4_096)
            .zip(wave.samples[..count].chunks(4_096))
        {
            check_decode_cancellation(cancelled)?;
            for (output, &sample) in output.iter_mut().zip(samples) {
                *output = output.saturating_add(
                    i32::from(sample).saturating_mul(i32::from(trigger.velocity)) / 127,
                );
            }
        }
    }
    Ok(mixed.into_clip())
}

#[cfg(test)]
#[path = "../../../../tests/unit/mmapi/smaf/pcm.rs"]
mod tests;
