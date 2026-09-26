use super::{EmuError, NativeRegistry, NativeSignature, NativeValue, cldc_error};
use natives::CharacterEncoding;

pub(super) fn register_text_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    registry.register(
        NativeSignature::new("java/lang/String", "fromChars", "([C)Ljava/lang/String;"),
        |context, args| {
            let [NativeValue::Reference(Some(reference))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "String.fromChars expects a non-null char array",
                ));
            };
            let value = context.read_java_char_array(*reference)?;
            Ok(Some(NativeValue::Reference(Some(
                context.intern_java_utf16(&value)?,
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/String", "initString", "(Ljava/lang/String;)V"),
        |context, args| {
            let [
                NativeValue::Reference(Some(receiver)),
                NativeValue::Reference(Some(source)),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "String.initString expects receiver and source",
                ));
            };
            let value = context.read_java_utf16(*source)?.to_vec();
            context.write_java_utf16(*receiver, value.into_boxed_slice())?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/String", "initChars", "([CII)V"),
        |context, args| {
            let [
                NativeValue::Reference(Some(receiver)),
                NativeValue::Reference(Some(source)),
                NativeValue::Int(offset),
                NativeValue::Int(count),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "String.initChars expects receiver, char array, offset and count",
                ));
            };
            let value = context
                .read_java_char_array_range(*source, *offset, *count)?
                .ok_or_else(|| cldc_error("string-index", "String char range is out of bounds"))?;
            context.write_java_utf16(*receiver, value.into_boxed_slice())?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/String", "initBytes", "([BIILjava/lang/String;)Z"),
        |context, args| {
            let [
                NativeValue::Reference(Some(receiver)),
                NativeValue::Reference(Some(source)),
                NativeValue::Int(offset),
                NativeValue::Int(count),
                NativeValue::Reference(encoding),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "String.initBytes expects receiver, byte array, range and optional encoding",
                ));
            };
            let bytes = context.read_java_byte_array_range(*source, *offset, *count)?;
            usize::try_from(*offset)
                .map_err(|_| cldc_error("string-index", "negative String offset"))?;
            usize::try_from(*count)
                .map_err(|_| cldc_error("string-index", "negative String count"))?;
            let bytes = bytes
                .ok_or_else(|| cldc_error("string-index", "String byte range is out of bounds"))?;
            let encoding = match encoding {
                Some(encoding) => {
                    CharacterEncoding::for_name(&context.read_java_string(*encoding)?)
                }
                None => CharacterEncoding::for_name(
                    context
                        .system_property("microedition.encoding")
                        .unwrap_or("UTF-8"),
                ),
            };
            let Some(encoding) = encoding else {
                return Ok(Some(NativeValue::Int(0)));
            };
            let value = encoding.decode(&bytes);
            context.write_java_utf16(*receiver, value.into_boxed_slice())?;
            Ok(Some(NativeValue::Int(1)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/String", "getBytes", "()[B"),
        |context, args| {
            let [NativeValue::Reference(Some(receiver))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "String.getBytes expects receiver",
                ));
            };
            let value = context.read_java_utf16(*receiver)?;
            let encoding = context
                .system_property("microedition.encoding")
                .unwrap_or("UTF-8");
            let encoding = CharacterEncoding::for_name(encoding).ok_or_else(|| {
                cldc_error(
                    "unsupported-encoding",
                    format!("unsupported platform character encoding: {encoding}"),
                )
            })?;
            let bytes = encoding.encode(value);
            Ok(Some(NativeValue::Reference(Some(
                context.allocate_java_byte_array(&bytes)?,
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/String", "getBytes", "(Ljava/lang/String;)[B"),
        |context, args| {
            let [
                NativeValue::Reference(Some(receiver)),
                NativeValue::Reference(encoding),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "String.getBytes expects receiver and encoding",
                ));
            };
            let Some(encoding) = encoding else {
                return Err(cldc_error(
                    "null-pointer-exception",
                    "String.getBytes encoding is null",
                ));
            };
            let value = context.read_java_utf16(*receiver)?;
            let encoding = context.read_java_string(*encoding)?;
            let encoding = CharacterEncoding::for_name(&encoding).ok_or_else(|| {
                cldc_error(
                    "unsupported-encoding",
                    format!("unsupported character encoding: {encoding}"),
                )
            })?;
            let bytes = encoding.encode(value);
            Ok(Some(NativeValue::Reference(Some(
                context.allocate_java_byte_array(&bytes)?,
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/String", "length", "()I"),
        |context, args| {
            let [NativeValue::Reference(Some(reference))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "String.length expects receiver",
                ));
            };
            let value = context.read_java_utf16(*reference)?;
            let length = i32::try_from(value.len())
                .map_err(|_| cldc_error("string-limit", "string length exceeds i32"))?;
            Ok(Some(NativeValue::Int(length)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/String", "charAt0", "(I)C"),
        |context, args| {
            let [
                NativeValue::Reference(Some(reference)),
                NativeValue::Int(index),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "String.charAt expects receiver and int",
                ));
            };
            let value = context.read_java_utf16(*reference)?;
            let index = usize::try_from(*index)
                .map_err(|_| cldc_error("string-index", "negative string index"))?;
            let unit = value
                .get(index)
                .copied()
                .ok_or_else(|| cldc_error("string-index", "string index out of bounds"))?;
            Ok(Some(NativeValue::Int(i32::from(unit))))
        },
    )?;

    Ok(())
}
