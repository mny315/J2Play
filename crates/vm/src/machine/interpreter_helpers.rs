use super::{
    Attribute, ClassFile, Constant, EmuError, Method, Value, ValueKind, default_value, type_error,
    vm_error,
};
use std::borrow::Cow;

#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn get_local(locals: &[Option<Value>], i: usize) -> Result<Value, EmuError> {
    locals
        .get(i)
        .and_then(|v| *v)
        .ok_or_else(|| vm_error("invalid-local", format!("uninitialized local {i}")))
}

#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn get_local_typed(
    locals: &[Option<Value>],
    index: usize,
    expected: ValueKind,
) -> Result<Value, EmuError> {
    let value = get_local(locals, index)?;
    require_kind(value, expected)?;
    Ok(value)
}

pub(super) fn valid_return_address(method: &Method, value: Value) -> bool {
    let Value::Int(offset) = value else {
        return false;
    };
    usize::try_from(offset)
        .ok()
        .and_then(|offset| method.instruction_index.get(offset))
        .is_some_and(|index| *index != u16::MAX)
}

#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn require_kind(value: Value, expected: ValueKind) -> Result<(), EmuError> {
    if value.kind() == expected {
        Ok(())
    } else {
        Err(type_error())
    }
}

#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn kind_for_typed_opcode(opcode: u8) -> Result<ValueKind, EmuError> {
    match opcode {
        0x15 | 0x1a..=0x1d | 0x36 | 0x3b..=0x3e => Ok(ValueKind::Int),
        0x16 | 0x1e..=0x21 | 0x37 | 0x3f..=0x42 => Ok(ValueKind::Long),
        0x17 | 0x22..=0x25 | 0x38 | 0x43..=0x46 => Ok(ValueKind::Float),
        0x18 | 0x26..=0x29 | 0x39 | 0x47..=0x4a => Ok(ValueKind::Double),
        0x19 | 0x2a..=0x2d => Ok(ValueKind::Reference),
        _ => Err(vm_error(
            "unsupported-opcode",
            format!("opcode 0x{opcode:02x} has no local variable type"),
        )),
    }
}

#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn kind_for_return_opcode(opcode: u8) -> Result<ValueKind, EmuError> {
    match opcode {
        0xac => Ok(ValueKind::Int),
        0xad => Ok(ValueKind::Long),
        0xae => Ok(ValueKind::Float),
        0xaf => Ok(ValueKind::Double),
        0xb0 => Ok(ValueKind::Reference),
        _ => Err(vm_error(
            "unsupported-opcode",
            "not a primitive return opcode",
        )),
    }
}
#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn set_local(locals: &mut [Option<Value>], i: usize, v: Value) -> Result<(), EmuError> {
    if i + v.slots() > locals.len() {
        return Err(vm_error(
            "invalid-local",
            format!("local {i} exceeds max_locals"),
        ));
    }
    if i > 0 && locals[i].is_none() && locals[i - 1].is_some_and(|previous| previous.slots() == 2) {
        locals[i - 1] = None;
    }
    locals[i] = Some(v);
    if v.slots() == 2 {
        locals[i + 1] = None
    }
    Ok(())
}
pub(super) fn constant_value(cp: &[Option<Constant>], i: u16) -> Result<Value, EmuError> {
    match cp.get(usize::from(i)).and_then(Option::as_ref) {
        Some(Constant::Integer(v)) => Ok(Value::Int(*v)),
        Some(Constant::Long(v)) => Ok(Value::Long(*v)),
        Some(Constant::Float(v)) => Ok(Value::Float(f32::from_bits(*v))),
        Some(Constant::Double(v)) => Ok(Value::Double(f64::from_bits(*v))),
        _ => Err(vm_error(
            "invalid-constant",
            format!("constant #{i} is not primitive"),
        )),
    }
}
pub(super) fn constant_field_value(
    class: &ClassFile,
    member: &classfile::Member,
    kind: ValueKind,
) -> Result<(Value, Option<Vec<u16>>), EmuError> {
    let Some(bytes) = member
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            Attribute::Raw { name, bytes, .. } if name == "ConstantValue" => Some(bytes),
            _ => None,
        })
    else {
        return Ok((default_value(kind), None));
    };
    let [high, low] = bytes.as_slice() else {
        return Err(vm_error(
            "invalid-constant-value",
            "invalid ConstantValue length",
        ));
    };
    let index = u16::from_be_bytes([*high, *low]);
    if kind == ValueKind::Reference {
        let Some(Constant::String { string_index }) = class
            .constant_pool
            .get(usize::from(index))
            .and_then(Option::as_ref)
        else {
            return Err(vm_error(
                "invalid-constant-value",
                "reference ConstantValue is not a string",
            ));
        };
        return Ok((
            Value::Reference(None),
            Some(constant_string_units(&class.constant_pool, *string_index)?),
        ));
    }
    let value = constant_value(&class.constant_pool, index)?;
    require_kind(value, kind)?;
    Ok((value, None))
}
pub(super) fn constant_utf8(cp: &[Option<Constant>], index: u16) -> Result<&str, EmuError> {
    cp.get(usize::from(index))
        .and_then(Option::as_ref)
        .and_then(Constant::utf8_text)
        .ok_or_else(|| {
            vm_error(
                "invalid-constant",
                format!("constant #{index} is not UTF-8"),
            )
        })
}

