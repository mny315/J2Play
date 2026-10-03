//! Strong manifest/JAD and bounded static-API launch evidence.

use crate::archive_name::profile_hint_tokens;
use crate::decision::{CanvasOrientation, SelectionReason};
use crate::resolver::{Candidate, EvidenceStrength};
use device_profile::{DeviceProfile, JavaCapability, dimension_pairs, java_capability_for_class};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Clone, Debug)]
pub(crate) struct DimensionSignal {
    pub(crate) dimensions: (u32, u32),
    pub(crate) property: String,
}

#[derive(Clone, Debug)]
pub(crate) enum SuiteDimensions {
    None,
    Unique(DimensionSignal),
    Ambiguous(BTreeSet<(u32, u32)>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SuiteOrientation {
    None,
    Unique(CanvasOrientation),
    Ambiguous,
}

pub(crate) fn strongest_suite_dimension(
    properties: &BTreeMap<String, String>,
    candidates: &[Candidate],
) -> SuiteDimensions {
    let supported = candidates
        .iter()
        .map(|candidate| candidate.selection.canvas_dimensions())
        .collect::<HashSet<_>>();
    let mut selected_priority = None;
    let mut selected = BTreeMap::new();
    for (key, value) in properties {
        let Some(priority) = dimension_property_priority(key) else {
            continue;
        };
        for dimensions in dimension_pairs(value) {
            if !supported.contains(&dimensions) {
                continue;
            }
            match selected_priority {
                Some(current) if priority < current => continue,
                Some(current) if priority > current => selected.clear(),
                Some(_) | None => {}
            }
            selected_priority = Some(priority);
            selected.entry(dimensions).or_insert_with(|| key.clone());
        }
    }
    if selected.len() > 1 {
        SuiteDimensions::Ambiguous(selected.into_keys().collect())
    } else if let Some((dimensions, property)) = selected.into_iter().next() {
        SuiteDimensions::Unique(DimensionSignal {
            dimensions,
            property,
        })
    } else {
        SuiteDimensions::None
    }
}

fn dimension_property_priority(key: &str) -> Option<u8> {
    let key = key.to_ascii_lowercase();
    if key.contains("target-display-size") {
        Some(5)
    } else if key.contains("original-display-size") {
        Some(4)
    } else if key.contains("display-size")
        || key.contains("screen-size")
        || key.contains("resolution")
    {
        Some(3)
    } else if key == "build-configuration" || key == "build-config" {
        Some(2)
    } else {
        None
    }
}

pub(crate) fn apply_suite_family_hints(
    candidates: &mut [Candidate],
    profiles: &[DeviceProfile],
    properties: &BTreeMap<String, String>,
) {
    let has_prefix = |prefix: &str| {
        properties.keys().any(|key| {
            key.get(..prefix.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
        })
    };
    let nokia = has_prefix("nokia-");
    let sony = has_prefix("sony-ericsson-") || has_prefix("sonyericsson-");
    let siemens = has_prefix("siemens-");
    let benq_siemens = has_prefix("benq-siemens-") || has_prefix("benqsiemens-");
    let samsung = has_prefix("samsung-");
    let profile_hint_values = properties
        .iter()
        .filter(|(key, _)| profile_hint_property(key))
        .map(|(_, value)| value.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if !nokia && !sony && !siemens && !benq_siemens && !samsung && profile_hint_values.is_empty() {
        return;
    }
    let descriptor_matches = profiles
        .iter()
        .map(|profile| descriptor_profile_matches(profile, &profile_hint_values))
        .collect::<Vec<_>>();
    let manufacturers = profiles
        .iter()
        .map(|profile| profile.device().manufacturer().to_ascii_lowercase())
        .collect::<Vec<_>>();
    for candidate in candidates.iter_mut().filter(|candidate| candidate.eligible) {
        let manufacturer = &manufacturers[candidate.profile_index];
        if nokia && manufacturer.contains("nokia") {
            candidate.add(
                700,
                0,
                EvidenceStrength::SuiteHint,
                EvidenceStrength::None,
                SelectionReason::new("suite-family", "Nokia descriptor attributes"),
            );
        }
        if sony && (manufacturer.contains("sony") || manufacturer.contains("ericsson")) {
            candidate.add(
                700,
                0,
                EvidenceStrength::SuiteHint,
                EvidenceStrength::None,
                SelectionReason::new("suite-family", "Sony Ericsson descriptor attributes"),
            );
        }
        if siemens && manufacturer.contains("siemens") {
            candidate.add(
                700,
                0,
                EvidenceStrength::SuiteHint,
                EvidenceStrength::None,
                SelectionReason::new("suite-family", "Siemens descriptor attributes"),
            );
        }
        if benq_siemens && manufacturer == "benq-siemens" {
            candidate.add(
                750,
                0,
                EvidenceStrength::SuiteHint,
                EvidenceStrength::None,
                SelectionReason::new("suite-family", "BenQ-Siemens descriptor attributes"),
            );
        }
        if samsung && manufacturer == "samsung" {
            candidate.add(
                700,
                0,
                EvidenceStrength::SuiteHint,
                EvidenceStrength::None,
                SelectionReason::new("suite-family", "Samsung descriptor attributes"),
            );
        }

        let descriptor_matches = descriptor_matches[candidate.profile_index];
        if descriptor_matches != 0 {
            candidate.add(
                i32::try_from(descriptor_matches).unwrap_or(4) * 120,
                0,
                EvidenceStrength::Descriptor,
                EvidenceStrength::None,
                SelectionReason::new(
                    "suite-profile-hint",
                    format!("{descriptor_matches} profile token match(es)"),
                ),
            );
        }
    }
}

fn descriptor_profile_matches(profile: &DeviceProfile, normalized_hints: &[String]) -> usize {
    if normalized_hints.is_empty() {
        return 0;
    }
    let tokens = profile_hint_tokens(profile);
    normalized_hints
        .iter()
        .flat_map(|hint| {
            tokens
                .iter()
                .filter(move |token| hint.contains(token.as_str()))
        })
        .take(4)
        .count()
}

fn profile_hint_property(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("platform")
        || key.contains("device")
        || key == "build-configuration"
        || key == "build-config"
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CapabilitySupport {
    Unsupported,
    Compatibility,
    Native,
}

pub(crate) fn apply_static_requirements(
    candidates: &mut [Candidate],
    profiles: &[DeviceProfile],
    external_classes: &[String],
) {
    let requirements = external_classes
        .iter()
        .filter_map(|name| java_capability_for_class(name))
        .collect::<BTreeSet<_>>();
    for requirement in requirements {
        let support = profiles
            .iter()
            .map(|profile| capability_support(profile, requirement))
            .collect::<Vec<_>>();
        if !support
            .iter()
            .any(|level| *level != CapabilitySupport::Unsupported)
        {
            continue;
        }
        for candidate in candidates.iter_mut().filter(|candidate| candidate.eligible) {
            let level = support[candidate.profile_index];
            if level == CapabilitySupport::Unsupported {
                candidate.eligible = false;
                continue;
            }
            match requirement {
                JavaCapability::VendorApi(api) => candidate.add(
                    if level == CapabilitySupport::Native {
                        800
                    } else {
                        450
                    },
                    0,
                    EvidenceStrength::Static,
                    EvidenceStrength::None,
                    SelectionReason::new(
                        "static-vendor-api",
                        format!(
                            "{api} {}",
                            if level == CapabilitySupport::Native {
                                "native"
                            } else {
                                "compatibility"
                            }
                        ),
                    ),
                ),
                JavaCapability::Jsr(jsr) => candidate.add(
                    80,
                    0,
                    EvidenceStrength::Static,
                    EvidenceStrength::None,
                    SelectionReason::new("static-jsr", jsr),
                ),
            }
        }
    }

    let manufacturer_hints = external_classes
        .iter()
        .filter_map(|name| {
            if name.starts_with("com/sonyericsson/") || name.starts_with("com/sony/") {
                Some("sony")
            } else if name.starts_with("com/nokia/") {
                Some("nokia")
            } else if name.starts_with("com/siemens/") {
                Some("siemens")
            } else if name.starts_with("com/samsung/") {
                Some("samsung")
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    for hint in manufacturer_hints {
        for candidate in candidates.iter_mut().filter(|candidate| candidate.eligible) {
            let manufacturer = profiles[candidate.profile_index]
                .device()
                .manufacturer()
                .to_ascii_lowercase();
            if manufacturer.contains(hint) {
                candidate.add(
                    300,
                    0,
                    EvidenceStrength::Static,
                    EvidenceStrength::None,
                    SelectionReason::new("static-manufacturer-api", hint),
                );
            }
        }
    }
}

fn capability_support(profile: &DeviceProfile, requirement: JavaCapability) -> CapabilitySupport {
    match requirement {
        JavaCapability::Jsr(jsr) => jsr_support(profile, jsr),
        JavaCapability::VendorApi(api) => vendor_api_support(profile, api),
    }
}

fn jsr_support(profile: &DeviceProfile, jsr: &str) -> CapabilitySupport {
    if profile
        .java()
        .jsrs()
        .get(jsr)
        .and_then(|value| value.value())
        == Some(&true)
    {
        CapabilitySupport::Native
    } else if profile
        .java()
        .compatibility_jsrs()
        .value()
        .is_some_and(|jsrs| jsrs.iter().any(|candidate| candidate == jsr))
    {
        CapabilitySupport::Compatibility
    } else {
        CapabilitySupport::Unsupported
    }
}

fn vendor_api_support(profile: &DeviceProfile, api: &str) -> CapabilitySupport {
    if profile
        .java()
        .vendor_apis()
        .value()
        .is_some_and(|apis| apis.iter().any(|candidate| candidate == api))
    {
        CapabilitySupport::Native
    } else if profile
        .java()
        .compatibility_apis()
        .value()
        .is_some_and(|apis| apis.iter().any(|candidate| candidate == api))
    {
        CapabilitySupport::Compatibility
    } else {
        CapabilitySupport::Unsupported
    }
}

pub(crate) fn suite_supports_pointer(properties: &BTreeMap<String, String>) -> bool {
    property_value(properties, "midlet-touch-support").is_some_and(is_affirmative)
}

fn property_value<'properties>(
    properties: &'properties BTreeMap<String, String>,
    name: &str,
) -> Option<&'properties str> {
    properties
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn is_affirmative(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "on" | "1"
    )
}

pub(crate) fn suite_orientation(properties: &BTreeMap<String, String>) -> SuiteOrientation {
    let mut requested = None;
    for (key, value) in properties {
        let key = key.to_ascii_lowercase();
        if key != "midlet-orientation" && !key.ends_with("-app-orientation") {
            continue;
        }
        let candidate = match value.trim().to_ascii_lowercase().as_str() {
            "portrait" => CanvasOrientation::Portrait,
            "landscape" => CanvasOrientation::Landscape,
            _ => continue,
        };
        match requested {
            Some(previous) if previous != candidate => return SuiteOrientation::Ambiguous,
            Some(_) => {}
            None => requested = Some(candidate),
        }
    }
    requested.map_or(SuiteOrientation::None, SuiteOrientation::Unique)
}
