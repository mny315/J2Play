//! Device-profile and logical-screen resolution implementation.

use crate::archive_evidence::ArchiveEvidence;
use crate::archive_name::apply_archive_hints;
use crate::catalog::validate_catalog;
use crate::decision::{
    DeviceCandidate, DeviceDecision, DeviceSelection, ProfileSelectionSource, SelectionConfidence,
    SelectionOverrides, SelectionReason,
};
use crate::suite_evidence::{apply_static_requirements, apply_suite_family_hints};
use device_profile::{DeviceProfile, ProfileOverrides, ScreenModeSelection};
use diagnostics::{Category, EmuError};
use std::collections::BTreeMap;

mod constraints;
mod recovery;
use constraints::{apply_profile_override, apply_suite_constraints};
pub use recovery::managed_heap_recovery_candidates;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum EvidenceStrength {
    None,
    // Application capability declarations can break a complete fallback tie,
    // but they do not describe the target hardware. An exact archive screen or
    // family hint must therefore outrank them.
    ApplicationHint,
    Archive,
    SuiteHint,
    Static,
    Descriptor,
    Explicit,
}

impl EvidenceStrength {
    const fn selection_rank(self) -> u8 {
        match self {
            Self::None | Self::ApplicationHint => 0,
            // A vendor-prefixed descriptor attribute without target geometry
            // is only a family affinity. Keep it in the same selection tier as
            // archive metadata so an exact declared Canvas can select the
            // concrete profile/screen pair.
            Self::Archive | Self::SuiteHint => 1,
            Self::Static => 2,
            Self::Descriptor => 3,
            Self::Explicit => 4,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    pub(crate) profile_index: usize,
    pub(crate) selection: DeviceSelection,
    pub(crate) canonical_orientation: bool,
    pub(crate) default_mode: bool,
    pub(crate) eligible: bool,
    pub(crate) profile_score: i32,
    pub(crate) screen_score: i32,
    pub(crate) profile_strength: EvidenceStrength,
    pub(crate) screen_strength: EvidenceStrength,
    pub(crate) reasons: Vec<SelectionReason>,
}

impl Candidate {
    pub(crate) fn add(
        &mut self,
        profile_score: i32,
        screen_score: i32,
        profile_strength: EvidenceStrength,
        screen_strength: EvidenceStrength,
        reason: SelectionReason,
    ) {
        self.profile_score = self.profile_score.saturating_add(profile_score);
        self.screen_score = self.screen_score.saturating_add(screen_score);
        self.profile_strength = self.profile_strength.max(profile_strength);
        self.screen_strength = self.screen_strength.max(screen_strength);
        if !self.reasons.iter().any(|existing| existing == &reason) {
            self.reasons.push(reason);
        }
    }
}

/// Automatically selects a profile family and exact screen mode.
///
/// Strong descriptor and static-API evidence constrains candidates first.
/// The archive leaf name is only a weak tie-breaker and never overrides an
/// explicit or descriptor constraint. When a generic suite contains no
/// distinguishing evidence, catalog order and each profile's declared default
/// mode provide a stable compatibility fallback. Equally supported pairs are
/// retained in [`DeviceDecision::candidates`] so an interactive frontend can
/// ask the user without reimplementing ranking. No guest bytecode is executed.
///
/// # Errors
/// Returns a controlled diagnostic for an invalid catalog, unknown override,
/// or contradictory requirements which match no declared profile/mode pair.
pub fn resolve_device_selection(
    profiles: &[DeviceProfile],
    suite_properties: &BTreeMap<String, String>,
    archive: &ArchiveEvidence,
    overrides: SelectionOverrides<'_>,
) -> Result<DeviceDecision, EmuError> {
    validate_catalog(profiles, overrides.profile_id.is_none())?;
    let candidates = selection_candidates(profiles, suite_properties, archive, overrides)?;
    let decision = choose_decision(&candidates, overrides.any())?;
    if overrides.profile_id.is_some() {
        Ok(decision)
    } else {
        Ok(prefer_compatibility_hosts(decision, profiles))
    }
}

fn selection_candidates(
    profiles: &[DeviceProfile],
    suite_properties: &BTreeMap<String, String>,
    archive: &ArchiveEvidence,
    overrides: SelectionOverrides<'_>,
) -> Result<Vec<Candidate>, EmuError> {
    let mut candidates = declared_candidates(profiles, overrides.canvas_dimensions)?;
    apply_profile_override(&mut candidates, profiles, overrides.profile_id)?;
    let archive_screen_hints_blocked = apply_suite_constraints(
        &mut candidates,
        profiles,
        suite_properties,
        overrides.canvas_dimensions,
        overrides.orientation,
        overrides.pointer_events,
    )?;
    apply_suite_family_hints(&mut candidates, profiles, suite_properties);
    apply_static_requirements(&mut candidates, profiles, archive.external_classes());
    apply_archive_hints(
        &mut candidates,
        profiles,
        archive,
        overrides.canvas_dimensions,
        archive_screen_hints_blocked,
    );
    Ok(candidates)
}

fn choose_decision(
    candidates: &[Candidate],
    has_override: bool,
) -> Result<DeviceDecision, EmuError> {
    let mut eligible = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.eligible)
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return Err(selection_error(
            "device-selection-no-match",
            "suite evidence and explicit overrides match no declared device variant",
        ));
    }
    eligible.sort_by(|left, right| compare_candidate_order(*left, *right));
    let (best_index, winner) = eligible[0];
    let profile_evidence_tied = eligible.iter().any(|(index, candidate)| {
        *index != best_index
            && candidate.profile_strength == winner.profile_strength
            && candidate.profile_score == winner.profile_score
            && candidate.selection.profile_id() != winner.selection.profile_id()
    });
    let mut reasons = winner.reasons.clone();
    let profile_source = profile_source(winner.profile_strength, profile_evidence_tied);
    let screen_source = screen_source(winner.screen_strength);
    if profile_source == ProfileSelectionSource::CompatibilityFallback {
        reasons.push(SelectionReason::new(
            "compatibility-fallback",
            "no unique device-family evidence; used stable catalog order",
        ));
    }
    if winner.screen_strength == EvidenceStrength::None {
        reasons.push(SelectionReason::new(
            "profile-default-mode",
            winner
                .selection
                .screen_mode_id()
                .unwrap_or("profile canvas")
                .to_owned(),
        ));
    }
    let confidence = decision_confidence(
        winner.profile_strength,
        winner.screen_strength,
        profile_evidence_tied,
        has_override,
    );
    let candidates = equally_supported_candidates(&eligible, winner)
        .into_iter()
        .map(|candidate| DeviceCandidate {
            target_selection: candidate.selection.clone(),
            selection: candidate.selection.clone(),
            reasons: candidate.reasons.clone(),
        })
        .collect();
    Ok(DeviceDecision {
        target_selection: winner.selection.clone(),
        selection: winner.selection.clone(),
        automatic_host_selected: false,
        profile_source,
        screen_source,
        confidence,
        reasons,
        candidates,
    })
}

