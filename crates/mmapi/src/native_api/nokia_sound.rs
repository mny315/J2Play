//! Nokia Sound adapter over the shared MMAPI runtime.

use super::{
    EmuError, NativeRegistry, NativeSignature, NativeValue, byte_array_argument, handle_argument,
    int_argument, java_handle, long_argument, media_error,
};
use crate::{OUTPUT_SAMPLE_RATE, note_phase_step};

pub(super) fn register_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    const SOUND: &str = "com/nokia/mid/sound/Sound";

    registry.register(
        NativeSignature::new(SOUND, "createBytes0", "([BI)J"),
        |context, arguments| {
            let bytes = byte_array_argument(context, arguments, 0)?;
            let format = int_argument(arguments, 1)?;
            let content_type = nokia_sound_content_type(format, &bytes)?;
            context
                .mmapi_create_bytes(content_type, &bytes)
                .and_then(java_handle)
        },
    )?;
    registry.register(
        NativeSignature::new(SOUND, "createTone0", "(IJ)J"),
        |context, arguments| {
            let frequency = int_argument(arguments, 0)?;
            let duration = long_argument(arguments, 1)?;
            let sequence = nokia_frequency_tone(frequency, duration)?;
            context
                .mmapi_create_bytes("audio/x-tone-seq", &sequence)
                .and_then(java_handle)
        },
    )?;
    registry.register(
        NativeSignature::new(SOUND, "close0", "(J)V"),
        |context, arguments| {
            let raw = long_argument(arguments, 0)?;
            if raw == 0 {
                return Ok(None);
            }
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context.mmapi_transition(handle_argument(arguments, 0)?, 5, now_micros)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(SOUND, "play0", "(JII)V"),
        |context, arguments| {
            let loop_count = nokia_sound_loop_count(int_argument(arguments, 1)?)?;
            let volume = nokia_sound_gain(int_argument(arguments, 2)?);
            if long_argument(arguments, 0)? == 0 {
                return Ok(None);
            }
            let handle = handle_argument(arguments, 0)?;
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context.mmapi_transition(handle, 1, now_micros)?;
            // Sound.play restarts an active sound. MMAPI only allows changing
            // its loop count after stopping it; prefetch already realizes it.
            context.mmapi_transition(handle, 3, now_micros)?;
            context.mmapi_set_media_time(handle, 0, now_micros)?;
            context.mmapi_set_loop_count(handle, loop_count)?;
            context.mmapi_set_volume(handle, volume)?;
            context.mmapi_transition(handle, 2, now_micros)?;
            Ok(None)
        },
    )?;
    for (name, transition) in [("resume0", 2), ("stop0", 3)] {
        registry.register(
            NativeSignature::new(SOUND, name, "(J)V"),
            move |context, arguments| {
                if long_argument(arguments, 0)? == 0 {
                    return Ok(None);
                }
                let handle = handle_argument(arguments, 0)?;
                let now_micros = context.monotonic_millis().saturating_mul(1_000);
                if transition == 2 {
                    resume_sound(context, handle, now_micros)?;
                } else {
                    context.mmapi_transition(handle, transition, now_micros)?;
                }
                Ok(None)
            },
        )?;
    }
    registry.register(
        NativeSignature::new(SOUND, "state0", "(J)I"),
        |context, arguments| {
            let raw = long_argument(arguments, 0)?;
            if raw == 0 {
                return Ok(Some(NativeValue::Int(3)));
            }
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            let state = context.mmapi_state(handle_argument(arguments, 0)?, now_micros)?;
            Ok(Some(NativeValue::Int(match state {
                400 => 0,
                0 => 3,
                _ => 1,
            })))
        },
    )?;
    registry.register(
        NativeSignature::new(SOUND, "setGain0", "(JI)I"),
        |context, arguments| {
            let gain = int_argument(arguments, 1)?.clamp(0, 255);
            if long_argument(arguments, 0)? != 0 {
                context.mmapi_set_volume(handle_argument(arguments, 0)?, nokia_sound_gain(gain))?;
            }
            Ok(Some(NativeValue::Int(gain)))
        },
    )
}

