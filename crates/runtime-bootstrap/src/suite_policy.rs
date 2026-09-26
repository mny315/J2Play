//! Shared bootstrap visibility policy for product and diagnostic frontends.

use super::BootstrapRequirement;
use std::collections::HashSet;
use std::hash::BuildHasher;

/// Whether typed class or member references require the optional JSR-239 surface.
/// Display strings and other unrelated UTF-8 constants do not enable an API.
#[must_use]
pub fn class_requests_jsr239(class: &classfile::ClassFile) -> bool {
    class
        .fields
        .iter()
        .chain(&class.methods)
        .filter_map(|member| class.utf8(member.descriptor_index))
        .any(descriptor_requests_jsr239)
        || class.constant_pool.iter().flatten().any(|constant| {
            use classfile::Constant;
            match constant {
                Constant::Class { name_index } => class
                    .utf8(*name_index)
                    .is_some_and(class_name_requests_jsr239),
                Constant::Fieldref {
                    name_and_type_index,
                    ..
                }
                | Constant::Methodref {
                    name_and_type_index,
                    ..
                }
                | Constant::InterfaceMethodref {
                    name_and_type_index,
                    ..
                } => member_reference_descriptor(class, *name_and_type_index)
                    .is_some_and(descriptor_requests_jsr239),
                _ => false,
            }
        })
}

fn member_reference_descriptor(class: &classfile::ClassFile, index: u16) -> Option<&str> {
    let classfile::Constant::NameAndType {
        descriptor_index, ..
    } = class
        .constant_pool
        .get(usize::from(index))
        .and_then(Option::as_ref)?
    else {
        return None;
    };
    class.utf8(*descriptor_index)
}

fn descriptor_requests_jsr239(descriptor: &str) -> bool {
    let mut remaining = descriptor;
    while let Some(start) = remaining.find('L') {
        remaining = &remaining[start + 1..];
        let Some(end) = remaining.find(';') else {
            return false;
        };
        if jsr239_class_name(&remaining[..end]) {
            return true;
        }
        remaining = &remaining[end + 1..];
    }
    false
}

fn class_name_requests_jsr239(name: &str) -> bool {
    let component = name.trim_start_matches('[');
    let component = component
        .strip_prefix('L')
        .and_then(|name| name.strip_suffix(';'))
        .unwrap_or(component);
    jsr239_class_name(component)
}

fn jsr239_class_name(name: &str) -> bool {
    matches!(
        device_profile::java_capability_for_class(name),
        Some(device_profile::JavaCapability::Jsr("239"))
    )
}

/// Enables declared profile APIs and suite-requested compatibility APIs.
/// A suite request alone cannot widen the persona's validated capabilities.
#[must_use]
pub fn profile_supports_suite_bootstrap<S: BuildHasher>(
    profile: &device_profile::DeviceProfile,
    requirement: BootstrapRequirement,
    requested_compatibility_jsrs: &HashSet<String, S>,
) -> bool {
    match requirement {
        BootstrapRequirement::Core => true,
        BootstrapRequirement::Jsr(jsr) => {
            profile
                .java()
                .jsrs()
                .get(jsr)
                .and_then(device_profile::Evidence::value)
                == Some(&true)
                || (requested_compatibility_jsrs.contains(jsr)
                    && profile
                        .java()
                        .compatibility_jsrs()
                        .value()
                        .is_some_and(|jsrs| jsrs.iter().any(|candidate| candidate == jsr)))
        }
        BootstrapRequirement::VendorApi(api) => profile.java().supports_vendor_api(api),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/runtime-bootstrap/suite_policy.rs"]
mod tests;
