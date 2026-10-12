//! Suite selection and merged manifest/JAD properties.

use diagnostics::{Category, EmuError};
use jar::{JadInfo, JarInfo, MidletInfo};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuiteDescriptor {
    pub midlet: MidletInfo,
    pub properties: BTreeMap<String, String>,
}

/// Combines declarations by index; a JAD overrides only the entries it supplies.
/// MIDP permits `MIDlet-<n>` attributes in either the manifest or the descriptor.
#[must_use]
pub fn suite_midlets(jar: &JarInfo, jad: Option<&JadInfo>) -> Vec<MidletInfo> {
    jar.midlets
        .iter()
        .chain(jad.into_iter().flat_map(|jad| &jad.midlets))
        .map(|midlet| (midlet.index, midlet))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .cloned()
        .collect()
}

/// Selects a `MIDlet` and merges JAD properties over manifest properties.
/// Without an explicit index, the first declared `MIDlet` is selected.
/// Property lookup remains case-insensitive as required by manifest/JAD parsing.
/// Application values omit surrounding MIDP space/tab padding after unfolding.
///
/// # Errors
/// Returns a controlled AMS diagnostic when the suite has no `MIDlet` or the
/// explicitly requested entry does not exist.
pub fn describe_suite(
    jar: &JarInfo,
    jad: Option<&JadInfo>,
    requested_index: Option<u32>,
) -> Result<SuiteDescriptor, EmuError> {
    let candidates = suite_midlets(jar, jad);
    let midlet = match requested_index {
        Some(index) => candidates.iter().find(|midlet| midlet.index == index),
        None => candidates.first(),
    }
    .cloned()
    .ok_or_else(|| {
        EmuError::new(
            Category::Api,
            "midlet-selection",
            if requested_index.is_some() {
                "requested MIDlet entry does not exist"
            } else {
                "suite does not declare a MIDlet"
            },
        )
    })?;
    let mut properties = BTreeMap::new();
    // JAD overrides the manifest after unfolding; trim only outer MIDP padding.
    for (key, value) in jar
        .manifest
        .iter()
        .chain(jad.into_iter().flat_map(|jad| &jad.properties))
    {
        properties.insert(
            key.to_ascii_lowercase(),
            value.trim_matches([' ', '\t']).to_owned(),
        );
    }
    Ok(SuiteDescriptor { midlet, properties })
}

#[cfg(test)]
#[path = "../../../tests/unit/midp/suite.rs"]
mod tests;
