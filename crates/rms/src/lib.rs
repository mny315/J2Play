//! Persistent, suite-isolated MIDP Record Management System core.

use diagnostics::{Category, EmuError};
use sha2::{Digest, Sha256};

mod native_api;
mod repository;
mod runtime;
mod snapshot;
mod store;

pub use native_api::register_natives;
pub use repository::Repository;
pub use runtime::Runtime;
pub use store::PersistentStore;

const MAGIC: &[u8; 8] = b"EMU24RMS";
const FORMAT_VERSION: u16 = 1;
const CHECKSUM_BYTES: usize = 32;
const MAX_RECORD_ID: u32 = i32::MAX as u32;

/// Explicit storage bounds. Device-specific values can replace these defaults
/// without changing the on-disk format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// Maximum number of committed stores in one suite namespace.
    pub max_stores_per_suite: usize,
    /// Maximum payload bytes in one record.
    pub max_record_bytes: usize,
    /// Maximum encoded bytes in one store, including format overhead.
    pub max_store_bytes: usize,
    /// Maximum encoded bytes across all committed stores in one suite.
    pub max_suite_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_stores_per_suite: 256,
            max_record_bytes: 1024 * 1024,
            max_store_bytes: 8 * 1024 * 1024,
            max_suite_bytes: 32 * 1024 * 1024,
        }
    }
}

/// Stable owner identity used to isolate RMS data between `MIDlet` suites.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuiteId {
    vendor: String,
    name: String,
}

impl SuiteId {
    /// Creates an identity from the mandatory suite vendor and name.
    ///
    /// # Errors
    /// Returns `invalid-suite` when either component is empty.
    pub fn new(vendor: impl Into<String>, name: impl Into<String>) -> Result<Self, EmuError> {
        let value = Self {
            vendor: vendor.into(),
            name: name.into(),
        };
        if value.vendor.is_empty() || value.name.is_empty() {
            return Err(rms_error(
                "invalid-suite",
                "suite vendor and name must be non-empty",
            ));
        }
        Ok(value)
    }

    fn digest(&self) -> [u8; 32] {
        framed_digest([self.vendor.as_bytes(), self.name.as_bytes()])
    }
}

fn validate_store_name(name: &str) -> Result<(), EmuError> {
    let units = name.encode_utf16().take(33).count();
    if !(1..=32).contains(&units) {
        return Err(rms_error(
            "store-name",
            "record store name must contain 1 to 32 UTF-16 code units",
        ));
    }
    Ok(())
}

fn validate_record(data: &[u8], limits: Limits) -> Result<(), EmuError> {
    if data.len() > limits.max_record_bytes {
        return Err(rms_error("record-limit", "record exceeds byte limit"));
    }
    Ok(())
}

fn store_digest(name: &str) -> [u8; 32] {
    framed_digest([name.as_bytes()])
}

fn framed_digest<const N: usize>(parts: [&[u8]; N]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    result
}

fn rms_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Api, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/rms/mod.rs"]
mod tests;
