//! User-controlled profile alternatives after a managed-heap exhaustion.

use super::{compare_candidate_order, selection_candidates, validate_catalog};
use crate::archive_evidence::ArchiveEvidence;
use crate::decision::{DeviceDecision, ManagedHeapRecoveryCandidate, SelectionOverrides};
use device_profile::DeviceProfile;
use diagnostics::EmuError;
use std::collections::BTreeMap;

/// Lists profiles for a user-selected retry after a managed-heap failure.
/// Candidates retain the Canvas size, descriptor and API constraints, exclude
/// the failed host, and require more effective heap (using the runtime fallback for
/// unknown limits). Automatic launches are compared by host capacity.
/// Same-manufacturer profiles come first, then known heaps in ascending order,
/// unknown limits and catalog order. Archive evidence ranks but does not exclude
/// candidates. No guest code is run.
///
/// # Errors
/// Returns a controlled diagnostic for an invalid catalog or a failed profile
/// which is absent from that catalog.
pub fn managed_heap_recovery_candidates(
    profiles: &[DeviceProfile],
    suite_properties: &BTreeMap<String, String>,
    archive: &ArchiveEvidence,
    failed: &DeviceDecision,
    runtime_fallback_heap_bytes: u64,
    pointer_override: bool,
) -> Result<Vec<ManagedHeapRecoveryCandidate>, EmuError> {
    validate_catalog(profiles, true)?;
    let failed_profile = failed.profile(profiles)?;
    let failed_selection = failed.selection();
    let failed_heap = if failed.automatic_host_selected {
        failed_profile
            .composition()
            .host()
            .capacity_limits()
            .map(device_profile::HostCapacityLimits::heap_bytes)
    } else {
        failed_profile.limits().heap_bytes().value().copied()
    };
    let failed_effective_heap = failed_heap.unwrap_or(runtime_fallback_heap_bytes);
    let failed_manufacturer = failed_profile.device().manufacturer();
    let (width, height) = failed_selection.canvas_dimensions();
    let can_retry = |profile: &DeviceProfile| {
        profile.profile_id() != failed_selection.profile_id()
            && profile
                .limits()
                .heap_bytes()
                .value()
                .copied()
                .unwrap_or(runtime_fallback_heap_bytes)
                > failed_effective_heap
    };
    if !profiles.iter().any(can_retry) {
        return Ok(Vec::new());
    }

    let mut overrides = SelectionOverrides::default().with_canvas_dimensions(width, height);
    if pointer_override {
        overrides = overrides.with_pointer_events();
    }
    // The fixed Canvas disables archive screen hints and descriptor size
    // inference. All remaining evidence can be evaluated once for the catalog:
    // an explicit profile adds the same score to every variant of that profile
    // and cannot change their relative order.
    let candidates = selection_candidates(profiles, suite_properties, archive, overrides)?;
    let mut ranked = Vec::new();
    for variants in candidates.chunk_by(|left, right| left.profile_index == right.profile_index) {
        let catalog_index = variants[0].profile_index;
        let profile = &profiles[catalog_index];
        if !can_retry(profile) {
            continue;
        }
        let Some((_, winner)) = variants
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.eligible)
            .min_by(|left, right| compare_candidate_order(*left, *right))
        else {
            continue;
        };
        let heap_bytes = profile.limits().heap_bytes().value().copied();
        ranked.push((
            profile.device().manufacturer() == failed_manufacturer,
            heap_bytes,
            catalog_index,
            ManagedHeapRecoveryCandidate {
                selection: winner.selection.clone(),
                heap_bytes,
            },
        ));
    }

    ranked.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| match (left.1, right.1) {
                (Some(left), Some(right)) => left.cmp(&right),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
            .then_with(|| left.2.cmp(&right.2))
    });
    Ok(ranked
        .into_iter()
        .map(|(_, _, _, candidate)| candidate)
        .collect())
}
