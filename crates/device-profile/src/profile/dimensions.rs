//! Screen dimension hints from manifest, JAD and archive metadata.

/// Extracts non-zero `width x height` pairs from loosely formatted metadata.
#[must_use]
pub fn dimension_pairs(value: &str) -> Vec<(u32, u32)> {
    let bytes = value.as_bytes();
    let mut pairs = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let width_start = index;
        let (width, width_end) = parse_ascii_u32(bytes, index);
        index = width_end;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let underscore_starts_nested_pair = if index < bytes.len() && bytes[index] == b'_' {
            let mut nested_start = index + 1;
            while nested_start < bytes.len() && bytes[nested_start].is_ascii_whitespace() {
                nested_start += 1;
            }
            let (_, mut nested_end) = parse_ascii_u32(bytes, nested_start);
            while nested_end < bytes.len() && bytes[nested_end].is_ascii_whitespace() {
                nested_end += 1;
            }
            let nested_separator_bytes = if nested_end < bytes.len()
                && matches!(bytes[nested_end], b'x' | b'X' | b'*' | b',' | b'_')
            {
                1
            } else if bytes[nested_end..].starts_with("×".as_bytes()) {
                "×".len()
            } else {
                0
            };
            let mut third_axis_start = nested_end + nested_separator_bytes;
            while third_axis_start < bytes.len() && bytes[third_axis_start].is_ascii_whitespace() {
                third_axis_start += 1;
            }
            nested_start != nested_end
                && nested_separator_bytes != 0
                && bytes.get(third_axis_start).is_some_and(u8::is_ascii_digit)
        } else {
            false
        };
        let separator_bytes =
            if index < bytes.len() && matches!(bytes[index], b'x' | b'X' | b'*' | b',') {
                1
            } else if index < bytes.len()
                && bytes[index] == b'_'
                && width.is_some_and(|width| (64..=4_096).contains(&width))
                && (width_start == 0 || !bytes[width_start - 1].is_ascii_alphabetic())
                && !underscore_starts_nested_pair
            {
                // Distribution archives commonly spell a Canvas as `240_320`.
                // Requiring a plausible, non-model-prefixed first axis prevents
                // `s40v3_240x320` and `E71_240x320` from consuming the real width.
                1
            } else if bytes[index..].starts_with("×".as_bytes()) {
                "×".len()
            } else {
                0
            };
        if separator_bytes == 0 {
            continue;
        }
        index += separator_bytes;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || !bytes[index].is_ascii_digit() {
            continue;
        }
        let (height, height_end) = parse_ascii_u32(bytes, index);
        index = height_end;
        if let (Some(width), Some(height)) = (width, height)
            && width != 0
            && height != 0
        {
            pairs.push((width, height));
        }
    }
    pairs
}

fn parse_ascii_u32(bytes: &[u8], mut index: usize) -> (Option<u32>, usize) {
    let mut value = Some(0_u32);
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        let digit = u32::from(bytes[index] - b'0');
        value = value.and_then(|current| current.checked_mul(10)?.checked_add(digit));
        index += 1;
    }
    (value, index)
}
