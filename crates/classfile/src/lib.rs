//! Bounded Java class-file parsing and validation.

use diagnostics::{Category, EmuError};
use std::borrow::Cow;

mod attributes;
mod constant_pool;
mod modified_utf8;
mod reader;
mod validation;

pub use constant_pool::parse_prefix;

use attributes::parse_attributes;
use constant_pool::{
    ConstantValidationCache, expect_class, expect_symbol, parse_prefix_with_budget,
    validate_constant_pool, validate_target_version,
};
use reader::{MaterializationBudget, Reader, check_count};
use validation::{
    validate_class_flags, validate_member_attributes, validate_member_flags,
    validate_special_method,
};

const CLASS_MAGIC: u32 = 0xCAFE_BABE;
const MAX_CONSTANT_POOL_ENTRIES: u16 = 16_384;
const MAX_MEMBERS: u16 = 8_192;
const MAX_ATTRIBUTES: u16 = 1_024;
const MAX_ATTRIBUTE_BYTES: usize = 16 * 1024 * 1024;
const MAX_CLASS_MATERIALIZED_BYTES: usize = 32 * 1024 * 1024;
const CLASS_MATERIALIZATION_RATIO: usize = 16;

#[derive(Clone, Copy, Eq, PartialEq)]
enum MemberKind {
    Field,
    Method,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum AttributeLocation {
    Class,
    Field,
    Method,
    Code,
}

/// The version and constant pool at the start of a class file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassPrefix {
    pub minor_version: u16,
    pub major_version: u16,
    /// Indexed exactly like a JVM constant pool. Slot zero and the second slot
    /// of `Long` and `Double` entries contain `None`.
    pub constant_pool: Vec<Option<Constant>>,
    /// Byte offset of each constant's tag; reserved slots contain `None`.
    pub constant_offsets: Vec<Option<usize>>,
    /// Offset of the access flags, immediately after the constant pool.
    pub next_offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Constant {
    Utf8(String),
    /// A `CONSTANT_Utf8` entry containing isolated surrogate code units.
    Utf16(Box<Utf16Constant>),
    Integer(i32),
    Float(u32),
    Long(i64),
    Double(u64),
    Class {
        name_index: u16,
    },
    String {
        string_index: u16,
    },
    Fieldref {
        class_index: u16,
        name_and_type_index: u16,
    },
    Methodref {
        class_index: u16,
        name_and_type_index: u16,
    },
    InterfaceMethodref {
        class_index: u16,
        name_and_type_index: u16,
    },
    NameAndType {
        name_index: u16,
        descriptor_index: u16,
    },
    MethodHandle {
        reference_kind: u8,
        reference_index: u16,
    },
    MethodType {
        descriptor_index: u16,
    },
    Dynamic {
        bootstrap_method_attr_index: u16,
        name_and_type_index: u16,
    },
    InvokeDynamic {
        bootstrap_method_attr_index: u16,
        name_and_type_index: u16,
    },
    Module {
        name_index: u16,
    },
    Package {
        name_index: u16,
    },
}

/// Exact Java code units alongside a lossy host metadata representation.
/// Boxed only for isolated surrogates, keeping ordinary constants compact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Utf16Constant {
    text: Box<str>,
    units: Box<[u16]>,
}

impl Utf16Constant {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn units(&self) -> &[u16] {
        &self.units
    }
}

impl Constant {
    /// Host metadata text; Java string contents must use `utf16_units`.
    #[must_use]
    pub fn utf8_text(&self) -> Option<&str> {
        match self {
            Self::Utf8(text) => Some(text),
            Self::Utf16(value) => Some(value.text()),
            _ => None,
        }
    }

    #[must_use]
    pub fn utf16_units(&self) -> Option<Cow<'_, [u16]>> {
        match self {
            Self::Utf8(text) => Some(Cow::Owned(text.encode_utf16().collect())),
            Self::Utf16(value) => Some(Cow::Borrowed(value.units())),
            _ => None,
        }
    }
}

