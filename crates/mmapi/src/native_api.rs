use super::{EmuError, NativeRegistry, NativeSignature, NativeValue, media_error};

mod nokia_sound;
mod samsung_audio_clip;

/// Registers the Java MMAPI implementation's exact native surface.
///
/// # Errors
/// Returns `duplicate-native` if a signature is already registered.
pub fn register_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    const PLAYER: &str = "javax/microedition/media/PlayerImpl";
    const MANAGER: &str = "javax/microedition/media/Manager";
    const SIEMENS_PLAYER: &str = "com/siemens/mp/media/PlayerImpl";
    const SIEMENS_MANAGER: &str = "com/siemens/mp/media/Manager";

    register_player_creation_natives(registry, PLAYER, false)?;
    register_player_natives(registry, PLAYER)?;
    register_manager_natives(registry, MANAGER)?;
    register_player_creation_natives(registry, SIEMENS_PLAYER, true)?;
    register_player_natives(registry, SIEMENS_PLAYER)?;
    register_manager_natives(registry, SIEMENS_MANAGER)?;
    samsung_audio_clip::register_natives(registry)?;
    nokia_sound::register_natives(registry)
}

fn register_player_creation_natives(
    registry: &mut NativeRegistry,
    class: &str,
    accept_absolute_resource_path: bool,
) -> Result<(), EmuError> {
    registry.register(
        NativeSignature::new(class, "createBytes0", "(Ljava/lang/String;[B)J"),
        |context, arguments| {
            let content_type = string_argument(context, arguments, 0)?;
            let bytes = byte_array_argument(context, arguments, 1)?;
            context
                .mmapi_create_bytes(&content_type, &bytes)
                .and_then(java_handle)
        },
    )?;
    registry.register(
        NativeSignature::new(class, "createLocator0", "(Ljava/lang/String;)J"),
        move |context, arguments| {
            let locator = string_argument(context, arguments, 0)?;
            if locator == "device://tone" {
                context.mmapi_create_tone().and_then(java_handle)
            } else if locator == "device://midi" {
                context.mmapi_create_midi().and_then(java_handle)
            } else if let Some(path) =
                resource_path_from_locator(&locator, accept_absolute_resource_path)
            {
                let bytes = context.read_resource(&path)?.ok_or_else(|| {
                    media_error("media-io", format!("media resource not found: {path}"))
                })?;
                let content_type = content_type_from_path(&path)?;
                context
                    .mmapi_create_bytes(content_type, &bytes)
                    .and_then(java_handle)
            } else {
                Err(media_error(
                    "media-unsupported-locator",
                    format!("unsupported MMAPI locator: {locator}"),
                ))
            }
        },
    )
}

fn register_manager_natives(registry: &mut NativeRegistry, class: &str) -> Result<(), EmuError> {
    registry.register(
        NativeSignature::new(class, "playTone0", "(III)V"),
        |context, arguments| {
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context.mmapi_play_tone(
                int_argument(arguments, 0)?,
                int_argument(arguments, 1)?,
                int_argument(arguments, 2)?,
                now_micros,
            )?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(class, "pumpTone0", "()V"),
        |context, _arguments| {
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context.mmapi_pump(now_micros)?;
            Ok(None)
        },
    )
}

fn resource_path_from_locator(
    locator: &str,
    accept_absolute_resource_path: bool,
) -> Option<String> {
    let path = locator
        .strip_prefix("resource://")
        .or_else(|| locator.strip_prefix("resource:"))
        .or_else(|| {
            (accept_absolute_resource_path && locator.starts_with('/')).then_some(locator)
        })?;
    Some(if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    })
}

