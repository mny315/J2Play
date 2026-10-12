//! Public launch decision and override value types.

use device_profile::{DeviceProfile, ProfileOverrides, ResolvedProfile, ScreenModeSelection};
use diagnostics::{Category, EmuError};

/// A concrete profile/screen pair chosen for one launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceSelection {
    pub(crate) profile_id: String,
    pub(crate) screen_mode_id: Option<String>,
    pub(crate) canvas_dimensions: (u32, u32),
}

/// Logical orientation requested from the guest-visible Canvas.
///
/// This is deliberately separate from host presentation rotation: changing
/// this value swaps `Canvas.getWidth()`/`getHeight()` and selects the matching
/// profile geometry, while presentation rotation only turns rendered pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasOrientation {
    Portrait,
    Landscape,
}

impl CanvasOrientation {
    /// Stable diagnostic label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Portrait => "portrait",
            Self::Landscape => "landscape",
        }
    }

    pub(crate) fn match_quality(self, dimensions: (u32, u32)) -> OrientationMatch {
        match dimensions.0.cmp(&dimensions.1) {
            std::cmp::Ordering::Less if self == Self::Portrait => OrientationMatch::Directional,
            std::cmp::Ordering::Greater if self == Self::Landscape => OrientationMatch::Directional,
            std::cmp::Ordering::Equal => OrientationMatch::Neutral,
            std::cmp::Ordering::Less | std::cmp::Ordering::Greater => OrientationMatch::Mismatch,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OrientationMatch {
    Mismatch,
    Neutral,
    Directional,
}

/// Explicit frontend constraints applied before automatic evidence.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SelectionOverrides<'value> {
    pub(crate) profile_id: Option<&'value str>,
    pub(crate) canvas_dimensions: Option<(u32, u32)>,
    pub(crate) orientation: Option<CanvasOrientation>,
    pub(crate) pointer_events: bool,
}

impl<'value> SelectionOverrides<'value> {
    /// Constrains the selected device family.
    #[must_use]
    pub const fn with_profile_id(mut self, profile_id: &'value str) -> Self {
        self.profile_id = Some(profile_id);
        self
    }

    /// Constrains the exact guest-visible full-screen Canvas dimensions.
    #[must_use]
    pub const fn with_canvas_dimensions(mut self, width: u32, height: u32) -> Self {
        self.canvas_dimensions = Some((width, height));
        self
    }

    /// Constrains only the logical Canvas orientation, retaining the selected
    /// profile's default screen mode when no stronger size evidence exists.
    #[must_use]
    pub const fn with_orientation(mut self, orientation: CanvasOrientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// Records that the frontend explicitly enables pointer input even when the
    /// selected base profile does not expose native pointer events.
    #[must_use]
    pub const fn with_pointer_events(mut self) -> Self {
        self.pointer_events = true;
        self
    }

    pub(crate) const fn any(self) -> bool {
        self.profile_id.is_some() || self.canvas_dimensions.is_some() || self.orientation.is_some()
    }
}

impl DeviceSelection {
    /// Creates a selection for validation against a catalog.
    #[must_use]
    pub fn new(
        profile_id: impl Into<String>,
        screen_mode_id: Option<String>,
        canvas_dimensions: (u32, u32),
    ) -> Self {
        Self {
            profile_id: profile_id.into(),
            screen_mode_id,
            canvas_dimensions,
        }
    }

    /// Stable profile identifier.
    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// Stable screen-mode identifier owned by the target persona. Runtime-host
    /// selections retain it for diagnostics even when the host does not
    /// enumerate persona geometry.
    #[must_use]
    pub fn screen_mode_id(&self) -> Option<&str> {
        self.screen_mode_id.as_deref()
    }

    /// Full-screen Canvas dimensions, including selected orientation.
    #[must_use]
    pub const fn canvas_dimensions(&self) -> (u32, u32) {
        self.canvas_dimensions
    }
}

/// Dominant evidence which selected the device family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileSelectionSource {
    /// A frontend supplied an explicit diagnostic override.
    ExplicitOverride,
    /// Manifest or JAD capabilities distinguished the family.
    SuiteMetadata,
    /// Statically referenced APIs distinguished the family.
    StaticApi,
    /// The distribution filename broke an otherwise unresolved tie.
    ArchiveName,
    /// No distinguishing evidence existed, so catalog fallback order won.
    CompatibilityFallback,
}

impl ProfileSelectionSource {
    /// Stable diagnostic label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitOverride => "explicit-override",
            Self::SuiteMetadata => "suite-metadata",
            Self::StaticApi => "static-api",
            Self::ArchiveName => "archive-name",
            Self::CompatibilityFallback => "compatibility-fallback",
        }
    }
}

/// Confidence attached to an automatic decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionConfidence {
    /// A frontend explicitly constrained the decision.
    Explicit,
    /// Descriptor or unique static-capability evidence selected the pair.
    High,
    /// Weak archive packaging evidence selected the pair.
    Medium,
    /// Catalog ordering/default mode was required to break a tie.
    Fallback,
}

impl SelectionConfidence {
    /// Stable diagnostic label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Fallback => "fallback",
        }
    }
}

/// One bounded, human-readable reason contributing to a launch decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionReason {
    code: &'static str,
    detail: String,
}

impl SelectionReason {
    pub(crate) fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    /// Stable reason code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }

    /// Concise evidence description suitable for diagnostics.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// One target-persona/runtime-host composition which remains equally supported
/// by the strongest evidence. Entries returned by
/// [`DeviceDecision::candidates`] are ordered from the stable automatic
/// recommendation to later target alternatives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCandidate {
    pub(crate) target_selection: DeviceSelection,
    pub(crate) selection: DeviceSelection,
    pub(crate) reasons: Vec<SelectionReason>,
}

