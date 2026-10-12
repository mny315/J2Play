//! Character operations shared by CLDC natives and VM intrinsics.

/// Returns the decimal value of a character in the supported Unicode digit blocks.
#[must_use]
pub fn unicode_digit(unit: u16) -> Option<u32> {
    const DECIMAL_ZEROES: &[u16] = &[
        0x0030, 0x0660, 0x06f0, 0x0966, 0x09e6, 0x0a66, 0x0ae6, 0x0b66, 0x0be6, 0x0c66, 0x0ce6,
        0x0d66, 0x0e50, 0x0ed0, 0x0f20, 0x1040, 0x17e0, 0x1810, 0xff10,
    ];
    DECIMAL_ZEROES.iter().find_map(|zero| {
        unit.checked_sub(*zero)
            .filter(|offset| *offset <= 9)
            .map(u32::from)
    })
}

/// Returns the CLDC digit value for a UTF-16 character and a radix from 2 to 36.
#[must_use]
pub fn character_digit(unit: u16, radix: u32) -> Option<u32> {
    if !(2..=36).contains(&radix) {
        return None;
    }
    let digit = match unit {
        0x30..=0x39 => u32::from(unit - 0x30),
        0x41..=0x5a => u32::from(unit - 0x41) + 10,
        0x61..=0x7a => u32::from(unit - 0x61) + 10,
        _ => unicode_digit(unit)?,
    };
    (digit < radix).then_some(digit)
}

/// Converts a Unicode character without expanding it into several characters.
///
/// Rust exposes full case mappings. Unicode's simple mapping occasionally
/// differs when the full mapping expands; CLDC's char API needs the simple one.
#[must_use]
#[inline]
pub fn simple_case_mapping(value: char, uppercase: bool) -> char {
    fn single(mut mapped: impl Iterator<Item = char>, original: char, uppercase: bool) -> char {
        let first = mapped.next().unwrap_or(original);
        if mapped.next().is_none() {
            return first;
        }
        // Simple mappings from UnicodeData fields 12/13, where Rust's full
        // conversion expands. These characters were already present in Unicode 3.
        // https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt
        match (uppercase, original) {
            (false, '\u{0130}') => 'i',
            (true, '\u{1f80}'..='\u{1f87}' | '\u{1f90}'..='\u{1f97}' | '\u{1fa0}'..='\u{1fa7}') => {
                char::from_u32(u32::from(original) + 8).unwrap_or(original)
            }
            (true, '\u{1fb3}') => '\u{1fbc}',
            (true, '\u{1fc3}') => '\u{1fcc}',
            (true, '\u{1ff3}') => '\u{1ffc}',
            _ => original,
        }
    }
    if uppercase {
        single(value.to_uppercase(), value, true)
    } else {
        single(value.to_lowercase(), value, false)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/natives/character/mod.rs"]
mod tests;
