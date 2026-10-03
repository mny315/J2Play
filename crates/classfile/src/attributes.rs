//! Bounded attribute bodies, bytecode containers and exception tables.

use diagnostics::EmuError;

use crate::constant_pool::{expect_class, expect_utf8, pool_entry};
use crate::reader::{MaterializationBudget, Reader, check_count};
use crate::{
    Attribute, AttributeLocation, CodeAttribute, Constant, ExceptionHandler, MAX_ATTRIBUTE_BYTES,
    MAX_ATTRIBUTES, parse_error,
};

pub(super) fn parse_attributes(
    reader: &mut Reader<'_>,
    pool: &[Option<Constant>],
    location: AttributeLocation,
    materialization: &mut MaterializationBudget,
) -> Result<Vec<Attribute>, EmuError> {
    let count_offset = reader.offset();
    let count = reader.u2()?;
    check_count(count, MAX_ATTRIBUTES, count_offset, "attributes")?;
    materialization.charge_items::<Attribute>(usize::from(count), count_offset)?;
    let mut attributes = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let name_offset = reader.offset();
        let name_index = reader.u2()?;
        let name = expect_utf8(pool, name_index, name_offset, "attribute name")?;
        let length_offset = reader.offset();
        let length_u32 = reader.u4()?;
        let length = usize::try_from(length_u32).map_err(|_| {
            parse_error(
                "attribute-limit",
                length_offset,
                "attribute length cannot fit in memory",
            )
        })?;
        if length > MAX_ATTRIBUTE_BYTES {
            return Err(parse_error(
                "attribute-limit",
                length_offset,
                format!("attribute length {length} exceeds limit {MAX_ATTRIBUTE_BYTES}"),
            ));
        }
        let body_offset = reader.offset();
        let body = reader.bytes(length)?;
        if name == "Code" {
            if location != AttributeLocation::Method {
                return Err(parse_error(
                    "invalid-attribute-location",
                    name_offset,
                    "Code attribute is only valid on methods",
                ));
            }
            attributes.push(Attribute::Code(parse_code(
                name_index,
                body,
                body_offset,
                pool,
                materialization,
            )?));
        } else {
            if matches!(
                name,
                "ConstantValue"
                    | "SourceFile"
                    | "Signature"
                    | "Exceptions"
                    | "Synthetic"
                    | "Deprecated"
            ) && attributes.iter().any(|attribute| match attribute {
                Attribute::Raw { name: existing, .. } => existing == name,
                Attribute::Code(_) => false,
            }) {
                return Err(parse_error(
                    "duplicate-attribute",
                    name_offset,
                    format!("duplicate {name} attribute"),
                ));
            }
            validate_known_attribute(name, body, body_offset, pool, location)?;
            materialization.charge(name.len().saturating_add(body.len()), name_offset)?;
            attributes.push(Attribute::Raw {
                name_index,
                name: name.to_owned(),
                bytes: body.to_vec(),
            });
        }
    }
    Ok(attributes)
}

fn parse_code(
    name_index: u16,
    bytes: &[u8],
    base_offset: usize,
    pool: &[Option<Constant>],
    materialization: &mut MaterializationBudget,
) -> Result<CodeAttribute, EmuError> {
    let mut reader = Reader::with_base(bytes, base_offset);
    let max_stack = reader.u2()?;
    let max_locals = reader.u2()?;
    let length_offset = reader.offset();
    let code_length = usize::try_from(reader.u4()?).map_err(|_| {
        parse_error(
            "code-limit",
            length_offset,
            "code length cannot fit in memory",
        )
    })?;
    if code_length == 0 || code_length > 65_535 {
        return Err(parse_error(
            "code-limit",
            length_offset,
            "code length must be in 1..=65535 bytes",
        ));
    }
    materialization.charge(code_length, length_offset)?;
    let code = reader.bytes(code_length)?.to_vec();
    let exception_count_offset = reader.offset();
    let exception_count = reader.u2()?;
    materialization
        .charge_items::<ExceptionHandler>(usize::from(exception_count), exception_count_offset)?;
    let mut exception_table = Vec::with_capacity(usize::from(exception_count));
    for _ in 0..exception_count {
        let entry_offset = reader.offset();
        let start_pc = reader.u2()?;
        let end_pc = reader.u2()?;
        let handler_pc = reader.u2()?;
        let catch_type = reader.u2()?;
        if start_pc >= end_pc
            || usize::from(end_pc) > code_length
            || usize::from(handler_pc) >= code_length
        {
            return Err(parse_error(
                "invalid-exception-handler",
                entry_offset,
                "exception handler range is outside code",
            ));
        }
        if catch_type != 0 {
            expect_class(pool, catch_type, entry_offset + 6, "catch_type")?;
        }
        exception_table.push(ExceptionHandler {
            start_pc,
            end_pc,
            handler_pc,
            catch_type,
        });
    }
    let attributes = parse_attributes(&mut reader, pool, AttributeLocation::Code, materialization)?;
    require_consumed(&reader, "Code")?;
    Ok(CodeAttribute {
        name_index,
        max_stack,
        max_locals,
        code,
        exception_table,
        attributes,
    })
}

