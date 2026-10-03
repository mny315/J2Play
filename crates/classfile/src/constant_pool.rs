//! Constant-pool decoding, indexing and reference validation.

use diagnostics::EmuError;

use crate::modified_utf8::parse_utf8_constant;
use crate::reader::{MaterializationBudget, Reader};
use crate::validation::{
    validate_class_constant_name, validate_field_descriptor, validate_method_descriptor,
    validate_special_method, validate_unqualified_name,
};
use crate::{
    CLASS_MAGIC, ClassPrefix, Constant, MAX_CONSTANT_POOL_ENTRIES, MemberKind, Utf16Constant,
    parse_error,
};

pub(super) fn validate_target_version(prefix: &ClassPrefix) -> Result<(), EmuError> {
    // Several production MIDlet obfuscators preserved the historical 45.3
    // minor stamp while rewriting the major version. KVM implementations
    // accepted these files and their bytecode remains within the CLDC range.
    let supported = (45..=50).contains(&prefix.major_version) && prefix.minor_version <= 3;
    if !supported {
        return Err(parse_error(
            "unsupported-class-version",
            4,
            format!(
                "class version {}.{} is outside target range 45.0..=50.3",
                prefix.major_version, prefix.minor_version
            ),
        ));
    }
    if prefix.constant_pool.iter().flatten().any(|constant| {
        matches!(
            constant,
            Constant::MethodHandle { .. }
                | Constant::MethodType { .. }
                | Constant::Dynamic { .. }
                | Constant::InvokeDynamic { .. }
                | Constant::Module { .. }
                | Constant::Package { .. }
        )
    }) {
        return Err(parse_error(
            "unsupported-constant-version",
            4,
            "constant-pool tag requires a class version newer than 50",
        ));
    }
    Ok(())
}

/// Parses the class-file header and constant pool without reading later
/// structures. All failures include the byte offset at which they were found.
///
/// # Errors
///
/// Returns a `class-loading` diagnostic for truncated, malformed, or
/// deliberately over-limit input.
pub fn parse_prefix(bytes: &[u8]) -> Result<ClassPrefix, EmuError> {
    parse_prefix_with_budget(bytes, &mut MaterializationBudget::for_input(bytes.len()))
}

pub(super) fn parse_prefix_with_budget(
    bytes: &[u8],
    materialization: &mut MaterializationBudget,
) -> Result<ClassPrefix, EmuError> {
    let mut reader = Reader::new(bytes);
    let magic_offset = reader.offset();
    let magic = reader.u4()?;
    if magic != CLASS_MAGIC {
        return Err(parse_error(
            "invalid-magic",
            magic_offset,
            format!("expected CAFEBABE, found {magic:08X}"),
        ));
    }

    let minor_version = reader.u2()?;
    let major_version = reader.u2()?;
    let count_offset = reader.offset();
    let count = reader.u2()?;
    if count == 0 {
        return Err(parse_error(
            "invalid-constant-pool-count",
            count_offset,
            "constant_pool_count must be at least 1",
        ));
    }
    if count > MAX_CONSTANT_POOL_ENTRIES {
        return Err(parse_error(
            "constant-pool-limit",
            count_offset,
            format!("constant_pool_count {count} exceeds limit {MAX_CONSTANT_POOL_ENTRIES}"),
        ));
    }

    materialization.charge_items::<Option<Constant>>(usize::from(count), count_offset)?;
    materialization.charge_items::<Option<usize>>(usize::from(count), count_offset)?;
    let mut constant_pool = Vec::with_capacity(usize::from(count));
    let mut constant_offsets = Vec::with_capacity(usize::from(count));
    constant_pool.push(None);
    constant_offsets.push(None);
    let mut index = 1_u16;
    while index < count {
        let tag_offset = reader.offset();
        let tag = reader.u1()?;
        let constant = parse_constant(tag, tag_offset, &mut reader)?;
        // UTF-8 wire lengths are u16, so temporary decoding storage is bounded
        // per entry. Account for retained storage before adding it to the pool.
        let owned_bytes = match &constant {
            Constant::Utf8(text) => text.capacity(),
            Constant::Utf16(value) => {
                std::mem::size_of::<Utf16Constant>()
                    + value.text.len()
                    + std::mem::size_of_val(value.units.as_ref())
            }
            _ => 0,
        };
        materialization.charge(owned_bytes, tag_offset)?;
        let wide = matches!(constant, Constant::Long(_) | Constant::Double(_));
        constant_pool.push(Some(constant));
        constant_offsets.push(Some(tag_offset));
        index += 1;
        if wide {
            if index >= count {
                return Err(parse_error(
                    "invalid-wide-constant",
                    tag_offset,
                    "long or double constant has no reserved following slot",
                ));
            }
            constant_pool.push(None);
            constant_offsets.push(None);
            index += 1;
        }
    }

    Ok(ClassPrefix {
        minor_version,
        major_version,
        constant_pool,
        constant_offsets,
        next_offset: reader.offset(),
    })
}

