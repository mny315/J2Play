use super::{EmuError, Handle, NativeValue, simple_case_unit, type_error, vm_error};

pub use heap::HeapValue as Value;
pub(super) use heap::ValueKind;

#[derive(Clone, Debug)]
pub(super) struct MethodDescriptor {
    pub(super) parameters: Vec<ValueKind>,
    /// JVM parameter slots, excluding the receiver.
    pub(super) parameter_slots: u8,
    pub(super) returns: Option<ValueKind>,
}

pub(super) fn value_to_native(value: Value) -> NativeValue {
    match value {
        Value::Int(value) => NativeValue::Int(value),
        Value::Long(value) => NativeValue::Long(value),
        Value::Float(value) => NativeValue::Float(value),
        Value::Double(value) => NativeValue::Double(value),
        Value::Reference(value) => NativeValue::Reference(value.map(Handle::to_raw)),
    }
}

pub(super) fn native_to_value(value: &NativeValue) -> Value {
    match value {
        NativeValue::Int(value) => Value::Int(*value),
        NativeValue::Long(value) => Value::Long(*value),
        NativeValue::Float(value) => Value::Float(*value),
        NativeValue::Double(value) => Value::Double(*value),
        NativeValue::Reference(value) => Value::Reference((*value).map(Handle::from_raw)),
    }
}

pub(super) fn reference_argument(args: &[Value], index: usize) -> Result<Handle, EmuError> {
    match args.get(index) {
        Some(Value::Reference(Some(handle))) => Ok(*handle),
        Some(Value::Reference(None)) => Err(vm_error("null-pointer-exception", "null reference")),
        _ => Err(type_error()),
    }
}

pub(super) fn optional_reference_argument(
    args: &[Value],
    index: usize,
) -> Result<Option<Handle>, EmuError> {
    match args.get(index) {
        Some(Value::Reference(value)) => Ok(*value),
        _ => Err(type_error()),
    }
}

pub(super) fn float_argument(args: &[Value], index: usize) -> Result<f32, EmuError> {
    match args.get(index) {
        Some(Value::Float(value)) => Ok(*value),
        _ => Err(type_error()),
    }
}

pub(super) fn finite_float_argument(args: &[Value], index: usize) -> Result<f32, EmuError> {
    let value = float_argument(args, index)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(vm_error(
            "illegal-argument-exception",
            "M3G value must be finite",
        ))
    }
}

pub(super) fn finite_non_negative(args: &[Value], index: usize) -> Result<f32, EmuError> {
    let value = finite_float_argument(args, index)?;
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(vm_error(
            "illegal-argument-exception",
            "M3G value must be finite and non-negative",
        ))
    }
}

pub(super) fn m3g_positive_usize(value: i32) -> Result<usize, EmuError> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value != 0)
        .ok_or_else(|| vm_error("illegal-argument-exception", "M3G count must be positive"))
}

pub(super) fn m3g_non_negative_usize(value: i32) -> Result<usize, EmuError> {
    usize::try_from(value).map_err(|_| {
        vm_error(
            "illegal-argument-exception",
            "M3G index must be non-negative",
        )
    })
}

pub(super) fn m3g_non_negative_u32(value: i32) -> Result<u32, EmuError> {
    u32::try_from(value).map_err(|_| {
        vm_error(
            "illegal-argument-exception",
            "M3G value must be non-negative",
        )
    })
}

pub(super) fn m3g_bounded_count(value: usize, limit: usize, label: &str) -> Result<(), EmuError> {
    if value > limit {
        return Err(vm_error(
            "resource-limit",
            format!("M3G {label} count exceeds the configured budget"),
        ));
    }
    Ok(())
}

pub(super) fn m3g_bounded_image(width: u32, height: u32, limit: usize) -> Result<usize, EmuError> {
    let pixels = m3g::Image2DState::validate_dimensions(width, height)?;
    m3g_bounded_count(pixels, limit, "texture pixel")?;
    Ok(pixels)
}

pub(super) fn long_argument(args: &[Value], index: usize) -> Result<i64, EmuError> {
    match args.get(index) {
        Some(Value::Long(value)) => Ok(*value),
        _ => Err(type_error()),
    }
}

pub(super) fn int_argument(args: &[Value], index: usize) -> Result<i32, EmuError> {
    match args.get(index) {
        Some(Value::Int(value)) => Ok(*value),
        _ => Err(type_error()),
    }
}

pub(super) fn java_utf16_case_equal(left: u16, right: u16) -> bool {
    left == right
        || simple_case_unit(left, true) == simple_case_unit(right, true)
        || simple_case_unit(left, false) == simple_case_unit(right, false)
}
