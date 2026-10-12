//! Samsung `AudioClip` adapter over the shared MMAPI runtime.

use super::{
    EmuError, NativeRegistry, NativeSignature, byte_array_reference, content_type_from_path,
    handle_argument, int_argument, java_handle, media_error, string_argument,
};

pub(super) fn register_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    const AUDIO_CLIP: &str = "com/samsung/util/AudioClip";

    registry.register(
        NativeSignature::new(AUDIO_CLIP, "createDefault0", "()J"),
        |context, _arguments| {
            let handle = context.mmapi_create_midi()?;
            prepare_samsung_player(context, handle)?;
            java_handle(handle)
        },
    )?;
    registry.register(
        NativeSignature::new(AUDIO_CLIP, "createBytes0", "(I[BII)J"),
        |context, arguments| {
            let media_type = int_argument(arguments, 0)?;
            let reference = byte_array_reference(arguments, 1)?;
            let data = context
                .read_java_byte_array_range(
                    reference,
                    int_argument(arguments, 2)?,
                    int_argument(arguments, 3)?,
                )?
                .ok_or_else(|| {
                    media_error(
                        "array-index-out-of-bounds-exception",
                        "AudioClip byte range is outside the array",
                    )
                })?;
            let content_type = samsung_content_type(media_type, &data, None)?;
            let handle = context.mmapi_create_bytes(content_type, &data)?;
            prepare_samsung_player(context, handle)?;
            java_handle(handle)
        },
    )?;
    registry.register(
        NativeSignature::new(AUDIO_CLIP, "createResource0", "(ILjava/lang/String;)J"),
        |context, arguments| {
            let media_type = int_argument(arguments, 0)?;
            let resource = string_argument(context, arguments, 1)?;
            let path = if resource.starts_with('/') {
                resource
            } else {
                format!("/{resource}")
            };
            let bytes = context.read_resource(&path)?.ok_or_else(|| {
                media_error("media-io", format!("AudioClip resource not found: {path}"))
            })?;
            let content_type = samsung_content_type(media_type, &bytes, Some(&path))?;
            let handle = context.mmapi_create_bytes(content_type, &bytes)?;
            prepare_samsung_player(context, handle)?;
            java_handle(handle)
        },
    )?;
    registry.register(
        NativeSignature::new(AUDIO_CLIP, "pause0", "(J)V"),
        |context, arguments| {
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context.mmapi_transition(handle_argument(arguments, 0)?, 3, now_micros)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(AUDIO_CLIP, "play0", "(JII)V"),
        |context, arguments| {
            let handle = handle_argument(arguments, 0)?;
            let loop_count = samsung_loop_count(int_argument(arguments, 1)?)?;
            let volume = samsung_volume(int_argument(arguments, 2)?)?;
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context.mmapi_transition(handle, 3, now_micros)?;
            context.mmapi_set_media_time(handle, 0, now_micros)?;
            context.mmapi_set_loop_count(handle, loop_count)?;
            context.mmapi_set_volume(handle, volume)?;
            context.mmapi_start_exclusive(handle, now_micros)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(AUDIO_CLIP, "resume0", "(J)V"),
        |context, arguments| {
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context.mmapi_start_exclusive(handle_argument(arguments, 0)?, now_micros)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(AUDIO_CLIP, "stop0", "(J)V"),
        |context, arguments| {
            let handle = handle_argument(arguments, 0)?;
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context.mmapi_transition(handle, 3, now_micros)?;
            context.mmapi_set_media_time(handle, 0, now_micros)?;
            Ok(None)
        },
    )
}

fn prepare_samsung_player(
    context: &mut dyn natives::NativeContext,
    handle: u64,
) -> Result<(), EmuError> {
    let now_micros = context.monotonic_millis().saturating_mul(1_000);
    for transition in [0, 1] {
        if let Err(error) = context.mmapi_transition(handle, transition, now_micros) {
            let _ = context.mmapi_transition(handle, 5, now_micros);
            return Err(error);
        }
    }
    Ok(())
}

fn samsung_content_type(
    media_type: i32,
    bytes: &[u8],
    path: Option<&str>,
) -> Result<&'static str, EmuError> {
    if !(1..=3).contains(&media_type) {
        return Err(media_error(
            "illegal-argument",
            format!("unknown Samsung AudioClip type: {media_type}"),
        ));
    }
    if bytes.starts_with(b"MMMD") {
        return Ok("audio/mmf");
    }
    if bytes.starts_with(b"MThd") {
        return Ok("audio/midi");
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        return Ok("audio/x-wav");
    }
    if bytes.starts_with(b"ID3")
        || bytes
            .get(..2)
            .is_some_and(|header| header[0] == 0xff && header[1] & 0xe0 == 0xe0)
    {
        return Ok("audio/mpeg");
    }
    if let Some(content_type) = path.and_then(|path| content_type_from_path(path).ok()) {
        return Ok(content_type);
    }
    match media_type {
        1 => Ok("audio/mmf"),
        2 => Ok("audio/mpeg"),
        3 => Ok("audio/midi"),
        _ => Err(media_error(
            "media-unsupported-format",
            format!("unknown Samsung AudioClip type: {media_type}"),
        )),
    }
}

fn samsung_loop_count(value: i32) -> Result<i32, EmuError> {
    match value {
        0 => Ok(1),
        1..=255 => Ok(value),
        _ => Err(media_error(
            "illegal-argument",
            "Samsung AudioClip loop count must be between 0 and 255",
        )),
    }
}

fn samsung_volume(value: i32) -> Result<i32, EmuError> {
    if (0..=5).contains(&value) {
        Ok(value * 20)
    } else {
        Err(media_error(
            "illegal-argument",
            "Samsung AudioClip volume must be between 0 and 5",
        ))
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/mmapi/native_api/samsung_audio_clip.rs"]
mod tests;
