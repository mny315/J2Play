//! Bounded JSR-82 UUID parsing; canonical values live in the managed Java heap.
//!
//! Contract: JSR-82 1.1.1, javax.bluetooth.UUID constructors and toString.

use super::{EmuError, NativeRegistry, NativeSignature, NativeValue, bluetooth_error};

const BASE: u128 = 0x0000_0000_0000_1000_8000_0080_5F9B_34FB;

pub(super) fn register_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    registry.register(
        NativeSignature::new("javax/bluetooth/UUID", "canonical", "(J)Ljava/lang/String;"),
        |context, args| {
            let [NativeValue::Long(value)] = args else {
                return Err(bluetooth_error(
                    "illegal-argument",
                    "UUID expects a long value",
                ));
            };
            let short = u32::try_from(*value).map_err(|_| {
                bluetooth_error(
                    "illegal-argument",
                    "UUID value must be an unsigned 32-bit integer",
                )
            })?;
            let text = canonical(u128::from(short), true);
            Ok(Some(NativeValue::Reference(Some(
                context.intern_java_string(&text)?,
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new(
            "javax/bluetooth/UUID",
            "canonical",
            "(Ljava/lang/String;Z)Ljava/lang/String;",
        ),
        |context, args| {
            let [
                NativeValue::Reference(Some(reference)),
                NativeValue::Int(short),
            ] = args
            else {
                return Err(bluetooth_error(
                    "null-pointer-exception",
                    "UUID expects a non-null string",
                ));
            };
            let text = parse(context.read_java_utf16(*reference)?, *short != 0)?;
            Ok(Some(NativeValue::Reference(Some(
                context.intern_java_string(&text)?,
            ))))
        },
    )
}

fn parse(units: &[u16], short: bool) -> Result<String, EmuError> {
    if units.is_empty() || units.len() > if short { 8 } else { 32 } {
        return Err(bluetooth_error(
            "illegal-argument",
            "UUID string has an invalid length",
        ));
    }
    let mut value = 0_u128;
    for unit in units {
        let digit = match unit {
            0x30..=0x39 => unit - 0x30,
            0x41..=0x46 => unit - 0x41 + 10,
            0x61..=0x66 => unit - 0x61 + 10,
            _ => {
                return Err(bluetooth_error(
                    "number-format",
                    "UUID must contain only hexadecimal digits",
                ));
            }
        };
        value = (value << 4) | u128::from(digit);
    }
    Ok(canonical(value, short))
}

fn canonical(value: u128, short: bool) -> String {
    let value = if short { (value << 96) | BASE } else { value };
    format!("{value:X}")
}
