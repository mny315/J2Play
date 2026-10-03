use super::{EmuError, OUTPUT_SAMPLE_RATE, check_decode_cancellation, clamp_i16};

pub(super) fn append_synth_note(
    samples: &mut Vec<i16>,
    note: u8,
    frames: usize,
    volume: i32,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EmuError> {
    let step = note_phase_step(note);
    let amplitude = 18_000_i32.saturating_mul(volume.clamp(0, 100)) / 100;
    let mut phase = 0_u32;
    samples.reserve(frames);
    for frame in 0..frames {
        if frame.is_multiple_of(4096) {
            check_decode_cancellation(cancelled)?;
        }
        let wave = triangle_wave(phase);
        let attack = (OUTPUT_SAMPLE_RATE as usize / 500).max(1); // about 2 ms
        let release = (OUTPUT_SAMPLE_RATE as usize / 100).max(1); // about 10 ms
        let mut envelope = 32_767_i32;
        if frame < attack {
            envelope = i32::try_from(frame.saturating_mul(32_767) / attack).unwrap_or(32_767);
        }
        let remaining = frames.saturating_sub(frame + 1);
        if remaining < release {
            envelope = envelope
                .min(i32::try_from(remaining.saturating_mul(32_767) / release).unwrap_or(32_767));
        }
        samples.push(scale_synth_sample(wave, amplitude, envelope));
        phase = phase.wrapping_add(step);
    }
    Ok(())
}

pub(super) fn note_phase_step(note: u8) -> u32 {
    // 440 Hz at 22.05 kHz in Q0.32 phase space.
    const A4_STEP: u64 = 85_704_563;
    const SEMITONE_Q32: u64 = 4_550_358_284;
    let mut step = A4_STEP;
    if note >= 69 {
        for _ in 69..note {
            step = step.saturating_mul(SEMITONE_Q32).saturating_add(1 << 31) >> 32;
        }
    } else {
        for _ in note..69 {
            step = ((step << 32).saturating_add(SEMITONE_Q32 / 2)) / SEMITONE_Q32;
        }
    }
    u32::try_from(step).unwrap_or(u32::MAX)
}

pub(super) fn triangle_wave(phase: u32) -> i32 {
    let position = i32::try_from(phase >> 16).unwrap_or(0);
    if position < 32_768 {
        -32_768 + position.saturating_mul(2)
    } else {
        32_767 - (position - 32_768).saturating_mul(2)
    }
}

pub(super) fn saw_wave(phase: u32) -> i32 {
    i32::try_from(phase >> 16).unwrap_or(0) - 32_768
}

pub(super) fn square_wave(phase: u32) -> i32 {
    if phase & 0x8000_0000 == 0 {
        32_767
    } else {
        -32_768
    }
}

pub(super) fn midi_waveform(phase: u32, program: u8) -> i32 {
    let family = program / 8;
    let triangle = triangle_wave(phase);
    let saw = saw_wave(phase);
    let square = square_wave(phase);
    match family {
        0 | 3 | 5 | 6 => (triangle.saturating_mul(3) + saw) / 4,
        1 | 2 => (triangle.saturating_mul(3) + square) / 4,
        7 => (saw.saturating_mul(2) + square) / 3,
        8 => i32::midpoint(triangle, square),
        10 => i32::midpoint(saw, square),
        12 | 14 => i32::midpoint(triangle, saw),
        13 => (triangle.saturating_mul(2) + square) / 3,
        _ => triangle,
    }
}

struct MidiEnvelope {
    attack: usize,
    release: usize,
    decays: bool,
}

impl MidiEnvelope {
    fn new(program: u8) -> Self {
        let family = program / 8;
        let (attack_ms, release_ms) = match family {
            5 | 6 | 11 => (25, 45),
            7..=9 => (8, 18),
            _ => (3, 18),
        };
        let attack = (OUTPUT_SAMPLE_RATE as usize * attack_ms / 1_000).max(1);
        let release = (OUTPUT_SAMPLE_RATE as usize * release_ms / 1_000).max(1);
        Self {
            attack,
            release,
            decays: matches!(family, 0 | 1 | 3 | 4 | 14),
        }
    }

    fn at(&self, frame: usize, frames: usize) -> i32 {
        let attack = self.attack;
        let release = self.release;
        let mut envelope = if frame < attack {
            i32::try_from(frame.saturating_mul(32_767) / attack).unwrap_or(32_767)
        } else {
            32_767
        };
        let remaining = frames.saturating_sub(frame + 1);
        if remaining < release {
            envelope = envelope
                .min(i32::try_from(remaining.saturating_mul(32_767) / release).unwrap_or(32_767));
        }
        if self.decays && frames > attack {
            let decay_position = frame.saturating_sub(attack);
            let decay_span = (OUTPUT_SAMPLE_RATE as usize / 2).max(1);
            let sustain = 18_000_i32;
            let decay = if decay_position >= decay_span {
                sustain
            } else {
                32_767
                    - i32::try_from(
                        decay_position
                            .saturating_mul(usize::try_from(32_767 - sustain).unwrap_or(0))
                            / decay_span,
                    )
                    .unwrap_or(0)
            };
            envelope = envelope.saturating_mul(decay) / 32_767;
        }
        envelope
    }
}

pub(super) fn mix_midi_note(
    target: &mut [i32],
    note: u8,
    velocity: u8,
    program: u8,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EmuError> {
    let step = note_phase_step(note);
    // Keep enough headroom for normal Java ME polyphony. The old 24k-per-note
    // square wave clipped almost every chord into a harsh buzz.
    let amplitude = 6_500_i32.saturating_mul(i32::from(velocity)) / 127;
    let mut phase = 0_u32;
    let frames = target.len();
    let envelope = MidiEnvelope::new(program);
    // Keep oscillator phase and the full-note envelope continuous across polls.
    for (block, chunk) in target.chunks_mut(4_096).enumerate() {
        check_decode_cancellation(cancelled)?;
        for (offset, mixed) in chunk.iter_mut().enumerate() {
            let frame = block * 4_096 + offset;
            let wave = midi_waveform(phase, program);
            let envelope = envelope.at(frame, frames);
            *mixed = mixed.saturating_add(i32::from(scale_synth_sample(wave, amplitude, envelope)));
            phase = phase.wrapping_add(step);
        }
    }
    Ok(())
}

pub(super) fn drum_duration_frames(note: u8) -> usize {
    let millis = match note {
        35 | 36 => 180,
        38 | 40 => 140,
        42 | 44 => 55,
        46 => 160,
        49 | 51 | 52 | 55 | 57 | 59 => 420,
        _ => 110,
    };
    (OUTPUT_SAMPLE_RATE as usize).saturating_mul(millis) / 1_000
}

pub(super) fn mix_drum(
    target: &mut [i32],
    note: u8,
    velocity: u8,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EmuError> {
    let frames = target.len();
    if frames == 0 {
        return Ok(());
    }
    let amplitude = 10_000_i32.saturating_mul(i32::from(velocity)) / 127;
    let mut phase = 0_u32;
    let pitch_note = if matches!(note, 35 | 36) { 35 } else { 50 };
    let step = note_phase_step(pitch_note);
    let mut noise = 0x9e37_79b9_u32 ^ u32::from(note).saturating_mul(0x045d_9f3b);
    // Keep oscillator phase and the full-note envelope continuous across polls.
    for (block, chunk) in target.chunks_mut(4_096).enumerate() {
        check_decode_cancellation(cancelled)?;
        for (offset, mixed) in chunk.iter_mut().enumerate() {
            let frame = block * 4_096 + offset;
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            let noise_sample = i32::from((noise >> 16) as u16) - 32_768;
            let tonal = triangle_wave(phase);
            let wave = match note {
                35 | 36 => tonal.saturating_mul(3) / 4 + noise_sample / 8,
                38 | 40 => noise_sample.saturating_mul(3) / 4 + tonal / 4,
                42 | 44 | 46 => noise_sample,
                49 | 51 | 52 | 55 | 57 | 59 => {
                    let metallic = square_wave(phase) / 3 + square_wave(phase.wrapping_mul(3)) / 3;
                    noise_sample / 2 + metallic
                }
                _ => noise_sample.saturating_mul(2) / 3 + tonal / 3,
            };
            let remaining = frames - frame;
            let envelope = i32::try_from(remaining.saturating_mul(32_767) / frames).unwrap_or(0);
            *mixed = mixed.saturating_add(i32::from(scale_synth_sample(wave, amplitude, envelope)));
            phase = phase.wrapping_add(step);
        }
    }
    Ok(())
}

pub(super) fn scale_synth_sample(wave: i32, amplitude: i32, envelope: i32) -> i16 {
    // Keep the intermediate product wider than i32. Multiplying a Q15 waveform,
    // amplitude and Q15 envelope in i32 saturates before the divisions and turns
    // normal notes into ~1-2 LSB samples (effectively silence).
    let scaled = i64::from(wave)
        .saturating_mul(i64::from(amplitude))
        .saturating_mul(i64::from(envelope))
        / 32_768
        / 32_767;
    i16::try_from(scaled.clamp(i64::from(i16::MIN), i64::from(i16::MAX)))
        .expect("scaled sample is clamped to i16")
}

pub(super) fn soft_limit_i16(sample: i32) -> i16 {
    let sign = if sample < 0 { -1 } else { 1 };
    let magnitude = sample.saturating_abs();
    let compressed = if magnitude <= 28_000 {
        magnitude
    } else {
        28_000 + (magnitude - 28_000) / 4
    };
    clamp_i16(sign * compressed.min(32_767))
}