fn equally_supported_candidates<'candidates>(
    ranked: &[(usize, &'candidates Candidate)],
    winner: &Candidate,
) -> Vec<&'candidates Candidate> {
    let winner_rank = candidate_evidence_rank(winner);
    let mut candidates = ranked
        .iter()
        .map(|(_, candidate)| *candidate)
        .take_while(|candidate| candidate_evidence_rank(candidate) == winner_rank)
        .collect::<Vec<_>>();
    let multiple_profiles = candidates
        .iter()
        .any(|candidate| candidate.profile_index != winner.profile_index);
    if multiple_profiles && winner.screen_strength == EvidenceStrength::None {
        candidates.retain(|candidate| candidate.default_mode && candidate.canonical_orientation);
    }
    candidates
}

fn prefer_compatibility_hosts(
    mut decision: DeviceDecision,
    profiles: &[DeviceProfile],
) -> DeviceDecision {
    let mut hosted = Vec::<DeviceCandidate>::new();
    for target in &decision.candidates {
        let target_selection = target.target_selection();
        let Some(target_profile) = profiles
            .iter()
            .find(|profile| profile.profile_id() == target_selection.profile_id())
        else {
            continue;
        };
        let host_id = target_profile.composition().persona().automatic_host();
        let Some(host) = profiles
            .iter()
            .find(|profile| profile.profile_id() == host_id)
        else {
            continue;
        };
        let dimensions = target_selection.canvas_dimensions();
        let host_selection = DeviceSelection::new(
            host.profile_id(),
            target_selection.screen_mode_id.clone(),
            dimensions,
        );

        let mut reasons = if target_selection == &decision.target_selection {
            decision.reasons.clone()
        } else {
            target.reasons().to_vec()
        };
        if host.profile_id() != target_selection.profile_id() {
            reasons.push(SelectionReason::new(
                "compatibility-host",
                format!(
                    "{} -> {} at {}x{}",
                    target_selection.profile_id(),
                    host.profile_id(),
                    dimensions.0,
                    dimensions.1,
                ),
            ));
        }
        hosted.push(DeviceCandidate {
            target_selection: target_selection.clone(),
            selection: host_selection,
            reasons,
        });
    }

    if let Some(winner) = hosted.first() {
        decision.target_selection = winner.target_selection.clone();
        decision.selection = winner.selection.clone();
        decision.automatic_host_selected = true;
        decision.reasons.clone_from(&winner.reasons);
        decision.candidates = hosted;
    }
    decision
}

