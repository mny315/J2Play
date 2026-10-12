use super::{
    ArrayAccessKind, ArrayKind, Constant, EmuError, Handle, HashMap, Heap, HeapError, M3gState,
    MANAGED_HEAP_LIMIT_MESSAGE, Micro3dState, Value, ValueKind, require_kind, vm_error,
};

pub(super) fn reject_canonical_string_mutation(
    handle: Handle,
    string_values: &super::StringValues,
    interned_strings: &HashMap<Vec<u16>, Handle>,
) -> Result<(), EmuError> {
    if string_values
        .get(&handle)
        .and_then(|units| interned_strings.get(units))
        == Some(&handle)
    {
        return Err(vm_error(
            "illegal-state",
            "canonical java/lang/String payload is immutable",
        ));
    }
    Ok(())
}
pub(super) fn reference_component(descriptor: &str) -> Option<&str> {
    if descriptor.starts_with('[') {
        Some(descriptor)
    } else {
        descriptor
            .strip_prefix('L')
            .and_then(|name| name.strip_suffix(';'))
    }
}

pub(super) fn slot_count(v: &[Value]) -> usize {
    v.iter().map(|v| v.slots()).sum()
}
pub(super) fn estimate_constant_pool(constants: &[Option<Constant>]) -> Result<usize, EmuError> {
    constants.iter().try_fold(
        constants
            .len()
            .checked_mul(std::mem::size_of::<Option<Constant>>())
            .ok_or_else(|| vm_error("memory-limit", "constant pool size overflow"))?,
        |bytes, constant| {
            let dynamic = match constant {
                Some(Constant::Utf8(value)) => value.len(),
                Some(Constant::Utf16(value)) => std::mem::size_of::<classfile::Utf16Constant>()
                    .checked_add(value.text().len())
                    .and_then(|bytes| {
                        value
                            .units()
                            .len()
                            .checked_mul(std::mem::size_of::<u16>())
                            .and_then(|units| bytes.checked_add(units))
                    })
                    .ok_or_else(|| vm_error("memory-limit", "constant pool size overflow"))?,
                _ => 0,
            };
            bytes
                .checked_add(dynamic)
                .ok_or_else(|| vm_error("memory-limit", "constant pool size overflow"))
        },
    )
}
pub(super) fn default_value(kind: ValueKind) -> Value {
    match kind {
        ValueKind::Int => Value::Int(0),
        ValueKind::Long => Value::Long(0),
        ValueKind::Float => Value::Float(0.0),
        ValueKind::Double => Value::Double(0.0),
        ValueKind::Reference => Value::Reference(None),
    }
}
pub(super) fn value_reference(value: &Value) -> Option<Handle> {
    if let Value::Reference(handle) = value {
        *handle
    } else {
        None
    }
}

pub(super) fn heap_error(error: HeapError) -> EmuError {
    match error {
        HeapError::LimitExceeded => {
            vm_error(super::MANAGED_HEAP_LIMIT_CODE, MANAGED_HEAP_LIMIT_MESSAGE)
        }
        HeapError::MetadataLimitExceeded => vm_error(
            "heap-metadata-limit",
            "Java type metadata exceeds the host memory budget",
        ),
        HeapError::NegativeArraySize => {
            vm_error("negative-array-size-exception", "negative array length")
        }
        HeapError::Bounds => vm_error(
            "array-index-out-of-bounds-exception",
            "array index is outside bounds",
        ),
        HeapError::TypeMismatch => {
            vm_error("array-store-exception", "heap value has incompatible type")
        }
        HeapError::InvalidHandle => vm_error("invalid-reference", "stale or invalid heap handle"),
    }
}

#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn array_opcode_error(error: HeapError) -> EmuError {
    if error == HeapError::TypeMismatch {
        vm_error(
            "type-mismatch",
            "array opcode does not match component type",
        )
    } else {
        heap_error(error)
    }
}

