use super::{EmuError, NativeValue, cldc_error};

pub(super) fn java_float_min(left: f32, right: f32) -> f32 {
    if left.is_nan() {
        left
    } else if right.is_nan() {
        right
    } else if left == 0.0 && right == 0.0 {
        f32::from_bits(left.to_bits() | right.to_bits())
    } else if left <= right {
        left
    } else {
        right
    }
}

pub(super) fn java_float_max(left: f32, right: f32) -> f32 {
    if left.is_nan() {
        left
    } else if right.is_nan() {
        right
    } else if left == 0.0 && right == 0.0 {
        f32::from_bits(left.to_bits() & right.to_bits())
    } else if left >= right {
        left
    } else {
        right
    }
}

pub(super) fn java_double_min(left: f64, right: f64) -> f64 {
    if left.is_nan() {
        left
    } else if right.is_nan() {
        right
    } else if left == 0.0 && right == 0.0 {
        f64::from_bits(left.to_bits() | right.to_bits())
    } else if left <= right {
        left
    } else {
        right
    }
}

pub(super) fn java_double_max(left: f64, right: f64) -> f64 {
    if left.is_nan() {
        left
    } else if right.is_nan() {
        right
    } else if left == 0.0 && right == 0.0 {
        f64::from_bits(left.to_bits() & right.to_bits())
    } else if left >= right {
        left
    } else {
        right
    }
}

pub(super) fn require_arity(arguments: &[NativeValue], expected: usize) -> Result<(), EmuError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(cldc_error(
            "native-arguments",
            format!("expected {expected} arguments, got {}", arguments.len()),
        ))
    }
}
/// Keep Rust's shortest round-trip digits, with CLDC's notation and decimal point.
pub(super) fn format_java_float<T>(value: T) -> String
where
    T: Copy + Into<f64> + std::fmt::Display + std::fmt::UpperExp,
{
    let number = value.into();
    if number.is_nan() {
        return "NaN".to_owned();
    }
    if number.is_infinite() {
        return if number.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        }
        .to_owned();
    }
    let mut text = if number != 0.0 && !(1.0e-3..1.0e7).contains(&number.abs()) {
        format!("{value:E}")
    } else {
        value.to_string()
    };
    let mantissa_end = text.find('E').unwrap_or(text.len());
    if !text[..mantissa_end].contains('.') {
        text.insert_str(mantissa_end, ".0");
    }
    text
}

fn java_float_lexeme(source: &str) -> Option<&str> {
    let trimmed = source.trim_matches(|character| character <= '\u{20}');
    let unsigned = trimmed.strip_prefix(['+', '-']).unwrap_or(trimmed);
    if matches!(unsigned, "NaN" | "Infinity") {
        return Some(trimmed);
    }
    // Type suffixes belong to decimal literals, not to NaN or Infinity. Rust
    // also accepts case-insensitive special values which Java does not.
    let decimal = trimmed
        .strip_suffix(['f', 'F', 'd', 'D'])
        .unwrap_or(trimmed);
    let unsigned = decimal.strip_prefix(['+', '-']).unwrap_or(decimal);
    unsigned
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'.')
        .then_some(decimal)
}
pub(super) fn parse_java_f32(source: &str) -> Option<f32> {
    java_float_lexeme(source)?.parse().ok()
}
pub(super) fn parse_java_f64(source: &str) -> Option<f64> {
    java_float_lexeme(source)?.parse().ok()
}
