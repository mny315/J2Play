//! SMAF Mobile and `HandyPhone` event streams, notes and timeline bounds.

use super::{
    DecodeState, EmuError, Limits, MidiKind, TrackContext, WaveTrigger, media_error,
    micros_to_frames,
};

#[derive(Clone, Copy)]
struct SmafNote {
    local: u8,
    note: u8,
    velocity: u8,
    gate: u32,
    current: u64,
}

pub(super) fn parse_mobile(
    bytes: &[u8],
    context: &TrackContext<'_>,
    state: &mut DecodeState,
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EmuError> {
    let mut offset = 0;
    let mut current = 0_u64;
    let mut velocities = [64_u8; 16];
    while offset < bytes.len() {
        state.count_message(limits.max_midi_events, cancelled)?;
        let delta = read_mobile_vlq(bytes, &mut offset)?;
        current = add_time(current, delta, context.duration_tick_micros, limits)?;
        state.end_micros = state.end_micros.max(current);
        let status = take(bytes, &mut offset, "truncated SMAF event")?;
        let local = status & 0x0f;
        match status {
            0x80..=0x9f => {
                let note = take_data(bytes, &mut offset, "truncated SMAF note")?;
                let velocity = if status & 0xf0 == 0x90 {
                    let value = take_data(bytes, &mut offset, "truncated SMAF note velocity")?;
                    velocities[usize::from(local)] = value;
                    value
                } else {
                    velocities[usize::from(local)]
                };
                let gate = read_mobile_vlq(bytes, &mut offset)?;
                add_note(
                    context,
                    state,
                    SmafNote {
                        local,
                        note,
                        velocity,
                        gate,
                        current,
                    },
                    limits,
                )?;
            }
            0xa0..=0xaf | 0xe0..=0xef => skip_data(bytes, &mut offset, 2)?,
            0xb0..=0xbf => {
                let controller = take_data(bytes, &mut offset, "truncated SMAF controller")?;
                let value = take_data(bytes, &mut offset, "truncated SMAF controller value")?;
                if context.wave_ids.is_empty() {
                    state.push_midi(
                        current,
                        MidiKind::ControlChange {
                            channel: context.channel(local),
                            controller,
                            value,
                        },
                        limits.max_midi_events,
                    )?;
                }
            }
            0xc0..=0xcf => {
                let program = take_data(bytes, &mut offset, "truncated SMAF program change")?;
                if context.wave_ids.is_empty() {
                    state.push_midi(
                        current,
                        MidiKind::ProgramChange {
                            channel: context.channel(local),
                            program,
                        },
                        limits.max_midi_events,
                    )?;
                }
            }
            0xd0..=0xdf => skip_data(bytes, &mut offset, 1)?,
            0xf0 => {
                let length = usize::try_from(read_mobile_vlq(bytes, &mut offset)?)
                    .map_err(|_| media_error("media-malformed", "SMAF sysex size overflow"))?;
                skip_bytes(bytes, &mut offset, length, "truncated SMAF sysex")?;
            }
            0xff => {
                let kind = take(bytes, &mut offset, "truncated SMAF meta event")?;
                if kind == 0 {
                    continue;
                }
                let length = usize::try_from(read_mobile_vlq(bytes, &mut offset)?)
                    .map_err(|_| media_error("media-malformed", "SMAF meta size overflow"))?;
                skip_bytes(bytes, &mut offset, length, "truncated SMAF meta event")?;
                if kind == 0x2f {
                    if length != 0 || offset != bytes.len() {
                        return Err(media_error("media-malformed", "invalid SMAF end event"));
                    }
                    return Ok(());
                }
            }
            _ => return Err(media_error("media-malformed", "invalid SMAF event status")),
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(super) fn parse_handy_phone(
    bytes: &[u8],
    context: &TrackContext<'_>,
    state: &mut DecodeState,
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EmuError> {
    let mut offset = 0;
    let mut current = 0_u64;
    let mut octave_shift = [0_i16; 4];
    while offset < bytes.len() {
        state.count_message(limits.max_midi_events, cancelled)?;
        let delta = read_handy_var(bytes, &mut offset)?;
        current = add_time(current, delta, context.duration_tick_micros, limits)?;
        state.end_micros = state.end_micros.max(current);
        let first = take(bytes, &mut offset, "truncated SMAF HandyPhone event")?;
        if first == 0xff {
            let kind = take(bytes, &mut offset, "truncated SMAF HandyPhone meta event")?;
            if kind == 0 {
                continue;
            }
            let length = usize::try_from(read_handy_var(bytes, &mut offset)?)
                .map_err(|_| media_error("media-malformed", "SMAF meta size overflow"))?;
            skip_bytes(
                bytes,
                &mut offset,
                length,
                "truncated SMAF HandyPhone meta event",
            )?;
            continue;
        }
        if first == 0 {
            let control = take(bytes, &mut offset, "truncated SMAF HandyPhone control")?;
            if control == 0 {
                let extended = take(bytes, &mut offset, "truncated SMAF HandyPhone end event")?;
                if extended == 0 {
                    if offset != bytes.len() {
                        return Err(media_error(
                            "media-malformed",
                            "data follows SMAF HandyPhone end event",
                        ));
                    }
                    return Ok(());
                }
                continue;
            }
            let local = (control >> 6) & 3;
            let class = (control >> 4) & 3;
            let data = control & 0x0f;
            if class == 3 {
                let value = take(
                    bytes,
                    &mut offset,
                    "truncated SMAF HandyPhone control value",
                )?;
                match data {
                    0 if context.wave_ids.is_empty() => state.push_midi(
                        current,
                        MidiKind::ProgramChange {
                            channel: context.channel(local),
                            program: value & 0x7f,
                        },
                        limits.max_midi_events,
                    )?,
                    2 => {
                        octave_shift[usize::from(local)] = match value {
                            1..=4 => i16::from(value) * 12,
                            0x81..=0x84 => -i16::from(value - 0x80) * 12,
                            _ => 0,
                        };
                    }
                    7 | 11 if context.wave_ids.is_empty() => state.push_midi(
                        current,
                        MidiKind::ControlChange {
                            channel: context.channel(local),
                            controller: data,
                            value: value & 0x7f,
                        },
                        limits.max_midi_events,
                    )?,
                    _ => {}
                }
            } else if class == 0 && context.wave_ids.is_empty() {
                let value = if data <= 1 {
                    0
                } else {
                    data.saturating_mul(8).saturating_add(15).min(127)
                };
                state.push_midi(
                    current,
                    MidiKind::ControlChange {
                        channel: context.channel(local),
                        controller: 11,
                        value,
                    },
                    limits.max_midi_events,
                )?;
            }
            continue;
        }

        let voice = first & 0x0f;
        if !(1..=0x0c).contains(&voice) {
            return Err(media_error(
                "media-malformed",
                "invalid SMAF HandyPhone note number",
            ));
        }
        let local = (first >> 6) & 3;
        let octave = (first >> 4) & 3;
        let note = i16::from(voice)
            .saturating_add(i16::from(octave) * 12)
            .saturating_add(36)
            .saturating_add(octave_shift[usize::from(local)])
            .clamp(0, 127);
        let gate = read_handy_var(bytes, &mut offset)?;
        if gate == 0 {
            return Err(media_error(
                "media-malformed",
                "zero SMAF HandyPhone gate time",
            ));
        }
        add_note(
            context,
            state,
            SmafNote {
                local,
                note: u8::try_from(note).unwrap_or_default(),
                velocity: 127,
                gate,
                current,
            },
            limits,
        )?;
    }
    Ok(())
}

fn add_note(
    context: &TrackContext<'_>,
    state: &mut DecodeState,
    note: SmafNote,
    limits: Limits,
) -> Result<(), EmuError> {
    let gate_micros = u64::from(note.gate)
        .checked_mul(context.gate_tick_micros)
        .ok_or_else(|| media_error("media-limit", "SMAF gate-time overflow"))?;
    let end = note
        .current
        .checked_add(gate_micros)
        .ok_or_else(|| media_error("media-limit", "SMAF note-time overflow"))?;
    check_time(end, limits)?;
    state.end_micros = state.end_micros.max(end);
    if note.gate == 0 || note.velocity == 0 {
        return Ok(());
    }
    state.sound_events = state.sound_events.saturating_add(1);
    if context.wave_ids.is_empty() {
        let channel = context.channel(note.local);
        state.push_midi(
            note.current,
            MidiKind::NoteOn {
                channel,
                note: note.note,
                velocity: note.velocity,
            },
            limits.max_midi_events,
        )?;
        state.push_midi(
            end,
            MidiKind::NoteOff {
                channel,
                note: note.note,
            },
            limits.max_midi_events,
        )?;
        return Ok(());
    }

    let preferred = note.note.checked_add(1);
    let wave_id = preferred
        .filter(|candidate| context.wave_ids.contains(candidate))
        .or_else(|| context.wave_ids.contains(&note.note).then_some(note.note))
        .or_else(|| (context.wave_ids.len() == 1).then_some(context.wave_ids[0]))
        .ok_or_else(|| {
            media_error(
                "media-unsupported-format",
                "SMAF note does not identify a stream-PCM wave",
            )
        })?;
    if state.triggers.len() >= limits.max_midi_events {
        return Err(media_error("media-limit", "SMAF event limit reached"));
    }
    state.triggers.push(WaveTrigger {
        key: (context.index, wave_id),
        start_micros: note.current,
        gate_micros,
        velocity: note.velocity,
    });
    Ok(())
}

fn read_mobile_vlq(bytes: &[u8], offset: &mut usize) -> Result<u32, EmuError> {
    let mut value = 0_u32;
    for index in 0..4 {
        let byte = take(bytes, offset, "truncated SMAF VLQ")?;
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            if index > 0 && value < (1 << (index * 7)) {
                return Err(media_error("media-malformed", "noncanonical SMAF VLQ"));
            }
            return Ok(value);
        }
    }
    Err(media_error(
        "media-malformed",
        "SMAF VLQ exceeds four bytes",
    ))
}

fn read_handy_var(bytes: &[u8], offset: &mut usize) -> Result<u32, EmuError> {
    let first = take(bytes, offset, "truncated SMAF HandyPhone duration")?;
    if first < 0x80 {
        return Ok(u32::from(first));
    }
    let second = take(bytes, offset, "truncated SMAF HandyPhone duration")?;
    Ok(((u32::from(first & 0x7f) + 1) << 7) | u32::from(second))
}

fn take(bytes: &[u8], offset: &mut usize, message: &'static str) -> Result<u8, EmuError> {
    let value = bytes
        .get(*offset)
        .copied()
        .ok_or_else(|| media_error("media-malformed", message))?;
    *offset = offset.saturating_add(1);
    Ok(value)
}

fn take_data(bytes: &[u8], offset: &mut usize, message: &'static str) -> Result<u8, EmuError> {
    let value = take(bytes, offset, message)?;
    if value > 127 {
        Err(media_error("media-malformed", "invalid SMAF data byte"))
    } else {
        Ok(value)
    }
}

fn skip_data(bytes: &[u8], offset: &mut usize, length: usize) -> Result<(), EmuError> {
    let start = *offset;
    skip_bytes(bytes, offset, length, "truncated SMAF event payload")?;
    if bytes[start..*offset].iter().any(|value| *value > 127) {
        return Err(media_error("media-malformed", "invalid SMAF data byte"));
    }
    Ok(())
}

fn skip_bytes(
    bytes: &[u8],
    offset: &mut usize,
    length: usize,
    message: &'static str,
) -> Result<(), EmuError> {
    *offset = offset
        .checked_add(length)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| media_error("media-malformed", message))?;
    Ok(())
}

fn add_time(current: u64, ticks: u32, scale: u64, limits: Limits) -> Result<u64, EmuError> {
    let next = u64::from(ticks)
        .checked_mul(scale)
        .and_then(|delta| current.checked_add(delta))
        .ok_or_else(|| media_error("media-limit", "SMAF timeline overflow"))?;
    check_time(next, limits)?;
    Ok(next)
}

fn check_time(micros: u64, limits: Limits) -> Result<(), EmuError> {
    if micros_to_frames(micros) > limits.max_pcm_frames {
        Err(media_error(
            "media-limit",
            "decoded SMAF exceeds duration limit",
        ))
    } else {
        Ok(())
    }
}
