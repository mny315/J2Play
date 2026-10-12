//! Bounded decoder for the Yamaha SMAF subset used by Samsung `AudioClip`.

mod pcm;
mod sequence;

use super::{
    Clip, Limits, MidiEvent, MidiKind, OUTPUT_SAMPLE_RATE, media_error, render_midi_events,
};
use diagnostics::EmuError;
use pcm::{mix_stream_pcm, parse_wave_bank};
use sequence::{parse_handy_phone, parse_mobile};
use std::collections::BTreeMap;

const MAX_SMAF_TRACKS: usize = 64;

#[derive(Clone, Copy)]
struct Chunk<'a> {
    id: [u8; 4],
    body: &'a [u8],
}

#[derive(Clone, Copy)]
struct WaveTrigger {
    key: (usize, u8),
    start_micros: u64,
    gate_micros: u64,
    velocity: u8,
}

struct DecodeState {
    events: Vec<MidiEvent>,
    messages: usize,
    sound_events: usize,
    end_micros: u64,
    waves: BTreeMap<(usize, u8), Clip>,
    wave_frames: usize,
    triggers: Vec<WaveTrigger>,
}

impl DecodeState {
    fn new() -> Self {
        Self {
            events: vec![MidiEvent {
                tick: 0,
                order: 0,
                kind: MidiKind::Tempo(1),
            }],
            messages: 0,
            sound_events: 0,
            end_micros: 0,
            waves: BTreeMap::new(),
            wave_frames: 0,
            triggers: Vec::new(),
        }
    }

    fn count_message(
        &mut self,
        limit: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EmuError> {
        if self.messages.is_multiple_of(1024) {
            super::check_decode_cancellation(cancelled)?;
        }
        self.messages = self
            .messages
            .checked_add(1)
            .filter(|count| *count <= limit)
            .ok_or_else(|| media_error("media-limit", "SMAF event limit reached"))?;
        Ok(())
    }

    fn push_midi(&mut self, tick: u64, kind: MidiKind, limit: usize) -> Result<(), EmuError> {
        if self.events.len() >= limit {
            return Err(media_error("media-limit", "SMAF event limit reached"));
        }
        self.events.push(MidiEvent {
            tick,
            order: self.events.len(),
            kind,
        });
        Ok(())
    }
}

struct TrackContext<'a> {
    format: u8,
    number: u8,
    index: usize,
    duration_tick_micros: u64,
    gate_tick_micros: u64,
    channel_status: &'a [u8],
    wave_ids: &'a [u8],
}

impl TrackContext<'_> {
    fn channel(&self, local: u8) -> u8 {
        let rhythm = if self.format == 0 {
            let byte = self
                .channel_status
                .get(usize::from(local) / 2)
                .copied()
                .unwrap_or_default();
            let nibble = if local & 1 == 0 {
                byte >> 4
            } else {
                byte & 0x0f
            };
            nibble & 3 == 3
        } else {
            self.channel_status
                .get(usize::from(local))
                .is_some_and(|status| status & 3 == 3)
        };
        if rhythm {
            return 9;
        }
        if self.format != 0 {
            return local.min(15);
        }
        let group = usize::from(self.number).min(3);
        let raw = group.saturating_mul(4).saturating_add(usize::from(local));
        u8::try_from(if raw == 9 { 15 } else { raw.min(15) }).unwrap_or(15)
    }
}

pub(super) fn decode(
    bytes: &[u8],
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    if bytes.len() < 10 || bytes.get(..4) != Some(b"MMMD") {
        return Err(media_error("media-malformed", "invalid SMAF header"));
    }
    let declared = usize::try_from(read_u32_be(bytes, 4)?)
        .map_err(|_| media_error("media-malformed", "SMAF size overflow"))?;
    if declared != bytes.len() - 8 || declared < 2 {
        return Err(media_error(
            "media-malformed",
            "SMAF container size does not match the input",
        ));
    }

    let mut state = DecodeState::new();
    let mut offset = 0;
    let mut tracks = 0;
    let root = &bytes[8..bytes.len() - 2]; // The final two bytes are the SMAF CRC.
    while offset < root.len() {
        super::check_decode_cancellation(cancelled)?;
        let chunk = next_chunk(root, &mut offset)?;
        if &chunk.id[..3] != b"MTR" {
            continue;
        }
        tracks += 1;
        if tracks > MAX_SMAF_TRACKS {
            return Err(media_error("media-limit", "SMAF track limit reached"));
        }
        decode_score_track(chunk, tracks - 1, &mut state, limits, cancelled)?;
    }
    if tracks == 0 || state.sound_events == 0 {
        return Err(media_error(
            "media-unsupported-format",
            "SMAF file has no playable score or stream-PCM events",
        ));
    }

    state.push_midi(
        state.end_micros,
        MidiKind::EndOfTrack,
        limits.max_midi_events,
    )?;
    state
        .events
        .sort_unstable_by_key(|event| (event.tick, event.order));
    let synthesized = render_midi_events(&state.events, 1, limits, cancelled)?;
    mix_stream_pcm(synthesized, &state, limits, cancelled)
}

