mod render;

pub(super) use render::{PcmMix, render_midi_events};

use super::{
    Clip, EmuError, Limits, MAX_MIDI_TRACKS, MAX_MIDI_TRUNCATED_TAIL_BYTES,
    MAX_MIDI_ZERO_PADDING_BYTES, media_error,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum MidiKind {
    EndOfTrack,
    Tempo(u32),
    ProgramChange {
        channel: u8,
        program: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        channel: u8,
        note: u8,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MidiEvent {
    pub(super) tick: u64,
    /// Unique insertion order resolves simultaneous events without stable sorting.
    pub(super) order: usize,
    pub(super) kind: MidiKind,
}

#[derive(Debug)]
pub(super) enum TrackError {
    // Only missing input can use the bounded legacy-tail recovery path.
    Incomplete(&'static str),
    Failure(EmuError),
}

impl TrackError {
    fn into_error(self) -> EmuError {
        match self {
            Self::Incomplete(message) => media_error("media-malformed", message),
            Self::Failure(error) => error,
        }
    }
}

impl From<EmuError> for TrackError {
    fn from(error: EmuError) -> Self {
        Self::Failure(error)
    }
}

pub(super) fn decode_midi(
    bytes: &[u8],
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    if bytes.len() < 14 || &bytes[..4] != b"MThd" || read_u32_be(bytes, 4)? != 6 {
        return Err(media_error("media-malformed", "invalid MIDI header"));
    }
    let format = read_u16_be(bytes, 8)?;
    let tracks = read_u16_be(bytes, 10)?;
    let division = read_u16_be(bytes, 12)?;
    if format > 1
        || tracks == 0
        || tracks > MAX_MIDI_TRACKS
        || (format == 0 && tracks != 1)
        || division == 0
        || division & 0x8000 != 0
    {
        return Err(media_error(
            "media-unsupported-format",
            "unsupported MIDI timing/header",
        ));
    }
    let mut offset = 14;
    let mut events = Vec::new();
    for track_index in 0..tracks {
        if bytes.get(offset..offset + 4) != Some(b"MTrk") {
            return Err(media_error("media-malformed", "missing MIDI track"));
        }
        let length = usize::try_from(read_u32_be(bytes, offset + 4)?)
            .map_err(|_| media_error("media-malformed", "MIDI track length overflow"))?;
        offset += 8;
        let declared_end = offset
            .checked_add(length)
            .ok_or_else(|| media_error("media-malformed", "MIDI track length overflow"))?;
        let missing_tail = declared_end.saturating_sub(bytes.len());
        let truncated_tail = declared_end > bytes.len()
            && track_index + 1 == tracks
            && missing_tail <= MAX_MIDI_TRUNCATED_TAIL_BYTES;
        let end = if truncated_tail {
            bytes.len()
        } else {
            declared_end
        };
        if end > bytes.len() {
            return Err(media_error("media-malformed", "truncated MIDI track"));
        }

        let event_count_before = events.len();
        match parse_midi_track(
            &bytes[offset..end],
            &mut events,
            limits.max_midi_events,
            cancelled,
        ) {
            Ok(()) => {}
            Err(TrackError::Incomplete(_))
                if truncated_tail && events.len() > event_count_before => {}
            Err(error) => return Err(error.into_error()),
        }
        offset = end;
    }
    let trailing = &bytes[offset..];
    if trailing.len() > MAX_MIDI_ZERO_PADDING_BYTES || trailing.iter().any(|byte| *byte != 0) {
        return Err(media_error("media-malformed", "trailing MIDI data"));
    }
    events.sort_unstable_by_key(|event| (event.tick, event.order));
    render_midi_events(&events, division, limits, cancelled).map(PcmMix::into_clip)
}

#[allow(clippy::too_many_lines)]
pub(super) fn parse_midi_track(
    bytes: &[u8],
    events: &mut Vec<MidiEvent>,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TrackError> {
    let mut offset = 0;
    let mut tick = 0_u64;
    let mut running = None;
    let mut messages = 0_usize;
    while offset < bytes.len() {
        // Include ignored messages: they can fill a track without adding any
        // retained events or ever reaching the synthesizer's cancellation points.
        if messages.is_multiple_of(1024) {
            super::check_decode_cancellation(cancelled)?;
        }
        messages += 1;
        let delta = read_vlq(bytes, &mut offset)?;
        tick = tick
            .checked_add(u64::from(delta))
            .ok_or_else(|| media_error("media-limit", "MIDI tick overflow"))?;
        let first = *bytes
            .get(offset)
            .ok_or(TrackError::Incomplete("truncated MIDI event"))?;
        let status = if first & 0x80 != 0 {
            offset += 1;
            if first < 0xf0 {
                running = Some(first);
            }
            first
        } else {
            running.ok_or_else(|| media_error("media-malformed", "invalid MIDI running status"))?
        };
        let kind = match status {
            0x80..=0x9f => {
                let pair = bytes
                    .get(offset..offset + 2)
                    .ok_or(TrackError::Incomplete("truncated MIDI note"))?;
                if pair[0] > 127 || pair[1] > 127 {
                    return Err(media_error("media-malformed", "invalid MIDI data byte").into());
                }
                let kind = if status & 0xf0 == 0x90 && pair[1] != 0 {
                    MidiKind::NoteOn {
                        channel: status & 0x0f,
                        note: pair[0],
                        velocity: pair[1],
                    }
                } else {
                    MidiKind::NoteOff {
                        channel: status & 0x0f,
                        note: pair[0],
                    }
                };
                offset += 2;
                Some(kind)
            }
            0xa0..=0xaf | 0xe0..=0xef => {
                offset = skip_midi(bytes, offset, 2)?;
                None
            }
            0xb0..=0xbf => {
                let pair = bytes
                    .get(offset..offset + 2)
                    .ok_or(TrackError::Incomplete("truncated MIDI controller"))?;
                if pair[0] > 127 || pair[1] > 127 {
                    return Err(media_error("media-malformed", "invalid MIDI data byte").into());
                }
                offset += 2;
                Some(MidiKind::ControlChange {
                    channel: status & 0x0f,
                    controller: pair[0],
                    value: pair[1],
                })
            }
            0xc0..=0xcf => {
                let program = *bytes
                    .get(offset)
                    .ok_or(TrackError::Incomplete("truncated MIDI program change"))?;
                if program > 127 {
                    return Err(media_error("media-malformed", "invalid MIDI data byte").into());
                }
                offset += 1;
                Some(MidiKind::ProgramChange {
                    channel: status & 0x0f,
                    program,
                })
            }
            0xd0..=0xdf => {
                offset = skip_midi(bytes, offset, 1)?;
                None
            }
            0xff => {
                running = None;
                let kind = *bytes
                    .get(offset)
                    .ok_or(TrackError::Incomplete("truncated MIDI meta event"))?;
                offset += 1;
                let length = usize::try_from(read_vlq(bytes, &mut offset)?)
                    .map_err(|_| media_error("media-malformed", "MIDI meta length overflow"))?;
                let payload = bytes
                    .get(offset..offset + length)
                    .ok_or(TrackError::Incomplete("truncated MIDI meta payload"))?;
                let event = if kind == 0x51 {
                    if payload.len() != 3 {
                        return Err(
                            media_error("media-malformed", "invalid MIDI tempo event").into()
                        );
                    }
                    let tempo = (u32::from(payload[0]) << 16)
                        | (u32::from(payload[1]) << 8)
                        | u32::from(payload[2]);
                    if tempo == 0 {
                        return Err(media_error("media-malformed", "zero MIDI tempo").into());
                    }
                    Some(MidiKind::Tempo(tempo))
                } else if kind == 0x2f {
                    if !payload.is_empty() {
                        return Err(media_error(
                            "media-malformed",
                            "invalid MIDI end-of-track event",
                        )
                        .into());
                    }
                    Some(MidiKind::EndOfTrack)
                } else {
                    None
                };
                offset += length;
                if kind == 0x2f && offset != bytes.len() {
                    return Err(
                        media_error("media-malformed", "data after MIDI end-of-track").into(),
                    );
                }
                event
            }
            0xf0 | 0xf7 => {
                running = None;
                let length = usize::try_from(read_vlq(bytes, &mut offset)?)
                    .map_err(|_| media_error("media-malformed", "MIDI sysex length overflow"))?;
                offset = skip_midi_bytes(bytes, offset, length)?;
                None
            }
            _ => return Err(media_error("media-malformed", "unsupported MIDI status").into()),
        };
        if let Some(kind) = kind {
            if events.len() >= limit {
                return Err(media_error("media-limit", "MIDI event limit reached").into());
            }
            events.push(MidiEvent {
                tick,
                order: events.len(),
                kind,
            });
            if matches!(kind, MidiKind::EndOfTrack) {
                return Ok(());
            }
        }
    }
    Err(TrackError::Incomplete(
        "MIDI track has no end-of-track event",
    ))
}

fn read_vlq(bytes: &[u8], offset: &mut usize) -> Result<u32, TrackError> {
    let mut value = 0_u32;
    for index in 0..4 {
        let byte = *bytes
            .get(*offset)
            .ok_or(TrackError::Incomplete("truncated MIDI VLQ"))?;
        *offset += 1;
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            if index > 0 && value < (1 << (index * 7)) {
                return Err(media_error("media-malformed", "noncanonical MIDI VLQ").into());
            }
            return Ok(value);
        }
    }
    Err(media_error("media-malformed", "MIDI VLQ exceeds four bytes").into())
}

fn skip_midi(bytes: &[u8], offset: usize, length: usize) -> Result<usize, TrackError> {
    let end = skip_midi_bytes(bytes, offset, length)?;
    if bytes[offset..end].iter().any(|byte| byte & 0x80 != 0) {
        return Err(media_error("media-malformed", "invalid MIDI data byte").into());
    }
    Ok(end)
}

fn skip_midi_bytes(bytes: &[u8], offset: usize, length: usize) -> Result<usize, TrackError> {
    offset
        .checked_add(length)
        .filter(|end| *end <= bytes.len())
        .ok_or(TrackError::Incomplete("truncated MIDI event payload"))
}

pub(super) fn read_u16_be(bytes: &[u8], offset: usize) -> Result<u16, EmuError> {
    bytes
        .get(offset..offset + 2)
        .map(|value| u16::from_be_bytes([value[0], value[1]]))
        .ok_or_else(|| media_error("media-malformed", "truncated big-endian value"))
}

pub(super) fn read_u32_be(bytes: &[u8], offset: usize) -> Result<u32, EmuError> {
    bytes
        .get(offset..offset + 4)
        .map(|value| u32::from_be_bytes([value[0], value[1], value[2], value[3]]))
        .ok_or_else(|| media_error("media-malformed", "truncated big-endian value"))
}

#[cfg(test)]
#[path = "../../../tests/unit/mmapi/midi.rs"]
mod tests;
