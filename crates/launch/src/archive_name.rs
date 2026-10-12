//! Weak, separator-insensitive evidence from distribution filenames.

use crate::archive_evidence::ArchiveEvidence;
use crate::decision::{CanvasOrientation, OrientationMatch, SelectionReason};
use crate::model_aliases::{
    ModelAliasIndex, nokia_code_alias, nokia_compact_model_alias, nokia_numeric_code,
    samsung_model_alias, samsung_short_model_alias, siemens_model_alias,
    sony_ericsson_model_tokens,
};
use crate::resolver::{Candidate, EvidenceStrength};
use device_profile::{DeviceProfile, dimension_pairs};
use std::collections::{BTreeSet, HashSet};

mod normalized;
use normalized::archive_orientation_hint;
pub(crate) use normalized::{NormalizedArchiveName, separator_tokens};

pub(crate) fn apply_archive_hints(
    candidates: &mut [Candidate],
    profiles: &[DeviceProfile],
    archive: &ArchiveEvidence,
    screen_override: Option<(u32, u32)>,
    archive_screen_hints_blocked: bool,
) {
    let Some(name) = archive.archive_name() else {
        return;
    };
    let archive_name = NormalizedArchiveName::new(name);
    apply_archive_model_hints(
        candidates,
        profiles,
        &archive_name,
        screen_override.is_none() && !archive_screen_hints_blocked,
    );
    let profile_tokens = profiles
        .iter()
        .map(archive_profile_hint_tokens)
        .collect::<Vec<_>>();
    let profile_matches = profile_tokens
        .iter()
        .map(|tokens| archive_name.profile_hint_matches(tokens).min(6))
        .collect::<Vec<_>>();
    if screen_override.is_none() && !archive_screen_hints_blocked {
        apply_archive_screen_hints(candidates, &archive_name, &profile_tokens, &profile_matches);
    }

    for candidate in candidates.iter_mut().filter(|candidate| candidate.eligible) {
        let matches = profile_matches[candidate.profile_index];
        if matches != 0 {
            candidate.add(
                i32::try_from(matches).unwrap_or(6) * 35,
                0,
                EvidenceStrength::Archive,
                EvidenceStrength::None,
                SelectionReason::new(
                    "archive-profile-token",
                    format!("{matches} profile token match(es)"),
                ),
            );
        }
    }
}

fn apply_archive_screen_hints(
    candidates: &mut [Candidate],
    archive_name: &NormalizedArchiveName,
    profile_tokens: &[BTreeSet<String>],
    profile_matches: &[usize],
) {
    let declared_dimensions = candidates
        .iter()
        .filter(|candidate| candidate.eligible)
        .map(|candidate| candidate.selection.canvas_dimensions())
        .collect::<HashSet<_>>();
    let pairs = dimension_pairs(&archive_name.normalized)
        .into_iter()
        .filter(|dimensions| declared_dimensions.contains(dimensions))
        .collect::<HashSet<_>>();
    let axis_hints = pairs.is_empty().then(|| archive_name.axis_hints());
    let orientation = archive_orientation_hint(archive_name);
    for candidate in candidates.iter_mut().filter(|candidate| candidate.eligible) {
        let dimensions = candidate.selection.canvas_dimensions();
        let separated_dimensions = pairs.contains(&dimensions);
        let compact_dimensions = !separated_dimensions
            && profile_matches[candidate.profile_index] != 0
            && archive_name.contains_contextual_dimensions(
                dimensions,
                &profile_tokens[candidate.profile_index],
            );
        if separated_dimensions || compact_dimensions {
            let (code, detail) = if compact_dimensions {
                (
                    "archive-compact-dimensions",
                    format!(
                        "{}{} -> {}x{}",
                        dimensions.0, dimensions.1, dimensions.0, dimensions.1
                    ),
                )
            } else {
                (
                    "archive-dimensions",
                    format!("{}x{}", dimensions.0, dimensions.1),
                )
            };
            candidate.add(
                350,
                350,
                EvidenceStrength::Archive,
                EvidenceStrength::Archive,
                SelectionReason::new(code, detail),
            );
        } else {
            apply_archive_axis_hint(candidate, dimensions, axis_hints.as_ref(), profile_matches);
        }
        if orientation.is_some_and(|orientation| {
            orientation.match_quality(dimensions) == OrientationMatch::Directional
        }) {
            candidate.add(
                0,
                200,
                EvidenceStrength::None,
                EvidenceStrength::Archive,
                SelectionReason::new(
                    "archive-orientation",
                    orientation.map_or("", CanvasOrientation::as_str),
                ),
            );
        }
    }
}