fn declared_candidates(
    profiles: &[DeviceProfile],
    screen_override: Option<(u32, u32)>,
) -> Result<Vec<Candidate>, EmuError> {
    let mut candidates = Vec::new();
    for (profile_index, profile) in profiles.iter().enumerate() {
        if profile.display().screen_modes().is_empty() {
            let dimensions = profile.canvas_dimensions().ok_or_else(|| {
                selection_error(
                    "device-selection-catalog",
                    format!(
                        "device profile {} has no runnable Canvas dimensions",
                        profile.profile_id()
                    ),
                )
            })?;
            push_candidate(
                &mut candidates,
                profile_index,
                profile,
                None,
                dimensions,
                true,
                true,
            );
            if dimensions.0 != dimensions.1 {
                push_candidate(
                    &mut candidates,
                    profile_index,
                    profile,
                    None,
                    (dimensions.1, dimensions.0),
                    false,
                    true,
                );
            }
            if let Some(override_dimensions) = screen_override
                && override_dimensions != dimensions
                && override_dimensions != (dimensions.1, dimensions.0)
                && profile
                    .resolve(
                        ProfileOverrides::default()
                            .with_canvas_dimensions(override_dimensions.0, override_dimensions.1),
                    )
                    .is_ok()
            {
                push_candidate(
                    &mut candidates,
                    profile_index,
                    profile,
                    None,
                    override_dimensions,
                    false,
                    false,
                );
            }
            continue;
        }
        for mode in profile.display().screen_modes() {
            let fullscreen = mode.fullscreen_canvas();
            let dimensions = (fullscreen.width(), fullscreen.height());
            let default_mode = profile.display().default_screen_mode() == Some(mode.id());
            push_candidate(
                &mut candidates,
                profile_index,
                profile,
                Some(mode.id().to_owned()),
                dimensions,
                true,
                default_mode,
            );
            if dimensions.0 != dimensions.1 {
                push_candidate(
                    &mut candidates,
                    profile_index,
                    profile,
                    Some(mode.id().to_owned()),
                    (dimensions.1, dimensions.0),
                    false,
                    default_mode,
                );
            }
        }
    }
    Ok(candidates)
}