pub(super) fn collect_native_heap(
    heap: &mut Heap,
    m3g: &M3gState,
    micro3d: &Micro3dState,
    timer_threads: &HashMap<Handle, Handle>,
    roots: impl IntoIterator<Item = Handle>,
) {
    // Java fields and arrays can lead to native-backed wrappers, whose native
    // ownership edges can in turn lead back to more Java objects. Resolve that
    // fixed point only when collection is actually required; `roots()` is also
    // called for every native method and must remain a cheap snapshot.
    heap.collect_with_external_edges(roots, |frontier| {
        let mut edges = m3g
            .runtime
            .guest_closure(frontier.iter().map(|handle| handle.to_raw()));
        edges.extend(
            micro3d
                .runtime
                .guest_closure(frontier.iter().map(|handle| handle.to_raw())),
        );
        // A live Timer owns its thread. The scheduler's cache must not keep
        // an unreachable Timer alive after its last pending task completes.
        if !timer_threads.is_empty() {
            edges.extend(
                frontier
                    .iter()
                    .filter_map(|timer| timer_threads.get(timer))
                    .map(|thread| thread.to_raw()),
            );
        }
        edges.into_iter().map(Handle::from_raw)
    });
}

#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn require_array_opcode(opcode: u8, value: Value) -> Result<(), EmuError> {
    let expected = match opcode {
        0x2e | 0x33..=0x35 => ValueKind::Int,
        0x2f => ValueKind::Long,
        0x30 => ValueKind::Float,
        0x31 => ValueKind::Double,
        0x32 => ValueKind::Reference,
        _ => return Err(vm_error("unsupported-opcode", "invalid array opcode")),
    };
    require_kind(value, expected)
}
#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn typed_array_access(opcode: u8) -> Result<ArrayAccessKind, EmuError> {
    match opcode {
        0x2e => Ok(ArrayAccessKind::Int),
        0x2f => Ok(ArrayAccessKind::Long),
        0x30 => Ok(ArrayAccessKind::Float),
        0x31 => Ok(ArrayAccessKind::Double),
        0x32 => Ok(ArrayAccessKind::Reference),
        0x33 => Ok(ArrayAccessKind::ByteOrBoolean),
        0x34 => Ok(ArrayAccessKind::Char),
        0x35 => Ok(ArrayAccessKind::Short),
        _ => Err(vm_error("unsupported-opcode", "invalid array opcode")),
    }
}
pub(super) fn primitive_array_kind(tag: u8) -> Result<ArrayKind, EmuError> {
    match tag {
        4 => Ok(ArrayKind::Boolean),
        5 => Ok(ArrayKind::Char),
        6 => Ok(ArrayKind::Float),
        7 => Ok(ArrayKind::Double),
        8 => Ok(ArrayKind::Byte),
        9 => Ok(ArrayKind::Short),
        10 => Ok(ArrayKind::Int),
        11 => Ok(ArrayKind::Long),
        _ => Err(vm_error("invalid-array-type", format!("atype {tag}"))),
    }
}
pub(super) fn array_kind_from_descriptor(descriptor: &str) -> Result<ArrayKind, EmuError> {
    match descriptor {
        "Z" => Ok(ArrayKind::Boolean),
        "B" => Ok(ArrayKind::Byte),
        "C" => Ok(ArrayKind::Char),
        "S" => Ok(ArrayKind::Short),
        "I" => Ok(ArrayKind::Int),
        "J" => Ok(ArrayKind::Long),
        "F" => Ok(ArrayKind::Float),
        "D" => Ok(ArrayKind::Double),
        _ => reference_component(descriptor)
            .filter(|component| !component.is_empty())
            .map(|component| ArrayKind::Reference(component.into()))
            .ok_or_else(|| vm_error("invalid-array-type", descriptor)),
    }
}
pub(super) fn array_descriptor(kind: &ArrayKind) -> std::borrow::Cow<'static, str> {
    match kind {
        ArrayKind::Boolean => "[Z".into(),
        ArrayKind::Byte => "[B".into(),
        ArrayKind::Char => "[C".into(),
        ArrayKind::Short => "[S".into(),
        ArrayKind::Int => "[I".into(),
        ArrayKind::Long => "[J".into(),
        ArrayKind::Float => "[F".into(),
        ArrayKind::Double => "[D".into(),
        ArrayKind::Reference(component) if component.starts_with('[') => {
            format!("[{component}").into()
        }
        ArrayKind::Reference(component) => format!("[L{component};").into(),
    }
}
