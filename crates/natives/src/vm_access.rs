//! VM-owned heap and scheduler access during a native invocation.

use super::native_error;
use diagnostics::EmuError;

/// Heap and scheduler access for a native invocation, supplied by the VM around
/// the active [`crate::HostServices`] object.
///
/// Operations reject invalid references and mismatched Java types, and report
/// allocation or scheduler limits through `EmuError`. Fallible defaults reject access
/// with `native-context`; range reads return `None` for invalid bounds.
#[allow(clippy::missing_errors_doc)]
pub trait VmAccess {
    /// Returns the first Java caller outside the active native's declaring class,
    /// skipping helper frames in that class for caller-sensitive APIs.
    fn native_caller_class(&self) -> Option<String> {
        None
    }

    /// Resolves an opaque VM reference as a Java string.
    fn read_java_string(&self, _reference: u64) -> Result<String, EmuError> {
        Err(native_error(
            "native-context",
            "Java string access is unavailable in this context",
        ))
    }

    /// Borrows a Java string without copying or losing unpaired UTF-16 surrogates.
    /// The borrow ends before any mutable VM operation can replace or collect it.
    fn read_java_utf16(&self, _reference: u64) -> Result<&[u16], EmuError> {
        Err(native_error(
            "native-context",
            "Java UTF-16 access is unavailable in this context",
        ))
    }

    /// Replaces the UTF-16 payload of an already allocated `java/lang/String`.
    /// This exists only for standard String constructors; normal strings are
    /// created through `intern_java_utf16`.
    /// Takes ownership of the exact-sized buffer so the VM can retain it directly.
    #[allow(clippy::boxed_local)] // Implementations retain the owned payload; this default rejects it.
    fn write_java_utf16(&mut self, _reference: u64, _value: Box<[u16]>) -> Result<(), EmuError> {
        Err(native_error(
            "native-context",
            "Java UTF-16 mutation is unavailable in this context",
        ))
    }

    /// Interns a Java string and returns its opaque VM reference.
    fn intern_java_string(&mut self, _value: &str) -> Result<u64, EmuError> {
        Err(native_error(
            "native-context",
            "Java string allocation is unavailable in this context",
        ))
    }

    /// Reads a primitive `char[]` as UTF-16 code units.
    fn read_java_char_array(&self, _reference: u64) -> Result<Vec<u16>, EmuError> {
        Err(native_error(
            "native-context",
            "Java char array access is unavailable in this context",
        ))
    }

    /// Reads only the requested range of a Java `char[]` as exact UTF-16 units.
    /// Returns `None` for a negative or out-of-bounds range.
    fn read_java_char_array_range(
        &self,
        reference: u64,
        offset: i32,
        length: i32,
    ) -> Result<Option<Vec<u16>>, EmuError> {
        let units = self.read_java_char_array(reference)?;
        let range = usize::try_from(offset).ok().and_then(|start| {
            usize::try_from(length)
                .ok()
                .and_then(|length| start.checked_add(length))
                .map(|end| start..end)
        });
        Ok(range.and_then(|range| units.get(range).map(<[u16]>::to_vec)))
    }

    /// Reads a primitive Java `int[]` without exposing heap handles.
    fn read_java_int_array(&self, _reference: u64) -> Result<Vec<i32>, EmuError> {
        Err(native_error(
            "native-context",
            "Java int array access is unavailable in this context",
        ))
    }

    /// Interns arbitrary Java UTF-16, including unpaired surrogate code units.
    fn intern_java_utf16(&mut self, _value: &[u16]) -> Result<u64, EmuError> {
        Err(native_error(
            "native-context",
            "Java UTF-16 allocation is unavailable in this context",
        ))
    }

    /// Returns the represented class name for a VM `java/lang/Class` object.
    fn read_java_class(&self, _reference: u64) -> Result<String, EmuError> {
        Err(native_error(
            "native-context",
            "Java class object access is unavailable in this context",
        ))
    }

    /// Interns the `java/lang/Class` object representing a loaded class.
    fn intern_java_class(&mut self, _name: &str) -> Result<u64, EmuError> {
        Err(native_error(
            "native-context",
            "Java class object allocation is unavailable in this context",
        ))
    }

    /// Allocates a Java `byte[]` initialized from host bytes.
    fn allocate_java_byte_array(&mut self, _value: &[u8]) -> Result<u64, EmuError> {
        Err(native_error(
            "native-context",
            "Java byte array allocation is unavailable in this context",
        ))
    }

    /// Allocates a Java `long[]` initialized from host values.
    fn allocate_java_long_array(&mut self, _value: &[i64]) -> Result<u64, EmuError> {
        Err(native_error(
            "native-context",
            "Java long array allocation is unavailable in this context",
        ))
    }

    /// Reads a primitive Java `byte[]`.
    fn read_java_byte_array(&self, _reference: u64) -> Result<Vec<u8>, EmuError> {
        Err(native_error(
            "native-context",
            "Java byte array access is unavailable in this context",
        ))
    }

    /// Reads only the requested range of a Java `byte[]`.
    /// Returns `None` for a negative or out-of-bounds range.
    fn read_java_byte_array_range(
        &self,
        reference: u64,
        offset: i32,
        length: i32,
    ) -> Result<Option<Vec<u8>>, EmuError> {
        let bytes = self.read_java_byte_array(reference)?;
        let range = usize::try_from(offset).ok().and_then(|start| {
            usize::try_from(length)
                .ok()
                .and_then(|length| start.checked_add(length))
                .map(|end| start..end)
        });
        Ok(range.and_then(|range| bytes.get(range).map(<[u8]>::to_vec)))
    }

    /// Allocates a Java `int[]` initialized from host values.
    fn allocate_java_int_array(&mut self, _value: &[i32]) -> Result<u64, EmuError> {
        Err(native_error(
            "native-context",
            "Java int array allocation is unavailable in this context",
        ))
    }

    /// Formats one captured Java frame, returning `None` after the last frame.
    /// Consumers can print frames without retaining the complete formatted stack.
    fn read_throwable_trace_frame(
        &self,
        _reference: u64,
        _index: usize,
    ) -> Result<Option<String>, EmuError> {
        Err(native_error(
            "native-context",
            "Throwable trace access is unavailable in this context",
        ))
    }

    /// Captures the current Java call stack for a Throwable reference.
    fn capture_throwable_trace(&mut self, _reference: u64) -> Result<(), EmuError> {
        Err(native_error(
            "native-context",
            "Throwable trace capture is unavailable in this context",
        ))
    }

    /// Registers a Java `TimerTask` with the VM cooperative scheduler.
    fn schedule_timer_task(
        &mut self,
        _timer: u64,
        _task: u64,
        _deadline: i64,
        _period: i64,
        _fixed_rate: bool,
    ) -> Result<(), EmuError> {
        Err(native_error(
            "native-context",
            "Timer scheduling is unavailable in this context",
        ))
    }

    /// Cancels every scheduled task owned by one Java Timer.
    fn cancel_timer(&mut self, _timer: u64) {}

    /// Returns the runtime class name of an opaque VM reference.
    fn object_class(&self, _reference: u64) -> Result<String, EmuError> {
        Err(native_error(
            "native-context",
            "object class access is unavailable in this context",
        ))
    }

    /// Reads a reference-valued instance field by its runtime key.
    fn read_reference_field(&self, _reference: u64, _field: &str) -> Result<Option<u64>, EmuError> {
        Err(native_error(
            "native-context",
            "object field access is unavailable in this context",
        ))
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/natives/vm_access.rs"]
mod tests;