#[allow(clippy::too_many_arguments)]
fn push_candidate(
    output: &mut Vec<Candidate>,
    profile_index: usize,
    profile: &DeviceProfile,
    screen_mode_id: Option<String>,
    canvas_dimensions: (u32, u32),
    canonical_orientation: bool,
    default_mode: bool,
) {
    output.push(Candidate {
        profile_index,
        selection: DeviceSelection::new(profile.profile_id(), screen_mode_id, canvas_dimensions),
        canonical_orientation,
        default_mode,
        eligible: true,
        profile_score: 0,
        screen_score: 0,
        profile_strength: EvidenceStrength::None,
        screen_strength: EvidenceStrength::None,
        reasons: Vec::new(),
    });
}

fn candidate_evidence_rank(candidate: &Candidate) -> (u8, u8, i32, i32) {
    // Within the same evidence tier, an exact screen match beats the family score.
    // Archive names still cannot override descriptor or API requirements.
    (
        candidate.profile_strength.selection_rank(),
        candidate.screen_strength.selection_rank(),
        candidate.profile_score,
        candidate.screen_score,
    )
}

fn compare_candidate_order(
    left: (usize, &Candidate),
    right: (usize, &Candidate),
) -> std::cmp::Ordering {
    let (left_index, left) = left;
    let (right_index, right) = right;
    candidate_evidence_rank(right)
        .cmp(&candidate_evidence_rank(left))
        .then_with(|| left.profile_index.cmp(&right.profile_index))
        .then_with(|| right.default_mode.cmp(&left.default_mode))
        .then_with(|| right.canonical_orientation.cmp(&left.canonical_orientation))
        .then_with(|| left_index.cmp(&right_index))
}

fn profile_source(strength: EvidenceStrength, tied_score: bool) -> ProfileSelectionSource {
    if tied_score {
        return ProfileSelectionSource::CompatibilityFallback;
    }
    match strength {
        EvidenceStrength::Explicit => ProfileSelectionSource::ExplicitOverride,
        EvidenceStrength::Descriptor
        | EvidenceStrength::SuiteHint
        | EvidenceStrength::ApplicationHint => ProfileSelectionSource::SuiteMetadata,
        EvidenceStrength::Static => ProfileSelectionSource::StaticApi,
        EvidenceStrength::Archive => ProfileSelectionSource::ArchiveName,
        EvidenceStrength::None => ProfileSelectionSource::CompatibilityFallback,
    }
}

fn screen_source(strength: EvidenceStrength) -> ScreenModeSelection {
    match strength {
        EvidenceStrength::Explicit => ScreenModeSelection::ExplicitOverride,
        EvidenceStrength::Descriptor => ScreenModeSelection::SuiteMetadata,
        EvidenceStrength::Archive => ScreenModeSelection::ArchiveMetadata,
        EvidenceStrength::ApplicationHint
        | EvidenceStrength::SuiteHint
        | EvidenceStrength::Static
        | EvidenceStrength::None => ScreenModeSelection::ProfileDefault,
    }
}

fn decision_confidence(
    profile: EvidenceStrength,
    screen: EvidenceStrength,
    tied_score: bool,
    has_override: bool,
) -> SelectionConfidence {
    if has_override && profile == EvidenceStrength::Explicit && screen == EvidenceStrength::Explicit
    {
        SelectionConfidence::Explicit
    } else if tied_score || profile == EvidenceStrength::None || screen == EvidenceStrength::None {
        SelectionConfidence::Fallback
    } else if profile.selection_rank() >= EvidenceStrength::Static.selection_rank()
        && screen.selection_rank() >= EvidenceStrength::Static.selection_rank()
    {
        SelectionConfidence::High
    } else {
        SelectionConfidence::Medium
    }
}

fn selection_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Api, code, message.into())
}

#[cfg(test)]
#[path = "../../../tests/unit/launch/resolver/mod.rs"]
mod tests;
