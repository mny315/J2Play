use diagnostics::{Category, EmuError};

pub const MAX_GUEST_LOCALE_BYTES: usize = 63;

/// Checks the MIDP language[-COUNTRY[-variant]] form, with bounded ASCII subtags.
/// Language-only codes and historical two-letter Java codes remain usable.
///
/// # Errors
/// Returns a stable diagnostic for malformed or oversized locale codes.
pub fn validate_guest_locale(locale: &str) -> Result<(), EmuError> {
    let mut parts = locale.split('-');
    let language = parts.next().unwrap_or_default();
    let valid = locale.len() <= MAX_GUEST_LOCALE_BYTES
        && language.len() == 2
        && language.bytes().all(|byte| byte.is_ascii_lowercase())
        && parts.next().is_none_or(|country| {
            country.len() == 2 && country.bytes().all(|byte| byte.is_ascii_uppercase())
        })
        && parts.all(|variant| {
            (1..=8).contains(&variant.len())
                && variant.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });
    if valid {
        Ok(())
    } else {
        Err(EmuError::new(
            Category::Api,
            "guest-locale",
            "Enter a valid language code, such as ru-RU or he-IL.",
        ))
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/device-profile/locale.rs"]
mod tests;