fn resume_sound(
    context: &mut dyn natives::NativeContext,
    handle: u64,
    now_micros: i64,
) -> Result<(), EmuError> {
    // A newly initialized sound has never played. Resuming it or a sound that
    // is already playing has no effect. Stopped tones start from the beginning.
    if matches!(context.mmapi_state(handle, now_micros)?, 100 | 400) {
        return Ok(());
    }
    if context.mmapi_content_type(handle)? == "audio/x-tone-seq" {
        context.mmapi_set_media_time(handle, 0, now_micros)?;
    }
    context.mmapi_transition(handle, 2, now_micros)
}

fn nokia_sound_content_type(format: i32, bytes: &[u8]) -> Result<&'static str, EmuError> {
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        return Ok("audio/x-wav");
    }
    if bytes.starts_with(b"MThd") {
        return Ok("audio/midi");
    }
    if bytes.get(..2) == Some(&[(-2_i8).cast_unsigned(), 1]) {
        return Ok("audio/x-tone-seq");
    }
    match format {
        1 => Ok("audio/x-tone-seq"),
        5 => Ok("audio/x-wav"),
        _ => Err(media_error(
            "media-unsupported-format",
            format!("unknown Nokia Sound format: {format}"),
        )),
    }
}

fn nokia_sound_loop_count(value: i32) -> Result<i32, EmuError> {
    match value {
        0 => Ok(-1),
        1.. => Ok(value),
        _ => Err(media_error(
            "illegal-argument",
            "Nokia Sound loop count must be nonnegative",
        )),
    }
}

fn nokia_sound_gain(value: i32) -> i32 {
    match value.clamp(0, 255) {
        0 => 0,
        // Every nonzero Nokia gain must remain audible at the MMAPI scale.
        value => ((value * 100 + 127) / 255).max(1),
    }
}

fn nokia_frequency_tone(frequency: i32, duration_millis: i64) -> Result<Vec<u8>, EmuError> {
    const MAX_DURATION_MILLIS: i64 = 10 * 60 * 1_000;
    if frequency < 0 || duration_millis <= 0 || duration_millis > MAX_DURATION_MILLIS {
        return Err(media_error(
            "illegal-argument",
            "Nokia Sound frequency must be nonnegative and bounded duration must be positive",
        ));
    }
    let frequency = u64::try_from(frequency)
        .map_err(|_| media_error("illegal-argument", "invalid Nokia Sound frequency"))?;
    let target_step = frequency.saturating_mul(1_u64 << 32) / u64::from(OUTPUT_SAMPLE_RATE);
    let note = if frequency == 0 {
        // Nokia's frequency constructor accepts zero as a silent tone, a
        // pattern used to create a reusable Sound object before real content
        // is selected.
        u8::MAX
    } else {
        (0_u8..=127)
            .min_by_key(|candidate| u64::from(note_phase_step(*candidate)).abs_diff(target_step))
            .expect("MIDI note range is nonempty")
    };

    // TEMPO=120 and RESOLUTION=100 make one duration unit exactly 5 ms.
    let total_units = u64::try_from(duration_millis)
        .unwrap_or(u64::MAX)
        .saturating_add(4)
        / 5;
    let events = total_units.saturating_add(126) / 127;
    if events > 10_000 {
        return Err(media_error(
            "media-limit",
            "Nokia Sound tone duration exceeds event limit",
        ));
    }
    let mut sequence = vec![(-2_i8).cast_unsigned(), 1, (-3_i8).cast_unsigned(), 120];
    sequence.extend([(-4_i8).cast_unsigned(), 100]);
    let mut remaining = total_units;
    while remaining > 0 {
        let units = remaining.min(127);
        sequence.extend([
            note,
            u8::try_from(units).expect("tone duration chunk fits u8"),
        ]);
        remaining -= units;
    }
    Ok(sequence)
}

#[cfg(test)]
#[path = "../../../../tests/unit/mmapi/native_api/nokia_sound.rs"]
mod tests;