#[allow(clippy::too_many_lines)]
fn register_player_natives(registry: &mut NativeRegistry, class: &str) -> Result<(), EmuError> {
    registry.register(
        NativeSignature::new(class, "state0", "(J)I"),
        |context, arguments| {
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context
                .mmapi_state(handle_argument(arguments, 0)?, now_micros)
                .map(|value| Some(NativeValue::Int(value)))
        },
    )?;
    for (name, method) in [
        ("realize0", 0),
        ("prefetch0", 1),
        ("start0", 2),
        ("stop0", 3),
        ("deallocate0", 4),
        ("close0", 5),
    ] {
        registry.register(
            NativeSignature::new(class, name, "(J)V"),
            move |context, arguments| {
                let now_micros = context.monotonic_millis().saturating_mul(1_000);
                context.mmapi_transition(handle_argument(arguments, 0)?, method, now_micros)?;
                Ok(None)
            },
        )?;
    }
    registry.register(
        NativeSignature::new(class, "contentType0", "(J)Ljava/lang/String;"),
        |context, arguments| {
            let value = context.mmapi_content_type(handle_argument(arguments, 0)?)?;
            context
                .intern_java_string(&value)
                .map(|reference| Some(NativeValue::Reference(Some(reference))))
        },
    )?;
    for (name, selector) in [("duration0", 0), ("mediaTime0", 1), ("eventData0", 2)] {
        registry.register(
            NativeSignature::new(class, name, "(J)J"),
            move |context, arguments| {
                let now_micros = context.monotonic_millis().saturating_mul(1_000);
                context
                    .mmapi_time(handle_argument(arguments, 0)?, selector, now_micros)
                    .map(|value| Some(NativeValue::Long(value)))
            },
        )?;
    }
    registry.register(
        NativeSignature::new(class, "setMediaTime0", "(JJ)J"),
        |context, arguments| {
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context
                .mmapi_set_media_time(
                    handle_argument(arguments, 0)?,
                    long_argument(arguments, 1)?,
                    now_micros,
                )
                .map(|value| Some(NativeValue::Long(value)))
        },
    )?;
    registry.register(
        NativeSignature::new(class, "setLoopCount0", "(JI)V"),
        |context, arguments| {
            context.mmapi_set_loop_count(
                handle_argument(arguments, 0)?,
                int_argument(arguments, 1)?,
            )?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(class, "volume0", "(J)I"),
        |context, arguments| {
            context
                .mmapi_volume(handle_argument(arguments, 0)?)
                .map(|value| Some(NativeValue::Int(value)))
        },
    )?;
    registry.register(
        NativeSignature::new(class, "setVolume0", "(JI)I"),
        |context, arguments| {
            context
                .mmapi_set_volume(handle_argument(arguments, 0)?, int_argument(arguments, 1)?)
                .map(|value| Some(NativeValue::Int(value)))
        },
    )?;
    registry.register(
        NativeSignature::new(class, "muted0", "(J)Z"),
        |context, arguments| {
            context
                .mmapi_muted(handle_argument(arguments, 0)?)
                .map(|value| Some(NativeValue::Int(i32::from(value))))
        },
    )?;
    registry.register(
        NativeSignature::new(class, "setMuted0", "(JZ)V"),
        |context, arguments| {
            context.mmapi_set_muted(
                handle_argument(arguments, 0)?,
                int_argument(arguments, 1)? != 0,
            )?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(class, "setToneSequence0", "(J[B)V"),
        |context, arguments| {
            let sequence = byte_array_argument(context, arguments, 1)?;
            context.mmapi_set_tone_sequence(handle_argument(arguments, 0)?, &sequence)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(class, "nextEvent0", "(J)I"),
        |context, arguments| {
            let now_micros = context.monotonic_millis().saturating_mul(1_000);
            context
                .mmapi_next_event(handle_argument(arguments, 0)?, now_micros)
                .map(|value| Some(NativeValue::Int(value)))
        },
    )
}

fn matches_ignore_ascii_case(value: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
}

pub(super) fn content_type_from_path(path: &str) -> Result<&'static str, EmuError> {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("wav") {
        Ok("audio/x-wav")
    } else if extension.eq_ignore_ascii_case("mid") || extension.eq_ignore_ascii_case("midi") {
        Ok("audio/midi")
    } else if extension.eq_ignore_ascii_case("jts") {
        Ok("audio/x-tone-seq")
    } else if matches_ignore_ascii_case(extension, &["mmf", "smaf"]) {
        Ok("audio/mmf")
    } else if matches_ignore_ascii_case(extension, &["mp3", "mp2", "mp1"]) {
        Ok("audio/mpeg")
    } else if matches_ignore_ascii_case(extension, &["m4a", "mp4", "aac"]) {
        Ok("audio/mp4a-latm")
    } else if extension.eq_ignore_ascii_case("amr") {
        Ok("audio/amr")
    } else {
        Err(media_error(
            "media-unsupported-format",
            "resource extension has no supported MMAPI decoder",
        ))
    }
}

fn string_argument(
    context: &dyn natives::NativeContext,
    arguments: &[NativeValue],
    index: usize,
) -> Result<String, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Reference(Some(reference))) => context.read_java_string(*reference),
        Some(NativeValue::Reference(None)) => Err(media_error(
            "null-pointer-exception",
            "MMAPI string is null",
        )),
        _ => Err(media_error(
            "native-arguments",
            "MMAPI native expected String",
        )),
    }
}

fn byte_array_argument(
    context: &dyn natives::NativeContext,
    arguments: &[NativeValue],
    index: usize,
) -> Result<Vec<u8>, EmuError> {
    context.read_java_byte_array(byte_array_reference(arguments, index)?)
}

fn byte_array_reference(arguments: &[NativeValue], index: usize) -> Result<u64, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Reference(Some(reference))) => Ok(*reference),
        Some(NativeValue::Reference(None)) => Err(media_error(
            "null-pointer-exception",
            "MMAPI byte array is null",
        )),
        _ => Err(media_error(
            "native-arguments",
            "MMAPI native expected byte[]",
        )),
    }
}

fn int_argument(arguments: &[NativeValue], index: usize) -> Result<i32, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Int(value)) => Ok(*value),
        _ => Err(media_error("native-arguments", "MMAPI native expected int")),
    }
}

fn long_argument(arguments: &[NativeValue], index: usize) -> Result<i64, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Long(value)) => Ok(*value),
        _ => Err(media_error(
            "native-arguments",
            "MMAPI native expected long",
        )),
    }
}

fn handle_argument(arguments: &[NativeValue], index: usize) -> Result<u64, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Long(value)) if *value > 0 => {
            u64::try_from(*value).map_err(|_| media_error("player-closed", "invalid MMAPI handle"))
        }
        _ => Err(media_error("player-closed", "invalid MMAPI handle")),
    }
}

pub(super) fn java_handle(handle: u64) -> Result<Option<NativeValue>, EmuError> {
    i64::try_from(handle)
        .map(|value| Some(NativeValue::Long(value)))
        .map_err(|_| media_error("player-limit", "MMAPI handle exceeds Java long"))
}
