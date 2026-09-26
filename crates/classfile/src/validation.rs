//! Class/member flags, names and JVM descriptor structure.

use diagnostics::EmuError;

use crate::{Attribute, MemberKind, parse_error};

pub(super) fn validate_class_constant_name(name: &str, offset: usize) -> Result<(), EmuError> {
    if name.starts_with('[') {
        return validate_field_descriptor(name, offset);
    }
    if name.is_empty()
        || name.starts_with('/')
        || name.ends_with('/')
        || name.contains("//")
        || name
            .chars()
            .any(|character| matches!(character, '.' | ';' | '['))
    {
        return Err(parse_error(
            "invalid-class-name",
            offset,
            format!("invalid internal class name {name:?}"),
        ));
    }
    Ok(())
}

pub(super) fn validate_class_flags(flags: u16, offset: usize) -> Result<(), EmuError> {
    validate_visibility(flags, offset, "class")?;
    if flags & 0x0006 != 0 {
        return Err(parse_error(
            "invalid-access-flags",
            offset,
            "class cannot be private or protected",
        ));
    }
    // Old preprocessors sometimes omitted ACC_ABSTRACT from ACC_INTERFACE.
    // Interface semantics already imply abstractness; normalize it in the VM.
    if flags & 0x0010 != 0 && flags & 0x0400 != 0 {
        return Err(parse_error(
            "invalid-access-flags",
            offset,
            "class cannot be both final and abstract",
        ));
    }
    Ok(())
}

pub(super) fn validate_member_flags(
    kind: MemberKind,
    flags: u16,
    flags_offset: usize,
) -> Result<(), EmuError> {
    validate_visibility(flags, flags_offset, "member")?;
    match kind {
        MemberKind::Field => {
            if flags & 0x0010 != 0 && flags & 0x0040 != 0 {
                return Err(parse_error(
                    "invalid-access-flags",
                    flags_offset,
                    "field cannot be both final and volatile",
                ));
            }
        }
        MemberKind::Method => {
            const ABSTRACT_INCOMPATIBLE: u16 = 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0100 | 0x0800;
            if flags & 0x0400 != 0 && flags & ABSTRACT_INCOMPATIBLE != 0 {
                return Err(parse_error(
                    "invalid-access-flags",
                    flags_offset,
                    "abstract method has incompatible access flags",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_special_method(
    flags: u16,
    name: &str,
    descriptor: &str,
    offset: usize,
) -> Result<(), EmuError> {
    if name == "<init>" && (flags & 0x0008 != 0 || !descriptor.ends_with(")V")) {
        return Err(parse_error(
            "invalid-constructor",
            offset,
            "instance constructor must be non-static and return void",
        ));
    }
    if name == "<clinit>" && (flags & 0x0008 == 0 || descriptor != "()V") {
        return Err(parse_error(
            "invalid-class-initializer",
            offset,
            "class initializer must be static with descriptor ()V",
        ));
    }
    Ok(())
}

fn validate_visibility(flags: u16, offset: usize, target: &str) -> Result<(), EmuError> {
    if (flags & 0x0007).count_ones() > 1 {
        return Err(parse_error(
            "invalid-access-flags",
            offset,
            format!("{target} has conflicting visibility flags"),
        ));
    }
    Ok(())
}

pub(super) fn validate_member_attributes(
    kind: MemberKind,
    flags: u16,
    attributes: &[Attribute],
    offset: usize,
) -> Result<(), EmuError> {
    let code_count = attributes
        .iter()
        .filter(|attribute| matches!(attribute, Attribute::Code(_)))
        .count();
    match kind {
        MemberKind::Field if code_count != 0 => Err(parse_error(
            "invalid-attribute-location",
            offset,
            "field cannot have a Code attribute",
        )),
        MemberKind::Method => {
            let has_no_code = flags & (0x0100 | 0x0400) != 0;
            if (has_no_code && code_count != 0) || (!has_no_code && code_count != 1) {
                return Err(parse_error(
                    "invalid-code-count",
                    offset,
                    if has_no_code {
                        "abstract or native method cannot have Code"
                    } else {
                        "concrete method must have exactly one Code attribute"
                    },
                ));
            }
            Ok(())
        }
        MemberKind::Field => Ok(()),
    }
}

pub(super) fn validate_unqualified_name(
    name: &str,
    method: bool,
    offset: usize,
) -> Result<(), EmuError> {
    let special = matches!(name, "<init>" | "<clinit>");
    if name.is_empty()
        || name.contains(['.', ';', '[', '/'])
        || (method && !special && name.contains(['<', '>']))
    {
        return Err(parse_error(
            "invalid-member-name",
            offset,
            format!("invalid member name {name:?}"),
        ));
    }
    Ok(())
}

pub(super) fn validate_field_descriptor(descriptor: &str, offset: usize) -> Result<(), EmuError> {
    let mut position = 0;
    if !parse_field_type(descriptor.as_bytes(), &mut position, false)
        || position != descriptor.len()
    {
        return Err(invalid_descriptor(descriptor, offset, "field"));
    }
    Ok(())
}

pub(super) fn validate_method_descriptor(descriptor: &str, offset: usize) -> Result<u8, EmuError> {
    let bytes = descriptor.as_bytes();
    let mut position = 0;
    if bytes.get(position) != Some(&b'(') {
        return Err(invalid_descriptor(descriptor, offset, "method"));
    }
    position += 1;
    let mut parameter_slots = 0_u8;
    while bytes.get(position).is_some_and(|byte| *byte != b')') {
        // JVMS 4.3.3: long/double use two slots; arrays remain references.
        let slots = if matches!(bytes[position], b'J' | b'D') {
            2
        } else {
            1
        };
        parameter_slots = parameter_slots
            .checked_add(slots)
            .ok_or_else(|| invalid_descriptor(descriptor, offset, "method"))?;
        if !parse_field_type(bytes, &mut position, false) {
            return Err(invalid_descriptor(descriptor, offset, "method"));
        }
    }
    if bytes.get(position) != Some(&b')') {
        return Err(invalid_descriptor(descriptor, offset, "method"));
    }
    position += 1;
    if !parse_field_type(bytes, &mut position, true) || position != bytes.len() {
        return Err(invalid_descriptor(descriptor, offset, "method"));
    }
    Ok(parameter_slots)
}

fn invalid_descriptor(descriptor: &str, offset: usize, kind: &str) -> EmuError {
    parse_error(
        "invalid-descriptor",
        offset,
        format!("invalid {kind} descriptor {descriptor:?}"),
    )
}

fn parse_field_type(bytes: &[u8], position: &mut usize, allow_void: bool) -> bool {
    let mut array_depth = 0_usize;
    while bytes.get(*position) == Some(&b'[') {
        array_depth += 1;
        if array_depth > 255 {
            return false;
        }
        *position += 1;
    }
    let Some(byte) = bytes.get(*position) else {
        return false;
    };
    match byte {
        b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' => {
            *position += 1;
            true
        }
        b'V' if allow_void && array_depth == 0 => {
            *position += 1;
            true
        }
        b'L' => {
            *position += 1;
            let start = *position;
            while bytes.get(*position).is_some_and(|value| *value != b';') {
                *position += 1;
            }
            let name = &bytes[start..*position];
            if name.is_empty()
                || bytes.get(*position) != Some(&b';')
                || name.first() == Some(&b'/')
                || name.last() == Some(&b'/')
                || name.windows(2).any(|pair| pair == b"//")
                || name.iter().any(|value| matches!(value, b'.' | b'['))
            {
                return false;
            }
            *position += 1;
            true
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/classfile/validation.rs"]
mod tests;
