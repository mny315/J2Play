//! Frontend-independent device-profile and logical-screen auto-detection.

mod archive_evidence;
mod archive_name;
mod catalog;
mod decision;
mod host_compatibility;
mod model_aliases;
mod resolver;
mod suite_evidence;

pub use archive_evidence::ArchiveEvidence;
pub use catalog::{builtin_device_profile, builtin_device_profiles};
pub use decision::{
    CanvasOrientation, DeviceCandidate, DeviceDecision, DeviceSelection,
    ManagedHeapRecoveryCandidate, ProfileSelectionSource, SelectionConfidence, SelectionOverrides,
    SelectionReason,
};
pub use resolver::{managed_heap_recovery_candidates, resolve_device_selection};
