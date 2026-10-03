//! Explicit and descriptor constraints on device-profile candidates.

use super::{Candidate, EvidenceStrength, selection_error};
use crate::decision::{CanvasOrientation, OrientationMatch, SelectionReason};
use crate::suite_evidence::{
    SuiteDimensions, SuiteOrientation, strongest_suite_dimension, suite_orientation,
    suite_supports_pointer,
};
use device_profile::{DeviceProfile, ProfileOverrides};
use diagnostics::EmuError;
use std::collections::BTreeMap;

pub(super) fn apply_profile_override(
    candidates: &mut [Candidate],
    profiles: &[DeviceProfile],
    profile_override: Option<&str>,
) -> Result<(), EmuError> {
    if let Some(profile_id) = profile_override {
        if !profiles
            .iter()
            .any(|profile| profile.profile_id() == profile_id)
        {
            let available = profiles
                .iter()
                .map(DeviceProfile::profile_id)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(selection_error(
                "device-selection-profile",
                format!("unknown device profile {profile_id}; available: {available}"),
            ));
        }
        for candidate in candidates {
            if candidate.selection.profile_id() == profile_id {
                candidate.add(
                    10_000,
                    0,
                    EvidenceStrength::Explicit,
                    EvidenceStrength::None,
                    SelectionReason::new("profile-override", profile_id),
                );
            } else {
                candidate.eligible = false;
            }
        }
    }
    Ok(())
}

pub(super) fn apply_suite_constraints(
    candidates: &mut [Candidate],
    profiles: &[DeviceProfile],
    suite_properties: &BTreeMap<String, String>,
    screen_override: Option<(u32, u32)>,
    orientation_override: Option<CanvasOrientation>,
    pointer_override: bool,
) -> Result<bool, EmuError> {
    let descriptor_orientation = suite_orientation(suite_properties);
    let effective_orientation = orientation_override.or(match descriptor_orientation {
        SuiteOrientation::Unique(orientation) => Some(orientation),
        SuiteOrientation::None | SuiteOrientation::Ambiguous => None,
    });
    let suite_dimensions = apply_screen_constraint(
        candidates,
        suite_properties,
        screen_override,
        effective_orientation,
    );
    apply_touch_support_hint(candidates, profiles, suite_properties, pointer_override)?;
    let orientation_ambiguous =
        apply_orientation_evidence(candidates, descriptor_orientation, orientation_override);
    Ok(!matches!(suite_dimensions, SuiteDimensions::None) || orientation_ambiguous)
}

fn apply_screen_constraint(
    candidates: &mut [Candidate],
    suite_properties: &BTreeMap<String, String>,
    screen_override: Option<(u32, u32)>,
    orientation_override: Option<CanvasOrientation>,
) -> SuiteDimensions {
    let suite_dimensions = if screen_override.is_none() {
        strongest_suite_dimension(suite_properties, candidates)
    } else {
        SuiteDimensions::None
    };
    if let Some(dimensions) = screen_override {
        let reason = SelectionReason::new(
            "screen-override",
            format!("{}x{}", dimensions.0, dimensions.1),
        );
        retain_dimensions(
            candidates,
            dimensions,
            9_000,
            EvidenceStrength::Explicit,
            &reason,
        );
    } else if let SuiteDimensions::Unique(signal) = &suite_dimensions {
        let selected_dimensions = orientation_override.map_or(signal.dimensions, |orientation| {
            dimensions_in_orientation(candidates, signal.dimensions, orientation)
        });
        let reason = SelectionReason::new(
            "suite-dimensions",
            if selected_dimensions == signal.dimensions {
                format!(
                    "{}={}x{}",
                    signal.property, signal.dimensions.0, signal.dimensions.1
                )
            } else {
                format!(
                    "{}={}x{}->{}x{}",
                    signal.property,
                    signal.dimensions.0,
                    signal.dimensions.1,
                    selected_dimensions.0,
                    selected_dimensions.1
                )
            },
        );
        retain_dimensions(
            candidates,
            selected_dimensions,
            4_000,
            EvidenceStrength::Descriptor,
            &reason,
        );
    } else if let SuiteDimensions::Ambiguous(dimensions) = &suite_dimensions {
        let detail = dimensions
            .iter()
            .map(|(width, height)| format!("{width}x{height}"))
            .collect::<Vec<_>>()
            .join("|");
        let reason = SelectionReason::new("suite-dimensions-ambiguous", detail);
        for candidate in candidates.iter_mut().filter(|candidate| candidate.eligible) {
            candidate.add(
                0,
                0,
                EvidenceStrength::None,
                EvidenceStrength::None,
                reason.clone(),
            );
        }
    }
    suite_dimensions
}