pub(super) fn constant_string_units(
    cp: &[Option<Constant>],
    index: u16,
) -> Result<Vec<u16>, EmuError> {
    cp.get(usize::from(index))
        .and_then(Option::as_ref)
        .and_then(Constant::utf16_units)
        .map(Cow::into_owned)
        .ok_or_else(|| {
            vm_error(
                "invalid-constant",
                format!("constant #{index} is not UTF-8"),
            )
        })
}
pub(super) fn binary(op: u8, a: Value, b: Value) -> Result<Value, EmuError> {
    macro_rules! z {
        ($b:expr) => {
            if $b == 0 {
                return Err(vm_error("arithmetic-exception", "division by zero"));
            }
        };
    }
    Ok(match (a, b) {
        (Value::Int(a), Value::Int(b)) => Value::Int(match op {
            0x60 => a.wrapping_add(b),
            0x64 => a.wrapping_sub(b),
            0x68 => a.wrapping_mul(b),
            0x6c => {
                z!(b);
                a.wrapping_div(b)
            }
            0x70 => {
                z!(b);
                a.wrapping_rem(b)
            }
            _ => return Err(type_error()),
        }),
        (Value::Long(a), Value::Long(b)) => Value::Long(match op {
            0x61 => a.wrapping_add(b),
            0x65 => a.wrapping_sub(b),
            0x69 => a.wrapping_mul(b),
            0x6d => {
                z!(b);
                a.wrapping_div(b)
            }
            0x71 => {
                z!(b);
                a.wrapping_rem(b)
            }
            _ => return Err(type_error()),
        }),
        (Value::Float(a), Value::Float(b)) => Value::Float(match op {
            0x62 => a + b,
            0x66 => a - b,
            0x6a => a * b,
            0x6e => a / b,
            0x72 => a % b,
            _ => return Err(type_error()),
        }),
        (Value::Double(a), Value::Double(b)) => Value::Double(match op {
            0x63 => a + b,
            0x67 => a - b,
            0x6b => a * b,
            0x6f => a / b,
            0x73 => a % b,
            _ => return Err(type_error()),
        }),
        _ => return Err(type_error()),
    })
}
pub(super) fn negate(op: u8, v: Value) -> Result<Value, EmuError> {
    Ok(match (op, v) {
        (0x74, Value::Int(v)) => Value::Int(v.wrapping_neg()),
        (0x75, Value::Long(v)) => Value::Long(v.wrapping_neg()),
        (0x76, Value::Float(v)) => Value::Float(-v),
        (0x77, Value::Double(v)) => Value::Double(-v),
        _ => return Err(type_error()),
    })
}
pub(super) fn bitwise(op: u8, a: Value, b: Value) -> Result<Value, EmuError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(match op {
            0x78 => a.wrapping_shl((b & 31) as u32),
            0x7a => a.wrapping_shr((b & 31) as u32),
            0x7c => ((a as u32) >> (b & 31)) as i32,
            0x7e => a & b,
            0x80 => a | b,
            0x82 => a ^ b,
            _ => return Err(type_error()),
        })),
        (Value::Long(a), Value::Int(b)) => Ok(Value::Long(match op {
            0x79 => a.wrapping_shl((b & 63) as u32),
            0x7b => a.wrapping_shr((b & 63) as u32),
            0x7d => ((a as u64) >> (b & 63)) as i64,
            _ => return Err(type_error()),
        })),
        (Value::Long(a), Value::Long(b)) => Ok(Value::Long(match op {
            0x7f => a & b,
            0x81 => a | b,
            0x83 => a ^ b,
            _ => return Err(type_error()),
        })),
        _ => Err(type_error()),
    }
}
pub(super) fn convert(op: u8, v: Value) -> Result<Value, EmuError> {
    Ok(match (op, v) {
        (0x85, Value::Int(v)) => Value::Long(i64::from(v)),
        (0x86, Value::Int(v)) => Value::Float(v as f32),
        (0x87, Value::Int(v)) => Value::Double(f64::from(v)),
        (0x88, Value::Long(v)) => Value::Int(v as i32),
        (0x89, Value::Long(v)) => Value::Float(v as f32),
        (0x8a, Value::Long(v)) => Value::Double(v as f64),
        // Rust casts truncate toward zero, saturate at the integer bounds,
        // and convert NaN to zero, matching Java's conversion rules.
        (0x8b, Value::Float(v)) => Value::Int(v as i32),
        (0x8c, Value::Float(v)) => Value::Long(v as i64),
        (0x8d, Value::Float(v)) => Value::Double(f64::from(v)),
        (0x8e, Value::Double(v)) => Value::Int(v as i32),
        (0x8f, Value::Double(v)) => Value::Long(v as i64),
        (0x90, Value::Double(v)) => Value::Float(v as f32),
        (0x91, Value::Int(v)) => Value::Int(i32::from(v as i8)),
        (0x92, Value::Int(v)) => Value::Int(i32::from(v as u16)),
        (0x93, Value::Int(v)) => Value::Int(i32::from(v as i16)),
        _ => return Err(type_error()),
    })
}
pub(super) fn compare(op: u8, a: Value, b: Value) -> Result<i32, EmuError> {
    let order = match (op, a, b) {
        (0x94, Value::Long(a), Value::Long(b)) => Some(a.cmp(&b)),
        (0x95 | 0x96, Value::Float(a), Value::Float(b)) => a.partial_cmp(&b),
        (0x97 | 0x98, Value::Double(a), Value::Double(b)) => a.partial_cmp(&b),
        _ => return Err(type_error()),
    };
    Ok(order.map_or(
        if matches!(op, 0x95 | 0x97) { -1 } else { 1 },
        |o| match o {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        },
    ))
}
pub(super) fn test_zero(op: u8, v: i32) -> bool {
    match op {
        0x99 => v == 0,
        0x9a => v != 0,
        0x9b => v < 0,
        0x9c => v >= 0,
        0x9d => v > 0,
        0x9e => v <= 0,
        _ => false,
    }
}
pub(super) fn test_pair(op: u8, a: i32, b: i32) -> bool {
    match op {
        0x9f => a == b,
        0xa0 => a != b,
        0xa1 => a < b,
        0xa2 => a >= b,
        0xa3 => a > b,
        0xa4 => a <= b,
        _ => false,
    }
}
pub(super) fn branch(pc: usize, b: &[u8]) -> Result<usize, EmuError> {
    target(pc, i32::from(i16::from_be_bytes([b[0], b[1]])))
}
pub(super) fn branch_wide(pc: usize, b: &[u8]) -> Result<usize, EmuError> {
    target(pc, i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}
pub(super) fn target(pc: usize, o: i32) -> Result<usize, EmuError> {
    usize::try_from((pc as i64) + i64::from(o))
        .map_err(|_| vm_error("invalid-branch", "branch target outside code"))
}
pub(super) fn read_i32(code: &[u8], at: usize) -> Result<i32, EmuError> {
    let b = code
        .get(at..)
        .and_then(|bytes| bytes.get(..4))
        .ok_or_else(|| vm_error("invalid-switch", "truncated switch"))?;
    Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}
pub(super) fn switch_table(pc: usize, code: &[u8], key: i32) -> Result<usize, EmuError> {
    let a = (pc + 4) & !3;
    let d = read_i32(code, a)?;
    let low = read_i32(code, a + 4)?;
    let high = read_i32(code, a + 8)?;
    let off = if key >= low && key <= high {
        let index = usize::try_from(i64::from(key) - i64::from(low))
            .map_err(|_| vm_error("invalid-switch", "case index is outside usize"))?;
        let at = index
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(a + 12))
            .ok_or_else(|| vm_error("invalid-switch", "case offset overflows"))?;
        read_i32(code, at)?
    } else {
        d
    };
    target(pc, off)
}
pub(super) fn switch_lookup(pc: usize, code: &[u8], key: i32) -> Result<usize, EmuError> {
    let a = (pc + 4) & !3;
    let d = read_i32(code, a)?;
    let n = usize::try_from(read_i32(code, a + 4)?)
        .map_err(|_| vm_error("invalid-switch", "negative pair count"))?;
    let pairs = code
        .get(a + 8..)
        .and_then(|bytes| bytes.as_chunks::<8>().0.get(..n))
        .ok_or_else(|| vm_error("invalid-switch", "truncated switch pairs"))?;
    // Decoding already verifies strictly increasing signed keys. Search the
    // borrowed table directly instead of rescanning every case per execution.
    let offset = pairs
        .binary_search_by_key(&key, |pair| {
            i32::from_be_bytes([pair[0], pair[1], pair[2], pair[3]])
        })
        .map_or(d, |index| {
            let pair = pairs[index];
            i32::from_be_bytes([pair[4], pair[5], pair[6], pair[7]])
        });
    target(pc, offset)
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/interpreter_helpers.rs"]
mod tests;
