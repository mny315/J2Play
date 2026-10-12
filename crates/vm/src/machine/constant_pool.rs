use super::{Constant, EmuError, MethodDescriptor, MethodKey, ValueKind, constant_utf8, vm_error};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FieldRef {
    pub(super) class: String,
    pub(super) name: String,
    pub(super) descriptor: String,
}
impl FieldRef {
    pub(super) fn id(&self) -> String {
        format!("{}::{}:{}", self.class, self.name, self.descriptor)
    }
}

// Native adapters use the readable Owner.name:descriptor form. Escape its
// member-name delimiter (and the escape itself) so distinct valid JVM names
// cannot share a heap/static-field key. Class names cannot contain '.'.
pub(super) fn field_key(class: &str, name: &str, descriptor: &str) -> String {
    let escapes = name
        .bytes()
        .filter(|byte| matches!(byte, b':' | b'\\'))
        .count();
    let mut key = String::with_capacity(class.len() + name.len() + descriptor.len() + 2 + escapes);
    key.push_str(class);
    key.push('.');
    if escapes == 0 {
        key.push_str(name);
    } else {
        for character in name.chars() {
            if matches!(character, ':' | '\\') {
                key.push('\\');
            }
            key.push(character);
        }
    }
    key.push(':');
    key.push_str(descriptor);
    key
}

pub(super) fn resolve_any_method(cp: &[Option<Constant>], i: u16) -> Result<MethodKey, EmuError> {
    let (class_index, name_and_type_index) = match cp.get(usize::from(i)).and_then(Option::as_ref) {
        Some(
            Constant::Methodref {
                class_index,
                name_and_type_index,
            }
            | Constant::InterfaceMethodref {
                class_index,
                name_and_type_index,
            },
        ) => (*class_index, *name_and_type_index),
        _ => return Err(vm_error("invalid-method-ref", format!("constant #{i}"))),
    };
    let class = resolve_class(cp, class_index)?;
    let Some(Constant::NameAndType {
        name_index,
        descriptor_index,
    }) = cp
        .get(usize::from(name_and_type_index))
        .and_then(Option::as_ref)
    else {
        return Err(vm_error("invalid-method-ref", "invalid name/type"));
    };
    Ok(MethodKey {
        class: class.to_owned(),
        name: constant_utf8(cp, *name_index)?.to_owned(),
        descriptor: constant_utf8(cp, *descriptor_index)?.to_owned(),
    })
}
pub(super) fn resolve_class(cp: &[Option<Constant>], index: u16) -> Result<&str, EmuError> {
    let Some(Constant::Class { name_index }) = cp.get(usize::from(index)).and_then(Option::as_ref)
    else {
        return Err(vm_error("invalid-class-ref", format!("constant #{index}")));
    };
    constant_utf8(cp, *name_index)
}
pub(super) fn resolve_field(cp: &[Option<Constant>], index: u16) -> Result<FieldRef, EmuError> {
    let Some(Constant::Fieldref {
        class_index,
        name_and_type_index,
    }) = cp.get(usize::from(index)).and_then(Option::as_ref)
    else {
        return Err(vm_error("invalid-field-ref", format!("constant #{index}")));
    };
    let class = resolve_class(cp, *class_index)?;
    let Some(Constant::NameAndType {
        name_index,
        descriptor_index,
    }) = cp
        .get(usize::from(*name_and_type_index))
        .and_then(Option::as_ref)
    else {
        return Err(vm_error("invalid-field-ref", "invalid name/type"));
    };
    Ok(FieldRef {
        class: class.to_owned(),
        name: constant_utf8(cp, *name_index)?.to_owned(),
        descriptor: constant_utf8(cp, *descriptor_index)?.to_owned(),
    })
}
pub(super) fn parse_method_descriptor(d: &str) -> Result<MethodDescriptor, EmuError> {
    let b = d.as_bytes();
    if b.first() != Some(&b'(') {
        return Err(vm_error("invalid-descriptor", d));
    }
    let mut i = 1;
    let mut parameters = Vec::new();
    let mut parameter_slots = 0_u8;
    while i < b.len() && b[i] != b')' {
        let kind = parse_descriptor_type(b, &mut i)?;
        let slots = if matches!(kind, ValueKind::Long | ValueKind::Double) {
            2
        } else {
            1
        };
        parameter_slots = parameter_slots
            .checked_add(slots)
            .ok_or_else(|| vm_error("invalid-descriptor", "method parameters exceed 255 slots"))?;
        parameters.push(kind);
    }
    if i >= b.len() {
        return Err(vm_error("invalid-descriptor", d));
    }
    i += 1;
    let returns = if b.get(i) == Some(&b'V') {
        i += 1;
        None
    } else {
        Some(parse_descriptor_type(b, &mut i)?)
    };
    if i != b.len() {
        return Err(vm_error("invalid-descriptor", d));
    }
    Ok(MethodDescriptor {
        parameters,
        parameter_slots,
        returns,
    })
}

pub(super) fn parse_descriptor_type(
    bytes: &[u8],
    position: &mut usize,
) -> Result<ValueKind, EmuError> {
    let mut dimensions = 0;
    while bytes.get(*position) == Some(&b'[') {
        dimensions += 1;
        if dimensions > 255 {
            return Err(vm_error(
                "invalid-descriptor",
                "array has more than 255 dimensions",
            ));
        }
        *position += 1;
    }
    let tag = *bytes
        .get(*position)
        .ok_or_else(|| vm_error("invalid-descriptor", "truncated descriptor"))?;
    *position += 1;
    let kind = match tag {
        b'B' | b'C' | b'I' | b'S' | b'Z' => ValueKind::Int,
        b'J' => ValueKind::Long,
        b'F' => ValueKind::Float,
        b'D' => ValueKind::Double,
        b'L' => {
            let rest = bytes
                .get(*position..)
                .ok_or_else(|| vm_error("invalid-descriptor", "truncated object type"))?;
            let end = rest
                .iter()
                .position(|byte| *byte == b';')
                .ok_or_else(|| vm_error("invalid-descriptor", "unterminated object type"))?;
            if end == 0 {
                return Err(vm_error("invalid-descriptor", "empty object type"));
            }
            *position += end + 1;
            ValueKind::Reference
        }
        _ => {
            return Err(vm_error(
                "invalid-descriptor",
                format!("invalid descriptor tag 0x{tag:02x}"),
            ));
        }
    };
    Ok(if dimensions == 0 {
        kind
    } else {
        ValueKind::Reference
    })
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/constant_pool.rs"]
mod tests;
