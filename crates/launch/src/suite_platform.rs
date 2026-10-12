//! Bounded device identity hints from manifest/JAD deployment metadata.

use crate::decision::SelectionReason;
use crate::resolver::{Candidate, EvidenceStrength};
use device_profile::DeviceProfile;
use std::collections::BTreeMap;

const MAX_DECLARATION_BYTES: usize = 4096;
const MAX_PATTERNS: usize = 64;
const MAX_IDENTITY_BYTES: usize = 128;

pub(crate) fn apply_suite_platform(
    candidates: &mut [Candidate],
    profiles: &[DeviceProfile],
    properties: &BTreeMap<String, String>,
) {
    let Some((_, declaration)) = properties
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("Nokia-Platform"))
    else {
        return;
    };
    let Some(patterns) = platform_patterns(declaration) else {
        return;
    };
    let identities = profiles
        .iter()
        .map(|profile| {
            if profile.device().manufacturer() != "Nokia" {
                return None;
            }
            let identity = profile
                .java()
                .system_properties()
                .value()?
                .get("microedition.platform")?;
            (identity.is_ascii()
                && identity.len() <= MAX_IDENTITY_BYTES
                && patterns
                    .iter()
                    .any(|pattern| platform_matches(identity, pattern)))
            .then_some(identity.as_str())
        })
        .collect::<Vec<_>>();
    // Unknown models remain unresolved hints. Never synthesize an identity
    // from guest text or replace an explicit profile with a different one.
    if !candidates
        .iter()
        .any(|candidate| candidate.eligible && identities[candidate.profile_index].is_some())
    {
        return;
    }
    for candidate in candidates.iter_mut().filter(|candidate| candidate.eligible) {
        if let Some(identity) = identities[candidate.profile_index] {
            candidate.add(
                1500,
                0,
                EvidenceStrength::Descriptor,
                EvidenceStrength::None,
                SelectionReason::new(
                    "suite-profile-hint",
                    format!("Nokia-Platform matches {identity}"),
                ),
            );
        } else {
            candidate.eligible = false;
        }
    }
}

fn platform_patterns(declaration: &str) -> Option<Vec<&str>> {
    if declaration.len() > MAX_DECLARATION_BYTES || !declaration.is_ascii() {
        return None;
    }
    let patterns = declaration.split('@').map(str::trim).collect::<Vec<_>>();
    if patterns.len() > MAX_PATTERNS
        || patterns
            .iter()
            .any(|pattern| pattern.is_empty() || pattern.len() > MAX_IDENTITY_BYTES)
    {
        return None;
    }
    Some(patterns)
}

// Anchored ASCII glob matching, with '?' for one byte and '*' for zero or
// more. Dynamic programming bounds work to pattern length * identity length;
// there is no recursion/backtracking for untrusted descriptor text.
fn platform_matches(identity: &str, pattern: &str) -> bool {
    let mut matched = [false; MAX_IDENTITY_BYTES + 1];
    matched[0] = true;
    for token in pattern.bytes() {
        if token == b'*' {
            for index in 1..=identity.len() {
                matched[index] |= matched[index - 1];
            }
        } else {
            for (index, byte) in identity.bytes().enumerate().rev() {
                matched[index + 1] = matched[index] && (token == b'?' || token == byte);
            }
            matched[0] = false;
        }
    }
    matched[identity.len()]
}

#[cfg(test)]
#[path = "../../../tests/unit/launch/suite_platform.rs"]
mod tests;