fn apply_archive_axis_hint(
    candidate: &mut Candidate,
    dimensions: (u32, u32),
    axis_hints: Option<&HashSet<u32>>,
    profile_matches: &[usize],
) {
    if profile_matches[candidate.profile_index] == 0 {
        return;
    }
    let Some(axis) = axis_hints.and_then(|hints| {
        [dimensions.0, dimensions.1]
            .into_iter()
            .find(|axis| hints.contains(axis))
    }) else {
        return;
    };
    candidate.add(
        0,
        if dimensions == (axis, axis) { 140 } else { 120 },
        EvidenceStrength::None,
        EvidenceStrength::Archive,
        SelectionReason::new(
            "archive-screen-axis",
            format!("{axis} -> {}x{}", dimensions.0, dimensions.1),
        ),
    );
}

#[derive(Debug, Default)]
pub(crate) struct ArchiveModelMatch {
    pub(crate) models: String,
    specificity: usize,
    dimensions: BTreeSet<(u32, u32)>,
}

fn apply_archive_model_hints(
    candidates: &mut [Candidate],
    profiles: &[DeviceProfile],
    archive_name: &NormalizedArchiveName,
    allow_screen_hint: bool,
) {
    let matches = archive_model_matches(profiles, archive_name);
    let orientation = archive_orientation_hint(archive_name);
    for candidate in candidates.iter_mut().filter(|candidate| candidate.eligible) {
        let model_match = &matches[candidate.profile_index];
        if model_match.models.is_empty() {
            continue;
        }
        candidate.add(
            420 + i32::try_from(model_match.specificity.min(16)).unwrap_or(16) * 20,
            0,
            EvidenceStrength::Archive,
            EvidenceStrength::None,
            SelectionReason::new("archive-device-model", model_match.models.clone()),
        );
        let canvas = candidate.selection.canvas_dimensions();
        let model_canvas = if model_match.dimensions.len() == 1 {
            model_match.dimensions.iter().next().copied()
        } else {
            None
        }
        .map(|dimensions| {
            if orientation.is_some_and(|orientation| {
                orientation.match_quality(dimensions) == OrientationMatch::Mismatch
            }) {
                (dimensions.1, dimensions.0)
            } else {
                dimensions
            }
        });
        if allow_screen_hint && model_canvas == Some(canvas) {
            candidate.add(
                0,
                150,
                EvidenceStrength::None,
                EvidenceStrength::Archive,
                SelectionReason::new(
                    "archive-model-screen",
                    format!("{}={}x{}", model_match.models, canvas.0, canvas.1),
                ),
            );
        }
    }
}

pub(crate) fn archive_model_matches(
    profiles: &[DeviceProfile],
    archive_name: &NormalizedArchiveName,
) -> Vec<ArchiveModelMatch> {
    let aliases = ModelAliasIndex::new(profiles);
    let matches = profiles
        .iter()
        .map(|profile| {
            let mut matches = Vec::new();
            for hint in profile.device().archive_name_hints() {
                for model in hint.model_names() {
                    if let Some(specificity) = archive_model_specificity(
                        archive_name,
                        model,
                        profile.device().manufacturer(),
                        &aliases,
                    ) {
                        matches.push((specificity, model, hint));
                    }
                }
            }
            matches
        })
        .collect::<Vec<_>>();
    let specificity = matches.iter().flatten().map(|(length, _, _)| *length).max();
    matches
        .into_iter()
        .map(|matches| {
            let mut result = ArchiveModelMatch::default();
            for (length, model, hint) in matches
                .into_iter()
                .filter(|(length, _, _)| Some(*length) == specificity)
            {
                if !result.models.is_empty() {
                    result.models.push('|');
                }
                result.models.push_str(model);
                result.specificity = length;
                let canvas = hint.fullscreen_canvas();
                result.dimensions.insert((canvas.width(), canvas.height()));
            }
            result
        })
        .collect()
}