impl DeviceCandidate {
    /// Guest-visible persona and screen represented by this candidate.
    #[must_use]
    pub const fn target_selection(&self) -> &DeviceSelection {
        &self.target_selection
    }

    /// Runtime host paired with the target persona and its exact Canvas.
    #[must_use]
    pub const fn selection(&self) -> &DeviceSelection {
        &self.selection
    }

    /// Target evidence plus any compatibility-host reason for this candidate.
    #[must_use]
    pub fn reasons(&self) -> &[SelectionReason] {
        &self.reasons
    }
}

/// One statically compatible exact profile offered after the current runtime
/// composition exhausts its managed heap.
///
/// Recovery keeps the failed launch's exact Canvas dimensions and excludes
/// profiles whose known heap is no larger than the failed composition's
/// effective heap. Automatic compositions use their host capacity; exact
/// profiles with an unknown heap use the frontend's bounded runtime fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedHeapRecoveryCandidate {
    pub(crate) selection: DeviceSelection,
    pub(crate) heap_bytes: Option<u64>,
}

impl ManagedHeapRecoveryCandidate {
    /// Concrete profile/screen pair to use for the user-requested retry.
    #[must_use]
    pub const fn selection(&self) -> &DeviceSelection {
        &self.selection
    }

    /// Evidence-backed managed-heap limit, or `None` when the profile leaves
    /// that device limit unknown.
    #[must_use]
    pub const fn heap_bytes(&self) -> Option<u64> {
        self.heap_bytes
    }
}

/// Complete frontend-independent auto-detection result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceDecision {
    pub(crate) target_selection: DeviceSelection,
    pub(crate) selection: DeviceSelection,
    pub(crate) automatic_host_selected: bool,
    pub(crate) profile_source: ProfileSelectionSource,
    pub(crate) screen_source: ScreenModeSelection,
    pub(crate) confidence: SelectionConfidence,
    pub(crate) reasons: Vec<SelectionReason>,
    pub(crate) candidates: Vec<DeviceCandidate>,
}

impl DeviceDecision {
    /// Device family and screen identified by suite/archive evidence before
    /// its declared automatic runtime host is applied.
    #[must_use]
    pub const fn target_selection(&self) -> &DeviceSelection {
        &self.target_selection
    }

    /// Compatibility-oriented runtime profile and the exact target Canvas.
    #[must_use]
    pub const fn selection(&self) -> &DeviceSelection {
        &self.selection
    }

    /// Dominant evidence for the detected target profile family.
    #[must_use]
    pub const fn profile_source(&self) -> ProfileSelectionSource {
        self.profile_source
    }

    /// Dominant evidence for the screen mode.
    #[must_use]
    pub const fn screen_source(&self) -> ScreenModeSelection {
        self.screen_source
    }

    /// Confidence of the detected target pair; host promotion never inflates it.
    #[must_use]
    pub const fn confidence(&self) -> SelectionConfidence {
        self.confidence
    }

    /// Bounded reasons which contributed to the winner.
    #[must_use]
    pub fn reasons(&self) -> &[SelectionReason] {
        &self.reasons
    }

    /// Compatibility-oriented runtime candidates for equally supported target
    /// choices. Distinct personas remain distinct even when they converge on
    /// the same host and Canvas, because their guest-visible contracts differ.
    /// The automatic selection is always the first entry.
    #[must_use]
    pub fn candidates(&self) -> &[DeviceCandidate] {
        &self.candidates
    }

    /// Returns the selected profile from the same catalog used by the resolver.
    ///
    /// # Errors
    /// Returns a controlled diagnostic if the catalog changed after selection.
    pub fn profile<'profiles>(
        &self,
        profiles: &'profiles [DeviceProfile],
    ) -> Result<&'profiles DeviceProfile, EmuError> {
        profiles
            .iter()
            .find(|profile| profile.profile_id() == self.selection.profile_id())
            .ok_or_else(|| {
                catalog_error(
                    "device-selection-catalog",
                    "selected device profile disappeared from the launch catalog",
                )
            })
    }

    /// Returns the evidence-bearing target persona from the selection catalog.
    ///
    /// # Errors
    /// Returns a controlled diagnostic if the catalog changed after selection.
    pub fn target_profile<'profiles>(
        &self,
        profiles: &'profiles [DeviceProfile],
    ) -> Result<&'profiles DeviceProfile, EmuError> {
        profiles
            .iter()
            .find(|profile| profile.profile_id() == self.target_selection.profile_id())
            .ok_or_else(|| {
                catalog_error(
                    "device-selection-catalog",
                    "target persona disappeared from the launch catalog",
                )
            })
    }

    /// Resolves the exact runtime view represented by this decision.
    ///
    /// # Errors
    /// Returns a profile diagnostic if the catalog or selected mode changed.
    pub fn resolve_profile<'profiles>(
        &self,
        profiles: &'profiles [DeviceProfile],
        overrides: ProfileOverrides,
    ) -> Result<ResolvedProfile<'profiles>, EmuError> {
        let target = self.target_profile(profiles)?;
        if self.automatic_host_selected {
            target.resolve_selected_canvas_with_host(
                self.profile(profiles)?,
                overrides,
                self.target_selection.canvas_dimensions(),
                self.screen_source,
            )
        } else {
            target.resolve_selected_canvas(
                overrides,
                self.target_selection.canvas_dimensions(),
                self.screen_source,
            )
        }
    }
}

fn catalog_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Api, code, message.into())
}
