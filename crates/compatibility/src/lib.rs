//! Bounded static archive evidence for Java ME profile selection.

use classfile::{ClassFile, Constant};
use diagnostics::{Category, EmuError};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const MAX_DEPENDENCIES: usize = 250_000;
// Static inspection retains all decoded classes together. Archive byte limits
// alone do not bound their expanded constant pools, members, and attributes.
const MAX_ANALYSIS_CLASS_BYTES: usize = 256 * 1024 * 1024;
const LEGACY_IGP_DESCRIPTOR_DEFAULTS: [(&str, &str); 2] =
    [("URL-OPERATOR", "0"), ("URL-SUPPORT", "0")];
const LEGACY_IGP_IAP_ENABLE_DEFAULT: (&str, &str) = ("IAP-EnableIAP", "0");
const LEGACY_CONTENT_DESCRIPTOR_DEFAULTS: [(&str, &str); 2] =
    [("Has_blood", "0"), ("HAS-BLOOD", "0")];
const LEGACY_C2M_LANGUAGE_LIST_PROPERTY: &str = "C2M-LangList";
const LEGACY_C2M_LANGUAGE_METADATA_RESOURCE: &str = "/res/lang/supported_lang.txt";
const MAX_LEGACY_C2M_LANGUAGE_METADATA_BYTES: usize = 64 * 1024;
const MAX_LEGACY_C2M_LANGUAGES: usize = 64;
const MAX_LEGACY_C2M_LANGUAGE_TAG_BYTES: usize = 32;

struct MemberReference<'a> {
    owner: &'a str,
    name: &'a str,
    descriptor: &'a str,
    descriptor_index: u16,
}

/// Static archive facts shared by launch-time profile selection and bounded
/// descriptor compatibility.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StaticArchiveEvidence {
    pub external_classes: Vec<String>,
    pub descriptorless_suite_property_defaults: BTreeMap<String, String>,
}

#[derive(Default)]
struct DescriptorlessSuitePropertyContracts {
    defaults: BTreeMap<String, String>,
    c2m_language_list: bool,
}

/// Collects launch-time class references and recognized descriptorless
/// deployment contracts in one bounded parse of the application classes.
///
/// The compatibility defaults model neutral values normally supplied by a JAD;
/// callers must preserve explicit manifest/JAD values and apply these only when
/// no external descriptor was provided.
///
/// # Errors
/// Returns a controlled archive, class-parser, model, or dependency-limit
/// diagnostic.
pub fn static_archive_evidence_from_jar_path(
    application_path: &Path,
) -> Result<StaticArchiveEvidence, EmuError> {
    static_archive_evidence_from_archive(&jar::ResourceArchive::open(application_path)?)
}

/// Collects launch-time evidence from archive bytes already opened by a frontend.
///
/// This is the byte-oriented equivalent of
/// [`static_archive_evidence_from_jar_path`] and never needs the source URI or
/// host path used by a platform document picker.
///
/// # Errors
/// Returns a controlled archive, class-parser, model, or dependency-limit diagnostic.
pub fn static_archive_evidence_from_jar_bytes(
    bytes: &[u8],
) -> Result<StaticArchiveEvidence, EmuError> {
    static_archive_evidence_from_archive(&jar::ResourceArchive::from_bytes(bytes)?)
}

fn static_archive_evidence_from_archive<B: AsRef<[u8]>>(
    archive: &jar::ResourceArchive<B>,
) -> Result<StaticArchiveEvidence, EmuError> {
    let application = parse_entries(archive.class_entries()?)?;
    static_archive_evidence_from_classes(&application, |name, maximum_bytes| {
        archive.read_with_limit(name, maximum_bytes)
    })
}

fn static_archive_evidence_from_classes(
    application: &[ClassFile],
    mut read_resource: impl FnMut(&str, u64) -> Result<Option<Vec<u8>>, EmuError>,
) -> Result<StaticArchiveEvidence, EmuError> {
    let mut contracts = recognized_descriptorless_suite_property_contracts(application)?;
    if contracts.c2m_language_list {
        let metadata = match read_resource(
            LEGACY_C2M_LANGUAGE_METADATA_RESOURCE,
            MAX_LEGACY_C2M_LANGUAGE_METADATA_BYTES as u64,
        ) {
            Ok(metadata) => metadata,
            // This optional contract ignores oversized tables. Enforce its
            // limit before decompression rather than after reading up to the
            // much larger general resource ceiling.
            Err(error)
                if error.category() == Category::Jar && error.code() == "entry-too-large" =>
            {
                None
            }
            Err(error) => return Err(error),
        };
        add_c2m_language_list_default(&mut contracts, metadata.as_deref());
    }
    static_archive_evidence_with_contracts(application, contracts)
}

#[cfg(test)]
fn static_archive_evidence(application: &[ClassFile]) -> Result<StaticArchiveEvidence, EmuError> {
    let contracts = recognized_descriptorless_suite_property_contracts(application)?;
    static_archive_evidence_with_contracts(application, contracts)
}