/// A complete, structurally validated Java class file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: Vec<Option<Constant>>,
    pub access_flags: u16,
    pub this_class: u16,
    pub super_class: u16,
    pub interfaces: Vec<u16>,
    pub fields: Vec<Member>,
    pub methods: Vec<Member>,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Attribute {
    Code(CodeAttribute),
    Raw {
        name_index: u16,
        name: String,
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeAttribute {
    pub name_index: u16,
    pub max_stack: u16,
    pub max_locals: u16,
    pub code: Vec<u8>,
    pub exception_table: Vec<ExceptionHandler>,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExceptionHandler {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

impl ClassFile {
    #[must_use]
    pub fn utf8(&self, index: u16) -> Option<&str> {
        self.constant_pool
            .get(usize::from(index))?
            .as_ref()?
            .utf8_text()
    }

    #[must_use]
    pub fn class_name(&self, index: u16) -> Option<&str> {
        let Constant::Class { name_index } =
            self.constant_pool.get(usize::from(index))?.as_ref()?
        else {
            return None;
        };
        self.utf8(*name_index)
    }
}

/// Parses and validates one complete class file.
///
/// # Errors
///
/// Returns a precise diagnostic for malformed structure, invalid constant-pool
/// references, exceeded limits, or trailing bytes.
pub fn parse(bytes: &[u8]) -> Result<ClassFile, EmuError> {
    let mut materialization = MaterializationBudget::for_input(bytes.len());
    parse_with_materialization(bytes, &mut materialization)
}

/// Parses a class while charging its decoded allocations to a caller's batch budget.
/// The normal per-class limits remain authoritative when they are lower.
/// Successful parses consume the charged bytes; a failed parse leaves the
/// caller's budget unchanged because its partial class has been discarded.
///
/// # Errors
/// Returns the same diagnostics as [`parse`], including exhaustion of the
/// remaining decoded allocation budget before a disallowed allocation.
pub fn parse_with_budget(bytes: &[u8], remaining_bytes: &mut usize) -> Result<ClassFile, EmuError> {
    let mut materialization =
        MaterializationBudget::for_input(bytes.len()).with_ceiling(*remaining_bytes);
    let initial = materialization.remaining();
    let class = parse_with_materialization(bytes, &mut materialization)?;
    *remaining_bytes -= initial - materialization.remaining();
    Ok(class)
}

fn parse_with_materialization(
    bytes: &[u8],
    materialization: &mut MaterializationBudget,
) -> Result<ClassFile, EmuError> {
    let prefix = parse_prefix_with_budget(bytes, materialization)?;
    validate_target_version(&prefix)?;
    let mut validated = validate_constant_pool(&prefix)?;
    let mut reader = Reader::at(bytes, prefix.next_offset)?;
    let access_flags = reader.u2()?;
    validate_class_flags(access_flags, prefix.next_offset)?;
    let this_offset = reader.offset();
    let this_class = reader.u2()?;
    expect_class(&prefix.constant_pool, this_class, this_offset, "this_class")?;
    let super_offset = reader.offset();
    let super_class = reader.u2()?;
    if super_class != 0 {
        expect_class(
            &prefix.constant_pool,
            super_class,
            super_offset,
            "super_class",
        )?;
    }

    let interfaces = parse_interfaces(&mut reader, &prefix.constant_pool, materialization)?;
    let fields = parse_members(
        &mut reader,
        &prefix.constant_pool,
        MemberKind::Field,
        materialization,
        &mut validated,
    )?;
    let methods = parse_members(
        &mut reader,
        &prefix.constant_pool,
        MemberKind::Method,
        materialization,
        &mut validated,
    )?;
    let attributes = parse_attributes(
        &mut reader,
        &prefix.constant_pool,
        AttributeLocation::Class,
        materialization,
    )?;
    if reader.remaining() != 0 {
        return Err(parse_error(
            "trailing-data",
            reader.offset(),
            format!("{} unconsumed bytes", reader.remaining()),
        ));
    }

    Ok(ClassFile {
        minor_version: prefix.minor_version,
        major_version: prefix.major_version,
        constant_pool: prefix.constant_pool,
        access_flags,
        this_class,
        super_class,
        interfaces,
        fields,
        methods,
        attributes,
    })
}

fn parse_interfaces(
    reader: &mut Reader<'_>,
    pool: &[Option<Constant>],
    materialization: &mut MaterializationBudget,
) -> Result<Vec<u16>, EmuError> {
    let count_offset = reader.offset();
    let count = reader.u2()?;
    check_count(count, MAX_MEMBERS, count_offset, "interfaces")?;
    materialization.charge_items::<u16>(usize::from(count), count_offset)?;
    let mut interfaces = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let offset = reader.offset();
        let index = reader.u2()?;
        expect_class(pool, index, offset, "interface")?;
        interfaces.push(index);
    }
    Ok(interfaces)
}

fn parse_members(
    reader: &mut Reader<'_>,
    pool: &[Option<Constant>],
    kind: MemberKind,
    materialization: &mut MaterializationBudget,
    validated: &mut ConstantValidationCache,
) -> Result<Vec<Member>, EmuError> {
    let label = if kind == MemberKind::Field {
        "fields"
    } else {
        "methods"
    };
    let count_offset = reader.offset();
    let count = reader.u2()?;
    check_count(count, MAX_MEMBERS, count_offset, label)?;
    materialization.charge_items::<Member>(usize::from(count), count_offset)?;
    let mut members = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let member_offset = reader.offset();
        let access_flags = reader.u2()?;
        let name_offset = reader.offset();
        let name_index = reader.u2()?;
        let name = expect_symbol(pool, name_index, name_offset, "member name")?;
        let descriptor_offset = reader.offset();
        let descriptor_index = reader.u2()?;
        let descriptor = expect_symbol(
            pool,
            descriptor_index,
            descriptor_offset,
            "member descriptor",
        )?;
        validate_member_flags(kind, access_flags, member_offset)?;
        // A shared constant can be tens of thousands of bytes long. Validate
        // each name/descriptor once per member kind, rather than rescanning it
        // for every field or overload which references the same pool entry.
        validated.member_name(name_index, kind, name, member_offset)?;
        validated.member_descriptor(descriptor_index, kind, descriptor, descriptor_offset)?;
        if kind == MemberKind::Method {
            validate_special_method(access_flags, name, descriptor, member_offset)?;
            if access_flags & 0x0008 == 0 {
                validated.require_receiver_slot(descriptor_index, descriptor_offset)?;
            }
        }
        let location = if kind == MemberKind::Field {
            AttributeLocation::Field
        } else {
            AttributeLocation::Method
        };
        let attributes = parse_attributes(reader, pool, location, materialization)?;
        validate_member_attributes(kind, access_flags, &attributes, member_offset)?;
        members.push(Member {
            access_flags,
            name_index,
            descriptor_index,
            attributes,
        });
    }
    Ok(members)
}

fn parse_error(code: &'static str, offset: usize, message: impl std::fmt::Display) -> EmuError {
    EmuError::new(
        Category::ClassLoading,
        code,
        format!("at offset {offset}: {message}"),
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/classfile/mod.rs"]
mod tests;
