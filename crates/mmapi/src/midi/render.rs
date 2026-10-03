//! Bounded MIDI event timelines and synthesis shared with SMAF.

use super::{MidiEvent, MidiKind};
use crate::{
    Clip, EmuError, HashMap, Limits, OUTPUT_SAMPLE_RATE, drum_duration_frames, media_error,
    mix_drum, mix_midi_note, soft_limit_i16,
};

/// Keep the mix at full precision until all synthesized and recorded voices
/// have been added. The output limiter runs once, when publishing the clip.
#[derive(Debug)]
pub(crate) struct PcmMix {
    pub(crate) samples: Vec<i32>,
    pub(crate) duration_micros: i64,
}

impl PcmMix {
    pub(crate) fn into_clip(self) -> Clip {
        Clip {
            samples: self.samples.into_iter().map(soft_limit_i16).collect(),
            duration_micros: self.duration_micros,
        }
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn render_midi_events(
    events: &[MidiEvent],
    division: u16,
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<PcmMix, EmuError> {
    let mut tempo = 500_000_u64;
    let mut previous_tick = 0_u64;
    let mut time_numerator = 0_u128;
    let mut time_micros = 0_u64;
    let mut programs = [0_u8; 16];
    let mut channel_volume = [100_u8; 16];
    let mut expression = [127_u8; 16];
    let mut active = HashMap::<(u8, u8), (u64, u8, u8)>::new();
    let mut spans = Vec::<(u64, u64, u8, u8, u8)>::new();
    let mut drums = Vec::<(u64, u8, u8)>::new();
    for (index, event) in events.iter().enumerate() {
        if index.is_multiple_of(1024) {
            crate::check_decode_cancellation(cancelled)?;
        }
        let delta = event.tick.saturating_sub(previous_tick);
        // Keep fractional microseconds across event and tempo boundaries.
        // Otherwise extra controller messages shorten the same musical phrase.
        time_numerator = time_numerator.saturating_add(u128::from(delta) * u128::from(tempo));
        time_micros = u64::try_from(time_numerator / u128::from(division)).unwrap_or(u64::MAX);
        previous_tick = event.tick;
        match event.kind {
            // The last track boundary includes silence in the playback loop.
            MidiKind::EndOfTrack => {}
            MidiKind::Tempo(value) => tempo = u64::from(value),
            MidiKind::ProgramChange { channel, program } => {
                programs[usize::from(channel)] = program;
            }
            MidiKind::ControlChange {
                channel,
                controller,
                value,
            } => match controller {
                7 => channel_volume[usize::from(channel)] = value,
                11 => expression[usize::from(channel)] = value,
                121 => {
                    channel_volume[usize::from(channel)] = 100;
                    expression[usize::from(channel)] = 127;
                }
                _ => {}
            },
            MidiKind::NoteOn {
                channel,
                note,
                velocity,
            } => {
                let scaled = u32::from(velocity)
                    .saturating_mul(u32::from(channel_volume[usize::from(channel)]))
                    .saturating_mul(u32::from(expression[usize::from(channel)]))
                    / 127
                    / 127;
                let effective_velocity = u8::try_from(scaled.min(127)).unwrap_or(127);
                if channel == 9 {
                    drums.push((time_micros, note, effective_velocity));
                } else {
                    let program = programs[usize::from(channel)];
                    if let Some((start, old_velocity, old_program)) =
                        active.insert((channel, note), (time_micros, effective_velocity, program))
                    {
                        spans.push((start, time_micros, note, old_velocity, old_program));
                    }
                }
            }
            MidiKind::NoteOff { channel, note } => {
                if channel != 9
                    && let Some((start, velocity, program)) = active.remove(&(channel, note))
                {
                    spans.push((start, time_micros, note, velocity, program));
                }
            }
        }
    }
    for ((_, note), (start, velocity, program)) in active {
        spans.push((start, time_micros, note, velocity, program));
    }
    let drum_tail_micros = drums
        .iter()
        .map(|(start, note, _)| {
            start.saturating_add(
                u64::try_from(drum_duration_frames(*note))
                    .unwrap_or(u64::MAX)
                    .saturating_mul(1_000_000)
                    / u64::from(OUTPUT_SAMPLE_RATE),
            )
        })
        .max()
        .unwrap_or(0);
    let total_micros = time_micros.max(drum_tail_micros);
    let frames =
        usize::try_from(total_micros.saturating_mul(u64::from(OUTPUT_SAMPLE_RATE)) / 1_000_000)
            .unwrap_or(usize::MAX);
    if frames > limits.max_pcm_frames {
        return Err(media_error(
            "media-limit",
            "decoded MIDI exceeds duration limit",
        ));
    }
    let mut voice_frames = 0_usize;
    for (start, end, ..) in &spans {
        let start_frame =
            usize::try_from(start.saturating_mul(u64::from(OUTPUT_SAMPLE_RATE)) / 1_000_000)
                .unwrap_or(usize::MAX)
                .min(frames);
        let end_frame =
            usize::try_from(end.saturating_mul(u64::from(OUTPUT_SAMPLE_RATE)) / 1_000_000)
                .unwrap_or(usize::MAX)
                .min(frames);
        voice_frames = voice_frames
            .checked_add(end_frame.saturating_sub(start_frame))
            .filter(|work| *work <= limits.max_midi_voice_frames)
            .ok_or_else(|| media_error("media-work-limit", "MIDI synthesis work limit reached"))?;
    }
    for (start, note, _) in &drums {
        let start_frame =
            usize::try_from(start.saturating_mul(u64::from(OUTPUT_SAMPLE_RATE)) / 1_000_000)
                .unwrap_or(usize::MAX)
                .min(frames);
        let drum_frames = drum_duration_frames(*note).min(frames.saturating_sub(start_frame));
        voice_frames = voice_frames
            .checked_add(drum_frames)
            .filter(|work| *work <= limits.max_midi_voice_frames)
            .ok_or_else(|| media_error("media-work-limit", "MIDI synthesis work limit reached"))?;
    }
    crate::check_decode_cancellation(cancelled)?;
    let mut mixed = vec![0_i32; frames];
    for (start, end, note, velocity, program) in spans {
        let start_frame =
            usize::try_from(start.saturating_mul(u64::from(OUTPUT_SAMPLE_RATE)) / 1_000_000)
                .unwrap_or(usize::MAX)
                .min(frames);
        let end_frame =
            usize::try_from(end.saturating_mul(u64::from(OUTPUT_SAMPLE_RATE)) / 1_000_000)
                .unwrap_or(usize::MAX)
                .min(frames);
        mix_midi_note(
            &mut mixed[start_frame..end_frame],
            note,
            velocity,
            program,
            cancelled,
        )?;
    }
    for (start, note, velocity) in drums {
        let start_frame =
            usize::try_from(start.saturating_mul(u64::from(OUTPUT_SAMPLE_RATE)) / 1_000_000)
                .unwrap_or(usize::MAX)
                .min(frames);
        let end = start_frame
            .saturating_add(drum_duration_frames(note))
            .min(frames);
        mix_drum(&mut mixed[start_frame..end], note, velocity, cancelled)?;
    }
    Ok(PcmMix {
        samples: mixed,
        duration_micros: i64::try_from(total_micros).unwrap_or(i64::MAX),
    })
}
