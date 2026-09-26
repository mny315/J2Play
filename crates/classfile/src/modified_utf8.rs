//! Java string decoding with exact preservation of isolated UTF-16 surrogates.

use diagnostics::EmuError;

use crate::{Constant, Utf16Constant, parse_error};

pub(super) fn parse_utf8_constant(bytes: &[u8], base_offset: usize) -> Result<Constant, EmuError> {
    // Ordinary UTF-8 is already the retained host representation. Modified
    // NULs and surrogate code units still need the Java-specific decoder.
    if let Ok(text) = std::str::from_utf8(bytes)
        && !bytes.contains(&0)
    {
        return Ok(Constant::Utf8(text.to_owned()));
    }
    let units = decode_modified_utf8(bytes, base_offset)?;
    // Modified UTF-8 occupies at least as many bytes as the host text. This
    // capacity avoids growing the String while combining surrogate pairs.
    let mut text = String::with_capacity(bytes.len());
    let mut isolated_surrogates = false;
    for character in char::decode_utf16(units.iter().copied()) {
        text.push(character.unwrap_or_else(|_| {
            isolated_surrogates = true;
            char::REPLACEMENT_CHARACTER
        }));
    }
    if isolated_surrogates {
        Ok(Constant::Utf16(Box::new(Utf16Constant {
            text: text.into_boxed_str(),
            units: units.into_boxed_slice(),
        })))
    } else {
        Ok(Constant::Utf8(text))
    }
}

fn decode_modified_utf8(bytes: &[u8], base_offset: usize) -> Result<Vec<u16>, EmuError> {
    let mut utf16 = Vec::with_capacity(bytes.len());
    let mut position = 0;
    while position < bytes.len() {
        let first = bytes[position];
        let (value, width) = if first & 0x80 == 0 {
            if first == 0 {
                return Err(parse_error(
                    "invalid-modified-utf8",
                    base_offset + position,
                    "NUL must be encoded as C0 80",
                ));
            }
            (u16::from(first), 1)
        } else if first & 0xE0 == 0xC0 {
            if position + 1 >= bytes.len() || bytes[position + 1] & 0xC0 != 0x80 {
                return Err(parse_error(
                    "invalid-modified-utf8",
                    base_offset + position,
                    "invalid two-byte sequence",
                ));
            }
            let value = (u16::from(first & 0x1F) << 6) | u16::from(bytes[position + 1] & 0x3F);
            if value != 0 && value < 0x80 {
                return Err(parse_error(
                    "invalid-modified-utf8",
                    base_offset + position,
                    "overlong two-byte sequence",
                ));
            }
            (value, 2)
        } else if first & 0xF0 == 0xE0 {
            if position + 2 >= bytes.len()
                || bytes[position + 1] & 0xC0 != 0x80
                || bytes[position + 2] & 0xC0 != 0x80
            {
                return Err(parse_error(
                    "invalid-modified-utf8",
                    base_offset + position,
                    "invalid three-byte sequence",
                ));
            }
            let value = (u16::from(first & 0x0F) << 12)
                | (u16::from(bytes[position + 1] & 0x3F) << 6)
                | u16::from(bytes[position + 2] & 0x3F);
            if value < 0x800 {
                return Err(parse_error(
                    "invalid-modified-utf8",
                    base_offset + position,
                    "overlong three-byte sequence",
                ));
            }
            (value, 3)
        } else if first & 0xF8 == 0xF0 {
            if position + 3 >= bytes.len()
                || bytes[position + 1] & 0xC0 != 0x80
                || bytes[position + 2] & 0xC0 != 0x80
                || bytes[position + 3] & 0xC0 != 0x80
            {
                return Err(parse_error(
                    "invalid-modified-utf8",
                    base_offset + position,
                    "invalid four-byte sequence",
                ));
            }
            let codepoint = (u32::from(first & 0x07) << 18)
                | (u32::from(bytes[position + 1] & 0x3F) << 12)
                | (u32::from(bytes[position + 2] & 0x3F) << 6)
                | u32::from(bytes[position + 3] & 0x3F);
            let character = char::from_u32(codepoint)
                .filter(|_| codepoint >= 0x1_0000)
                .ok_or_else(|| {
                    parse_error(
                        "invalid-modified-utf8",
                        base_offset + position,
                        "invalid four-byte code point",
                    )
                })?;
            let mut pair = [0; 2];
            character.encode_utf16(&mut pair);
            utf16.push(pair[0]);
            (pair[1], 4)
        } else {
            return Err(parse_error(
                "invalid-modified-utf8",
                base_offset + position,
                "unsupported leading byte",
            ));
        };
        utf16.push(value);
        position += width;
    }
    Ok(utf16)
}

#[cfg(test)]
#[path = "../../../tests/unit/classfile/modified_utf8.rs"]
mod tests;