fn static_archive_evidence_with_contracts(
    application: &[ClassFile],
    contracts: DescriptorlessSuitePropertyContracts,
) -> Result<StaticArchiveEvidence, EmuError> {
    let application_names = application
        .iter()
        .map(class_name)
        .collect::<Result<BTreeSet<_>, _>>()?;
    let external_classes = collect_external_classes(application, &application_names)?
        .into_iter()
        .map(str::to_owned)
        .collect();
    Ok(StaticArchiveEvidence {
        external_classes,
        descriptorless_suite_property_defaults: contracts.defaults,
    })
}

fn recognized_descriptorless_suite_property_contracts(
    application: &[ClassFile],
) -> Result<DescriptorlessSuitePropertyContracts, EmuError> {
    let mut reads_app_property = false;
    let mut has_operator_key = false;
    let mut has_support_key = false;
    let mut has_iap_enable_key = false;
    let mut has_c2m_language_list_key = false;
    let mut content_defaults = BTreeMap::new();
    for class in application {
        for constant in class.constant_pool.iter().flatten() {
            match constant {
                Constant::Methodref {
                    class_index,
                    name_and_type_index,
                } => {
                    let member = member_ref(class, *class_index, *name_and_type_index)?;
                    reads_app_property |= member.owner == "javax/microedition/midlet/MIDlet"
                        && member.name == "getAppProperty"
                        && member.descriptor == "(Ljava/lang/String;)Ljava/lang/String;";
                }
                Constant::String { string_index } => {
                    let literal = required_utf8(class, *string_index, "string value")?;
                    has_operator_key |= literal == "URL-OPERATOR";
                    has_support_key |= literal == "URL-SUPPORT";
                    has_iap_enable_key |= literal == LEGACY_IGP_IAP_ENABLE_DEFAULT.0;
                    has_c2m_language_list_key |= literal == LEGACY_C2M_LANGUAGE_LIST_PROPERTY;
                    for (name, value) in LEGACY_CONTENT_DESCRIPTOR_DEFAULTS {
                        if literal == name {
                            content_defaults.insert(name.to_owned(), value.to_owned());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if !reads_app_property {
        return Ok(DescriptorlessSuitePropertyContracts::default());
    }
    let mut defaults = BTreeMap::new();
    if has_operator_key && has_support_key {
        defaults.extend(
            LEGACY_IGP_DESCRIPTOR_DEFAULTS
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value.to_owned())),
        );
        if has_iap_enable_key {
            defaults.insert(
                LEGACY_IGP_IAP_ENABLE_DEFAULT.0.to_owned(),
                LEGACY_IGP_IAP_ENABLE_DEFAULT.1.to_owned(),
            );
        }
    }
    defaults.extend(content_defaults);
    Ok(DescriptorlessSuitePropertyContracts {
        defaults,
        c2m_language_list: has_c2m_language_list_key,
    })
}

fn add_c2m_language_list_default(
    contracts: &mut DescriptorlessSuitePropertyContracts,
    metadata: Option<&[u8]>,
) {
    if !contracts.c2m_language_list {
        return;
    }
    if let Some(value) = metadata.and_then(c2m_language_list_default) {
        contracts
            .defaults
            .insert(LEGACY_C2M_LANGUAGE_LIST_PROPERTY.to_owned(), value);
    }
}

fn c2m_language_list_default(metadata: &[u8]) -> Option<String> {
    if metadata.len() > MAX_LEGACY_C2M_LANGUAGE_METADATA_BYTES {
        return None;
    }
    let mut lines = metadata.split(|byte| *byte == b'\n');
    let count = std::str::from_utf8(lines.next()?.trim_ascii())
        .ok()?
        .parse::<usize>()
        .ok()?;
    if !(1..=MAX_LEGACY_C2M_LANGUAGES).contains(&count) {
        return None;
    }

    let mut languages = Vec::with_capacity(count);
    let mut unique = BTreeSet::new();
    for line in lines.take(count) {
        let language = line.trim_ascii();
        if language.is_empty()
            || language.len() > MAX_LEGACY_C2M_LANGUAGE_TAG_BYTES
            || !language
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return None;
        }
        let language = std::str::from_utf8(language).ok()?;
        if !unique.insert(language) {
            return None;
        }
        languages.push(language);
    }
    if languages.len() != count {
        return None;
    }
    Some(languages.join(","))
}

fn parse_entries(entries: Vec<jar::ClassResource>) -> Result<Vec<ClassFile>, EmuError> {
    let mut remaining_bytes = MAX_ANALYSIS_CLASS_BYTES;
    parse_entries_with_budget(entries, &mut remaining_bytes)
}

fn parse_entries_with_budget(
    entries: Vec<jar::ClassResource>,
    remaining_bytes: &mut usize,
) -> Result<Vec<ClassFile>, EmuError> {
    *remaining_bytes = entries
        .len()
        .checked_mul(std::mem::size_of::<ClassFile>())
        .and_then(|bytes| remaining_bytes.checked_sub(bytes))
        .ok_or_else(|| {
            model_error("decoded class container exceeds the static analysis memory budget")
        })?;
    entries
        .into_iter()
        .map(|resource| {
            classfile::parse_with_budget(&resource.bytes, remaining_bytes).map_err(|error| {
                EmuError::with_source(
                    Category::ClassLoading,
                    "compatibility-class-parse",
                    format!("cannot parse {}: {error}", resource.name),
                    error,
                )
            })
        })
        .collect()
}

fn collect_external_classes<'a>(
    application: &'a [ClassFile],
    application_names: &BTreeSet<&str>,
) -> Result<BTreeSet<&'a str>, EmuError> {
    let mut references = BTreeSet::new();
    for class in application {
        let mut descriptors = BTreeSet::new();
        let mut class_names = BTreeSet::new();
        let mut members = BTreeSet::new();
        for member in class.fields.iter().chain(&class.methods) {
            if descriptors.insert(member.descriptor_index) {
                let descriptor =
                    required_utf8(class, member.descriptor_index, "member descriptor")?;
                insert_descriptor_classes(descriptor, application_names, &mut references)?;
            }
        }
        for constant in class.constant_pool.iter().flatten() {
            match constant {
                Constant::Class { name_index } => {
                    // Member owners already refer to these Class constants.
                    // Normalize array owners here once, alongside ordinary names.
                    if class_names.insert(*name_index) {
                        let name = required_utf8(class, *name_index, "class name")?;
                        if let Some(name) = external_class_name(name, application_names) {
                            insert_bounded(&mut references, name)?;
                        }
                    }
                }
                Constant::Methodref {
                    class_index,
                    name_and_type_index,
                }
                | Constant::InterfaceMethodref {
                    class_index,
                    name_and_type_index,
                }
                | Constant::Fieldref {
                    class_index,
                    name_and_type_index,
                } => {
                    if !members.insert((*class_index, *name_and_type_index)) {
                        continue;
                    }
                    let member = member_ref(class, *class_index, *name_and_type_index)?;
                    if descriptors.insert(member.descriptor_index) {
                        insert_descriptor_classes(
                            member.descriptor,
                            application_names,
                            &mut references,
                        )?;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(references)
}

fn class_name(class: &ClassFile) -> Result<&str, EmuError> {
    class
        .class_name(class.this_class)
        .ok_or_else(|| model_error("class has no valid this_class name"))
}

fn member_ref(
    class: &ClassFile,
    class_index: u16,
    name_and_type_index: u16,
) -> Result<MemberReference<'_>, EmuError> {
    let owner = class
        .class_name(class_index)
        .ok_or_else(|| model_error("member reference has invalid owner"))?;
    let Some(Constant::NameAndType {
        name_index,
        descriptor_index,
    }) = class
        .constant_pool
        .get(usize::from(name_and_type_index))
        .and_then(Option::as_ref)
    else {
        return Err(model_error("member reference has invalid name_and_type"));
    };
    Ok(MemberReference {
        owner,
        name: required_utf8(class, *name_index, "member name")?,
        descriptor: required_utf8(class, *descriptor_index, "member descriptor")?,
        descriptor_index: *descriptor_index,
    })
}

fn required_utf8<'a>(class: &'a ClassFile, index: u16, what: &str) -> Result<&'a str, EmuError> {
    class
        .utf8(index)
        .ok_or_else(|| model_error(format!("invalid {what} constant")))
}

fn external_class_name<'a>(name: &'a str, application: &BTreeSet<&str>) -> Option<&'a str> {
    if name.starts_with('[') {
        let component = name.trim_start_matches('[');
        if component.starts_with('L') && component.ends_with(';') {
            let component = &component[1..component.len() - 1];
            return (!application.contains(component)).then_some(component);
        }
        None
    } else {
        (!application.contains(name)).then_some(name)
    }
}

fn insert_descriptor_classes<'a>(
    descriptor: &'a str,
    application: &BTreeSet<&str>,
    external: &mut BTreeSet<&'a str>,
) -> Result<(), EmuError> {
    let mut remaining = descriptor;
    while let Some(start) = remaining.find('L') {
        remaining = &remaining[start + 1..];
        let Some(end) = remaining.find(';') else {
            return Err(model_error(format!(
                "descriptor has an unterminated object type: {descriptor:?}"
            )));
        };
        let name = &remaining[..end];
        if !application.contains(name) {
            insert_bounded(external, name)?;
        }
        remaining = &remaining[end + 1..];
    }
    Ok(())
}

fn insert_bounded<T: Ord>(set: &mut BTreeSet<T>, value: T) -> Result<(), EmuError> {
    if set.len() == MAX_DEPENDENCIES && !set.contains(&value) {
        return Err(EmuError::new(
            Category::ClassLoading,
            "compatibility-limit",
            format!("static archive evidence exceeds {MAX_DEPENDENCIES} unique external classes"),
        ));
    }
    set.insert(value);
    Ok(())
}

fn model_error(message: impl Into<String>) -> EmuError {
    EmuError::new(Category::ClassLoading, "compatibility-model", message)
}

#[cfg(test)]
#[path = "../../../tests/unit/compatibility/mod.rs"]
mod tests;
