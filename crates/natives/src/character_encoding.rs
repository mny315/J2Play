//! Character encodings shared by String natives and streaming readers.

/// Supported conversions between Java UTF-16 code units and external bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacterEncoding {
    Utf8,
    Latin1,
    Ascii,
    Utf16Be,
    Utf16Le,
    Utf16,
}

impl CharacterEncoding {
    /// Resolves the common Java ME names without allocating a normalized String.
    /// Names are case-insensitive; hyphens, underscores and spaces are ignored.
    #[must_use]
    pub fn for_name(name: &str) -> Option<Self> {
        // ISO88591 is the longest supported normalized name.
        let mut normalized = [0; 8];
        let mut length = 0;
        for byte in name
            .bytes()
            .filter(|byte| !matches!(byte, b'-' | b'_' | b' '))
        {
            *normalized.get_mut(length)? = byte.to_ascii_uppercase();
            length += 1;
        }
        match &normalized[..length] {
            b"UTF8" => Some(Self::Utf8),
            b"ISO88591" | b"ISO8859" | b"LATIN1" => Some(Self::Latin1),
            b"USASCII" | b"ASCII" => Some(Self::Ascii),
            b"UTF16BE" => Some(Self::Utf16Be),
            b"UTF16LE" => Some(Self::Utf16Le),
            b"UTF16" => Some(Self::Utf16),
            _ => None,
        }
    }

    /// Encodes bounded caller-owned Java text. Unrepresentable single-byte
    /// characters become `?`; UTF-16 preserves the original code units.
    #[must_use]
    pub fn encode(self, units: &[u16]) -> Vec<u8> {
        match self {
            Self::Utf8 => String::from_utf16_lossy(units).into_bytes(),
            Self::Latin1 => units
                .iter()
                .map(|unit| u8::try_from(*unit).unwrap_or(b'?'))
                .collect(),
            Self::Ascii => units
                .iter()
                .map(|unit| {
                    u8::try_from(*unit)
                        .ok()
                        .filter(u8::is_ascii)
                        .unwrap_or(b'?')
                })
                .collect(),
            Self::Utf16Be => units.iter().flat_map(|unit| unit.to_be_bytes()).collect(),
            Self::Utf16Le => units.iter().flat_map(|unit| unit.to_le_bytes()).collect(),
            Self::Utf16 => [0xfe, 0xff]
                .into_iter()
                .chain(units.iter().flat_map(|unit| unit.to_be_bytes()))
                .collect(),
        }
    }

    /// Decodes bounded caller-owned bytes into Java code units. UTF-16 detects
    /// an initial byte-order mark and otherwise defaults to big endian;
    /// explicitly ordered encodings retain the mark as text.
    #[must_use]
    pub fn decode(self, bytes: &[u8]) -> Vec<u16> {
        match self {
            Self::Utf8 => {
                // Convert borrowed valid spans directly, without allocating a
                // temporary UTF-8 String expanded by replacement characters.
                let mut units = Vec::new();
                for chunk in bytes.utf8_chunks() {
                    units.extend(chunk.valid().encode_utf16());
                    if !chunk.invalid().is_empty() {
                        units.push(0xfffd);
                    }
                }
                units
            }
            Self::Latin1 => bytes.iter().map(|byte| u16::from(*byte)).collect(),
            Self::Ascii => bytes
                .iter()
                .map(|byte| {
                    if byte.is_ascii() {
                        u16::from(*byte)
                    } else {
                        0xfffd
                    }
                })
                .collect(),
            Self::Utf16Be | Self::Utf16Le | Self::Utf16 => {
                let (little_endian, bytes) = match (self, bytes) {
                    (Self::Utf16, [0xff, 0xfe, rest @ ..]) => (true, rest),
                    (Self::Utf16, [0xfe, 0xff, rest @ ..]) => (false, rest),
                    _ => (self == Self::Utf16Le, bytes),
                };
                let (pairs, remainder) = bytes.as_chunks::<2>();
                let mut units = Vec::with_capacity(bytes.len().div_ceil(2));
                units.extend(pairs.iter().map(|pair| {
                    if little_endian {
                        u16::from_le_bytes(*pair)
                    } else {
                        u16::from_be_bytes(*pair)
                    }
                }));
                if !remainder.is_empty() {
                    units.push(0xfffd);
                }
                units
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/natives/character_encoding.rs"]
mod tests;