pub(super) fn validate_constant_pool(
    prefix: &ClassPrefix,
) -> Result<ConstantValidationCache, EmuError> {
    let mut cache = ConstantValidationCache::new(prefix.constant_pool.len());
    for (index, constant) in prefix.constant_pool.iter().enumerate().skip(1) {
        let Some(constant) = constant else { continue };
        let offset = prefix.constant_offsets[index].unwrap_or(10);
        validate_constant(constant, &prefix.constant_pool, offset, &mut cache)?;
    }
    Ok(cache)
}

pub(super) struct ConstantValidationCache {
    validated: Vec<u8>,
}

const CLASS_NAME: u8 = 1;
const FIELD_NAME: u8 = 1 << 1;
const METHOD_NAME: u8 = 1 << 2;
const FIELD_DESCRIPTOR: u8 = 1 << 3;
const METHOD_DESCRIPTOR: u8 = 1 << 4;
// A valid 255-slot descriptor fits only a static method. Retain this boundary
// in the existing cache byte so declarations need not rescan shared strings.
const METHOD_REQUIRES_STATIC: u8 = 1 << 5;

impl ConstantValidationCache {
    pub(super) fn new(pool_length: usize) -> Self {
        Self {
            validated: vec![0; pool_length],
        }
    }

    fn first_use(&mut self, index: u16, role: u8) -> bool {
        // Callers resolve the pool entry before consulting this cache. One byte
        // per slot bounds scratch storage to the already checked pool length.
        let validated = &mut self.validated[usize::from(index)];
        let first = *validated & role == 0;
        *validated |= role;
        first
    }

    pub(super) fn member_name(
        &mut self,
        index: u16,
        kind: MemberKind,
        name: &str,
        offset: usize,
    ) -> Result<(), EmuError> {
        let role = match kind {
            MemberKind::Field => FIELD_NAME,
            MemberKind::Method => METHOD_NAME,
        };
        if self.first_use(index, role) {
            validate_unqualified_name(name, kind == MemberKind::Method, offset)?;
        }
        Ok(())
    }

