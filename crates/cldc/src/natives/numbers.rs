use super::{
    EmuError, NativeRegistry, NativeSignature, NativeValue, character_digit, cldc_error,
    format_java_float, parse_java_f32, parse_java_f64, simple_case_mapping, unicode_digit,
};

pub(super) fn register_number_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    registry.register(
        NativeSignature::new("java/lang/Float", "toString", "(F)Ljava/lang/String;"),
        |context, args| {
            let [NativeValue::Float(value)] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Float.toString expects float",
                ));
            };
            let text = format_java_float(*value);
            Ok(Some(NativeValue::Reference(Some(
                context.intern_java_string(&text)?,
            ))))
        },
    )?;
    for (name, uppercase) in [("isUpperCase", true), ("isLowerCase", false)] {
        registry.register(
            NativeSignature::new("java/lang/Character", name, "(C)Z"),
            move |_, args| {
                let [NativeValue::Int(unit)] = args else {
                    return Err(cldc_error(
                        "native-arguments",
                        "Character case query expects char",
                    ));
                };
                let matches = char::from_u32(*unit as u32).is_some_and(|value| {
                    if uppercase {
                        value.is_uppercase()
                    } else {
                        value.is_lowercase()
                    }
                });
                Ok(Some(NativeValue::Int(i32::from(matches))))
            },
        )?;
    }
    registry.register(
        NativeSignature::new("java/lang/Character", "digit", "(CI)I"),
        |_, args| {
            let [NativeValue::Int(unit), NativeValue::Int(radix)] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Character.digit expects char and radix",
                ));
            };
            let value = u32::try_from(*radix)
                .ok()
                .and_then(|radix| character_digit(*unit as u16, radix))
                .map_or(-1, |digit| digit as i32);
            Ok(Some(NativeValue::Int(value)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/Character", "isDigit", "(C)Z"),
        |_, args| {
            let [NativeValue::Int(unit)] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Character.isDigit expects char",
                ));
            };
            Ok(Some(NativeValue::Int(i32::from(
                unicode_digit(*unit as u16).is_some(),
            ))))
        },
    )?;
    for (name, uppercase) in [("toUpperCase", true), ("toLowerCase", false)] {
        registry.register(
            NativeSignature::new("java/lang/Character", name, "(C)C"),
            move |_, args| {
                let [NativeValue::Int(unit)] = args else {
                    return Err(cldc_error(
                        "native-arguments",
                        "Character case conversion expects char",
                    ));
                };
                let Some(value) = char::from_u32(*unit as u32) else {
                    return Ok(Some(NativeValue::Int(*unit)));
                };
                let result = simple_case_mapping(value, uppercase);
                Ok(Some(NativeValue::Int(result as i32)))
            },
        )?;
    }
    registry.register(
        NativeSignature::new("java/lang/Float", "parseFloat", "(Ljava/lang/String;)F"),
        |context, args| {
            let [NativeValue::Reference(reference)] = args else {
                return Err(cldc_error(
                    "number-format",
                    "Float.parseFloat expects String",
                ));
            };
            let reference = reference.ok_or_else(|| {
                cldc_error("null-pointer-exception", "Float.parseFloat received null")
            })?;
            let text = context.read_java_string(reference)?;
            let value = parse_java_f32(&text).ok_or_else(|| cldc_error("number-format", &text))?;
            Ok(Some(NativeValue::Float(value)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/Float", "floatToIntBits", "(F)I"),
        |_, args| {
            let [NativeValue::Float(value)] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "floatToIntBits expects float",
                ));
            };
            let bits = if value.is_nan() {
                0x7fc0_0000
            } else {
                value.to_bits()
            };
            Ok(Some(NativeValue::Int(bits as i32)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/Float", "intBitsToFloat", "(I)F"),
        |_, args| {
            let [NativeValue::Int(value)] = args else {
                return Err(cldc_error("native-arguments", "intBitsToFloat expects int"));
            };
            Ok(Some(NativeValue::Float(f32::from_bits(*value as u32))))
        },
    )?;
    for (class, method, error) in [
        (
            "java/lang/Double",
            "toString",
            "Double.toString expects double",
        ),
        (
            "java/lang/String",
            "valueOf",
            "String.valueOf expects double",
        ),
    ] {
        registry.register(
            NativeSignature::new(class, method, "(D)Ljava/lang/String;"),
            move |context, args| {
                let [NativeValue::Double(value)] = args else {
                    return Err(cldc_error("native-arguments", error));
                };
                let text = format_java_float(*value);
                Ok(Some(NativeValue::Reference(Some(
                    context.intern_java_string(&text)?,
                ))))
            },
        )?;
    }
    registry.register(
        NativeSignature::new("java/lang/Double", "parseDouble", "(Ljava/lang/String;)D"),
        |context, args| {
            let [NativeValue::Reference(reference)] = args else {
                return Err(cldc_error(
                    "number-format",
                    "Double.parseDouble expects String",
                ));
            };
            let reference = reference.ok_or_else(|| {
                cldc_error("null-pointer-exception", "Double.parseDouble received null")
            })?;
            let text = context.read_java_string(reference)?;
            let value = parse_java_f64(&text).ok_or_else(|| cldc_error("number-format", &text))?;
            Ok(Some(NativeValue::Double(value)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/Double", "doubleToLongBits", "(D)J"),
        |_, args| {
            let [NativeValue::Double(value)] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "doubleToLongBits expects double",
                ));
            };
            let bits = if value.is_nan() {
                0x7ff8_0000_0000_0000
            } else {
                value.to_bits()
            };
            Ok(Some(NativeValue::Long(bits as i64)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/Double", "longBitsToDouble", "(J)D"),
        |_, args| {
            let [NativeValue::Long(value)] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "longBitsToDouble expects long",
                ));
            };
            Ok(Some(NativeValue::Double(f64::from_bits(*value as u64))))
        },
    )?;

    Ok(())
}