fn decode_score_track(
    chunk: Chunk<'_>,
    track_index: usize,
    state: &mut DecodeState,
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EmuError> {
    let format = *chunk
        .body
        .first()
        .ok_or_else(|| media_error("media-malformed", "truncated SMAF score-track header"))?;
    let header_len = match format {
        0 => 6,
        2 => 20,
        1 => {
            return Err(media_error(
                "media-unsupported-format",
                "Huffman-compressed SMAF score tracks are unsupported",
            ));
        }
        _ => {
            return Err(media_error(
                "media-unsupported-format",
                format!("unsupported SMAF score format: {format}"),
            ));
        }
    };
    let header = chunk
        .body
        .get(..header_len)
        .ok_or_else(|| media_error("media-malformed", "truncated SMAF score-track header"))?;
    if header[1] != 0 {
        return Err(media_error(
            "media-unsupported-format",
            "SMAF sub-sequence score tracks are unsupported",
        ));
    }
    let duration_tick_micros = timebase_micros(header[2])?;
    let gate_tick_micros = timebase_micros(header[3])?;
    let channel_status = &header[4..];
    let mut sequence = None;
    let mut wave_ids = Vec::new();
    let mut offset = header_len;
    while offset < chunk.body.len() {
        super::check_decode_cancellation(cancelled)?;
        let child = next_chunk(chunk.body, &mut offset)?;
        match &child.id {
            b"Mtsq" => {
                if sequence.replace(child.body).is_some() {
                    return Err(media_error(
                        "media-malformed",
                        "duplicate SMAF score sequence",
                    ));
                }
            }
            b"Mtsp" => parse_wave_bank(
                child.body,
                track_index,
                state,
                &mut wave_ids,
                limits,
                cancelled,
            )?,
            _ => {}
        }
    }
    let sequence = sequence.ok_or_else(|| {
        media_error(
            "media-unsupported-format",
            "SMAF score track has no sequence data",
        )
    })?;
    let context = TrackContext {
        format,
        number: chunk.id[3],
        index: track_index,
        duration_tick_micros,
        gate_tick_micros,
        channel_status,
        wave_ids: &wave_ids,
    };
    if format == 0 {
        parse_handy_phone(sequence, &context, state, limits, cancelled)
    } else {
        parse_mobile(sequence, &context, state, limits, cancelled)
    }
}

fn next_chunk<'a>(bytes: &'a [u8], offset: &mut usize) -> Result<Chunk<'a>, EmuError> {
    let header = bytes
        .get(*offset..offset.saturating_add(8))
        .ok_or_else(|| media_error("media-malformed", "truncated SMAF chunk header"))?;
    let id = [header[0], header[1], header[2], header[3]];
    let length = usize::try_from(u32::from_be_bytes([
        header[4], header[5], header[6], header[7],
    ]))
    .map_err(|_| media_error("media-malformed", "SMAF chunk size overflow"))?;
    let body_start = offset
        .checked_add(8)
        .ok_or_else(|| media_error("media-malformed", "SMAF chunk offset overflow"))?;
    let end = body_start
        .checked_add(length)
        .ok_or_else(|| media_error("media-malformed", "SMAF chunk size overflow"))?;
    let body = bytes
        .get(body_start..end)
        .ok_or_else(|| media_error("media-malformed", "truncated SMAF chunk body"))?;
    *offset = end;
    Ok(Chunk { id, body })
}

fn read_u32_be(bytes: &[u8], offset: usize) -> Result<u32, EmuError> {
    bytes
        .get(offset..offset.saturating_add(4))
        .map(|value| u32::from_be_bytes([value[0], value[1], value[2], value[3]]))
        .ok_or_else(|| media_error("media-malformed", "truncated SMAF integer"))
}

fn timebase_micros(value: u8) -> Result<u64, EmuError> {
    let millis = match value {
        0x00 => 1,
        0x01 => 2,
        0x02 => 4,
        0x03 => 5,
        0x10 => 10,
        0x11 => 20,
        0x12 => 40,
        0x13 => 50,
        _ => {
            return Err(media_error(
                "media-unsupported-format",
                format!("unsupported SMAF timebase: 0x{value:02x}"),
            ));
        }
    };
    Ok(millis * 1_000)
}

fn micros_to_frames(micros: u64) -> usize {
    usize::try_from(micros.saturating_mul(u64::from(OUTPUT_SAMPLE_RATE)) / 1_000_000)
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
#[path = "../../../tests/unit/mmapi/smaf/mod.rs"]
mod tests;
