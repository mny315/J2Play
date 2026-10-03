//! Host-independent CLDC 1.1 bootstrap services.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::too_many_lines
)]

use device_profile::DeviceProfile;
use diagnostics::{Category, EmuError};
use natives::{
    NativeRegistry, NativeSignature, NativeValue, character_digit, simple_case_mapping,
    unicode_digit,
};
use std::collections::HashMap;

mod calendar;
#[path = "natives/mod.rs"]
mod native_registration;
mod numeric;

use calendar::{
    calendar_add_field, calendar_field, calendar_replace_date_time, calendar_replace_field,
    calendar_roll_field,
};
use numeric::{
    format_java_float, java_double_max, java_double_min, java_float_max, java_float_min,
    parse_java_f32, parse_java_f64, require_arity,
};

/// Deterministic system property snapshot derived from the selected profile.
#[derive(Clone, Debug, Default)]
pub struct SystemProperties(HashMap<String, String>);
impl SystemProperties {
    #[must_use]
    pub fn from_profile(profile: &DeviceProfile) -> Self {
        let mut values: HashMap<_, _> = profile
            .java()
            .system_properties()
            .value()
            .into_iter()
            .flatten()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        values
            .entry("microedition.configuration".into())
            .or_insert_with(|| {
                profile
                    .java()
                    .configuration()
                    .value()
                    .cloned()
                    .unwrap_or_default()
            });
        values
            .entry("microedition.profiles".into())
            .or_insert_with(|| {
                profile
                    .java()
                    .profile()
                    .value()
                    .cloned()
                    .unwrap_or_default()
            });
        // Profiles may override the deterministic locale fallback.
        values
            .entry("microedition.locale".into())
            .or_insert_with(|| "en-US".into());
        // CLDC defaults to ISO-8859-1 when the profile supplies no encoding.
        values
            .entry("microedition.encoding".into())
            .or_insert_with(|| "ISO-8859-1".into());
        if let Some(version) = profile.m3g().and_then(|m3g| m3g.version().value()) {
            values
                .entry("microedition.m3g.version".into())
                .or_insert_with(|| version.clone());
        }
        values.retain(|_, value| !value.is_empty());
        Self(values)
    }
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }
}

/// Registers bootstrap natives which do not require direct heap access.
pub fn register_core_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    native_registration::register_all(registry)
}

fn cldc_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Api, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/cldc/mod.rs"]
mod tests;