fn dimensions_in_orientation(
    candidates: &[Candidate],
    dimensions: (u32, u32),
    orientation: CanvasOrientation,
) -> (u32, u32) {
    if orientation.match_quality(dimensions) != OrientationMatch::Mismatch {
        return dimensions;
    }
    let transposed = (dimensions.1, dimensions.0);
    if candidates.iter().any(|candidate| {
        candidate.eligible && candidate.selection.canvas_dimensions() == transposed
    }) {
        transposed
    } else {
        dimensions
    }
}

fn apply_touch_support_hint(
    candidates: &mut [Candidate],
    profiles: &[DeviceProfile],
    suite_properties: &BTreeMap<String, String>,
    pointer_override: bool,
) -> Result<(), EmuError> {
    if suite_supports_pointer(suite_properties) {
        let pointer_profiles = profiles
            .iter()
            .map(|profile| {
                profile
                    .resolve(ProfileOverrides::default())
                    .map(|resolved| resolved.pointer_events())
            })
            .collect::<Result<Vec<_>, _>>()?;
        for candidate in candidates.iter_mut() {
            if !candidate.eligible {
                continue;
            }
            if pointer_profiles[candidate.profile_index] {
                candidate.add(
                    250,
                    0,
                    EvidenceStrength::ApplicationHint,
                    EvidenceStrength::None,
                    SelectionReason::new(
                        "suite-touch-support",
                        "application advertises optional pointer support",
                    ),
                );
            } else if pointer_override {
                candidate.add(
                    0,
                    0,
                    EvidenceStrength::None,
                    EvidenceStrength::None,
                    SelectionReason::new(
                        "pointer-override",
                        "frontend explicitly enables pointer input",
                    ),
                );
            } else {
                candidate.add(
                    0,
                    0,
                    EvidenceStrength::None,
                    EvidenceStrength::None,
                    SelectionReason::new(
                        "suite-touch-support",
                        "application pointer support does not require pointer hardware",
                    ),
                );
            }
        }
    }
    Ok(())
}

fn apply_orientation_evidence(
    candidates: &mut [Candidate],
    descriptor_orientation: SuiteOrientation,
    orientation_override: Option<CanvasOrientation>,
) -> bool {
    if let Some(orientation) = orientation_override {
        apply_orientation_constraint(
            candidates,
            orientation,
            EvidenceStrength::Explicit,
            "orientation-override",
        );
        false
    } else {
        match descriptor_orientation {
            SuiteOrientation::Unique(orientation) => {
                apply_orientation_constraint(
                    candidates,
                    orientation,
                    EvidenceStrength::Descriptor,
                    "suite-orientation",
                );
                false
            }
            SuiteOrientation::Ambiguous => {
                let reason = SelectionReason::new(
                    "suite-orientation-ambiguous",
                    "conflicting portrait and landscape descriptor values",
                );
                for candidate in candidates.iter_mut().filter(|candidate| candidate.eligible) {
                    candidate.add(
                        0,
                        0,
                        EvidenceStrength::None,
                        EvidenceStrength::None,
                        reason.clone(),
                    );
                }
                true
            }
            SuiteOrientation::None => false,
        }
    }
}

fn apply_orientation_constraint(
    candidates: &mut [Candidate],
    orientation: CanvasOrientation,
    strength: EvidenceStrength,
    reason_code: &'static str,
) {
    let reason = SelectionReason::new(reason_code, orientation.as_str());
    for candidate in candidates.iter_mut().filter(|candidate| candidate.eligible) {
        match orientation.match_quality(candidate.selection.canvas_dimensions()) {
            OrientationMatch::Mismatch => candidate.eligible = false,
            OrientationMatch::Neutral => {
                candidate.add(0, 500, EvidenceStrength::None, strength, reason.clone());
            }
            OrientationMatch::Directional => {
                candidate.add(0, 1_000, EvidenceStrength::None, strength, reason.clone());
            }
        }
    }
}

fn retain_dimensions(
    candidates: &mut [Candidate],
    dimensions: (u32, u32),
    score: i32,
    strength: EvidenceStrength,
    reason: &SelectionReason,
) {
    for candidate in candidates {
        if candidate.eligible && candidate.selection.canvas_dimensions() != dimensions {
            candidate.eligible = false;
        } else if candidate.eligible {
            candidate.add(score, score, strength, strength, reason.clone());
        }
    }
}
