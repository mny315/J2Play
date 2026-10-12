//! `MIDlet` and LCDUI native bindings to host services and VM-owned values.

use diagnostics::{Category, EmuError};
use natives::{
    LifecycleCallback, LifecycleOutcome, MidletLifecycleEvent, MidletNotification, NativeRegistry,
    NativeSignature, NativeValue,
};

/// Registers host-backed `MIDlet` and LCDUI native bindings.
/// Heap-owned graphics and frame presentation are dispatched by the VM.
///
/// # Errors
/// Returns an error if another library already registered one of the exact
/// signatures.
#[allow(clippy::too_many_lines)]
pub fn register_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    registry.register(
        NativeSignature::new(
            "javax/microedition/lcdui/Display",
            "__setTextInputActive",
            "(Z)V",
        ),
        |context, arguments| {
            let [NativeValue::Int(active)] = arguments else {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "Display text-input state requires one boolean",
                ));
            };
            context.set_text_input_active(*active != 0);
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new("javax/microedition/lcdui/Display", "numColors", "()I"),
        |context, arguments| {
            if arguments.len() != 1 {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "Display.numColors requires a receiver",
                ));
            }
            Ok(Some(NativeValue::Int(context.display_colors())))
        },
    )?;
    registry.register(
        NativeSignature::new(
            "javax/microedition/midlet/MIDlet",
            "lifecycleCallback",
            "(II)V",
        ),
        |context, arguments| {
            let [
                NativeValue::Reference(Some(_)),
                NativeValue::Int(callback),
                NativeValue::Int(outcome),
            ] = arguments
            else {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "MIDlet.lifecycleCallback requires receiver, callback and outcome",
                ));
            };
            let callback = match callback {
                1 => LifecycleCallback::Start,
                2 => LifecycleCallback::Pause,
                3 => LifecycleCallback::Destroy,
                _ => {
                    return Err(EmuError::new(
                        Category::Api,
                        "lifecycle-callback",
                        "unknown MIDlet lifecycle callback",
                    ));
                }
            };
            let outcome = match outcome {
                0 => LifecycleOutcome::Completed,
                1 => LifecycleOutcome::StateChangeRejected,
                2 => LifecycleOutcome::RuntimeFailure,
                _ => {
                    return Err(EmuError::new(
                        Category::Api,
                        "lifecycle-callback",
                        "unknown MIDlet lifecycle outcome",
                    ));
                }
            };
            context.midlet_lifecycle_event(MidletLifecycleEvent::Callback { callback, outcome })?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(
            "javax/microedition/lcdui/Image",
            "readResource",
            "(Ljava/lang/String;)[B",
        ),
        |context, arguments| {
            let [NativeValue::Reference(Some(reference))] = arguments else {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "Image.readResource requires a String",
                ));
            };
            let name = context.read_java_string(*reference)?;
            let path = name.strip_prefix('/').unwrap_or(&name);
            let mut bytes = context.read_resource(path)?;
            if bytes.is_none()
                && !name.starts_with('/')
                && let Some(caller) = context.native_caller_class()
                && let Some((package, _)) = caller.rsplit_once('/')
            {
                bytes = context.read_resource(&format!("{package}/{name}"))?;
            }
            match bytes {
                Some(bytes) => context
                    .allocate_java_byte_array(&bytes)
                    .map(|reference| Some(NativeValue::Reference(Some(reference)))),
                None => Ok(Some(NativeValue::Reference(None))),
            }
        },
    )?;
    registry.register(
        NativeSignature::new("javax/microedition/lcdui/Canvas", "hasPointerEvents", "()Z"),
        |context, arguments| {
            let [NativeValue::Reference(Some(_))] = arguments else {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "Canvas.hasPointerEvents requires receiver",
                ));
            };
            Ok(Some(NativeValue::Int(i32::from(
                context.canvas_pointer_events(),
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new(
            "javax/microedition/lcdui/Canvas",
            "hasPointerMotionEvents",
            "()Z",
        ),
        |context, arguments| {
            let [NativeValue::Reference(Some(_))] = arguments else {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "Canvas.hasPointerMotionEvents requires receiver",
                ));
            };
            Ok(Some(NativeValue::Int(i32::from(
                context.canvas_pointer_motion_events(),
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new("javax/microedition/lcdui/Canvas", "getGameAction", "(I)I"),
        |context, arguments| {
            let [NativeValue::Reference(Some(_)), NativeValue::Int(key_code)] = arguments else {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "Canvas.getGameAction requires receiver and key code",
                ));
            };
            context
                .canvas_game_action(*key_code)
                .map(|value| Some(NativeValue::Int(value)))
                .ok_or_else(|| {
                    EmuError::new(
                        Category::Api,
                        "illegal-argument",
                        "key code is not defined by the active device profile",
                    )
                })
        },
    )?;
    registry.register(
        NativeSignature::new("javax/microedition/lcdui/Canvas", "getKeyCode", "(I)I"),
        |context, arguments| {
            let [
                NativeValue::Reference(Some(_)),
                NativeValue::Int(game_action),
            ] = arguments
            else {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "Canvas.getKeyCode requires receiver and game action",
                ));
            };
            context
                .canvas_key_code(*game_action)
                .map(|value| Some(NativeValue::Int(value)))
                .ok_or_else(|| {
                    EmuError::new(
                        Category::Api,
                        "illegal-argument",
                        "game action is not defined by the active device profile",
                    )
                })
        },
    )?;
    registry.register(
        NativeSignature::new(
            "javax/microedition/lcdui/Canvas",
            "getKeyName",
            "(I)Ljava/lang/String;",
        ),
        |context, arguments| {
            let [NativeValue::Reference(Some(_)), NativeValue::Int(key_code)] = arguments else {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "Canvas.getKeyName requires receiver and key code",
                ));
            };
            let name = context
                .canvas_key_name(*key_code)
                .map(str::to_owned)
                .ok_or_else(|| {
                    EmuError::new(
                        Category::Api,
                        "illegal-argument",
                        "key code is not defined by the active device profile",
                    )
                })?;
            Ok(Some(NativeValue::Reference(Some(
                context.intern_java_string(&name)?,
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new("javax/microedition/lcdui/Image", "decodePng", "([BII)[I"),
        |context, arguments| {
            let [
                NativeValue::Reference(Some(reference)),
                NativeValue::Int(offset),
                NativeValue::Int(length),
            ] = arguments
            else {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "Image.decodePng requires byte[], offset and length",
                ));
            };
            let encoded = context
                .read_java_byte_array_range(*reference, *offset, *length)?
                .ok_or_else(|| {
                    EmuError::new(
                        Category::Api,
                        "array-index-out-of-bounds-exception",
                        "PNG byte region is outside input",
                    )
                })?;
            let image = graphics::Image::from_midp_encoded(&encoded).map_err(|error| {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let mut signature = String::with_capacity(16);
                for byte in encoded.iter().take(8) {
                    signature.push(char::from(HEX[usize::from(byte >> 4)]));
                    signature.push(char::from(HEX[usize::from(byte & 0x0f)]));
                }
                EmuError::with_source(
                    Category::Api,
                    "illegal-argument",
                    format!(
                        "image data cannot be decoded: {}; length={} signature={signature}",
                        error.message(),
                        encoded.len()
                    ),
                    error,
                )
            })?;
            let mut decoded = Vec::with_capacity(image.pixels().len() + 2);
            decoded.push(image.width().cast_signed());
            decoded.push(image.height().cast_signed());
            decoded.extend(image.pixels().iter().map(|pixel| pixel.cast_signed()));
            context
                .allocate_java_int_array(&decoded)
                .map(|reference| Some(NativeValue::Reference(Some(reference))))
        },
    )?;
    registry.register(
        NativeSignature::new(
            "javax/microedition/lcdui/Image",
            "createMutablePixels",
            "(II)[I",
        ),
        |context, arguments| {
            let [NativeValue::Int(width), NativeValue::Int(height)] = arguments else {
                return Err(EmuError::new(
                    Category::Api,
                    "native-arguments",
                    "Image.createMutablePixels requires width and height",
                ));
            };
            if *width <= 0 || *height <= 0 || i64::from(*width) * i64::from(*height) > 4_194_304 {
                return Err(EmuError::new(
                    Category::Api,
                    "image-size",
                    "invalid or oversized mutable image",
                ));
            }
            let length = usize::try_from(i64::from(*width) * i64::from(*height)).map_err(|_| {
                EmuError::new(Category::Api, "image-size", "mutable image size overflow")
            })?;
            let pixels = vec![-1; length];
            context
                .allocate_java_int_array(&pixels)
                .map(|reference| Some(NativeValue::Reference(Some(reference))))
        },
    )?;
    registry.register(
        NativeSignature::new(
            "javax/microedition/midlet/MIDlet",
            "getAppProperty",
            "(Ljava/lang/String;)Ljava/lang/String;",
        ),
        |context, arguments| {
            let key = string_argument(context, arguments, 1)?;
            let value = context.midlet_property(&key).map(str::to_owned);
            value.map_or(Ok(Some(NativeValue::Reference(None))), |value| {
                context
                    .intern_java_string(&value)
                    .map(|reference| Some(NativeValue::Reference(Some(reference))))
            })
        },
    )?;
    for (method, notification) in [
        ("notifyDestroyed", MidletNotification::Destroyed),
        ("notifyPaused", MidletNotification::Paused),
        ("resumeRequest", MidletNotification::ResumeRequested),
    ] {
        registry.register(
            NativeSignature::new("javax/microedition/midlet/MIDlet", method, "()V"),
            move |context, _| {
                context.midlet_lifecycle_event(MidletLifecycleEvent::Notification(notification))?;
                Ok(None)
            },
        )?;
    }
    registry.register(
        NativeSignature::new(
            "javax/microedition/midlet/MIDlet",
            "platformRequest",
            "(Ljava/lang/String;)Z",
        ),
        |context, arguments| {
            let url = string_argument(context, arguments, 1)?;
            context
                .platform_request(&url)
                .map(|exit| Some(NativeValue::Int(i32::from(exit))))
        },
    )?;
    registry.register(
        NativeSignature::new(
            "javax/microedition/midlet/MIDlet",
            "checkPermission",
            "(Ljava/lang/String;)I",
        ),
        |context, arguments| {
            let permission = string_argument(context, arguments, 1)?;
            context
                .check_permission(&permission)
                .map(|status| Some(NativeValue::Int(status)))
        },
    )
}

fn string_argument(
    context: &dyn natives::NativeContext,
    arguments: &[NativeValue],
    index: usize,
) -> Result<String, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Reference(Some(reference))) => context.read_java_string(*reference),
        Some(NativeValue::Reference(None)) => Err(EmuError::new(
            Category::Api,
            "null-pointer-exception",
            "MIDlet string argument is null",
        )),
        _ => Err(EmuError::new(
            Category::Api,
            "native-arguments",
            "invalid MIDlet native arguments",
        )),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/midp/native_api.rs"]
mod tests;
