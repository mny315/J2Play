//! Type-safe native method registry and host bridge boundary.

mod character;
mod character_encoding;
mod frame_rate;
mod host;
mod lifecycle;
mod vm_access;
pub use character::{character_digit, simple_case_mapping, unicode_digit};
pub use character_encoding::CharacterEncoding;
pub use frame_rate::FrameRateControl;
pub use host::{
    GcfFileMetadata, GcfHttpRequest, GcfHttpResponse, HostServices, ManagedHeapLimitDecision,
    ManagedHeapLimitNotice, RmsMetadataField, VibrationRequest, VmTelemetry,
};
pub use lifecycle::{
    LifecycleCallback, LifecycleOutcome, MidletLifecycleEvent, MidletNotification,
};
pub use vm_access::VmAccess;

use diagnostics::{Category, EmuError};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

/// A value crossing the VM/native boundary. References remain opaque handles.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Reference(Option<u64>),
}

/// Fully-qualified method identity shared by VM linkage and native dispatch.
/// Class names use the JVM internal form.
#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct MethodSignature {
    pub class: String,
    pub name: String,
    pub descriptor: String,
}

/// Method identity used when registering and invoking a native implementation.
pub type NativeSignature = MethodSignature;

impl MethodSignature {
    #[must_use]
    pub fn new(
        class: impl Into<String>,
        name: impl Into<String>,
        descriptor: impl Into<String>,
    ) -> Self {
        Self {
            class: class.into(),
            name: name.into(),
            descriptor: descriptor.into(),
        }
    }

    #[must_use]
    pub fn display(&self) -> String {
        format!("{}::{}{}", self.class, self.name, self.descriptor)
    }
}

/// Complete context visible to a registered native method.
///
/// This composition keeps host capabilities separate from VM internals while
/// preserving one object-safe invocation boundary for the native registry.
pub trait NativeContext: HostServices + VmAccess {}

impl<T: HostServices + VmAccess + ?Sized> NativeContext for T {}

pub type NativeResult = Result<Option<NativeValue>, EmuError>;
type NativeMethod = dyn Fn(&mut dyn NativeContext, &[NativeValue]) -> NativeResult + Send + Sync;

/// Immutable-after-construction registry used by a VM instance.
#[derive(Default)]
pub struct NativeRegistry {
    methods: HashMap<NativeSignature, Box<NativeMethod>>,
}

impl NativeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one exact signature and rejects accidental replacement.
    ///
    /// # Errors
    /// Returns `duplicate-native` if the complete signature is already present.
    pub fn register<F>(&mut self, signature: NativeSignature, method: F) -> Result<(), EmuError>
    where
        F: Fn(&mut dyn NativeContext, &[NativeValue]) -> NativeResult + Send + Sync + 'static,
    {
        match self.methods.entry(signature) {
            Entry::Vacant(entry) => {
                entry.insert(Box::new(method));
            }
            Entry::Occupied(entry) => {
                return Err(native_error(
                    "duplicate-native",
                    format!(
                        "native method already registered: {}",
                        entry.key().display()
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Invokes an exact signature. Missing-method diagnostics always include it.
    ///
    /// # Errors
    /// Returns `unsupported-native` for an unknown signature or propagates the
    /// registered implementation's categorized error.
    pub fn invoke(
        &self,
        signature: &NativeSignature,
        context: &mut dyn NativeContext,
        arguments: &[NativeValue],
    ) -> NativeResult {
        let method = self.methods.get(signature).ok_or_else(|| {
            native_error(
                "unsupported-native",
                format!("unsupported native method: {}", signature.display()),
            )
        })?;
        method(context, arguments)
    }

    #[must_use]
    pub fn contains(&self, signature: &NativeSignature) -> bool {
        self.methods.contains_key(signature)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.methods.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.methods.is_empty()
    }
}

fn native_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Api, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/natives/mod.rs"]
mod tests;
