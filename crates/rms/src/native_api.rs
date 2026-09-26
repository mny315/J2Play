//! Native `RecordStore` argument marshalling and result conversion.

use diagnostics::EmuError;
use natives::{NativeRegistry, NativeSignature, NativeValue, RmsMetadataField};

use crate::rms_error;
use crate::runtime::store_not_open;

/// Registers the host-backed native half of `javax.microedition.rms.RecordStore`.
///
/// Java owns object identity, listeners, and enumerations; this boundary owns
/// durable bytes and opaque balanced handles.
///
/// # Errors
/// Returns `duplicate-native` if a signature was registered already.
#[allow(clippy::too_many_lines)]
pub fn register_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    const CLASS: &str = "javax/microedition/rms/RecordStore";
    registry.register(
        NativeSignature::new(CLASS, "open0", "(Ljava/lang/String;Z)J"),
        |context, arguments| {
            let name = string_argument(context, arguments, 0)?;
            let create = int_argument(arguments, 1)? != 0;
            context.rms_open(&name, create).and_then(java_handle)
        },
    )?;
    registry.register(
        NativeSignature::new(
            CLASS,
            "openOwned0",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)J",
        ),
        |context, arguments| {
            let name = string_argument(context, arguments, 0)?;
            let vendor = string_argument(context, arguments, 1)?;
            let suite = string_argument(context, arguments, 2)?;
            context
                .rms_open_owned(&name, &vendor, &suite)
                .and_then(java_handle)
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "close0", "(J)V"),
        |context, arguments| {
            context.rms_close(handle_argument(arguments, 0)?)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "deleteStore0", "(Ljava/lang/String;)V"),
        |context, arguments| {
            let name = string_argument(context, arguments, 0)?;
            context.rms_delete_store(&name)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "list0", "()Ljava/lang/String;"),
        |context, arguments| {
            if !arguments.is_empty() {
                return Err(native_arguments("list0 takes no arguments"));
            }
            let names = context.rms_list_stores()?;
            if names.is_empty() {
                return Ok(Some(NativeValue::Reference(None)));
            }
            let count = u16::try_from(names.len())
                .map_err(|_| rms_error("store-limit", "too many store names to return"))?;
            let mut packed = Vec::with_capacity(1 + names.len() * 8);
            packed.push(count);
            for name in names {
                let length_slot = packed.len();
                packed.push(0);
                packed.extend(name.encode_utf16());
                packed[length_slot] = u16::try_from(packed.len() - length_slot - 1)
                    .map_err(|_| rms_error("corrupt-store", "store name exceeds UTF-16 limit"))?;
            }
            context
                .intern_java_utf16(&packed)
                .map(|reference| Some(NativeValue::Reference(Some(reference))))
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "metadataInt0", "(JI)I"),
        |context, arguments| {
            let handle = handle_argument(arguments, 0)?;
            let field = match int_argument(arguments, 1)? {
                0 => RmsMetadataField::Version,
                1 => RmsMetadataField::RecordCount,
                2 => RmsMetadataField::Size,
                3 => RmsMetadataField::SizeAvailable,
                4 => RmsMetadataField::NextRecordId,
                _ => return Err(native_arguments("unknown RMS metadata selector")),
            };
            let value = i32::try_from(context.rms_metadata(handle, field)?)
                .map_err(|_| rms_error("metadata-range", "RMS metadata exceeds Java int"))?;
            Ok(Some(NativeValue::Int(value)))
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "lastModified0", "(J)J"),
        |context, arguments| {
            context
                .rms_metadata(
                    handle_argument(arguments, 0)?,
                    RmsMetadataField::LastModified,
                )
                .map(|value| Some(NativeValue::Long(value)))
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "ids0", "(J)[I"),
        |context, arguments| {
            let ids = context.rms_record_ids(handle_argument(arguments, 0)?)?;
            context
                .allocate_java_int_array(&ids)
                .map(|reference| Some(NativeValue::Reference(Some(reference))))
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "recordSize0", "(JI)I"),
        |context, arguments| {
            let length = context
                .rms_record_size(handle_argument(arguments, 0)?, int_argument(arguments, 1)?)?;
            let length = i32::try_from(length)
                .map_err(|_| rms_error("metadata-range", "RMS record size exceeds Java int"))?;
            Ok(Some(NativeValue::Int(length)))
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "get0", "(JI)[B"),
        |context, arguments| {
            let bytes =
                context.rms_get(handle_argument(arguments, 0)?, int_argument(arguments, 1)?)?;
            if bytes.is_empty() {
                Ok(Some(NativeValue::Reference(None)))
            } else {
                context
                    .allocate_java_byte_array(&bytes)
                    .map(|reference| Some(NativeValue::Reference(Some(reference))))
            }
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "add0", "(J[B)I"),
        |context, arguments| {
            let handle = handle_argument(arguments, 0)?;
            let data = byte_array_argument(context, arguments, 1)?;
            context
                .rms_add(handle, &data)
                .map(|id| Some(NativeValue::Int(id)))
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "set0", "(JI[B)V"),
        |context, arguments| {
            let handle = handle_argument(arguments, 0)?;
            let record_id = int_argument(arguments, 1)?;
            let data = byte_array_argument(context, arguments, 2)?;
            context.rms_set(handle, record_id, &data)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(CLASS, "delete0", "(JI)V"),
        |context, arguments| {
            context.rms_delete(handle_argument(arguments, 0)?, int_argument(arguments, 1)?)?;
            Ok(None)
        },
    )
}

fn string_argument(
    context: &dyn natives::NativeContext,
    arguments: &[NativeValue],
    index: usize,
) -> Result<String, EmuError> {
    let reference = match arguments.get(index) {
        Some(NativeValue::Reference(Some(reference))) => *reference,
        Some(NativeValue::Reference(None)) => {
            return Err(rms_error("null-pointer-exception", "RMS string is null"));
        }
        _ => return Err(native_arguments("RMS native expected a String")),
    };
    String::from_utf16(context.read_java_utf16(reference)?)
        .map_err(|_| rms_error("store-name", "RMS string contains an unpaired surrogate"))
}

fn byte_array_argument(
    context: &dyn natives::NativeContext,
    arguments: &[NativeValue],
    index: usize,
) -> Result<Vec<u8>, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Reference(Some(reference))) => context.read_java_byte_array(*reference),
        Some(NativeValue::Reference(None)) => Ok(Vec::new()),
        _ => Err(native_arguments("RMS native expected a byte[]")),
    }
}

fn int_argument(arguments: &[NativeValue], index: usize) -> Result<i32, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Int(value)) => Ok(*value),
        _ => Err(native_arguments("RMS native expected an int")),
    }
}

fn handle_argument(arguments: &[NativeValue], index: usize) -> Result<u64, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Long(value)) if *value > 0 => {
            u64::try_from(*value).map_err(|_| store_not_open())
        }
        _ => Err(store_not_open()),
    }
}

fn java_handle(handle: u64) -> Result<Option<NativeValue>, EmuError> {
    i64::try_from(handle)
        .map(|value| Some(NativeValue::Long(value)))
        .map_err(|_| rms_error("open-limit", "RMS handle exceeds Java long"))
}

fn native_arguments(message: &'static str) -> EmuError {
    rms_error("native-arguments", message)
}

#[cfg(test)]
#[path = "../../../tests/unit/rms/native_api.rs"]
mod tests;