    pub(super) fn member_descriptor(
        &mut self,
        index: u16,
        kind: MemberKind,
        descriptor: &str,
        offset: usize,
    ) -> Result<(), EmuError> {
        let role = match kind {
            MemberKind::Field => FIELD_DESCRIPTOR,
            MemberKind::Method => METHOD_DESCRIPTOR,
        };
        if self.first_use(index, role) {
            match kind {
                MemberKind::Field => validate_field_descriptor(descriptor, offset)?,
                MemberKind::Method => {
                    if validate_method_descriptor(descriptor, offset)? == u8::MAX {
                        self.validated[usize::from(index)] |= METHOD_REQUIRES_STATIC;
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn require_receiver_slot(&self, index: u16, offset: usize) -> Result<(), EmuError> {
        debug_assert!(self.validated[usize::from(index)] & METHOD_DESCRIPTOR != 0);
        if self.validated[usize::from(index)] & METHOD_REQUIRES_STATIC != 0 {
            return Err(parse_error(
                "invalid-descriptor",
                offset,
                "instance method parameters exceed 255 slots including the receiver",
            ));
        }
        Ok(())
    }
}

fn validate_constant(
    constant: &Constant,
    pool: &[Option<Constant>],
    offset: usize,
    cache: &mut ConstantValidationCache,
) -> Result<(), EmuError> {
    match constant {
        Constant::Class { name_index } => {
            let name = expect_symbol(pool, *name_index, offset, "class name")?;
            if cache.first_use(*name_index, CLASS_NAME) {
                validate_class_constant_name(name, offset)?;
            }
        }
        Constant::String {
            string_index: name_index,
        }
        | Constant::MethodType {
            descriptor_index: name_index,
        }
        | Constant::Module { name_index }
        | Constant::Package { name_index } => {
            expect_utf8(pool, *name_index, offset, "constant reference")?;
        }
        Constant::Fieldref {
            class_index,
            name_and_type_index,
        } => {
            expect_class(pool, *class_index, offset, "member class")?;
            validate_name_and_type_reference(
                pool,
                *name_and_type_index,
                offset,
                MemberKind::Field,
                cache,
            )?;
        }
        Constant::Methodref {
            class_index,
            name_and_type_index,
        }
        | Constant::InterfaceMethodref {
            class_index,
            name_and_type_index,
        } => {
            expect_class(pool, *class_index, offset, "member class")?;
            let descriptor_index = validate_name_and_type_reference(
                pool,
                *name_and_type_index,
                offset,
                MemberKind::Method,
                cache,
            )?;
            if matches!(constant, Constant::InterfaceMethodref { .. }) {
                cache.require_receiver_slot(descriptor_index, offset)?;
            }
        }
        Constant::NameAndType {
            name_index,
            descriptor_index,
        } => {
            expect_utf8(pool, *name_index, offset, "member name")?;
            expect_utf8(pool, *descriptor_index, offset, "member descriptor")?;
        }
        Constant::MethodHandle {
            reference_kind,
            reference_index,
        } => {
            if !(1..=9).contains(reference_kind) {
                return Err(parse_error(
                    "invalid-method-handle-kind",
                    offset,
                    format!("reference_kind {reference_kind} is outside 1..=9"),
                ));
            }
            expect_present(pool, *reference_index, offset, "method handle reference")?;
        }
        Constant::Dynamic {
            name_and_type_index,
            ..
        }
        | Constant::InvokeDynamic {
            name_and_type_index,
            ..
        } => {
            expect_name_and_type(pool, *name_and_type_index, offset, "dynamic name_and_type")?;
        }
        Constant::Utf8(_)
        | Constant::Utf16(_)
        | Constant::Integer(_)
        | Constant::Float(_)
        | Constant::Long(_)
        | Constant::Double(_) => {}
    }
    Ok(())
}

fn validate_name_and_type_reference(
    pool: &[Option<Constant>],
    index: u16,
    offset: usize,
    kind: MemberKind,
    cache: &mut ConstantValidationCache,
) -> Result<u16, EmuError> {
    let Constant::NameAndType {
        name_index,
        descriptor_index,
    } = pool_entry(pool, index, offset, "member name_and_type")?
    else {
        return Err(parse_error(
            "invalid-constant-type",
            offset,
            format!("member name_and_type index {index} does not reference NameAndType"),
        ));
    };
    let name = expect_symbol(pool, *name_index, offset, "referenced member name")?;
    let descriptor = expect_symbol(pool, *descriptor_index, offset, "referenced descriptor")?;
    cache.member_name(*name_index, kind, name, offset)?;
    cache.member_descriptor(*descriptor_index, kind, descriptor, offset)?;
    if kind == MemberKind::Method {
        validate_special_method(0, name, descriptor, offset)?;
        if name == "<init>" {
            cache.require_receiver_slot(*descriptor_index, offset)?;
        }
    }
    Ok(*descriptor_index)
}

pub(super) fn pool_entry<'a>(
    pool: &'a [Option<Constant>],
    index: u16,
    offset: usize,
    role: &str,
) -> Result<&'a Constant, EmuError> {
    pool.get(usize::from(index))
        .and_then(Option::as_ref)
        .ok_or_else(|| {
            parse_error(
                "invalid-constant-index",
                offset,
                format!("{role} index {index} is zero, reserved, or out of range"),
            )
        })
}

fn expect_present(
    pool: &[Option<Constant>],
    index: u16,
    offset: usize,
    role: &str,
) -> Result<(), EmuError> {
    pool_entry(pool, index, offset, role).map(|_| ())
}

pub(super) fn expect_utf8<'a>(
    pool: &'a [Option<Constant>],
    index: u16,
    offset: usize,
    role: &str,
) -> Result<&'a str, EmuError> {
    pool_entry(pool, index, offset, role)?
        .utf8_text()
        .ok_or_else(|| {
            parse_error(
                "invalid-constant-type",
                offset,
                format!("{role} index {index} does not reference Utf8"),
            )
        })
}

pub(super) fn expect_symbol<'a>(
    pool: &'a [Option<Constant>],
    index: u16,
    offset: usize,
    role: &str,
) -> Result<&'a str, EmuError> {
    match pool_entry(pool, index, offset, role)? {
        Constant::Utf8(text) => Ok(text),
        // Utf16's display text replaces isolated surrogates. Using it as a
        // linkage key would alias distinct Java class or member names.
        Constant::Utf16(_) => Err(parse_error(
            "unsupported-symbol-encoding",
            offset,
            format!("{role} index {index} contains an unsupported isolated UTF-16 surrogate"),
        )),
        _ => Err(parse_error(
            "invalid-constant-type",
            offset,
            format!("{role} index {index} does not reference Utf8"),
        )),
    }
}