fn validate_known_attribute(
    name: &str,
    bytes: &[u8],
    base_offset: usize,
    pool: &[Option<Constant>],
    location: AttributeLocation,
) -> Result<(), EmuError> {
    let valid_location = match name {
        "ConstantValue" => location == AttributeLocation::Field,
        "SourceFile" => location == AttributeLocation::Class,
        "Exceptions" => location == AttributeLocation::Method,
        "LineNumberTable" | "LocalVariableTable" | "LocalVariableTypeTable" => {
            location == AttributeLocation::Code
        }
        "Signature" | "Synthetic" | "Deprecated" => location != AttributeLocation::Code,
        _ => true,
    };
    if !valid_location {
        return Err(parse_error(
            "invalid-attribute-location",
            base_offset,
            format!("{name} attribute is not valid in this location"),
        ));
    }
    match name {
        "Synthetic" | "Deprecated" if !bytes.is_empty() => Err(parse_error(
            "attribute-length-mismatch",
            base_offset,
            format!("{name} attribute must be empty"),
        )),
        "ConstantValue" => {
            let mut reader = Reader::with_base(bytes, base_offset);
            let index_offset = reader.offset();
            let index = reader.u2()?;
            match pool_entry(pool, index, index_offset, "ConstantValue")? {
                Constant::Integer(_)
                | Constant::Float(_)
                | Constant::Long(_)
                | Constant::Double(_)
                | Constant::String { .. } => {}
                _ => {
                    return Err(parse_error(
                        "invalid-constant-type",
                        index_offset,
                        "ConstantValue has an invalid constant type",
                    ));
                }
            }
            require_consumed(&reader, name)
        }
        "SourceFile" | "Signature" => {
            let mut reader = Reader::with_base(bytes, base_offset);
            let index_offset = reader.offset();
            let index = reader.u2()?;
            expect_utf8(pool, index, index_offset, name)?;
            require_consumed(&reader, name)
        }
        "Exceptions" => {
            let mut reader = Reader::with_base(bytes, base_offset);
            let count = reader.u2()?;
            for _ in 0..count {
                let offset = reader.offset();
                let index = reader.u2()?;
                expect_class(pool, index, offset, "exception class")?;
            }
            require_consumed(&reader, name)
        }
        "LineNumberTable" => validate_table_width(bytes, base_offset, name, 4),
        "LocalVariableTable" | "LocalVariableTypeTable" => {
            validate_table_width(bytes, base_offset, name, 10)
        }
        _ => Ok(()),
    }
}

fn validate_table_width(
    bytes: &[u8],
    base_offset: usize,
    name: &str,
    entry_width: usize,
) -> Result<(), EmuError> {
    let mut reader = Reader::with_base(bytes, base_offset);
    let count = usize::from(reader.u2()?);
    let expected = count.checked_mul(entry_width).ok_or_else(|| {
        parse_error(
            "attribute-length-mismatch",
            base_offset,
            format!("{name} table size overflows"),
        )
    })?;
    reader.bytes(expected)?;
    require_consumed(&reader, name)
}

fn require_consumed(reader: &Reader<'_>, name: &str) -> Result<(), EmuError> {
    if reader.remaining() != 0 {
        return Err(parse_error(
            "attribute-length-mismatch",
            reader.offset(),
            format!("{name} attribute has {} trailing bytes", reader.remaining()),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit/classfile/attributes.rs"]
mod tests;
