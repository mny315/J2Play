//! Native argument marshalling for HTTP and `FileConnection` guest APIs.

use super::http_date::parse_http_date;
use crate::api_error;
use diagnostics::EmuError;
use natives::{GcfHttpRequest, GcfHttpResponse, NativeRegistry, NativeSignature, NativeValue};

/// Registers native methods used by the Java GCF bootstrap implementation.
///
/// # Errors
/// Returns an error when another crate registered an identical signature.
#[allow(clippy::too_many_lines)]
pub fn register_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    let http = "javax/microedition/io/HttpConnectionImpl";
    registry.register(
        NativeSignature::new(
            http,
            "request0",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;[B)[B",
        ),
        |context, arguments| {
            let url = string_argument(context, arguments, 0)?;
            let method = string_argument(context, arguments, 1)?;
            let flattened = string_argument(context, arguments, 2)?;
            let headers = parse_flat_headers(&flattened)?;
            let body = byte_array_argument(context, arguments, 3)?;
            let response = context.gcf_http(GcfHttpRequest {
                url,
                method,
                headers,
                body,
            })?;
            let encoded = encode_http_response(&response)?;
            context
                .allocate_java_byte_array(&encoded)
                .map(|reference| Some(NativeValue::Reference(Some(reference))))
        },
    )?;
    registry.register(
        NativeSignature::new(http, "parseDate0", "(Ljava/lang/String;)J"),
        |context, arguments| {
            parse_http_date(
                &string_argument(context, arguments, 0)?,
                context.wall_clock_millis(),
            )
            .map(|value| Some(NativeValue::Long(value)))
        },
    )?;
    let file = "javax/microedition/io/file/FileConnectionImpl";
    registry.register(
        NativeSignature::new(file, "revision0", "()J"),
        |context, _| {
            let token = context.gcf_file_revision()?;
            Ok(Some(NativeValue::Long(i64::from_ne_bytes(
                token.to_ne_bytes(),
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new(file, "outputOffset0", "(Ljava/lang/String;J)J"),
        |context, arguments| {
            let url = string_argument(context, arguments, 0)?;
            let offset = u64::try_from(long_argument(arguments, 1)?)
                .map_err(|_| api_error("illegal-argument", "negative output offset"))?;
            let position = context.gcf_file_output_offset(&url, offset)?;
            let position = i64::try_from(position)
                .map_err(|_| api_error("file-limit", "output position exceeds Java long"))?;
            Ok(Some(NativeValue::Long(position)))
        },
    )?;
    registry.register(
        NativeSignature::new(file, "metadata0", "(Ljava/lang/String;)[J"),
        |context, arguments| {
            let metadata = context.gcf_file_metadata(&string_argument(context, arguments, 0)?)?;
            context
                .allocate_java_long_array(&[
                    i64::from(metadata.exists),
                    i64::from(metadata.directory),
                    i64::try_from(metadata.size).unwrap_or(i64::MAX),
                    metadata.modified_millis,
                    i64::from(metadata.readable),
                    i64::from(metadata.writable),
                ])
                .map(|reference| Some(NativeValue::Reference(Some(reference))))
        },
    )?;
    registry.register(
        NativeSignature::new(file, "read0", "(Ljava/lang/String;)[B"),
        |context, arguments| {
            let data = context.gcf_file_read(&string_argument(context, arguments, 0)?)?;
            context
                .allocate_java_byte_array(&data)
                .map(|reference| Some(NativeValue::Reference(Some(reference))))
        },
    )?;
    registry.register(
        NativeSignature::new(file, "write0", "(Ljava/lang/String;[BZ)V"),
        |context, arguments| {
            let url = string_argument(context, arguments, 0)?;
            let data = byte_array_argument(context, arguments, 1)?;
            let append = int_argument(arguments, 2)? != 0;
            context.gcf_file_write(&url, &data, append)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(file, "writeAt0", "(Ljava/lang/String;[BJ)V"),
        |context, arguments| {
            let url = string_argument(context, arguments, 0)?;
            let data = byte_array_argument(context, arguments, 1)?;
            let offset = long_argument(arguments, 2).and_then(|value| {
                u64::try_from(value)
                    .map_err(|_| api_error("illegal-argument", "negative output offset"))
            })?;
            context.gcf_file_write_at(&url, &data, offset)?;
            Ok(None)
        },
    )?;
    for (method, operation) in [("create0", 0), ("mkdir0", 1), ("delete0", 2)] {
        registry.register(
            NativeSignature::new(file, method, "(Ljava/lang/String;)V"),
            move |context, arguments| {
                let url = string_argument(context, arguments, 0)?;
                match operation {
                    0 => context.gcf_file_create(&url),
                    1 => context.gcf_file_mkdir(&url),
                    _ => context.gcf_file_delete(&url),
                }?;
                Ok(None)
            },
        )?;
    }
    registry.register(
        NativeSignature::new(
            file,
            "rename0",
            "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
        ),
        |context, arguments| {
            let value = context.gcf_file_rename(
                &string_argument(context, arguments, 0)?,
                &string_argument(context, arguments, 1)?,
            )?;
            context
                .intern_java_string(&value)
                .map(|reference| Some(NativeValue::Reference(Some(reference))))
        },
    )?;
    registry.register(
        NativeSignature::new(file, "truncate0", "(Ljava/lang/String;J)V"),
        |context, arguments| {
            let size = long_argument(arguments, 1).and_then(|value| {
                u64::try_from(value)
                    .map_err(|_| api_error("illegal-argument", "negative truncate size"))
            })?;
            context.gcf_file_truncate(&string_argument(context, arguments, 0)?, size)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new(file, "list0", "(Ljava/lang/String;)[B"),
        |context, arguments| {
            let values = context.gcf_file_list(&string_argument(context, arguments, 0)?)?;
            let encoded = encode_strings(&values)?;
            context
                .allocate_java_byte_array(&encoded)
                .map(|reference| Some(NativeValue::Reference(Some(reference))))
        },
    )?;
    registry.register(
        NativeSignature::new(file, "directorySize0", "(Ljava/lang/String;Z)J"),
        |context, arguments| {
            context
                .gcf_file_directory_size(
                    &string_argument(context, arguments, 0)?,
                    int_argument(arguments, 1)? != 0,
                )
                .and_then(java_long)
        },
    )?;
    registry.register(
        NativeSignature::new(file, "space0", "(I)J"),
        |context, arguments| {
            context
                .gcf_file_space(int_argument(arguments, 0)?)
                .and_then(java_long)
        },
    )
}

fn parse_flat_headers(value: &str) -> Result<Vec<(String, String)>, EmuError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    // Bound the Java bridge before splitting into owned strings or copying
    // the request body. Framing adds at most a colon and newline per header.
    let limits = super::Limits::default();
    let maximum_flattened_bytes = limits
        .max_header_bytes
        .saturating_add(limits.max_headers.saturating_mul(2));
    if value.len() > maximum_flattened_bytes {
        return Err(api_error(
            "network-limit",
            "HTTP headers exceed configured limit",
        ));
    }
    let headers = value
        .split('\n')
        .take(limits.max_headers.saturating_add(1))
        .map(|line| {
            line.split_once(':')
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .ok_or_else(|| api_error("network-header", "malformed flattened HTTP header"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    super::http::validate_headers(&headers, &limits)?;
    Ok(headers)
}

fn encode_http_response(response: &GcfHttpResponse) -> Result<Vec<u8>, EmuError> {
    let mut output = Vec::new();
    write_i32(&mut output, response.status);
    write_utf(&mut output, &response.reason)?;
    write_i32(
        &mut output,
        i32::try_from(response.headers.len())
            .map_err(|_| api_error("network-limit", "too many response headers"))?,
    );
    for (name, value) in &response.headers {
        write_utf(&mut output, name)?;
        write_utf(&mut output, value)?;
    }
    write_i32(
        &mut output,
        i32::try_from(response.body.len())
            .map_err(|_| api_error("network-limit", "response body is too large"))?,
    );
    output.extend_from_slice(&response.body);
    Ok(output)
}

fn encode_strings(values: &[String]) -> Result<Vec<u8>, EmuError> {
    let mut output = Vec::new();
    write_i32(
        &mut output,
        i32::try_from(values.len())
            .map_err(|_| api_error("file-limit", "too many directory entries"))?,
    );
    for value in values {
        write_utf(&mut output, value)?;
    }
    Ok(output)
}

fn write_i32(output: &mut Vec<u8>, value: i32) {
    output.extend_from_slice(&value.to_be_bytes());
}
fn write_utf(output: &mut Vec<u8>, value: &str) -> Result<(), EmuError> {
    let length = value
        .encode_utf16()
        .try_fold(0_u16, |length, unit| {
            length.checked_add(match unit {
                1..=0x7f => 1,
                0..=0x7ff => 2,
                _ => 3,
            })
        })
        .ok_or_else(|| api_error("network-limit", "encoded string is too large"))?;
    output.extend_from_slice(&length.to_be_bytes());
    for unit in value.encode_utf16() {
        match unit {
            1..=0x7f => output.push(low_byte(unit)),
            0..=0x7ff => {
                output.push(low_byte(0xc0 | (unit >> 6)));
                output.push(low_byte(0x80 | (unit & 0x3f)));
            }
            _ => {
                output.push(low_byte(0xe0 | (unit >> 12)));
                output.push(low_byte(0x80 | ((unit >> 6) & 0x3f)));
                output.push(low_byte(0x80 | (unit & 0x3f)));
            }
        }
    }
    Ok(())
}

fn low_byte(value: u16) -> u8 {
    value.to_be_bytes()[1]
}

fn string_argument(
    context: &dyn natives::NativeContext,
    arguments: &[NativeValue],
    index: usize,
) -> Result<String, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Reference(Some(value))) => context.read_java_string(*value),
        Some(NativeValue::Reference(None)) => {
            Err(api_error("null-pointer-exception", "GCF string is null"))
        }
        _ => Err(api_error("native-arguments", "GCF native expected String")),
    }
}
fn byte_array_argument(
    context: &dyn natives::NativeContext,
    arguments: &[NativeValue],
    index: usize,
) -> Result<Vec<u8>, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Reference(Some(value))) => context.read_java_byte_array(*value),
        Some(NativeValue::Reference(None)) => Ok(Vec::new()),
        _ => Err(api_error("native-arguments", "GCF native expected byte[]")),
    }
}
fn int_argument(arguments: &[NativeValue], index: usize) -> Result<i32, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Int(value)) => Ok(*value),
        _ => Err(api_error("native-arguments", "GCF native expected int")),
    }
}
fn long_argument(arguments: &[NativeValue], index: usize) -> Result<i64, EmuError> {
    match arguments.get(index) {
        Some(NativeValue::Long(value)) => Ok(*value),
        _ => Err(api_error("native-arguments", "GCF native expected long")),
    }
}
fn java_long(value: u64) -> Result<Option<NativeValue>, EmuError> {
    i64::try_from(value)
        .map(|value| Some(NativeValue::Long(value)))
        .map_err(|_| api_error("file-limit", "file value exceeds Java long"))
}

#[cfg(test)]
#[path = "../../../../tests/unit/gcf/native_api.rs"]
mod tests;