pub(crate) fn archive_model_specificity(
    archive_name: &NormalizedArchiveName,
    model: &str,
    manufacturer: &str,
    aliases: &ModelAliasIndex,
) -> Option<usize> {
    let model_tokens = separator_tokens(model);
    let compact_model = model_tokens.concat();
    if archive_name.contains_token_sequence(&model_tokens) {
        return Some(compact_model.len());
    }
    if archive_name.contains_compact_identifier(&compact_model) {
        return Some(compact_model.len());
    }
    if let Some(alias) = sony_ericsson_model_tokens(&model_tokens) {
        let alias = alias.concat();
        if !aliases.ambiguous.contains(&alias) && archive_name.contains_compact_identifier(&alias) {
            return Some(alias.len());
        }
    }
    if let Some(alias) = siemens_model_alias(&model_tokens, manufacturer)
        && aliases.manufacturer.contains(&alias)
        && archive_name.contains_compact_identifier(&alias)
    {
        return Some(alias.len());
    }
    if let Some(alias) = samsung_model_alias(&model_tokens, manufacturer)
        && aliases.samsung.contains(&alias)
        && archive_name.contains_compact_identifier(&alias)
    {
        return Some(alias.len());
    }
    if let Some(alias) = samsung_short_model_alias(&model_tokens, manufacturer)
        && aliases.samsung.contains(&alias)
        && archive_name.contains_compact_identifier(&alias)
    {
        return Some(alias.len());
    }
    if let Some(alias) = nokia_code_alias(&model_tokens)
        && alias != compact_model
        && aliases.nokia_code.contains(&alias)
        && archive_name.contains_compact_identifier(&alias)
    {
        return Some(alias.len());
    }
    if let Some(code) = nokia_numeric_code(&model_tokens)
        && aliases.nokia_numeric.contains(&code)
        && archive_name.contains_digit_run_with_dimension_affix(&code, &aliases.dimension_compacts)
    {
        return Some(code.len());
    }
    let alias = nokia_compact_model_alias(&model_tokens)?;
    (!aliases.ambiguous.contains(&alias) && archive_name.contains_compact_identifier(&alias))
        .then_some(alias.len())
}

pub(crate) fn profile_hint_tokens(profile: &DeviceProfile) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    for value in [
        Some(profile.profile_id()),
        Some(profile.family_id()),
        Some(profile.device().manufacturer()),
        Some(profile.device().model()),
        profile.device().java_platform().value().map(String::as_str),
    ]
    .into_iter()
    .flatten()
    {
        let normalized = value.to_ascii_lowercase();
        let value_tokens = normalized
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
            .collect::<Vec<_>>();
        for token in &value_tokens {
            if useful_profile_token(token) {
                tokens.insert((*token).to_owned());
            }
        }
        for pair in value_tokens.windows(2) {
            if pair[0] == "series" && pair[1].bytes().all(|byte| byte.is_ascii_digit()) {
                tokens.insert(format!("s{}", pair[1]));
            }
        }
    }
    let generation_aliases = tokens
        .iter()
        .filter_map(|token| series_generation(token))
        .flat_map(|(series, edition)| {
            [
                format!("{series}e{edition}"),
                format!("{series}{edition}ed"),
            ]
        })
        .collect::<Vec<_>>();
    tokens.extend(generation_aliases);
    tokens
}

fn archive_profile_hint_tokens(profile: &DeviceProfile) -> BTreeSet<String> {
    let mut tokens = profile_hint_tokens(profile);
    if profile
        .device()
        .manufacturer()
        .eq_ignore_ascii_case("Siemens")
    {
        // `SIE` is a common bounded distribution tag for pre-BenQ Siemens
        // builds. Keep it archive-only and exact-token-only so descriptor
        // prose and ordinary words cannot become device-family evidence, and
        // so BenQ-Siemens remains distinct.
        tokens.insert("sie".to_owned());
    }
    if profile
        .device()
        .manufacturer()
        .eq_ignore_ascii_case("Samsung")
    {
        // SGH is Samsung's documented handset prefix and is common in
        // distribution names even when the manufacturer word is absent.
        tokens.insert("sgh".to_owned());
    }
    tokens
}

fn series_generation(token: &str) -> Option<(&str, &str)> {
    for series in ["s40", "s60"] {
        if let Some(edition) = token
            .strip_prefix(series)
            .and_then(|value| value.strip_prefix('v'))
            && !edition.is_empty()
            && edition.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Some((series, edition));
        }
    }
    None
}

fn useful_profile_token(token: &str) -> bool {
    token.len() >= 3
        && !matches!(
            token,
            "compatibility"
                | "family"
                | "reference"
                | "profile"
                | "device"
                | "edition"
                | "feature"
                | "phone"
                | "and"
        )
}