pub(super) fn expect_class(
    pool: &[Option<Constant>],
    index: u16,
    offset: usize,
    role: &str,
) -> Result<(), EmuError> {
    match pool_entry(pool, index, offset, role)? {
        Constant::Class { .. } => Ok(()),
        _ => Err(parse_error(
            "invalid-constant-type",
            offset,
            format!("{role} index {index} does not reference Class"),
        )),
    }
}

fn expect_name_and_type(
    pool: &[Option<Constant>],
    index: u16,
    offset: usize,
    role: &str,
) -> Result<(), EmuError> {
    match pool_entry(pool, index, offset, role)? {
        Constant::NameAndType { .. } => Ok(()),
        _ => Err(parse_error(
            "invalid-constant-type",
            offset,
            format!("{role} index {index} does not reference NameAndType"),
        )),
    }
}

fn parse_constant(
    tag: u8,
    tag_offset: usize,
    reader: &mut Reader<'_>,
) -> Result<Constant, EmuError> {
    let pair = |reader: &mut Reader<'_>| -> Result<(u16, u16), EmuError> {
        Ok((reader.u2()?, reader.u2()?))
    };
    Ok(match tag {
        1 => {
            let length = usize::from(reader.u2()?);
            let data_offset = reader.offset();
            parse_utf8_constant(reader.bytes(length)?, data_offset)?
        }
        3 => Constant::Integer(reader.u4()?.cast_signed()),
        4 => Constant::Float(reader.u4()?),
        5 => Constant::Long(reader.u8()?.cast_signed()),
        6 => Constant::Double(reader.u8()?),
        7 => Constant::Class {
            name_index: reader.u2()?,
        },
        8 => Constant::String {
            string_index: reader.u2()?,
        },
        9..=11 => {
            let (class_index, name_and_type_index) = pair(reader)?;
            match tag {
                9 => Constant::Fieldref {
                    class_index,
                    name_and_type_index,
                },
                10 => Constant::Methodref {
                    class_index,
                    name_and_type_index,
                },
                _ => Constant::InterfaceMethodref {
                    class_index,
                    name_and_type_index,
                },
            }
        }
        12 => {
            let (name_index, descriptor_index) = pair(reader)?;
            Constant::NameAndType {
                name_index,
                descriptor_index,
            }
        }
        15 => Constant::MethodHandle {
            reference_kind: reader.u1()?,
            reference_index: reader.u2()?,
        },
        16 => Constant::MethodType {
            descriptor_index: reader.u2()?,
        },
        17 | 18 => {
            let (bootstrap_method_attr_index, name_and_type_index) = pair(reader)?;
            if tag == 17 {
                Constant::Dynamic {
                    bootstrap_method_attr_index,
                    name_and_type_index,
                }
            } else {
                Constant::InvokeDynamic {
                    bootstrap_method_attr_index,
                    name_and_type_index,
                }
            }
        }
        19 => Constant::Module {
            name_index: reader.u2()?,
        },
        20 => Constant::Package {
            name_index: reader.u2()?,
        },
        _ => {
            return Err(parse_error(
                "unknown-constant-tag",
                tag_offset,
                format!("unknown constant-pool tag {tag}"),
            ));
        }
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/classfile/constant_pool.rs"]
mod tests;
