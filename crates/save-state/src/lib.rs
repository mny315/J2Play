//! Bounded, architecture-independent encoding for automatic resume checkpoints.
//!
//! Owners serialize semantic state only. Program code, host objects, paths,
//! permissions and compiled code are reconstructed by their owners on restore.

use diagnostics::{Category, EmuError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::io::{self, Write};

mod buffer;

/// Maximum uncompressed bytes in one checkpoint component.
pub const MAX_COMPONENT_BYTES: usize = 192 * 1024 * 1024;

/// Serializes an already encoded byte component as a single byte string.
///
/// # Errors
/// Returns the serializer's size, cancellation or output error.
pub fn serialize_bytes<S: serde::Serializer, T: AsRef<[u8]>>(
    bytes: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_bytes(bytes.as_ref())
}

/// Decodes an encoded component as one byte block, without visiting each byte.
/// Postcard byte strings have the same wire format as the original `Vec<u8>`.
///
/// # Errors
/// Returns the deserializer's error for an incomplete or malformed byte string.
pub fn deserialize_bytes<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: From<Vec<u8>>,
{
    struct Bytes;

    impl serde::de::Visitor<'_> for Bytes {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a checkpoint byte string")
        }

        fn visit_bytes<E: serde::de::Error>(self, bytes: &[u8]) -> Result<Self::Value, E> {
            Ok(bytes.to_vec())
        }

        fn visit_byte_buf<E: serde::de::Error>(self, bytes: Vec<u8>) -> Result<Self::Value, E> {
            Ok(bytes)
        }
    }

    deserializer.deserialize_byte_buf(Bytes).map(T::from)
}

/// Serializes optional binary data in blocks while preserving its presence tag.
///
/// # Errors
/// Returns the serializer's size, cancellation or output error.
pub fn serialize_optional_bytes<S: serde::Serializer, T: AsRef<[u8]>>(
    bytes: &Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Bytes<'a>(#[serde(serialize_with = "serialize_bytes")] &'a [u8]);

    bytes
        .as_ref()
        .map(|bytes| Bytes(bytes.as_ref()))
        .serialize(serializer)
}

/// Decodes optional binary data as a block, preserving absent and empty values.
///
/// # Errors
/// Returns the deserializer's error for an invalid tag, length or byte payload.
pub fn deserialize_optional_bytes<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: From<Vec<u8>>,
{
    #[derive(Deserialize)]
    struct Bytes(#[serde(deserialize_with = "deserialize_bytes")] Vec<u8>);

    Option::<Bytes>::deserialize(deserializer).map(|bytes| bytes.map(|bytes| T::from(bytes.0)))
}

/// Encodes a component without allowing the output buffer to grow past its bound.
///
/// # Errors
/// Returns a diagnostic if the component exceeds its byte limit or cannot be encoded.
pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, EmuError> {
    encode_bounded(value, MAX_COMPONENT_BYTES)
}

/// Encodes a component under a smaller owner-specific byte limit.
///
/// # Errors
/// Returns a diagnostic if the component exceeds its byte limit or cannot be encoded.
pub fn encode_bounded<T: Serialize + ?Sized>(value: &T, limit: usize) -> Result<Vec<u8>, EmuError> {
    encode_cancellable(value, limit, &|| false)
}

/// Encodes a component while checking the host's cancellation/deadline signal.
///
/// # Errors
/// Returns an error for cancellation, exceeded byte limits or encoding failures.
pub fn encode_cancellable<T: Serialize + ?Sized>(
    value: &T,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EmuError> {
    buffer::encode(value, limit, cancelled)
}

/// Streams bounded state into an owner's writer without another full-size copy.
///
/// # Errors
/// Returns an error for cancellation, exceeded byte limits or writing failures.
pub fn write<T: Serialize + ?Sized>(
    value: &T,
    writer: impl Write,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EmuError> {
    let mut writer = BoundedWriter {
        writer,
        count: 0,
        checked: 0,
        limit: limit.min(MAX_COMPONENT_BYTES),
        cancelled,
    };
    if cancelled() {
        return Err(error(
            "checkpoint-cancelled",
            "Automatic saving was interrupted.",
        ));
    }
    postcard::to_io(value, &mut writer).map_err(|_| {
        error(
            "checkpoint-encode",
            "Automatic saving exceeded its time or size limit, or could not write its data.",
        )
    })?;
    Ok(writer.count)
}

/// Decodes a complete component; trailing bytes and oversized inputs are rejected.
/// The enclosing checkpoint must pass its checksum and compatibility checks first.
/// Owners validate semantic invariants before making decoded state active.
///
/// # Errors
/// Returns a diagnostic for an oversized, truncated or malformed component.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, EmuError> {
    if bytes.len() > MAX_COMPONENT_BYTES {
        return Err(error(
            "checkpoint-size",
            "The game checkpoint is too large.",
        ));
    }
    let (value, rest) = postcard::take_from_bytes(bytes)
        .map_err(|_| error("checkpoint-decode", "The game checkpoint is damaged."))?;
    if !rest.is_empty() {
        return Err(error(
            "checkpoint-decode",
            "The game checkpoint has trailing data.",
        ));
    }
    Ok(value)
}

/// A stable diagnostic shared by checkpoint owners.
#[must_use]
pub fn error(code: &'static str, message: &str) -> EmuError {
    EmuError::new(Category::Vm, code, message)
}

struct BoundedWriter<'a, W> {
    writer: W,
    count: usize,
    checked: usize,
    limit: usize,
    cancelled: &'a dyn Fn() -> bool,
}

impl<W: Write> Write for BoundedWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.count) {
            return Err(io::Error::other("checkpoint byte limit exceeded"));
        }
        for chunk in bytes.chunks(64 * 1024) {
            if self.count.saturating_sub(self.checked) >= 64 * 1024 {
                if (self.cancelled)() {
                    return Err(io::Error::other("checkpoint cancelled"));
                }
                self.checked = self.count;
            }
            self.writer.write_all(chunk)?;
            self.count += chunk.len();
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/save-state/mod.rs"]
mod tests;
