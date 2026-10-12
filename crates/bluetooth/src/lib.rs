//! JSR-82 UUID values and profile-driven device properties.
//!
//! Radio discovery and transports remain separate host capabilities. This
//! property registry exposes evidence-bearing handset properties and returns the
//! JSR-82 specified `null` result for properties which a profile does not know.

use device_profile::DeviceProfile;
use diagnostics::{Category, EmuError};
use natives::{NativeRegistry, NativeSignature, NativeValue};
use std::collections::BTreeMap;

mod uuid;

/// Immutable JSR-82 property snapshot selected by a device profile.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Properties(BTreeMap<String, String>);

impl Properties {
    /// Builds the guest-visible property set without adding model-specific
    /// fallback values.
    #[must_use]
    pub fn from_profile(profile: &DeviceProfile) -> Self {
        Self(
            profile
                .java()
                .bluetooth_properties()
                .and_then(|properties| properties.value())
                .cloned()
                .unwrap_or_default(),
        )
    }

    /// Returns one case-sensitive JSR-82 property value.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }
}

/// Registers JSR-82 natives. Callers install these only when the selected
/// profile exposes JSR-82.
///
/// # Errors
///
/// Returns an error if a native signature is already registered.
pub fn register_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    uuid::register_natives(registry)?;
    registry.register(
        NativeSignature::new(
            "javax/bluetooth/LocalDevice",
            "getProperty",
            "(Ljava/lang/String;)Ljava/lang/String;",
        ),
        |context, args| {
            let [NativeValue::Reference(Some(reference))] = args else {
                return Err(bluetooth_error(
                    "null-pointer-exception",
                    "LocalDevice.getProperty expects a non-null property name",
                ));
            };
            let name = context.read_java_string(*reference)?;
            match context.bluetooth_property(&name).map(str::to_owned) {
                Some(value) => Ok(Some(NativeValue::Reference(Some(
                    context.intern_java_string(&value)?,
                )))),
                None => Ok(Some(NativeValue::Reference(None))),
            }
        },
    )
}

fn bluetooth_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Api, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/bluetooth/mod.rs"]
mod tests;
