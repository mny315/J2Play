use crate::{GameSettings, ProfileChoice};
use device_profile::DeviceProfile;
use diagnostics::{Category, EmuError};
use jar::{JadInfo, JarInfo, MidletInfo};
use launch::{ArchiveEvidence, DeviceDecision, SelectionOverrides};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const MAX_ARCHIVE_LEAF_NAME_BYTES: usize = 512;

/// Bytes returned by a platform picker. The shared layer deliberately accepts
/// no source path or URI and therefore cannot persist picker grants or credentials.
#[derive(Clone, Debug)]
pub struct ImportSource {
    pub(crate) archive_leaf_name: Option<String>,
    pub(crate) jar_bytes: Arc<Vec<u8>>,
    pub(crate) jad_bytes: Option<Vec<u8>>,
}

impl ImportSource {
    #[must_use]
    pub fn new(
        archive_leaf_name: Option<String>,
        archive_bytes: Vec<u8>,
        descriptor_bytes: Option<Vec<u8>>,
    ) -> Self {
        Self {
            archive_leaf_name,
            jar_bytes: Arc::new(archive_bytes),
            jad_bytes: descriptor_bytes,
        }
    }

    /// Replaces the descriptor while sharing the unchanged archive with the
    /// previous draft, which remains available if inspection or staging fails.
    #[must_use]
    pub fn with_jad(&self, descriptor_bytes: Vec<u8>) -> Self {
        Self {
            archive_leaf_name: self.archive_leaf_name.clone(),
            jar_bytes: Arc::clone(&self.jar_bytes),
            jad_bytes: Some(descriptor_bytes),
        }
    }

    /// Returns the bounded picker bytes when restoring an unfinished import.
    #[must_use]
    pub fn into_parts(self) -> (Option<String>, Vec<u8>, Option<Vec<u8>>) {
        (
            self.archive_leaf_name,
            Arc::unwrap_or_clone(self.jar_bytes),
            self.jad_bytes,
        )
    }

    #[must_use]
    pub const fn has_jad(&self) -> bool {
        self.jad_bytes.is_some()
    }
}

/// Bounded inspection result retained while the UI asks for a `MIDlet` choice.
pub struct ImportInspection {
    source: ImportSource,
    jar: JarInfo,
    jad: Option<JadInfo>,
    midlets: Vec<MidletInfo>,
    evidence: ArchiveEvidence,
}

impl ImportInspection {
    /// Exact private bytes retained for descriptor changes and atomic staging.
    #[must_use]
    pub const fn source(&self) -> &ImportSource {
        &self.source
    }

    #[must_use]
    pub fn midlets(&self) -> &[MidletInfo] {
        &self.midlets
    }

    #[must_use]
    pub fn jar_sha256(&self) -> &str {
        &self.jar.sha256
    }

    /// Finishes preparation for exactly one declared `MIDlet`.
    ///
    /// # Errors
    /// Returns a controlled descriptor or profile-selection diagnostic.
    pub fn select_midlet(self, index: u32) -> Result<PreparedImport, EmuError> {
        let mut suite = midp::describe_suite(&self.jar, self.jad.as_ref(), Some(index))?;
        let descriptor_defaults = if self.jad.is_none() {
            self.evidence
                .apply_descriptorless_suite_property_defaults(&mut suite.properties)
        } else {
            Vec::new()
        };
        let profiles = launch::builtin_device_profiles()?;
        let automatic_decision = launch::resolve_device_selection(
            profiles,
            &suite.properties,
            &self.evidence,
            SelectionOverrides::default(),
        )?;
        let profile_catalog = catalog_fingerprint(profiles);
        Ok(PreparedImport {
            source: self.source,
            jar: self.jar,
            suite,
            evidence: self.evidence,
            descriptor_defaults,
            automatic_decision,
            profile_catalog,
        })
    }
}

/// Complete import preparation retained through confirmation and settings UI.
pub struct PreparedImport {
    source: ImportSource,
    jar: JarInfo,
    suite: midp::SuiteDescriptor,
    evidence: ArchiveEvidence,
    descriptor_defaults: Vec<(String, String)>,
    automatic_decision: DeviceDecision,
    profile_catalog: String,
}

impl PreparedImport {
    /// Exact private bytes retained for descriptor changes and atomic staging.
    #[must_use]
    pub const fn source(&self) -> &ImportSource {
        &self.source
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.suite.midlet.name
    }

    #[must_use]
    pub fn midlet(&self) -> &MidletInfo {
        &self.suite.midlet
    }

    #[must_use]
    pub fn jar_sha256(&self) -> &str {
        &self.jar.sha256
    }

    #[must_use]
    pub fn archive_leaf_name(&self) -> Option<&str> {
        self.source.archive_leaf_name.as_deref()
    }

    #[must_use]
    pub fn descriptor_defaults(&self) -> &[(String, String)] {
        &self.descriptor_defaults
    }

    #[must_use]
    pub const fn automatic_decision(&self) -> &DeviceDecision {
        &self.automatic_decision
    }

    #[must_use]
    pub fn profile_catalog_fingerprint(&self) -> &str {
        &self.profile_catalog
    }

    #[must_use]
    pub fn automatic_profile_summary(&self) -> ProfileSummary {
        ProfileSummary::from_decision(&self.automatic_decision)
    }

    /// Resolves this entry for current per-game settings. Automatic mode always
    /// runs the resolver again instead of trusting display-only cached metadata.
    ///
    /// # Errors
    /// Returns a stable settings or profile-selection diagnostic.
    pub fn launch_plan(&self, settings: &GameSettings) -> Result<LaunchPlan, EmuError> {
        self.launch_plan_with_orientation(settings, None)
    }

    /// Resolves this entry with an optional transient guest-visible Canvas
    /// orientation. The override belongs to one launch attempt and is not
    /// persisted in per-game settings.
    ///
    /// # Errors
    /// Returns a stable settings or profile-selection diagnostic.
    pub fn launch_plan_with_orientation(
        &self,
        settings: &GameSettings,
        orientation: Option<launch::CanvasOrientation>,
    ) -> Result<LaunchPlan, EmuError> {
        let profiles = launch::builtin_device_profiles()?;
        settings.validate(profiles)?;
        let mut overrides = match &settings.device_profile {
            ProfileChoice::Automatic => SelectionOverrides::default(),
            ProfileChoice::Manual { profile_id } => {
                let selected = profiles
                    .iter()
                    .find(|profile| profile.profile_id() == profile_id)
                    .and_then(DeviceProfile::canvas_dimensions)
                    .ok_or_else(|| {
                        EmuError::new(
                            Category::Platform,
                            "settings-profile-canvas",
                            format!(
                                "manual device profile {profile_id} has no runnable Canvas dimensions"
                            ),
                        )
                    })?;
                let (width, height) = dimensions_in_orientation(selected, orientation);
                SelectionOverrides::default()
                    .with_profile_id(profile_id)
                    .with_canvas_dimensions(width, height)
            }
        };
        if let Some(orientation) = orientation {
            overrides = overrides.with_orientation(orientation);
        }
        let decision = launch::resolve_device_selection(
            profiles,
            &self.suite.properties,
            &self.evidence,
            overrides,
        )?;
        let resolved =
            decision.resolve_profile(profiles, device_profile::ProfileOverrides::default())?;
        let pointer_events = resolved.pointer_events();
        let automatic_fps_limit = resolved.runtime_host().runtime().frames_per_second();
        Ok(LaunchPlan {
            decision,
            profile_catalog: catalog_fingerprint(profiles),
            midlet: self.suite.midlet.clone(),
            jar_sha256: self.jar.sha256.clone(),
            manual_fps_limit: settings.manual_fps_limit(),
            automatic_fps_limit,
            pointer_events,
        })
    }

    pub(crate) fn jar_bytes(&self) -> &[u8] {
        &self.source.jar_bytes
    }

    pub(crate) fn jad_bytes(&self) -> Option<&[u8]> {
        self.source.jad_bytes.as_deref()
    }

    pub(crate) const fn suite(&self) -> &midp::SuiteDescriptor {
        &self.suite
    }

    pub(crate) const fn evidence(&self) -> &ArchiveEvidence {
        &self.evidence
    }

    pub(crate) fn into_jar_bytes(self) -> Vec<u8> {
        Arc::unwrap_or_clone(self.source.jar_bytes)
    }
}

fn dimensions_in_orientation(
    dimensions: (u32, u32),
    orientation: Option<launch::CanvasOrientation>,
) -> (u32, u32) {
    match (orientation, dimensions.0.cmp(&dimensions.1)) {
        (Some(launch::CanvasOrientation::Portrait), std::cmp::Ordering::Greater)
        | (Some(launch::CanvasOrientation::Landscape), std::cmp::Ordering::Less) => {
            (dimensions.1, dimensions.0)
        }
        _ => dimensions,
    }
}

/// UI-facing resolved profile details with no borrowed catalog state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileSummary {
    pub target_profile_id: String,
    pub runtime_profile_id: String,
    pub canvas_dimensions: (u32, u32),
    pub confidence: &'static str,
    pub reason: String,
}

impl ProfileSummary {
    fn from_decision(decision: &DeviceDecision) -> Self {
        Self {
            target_profile_id: decision.target_selection().profile_id().to_owned(),
            runtime_profile_id: decision.selection().profile_id().to_owned(),
            canvas_dimensions: decision.selection().canvas_dimensions(),
            confidence: decision.confidence().as_str(),
            reason: profile_selection_reason(decision),
        }
    }
}

fn profile_selection_reason(decision: &DeviceDecision) -> String {
    decision
        .reasons()
        .iter()
        .find_map(|reason| {
            let explanation = match reason.code() {
                "profile-override" => "Selected in Game settings.",
                "screen-override" => "Matches the selected screen size.",
                "orientation-override" => "Matches the selected screen orientation.",
                "pointer-override" => "Supports the selected touch input.",
                "suite-dimensions" => "Matches the screen size declared by the game.",
                "suite-orientation" => "Matches the screen orientation declared by the game.",
                "suite-family" | "suite-profile-hint" => {
                    "Matches the device family declared by the game."
                }
                "suite-touch-support" => "Matches the game's declared input support.",
                "static-jsr" => {
                    return Some(format!(
                        "Supports the game's Java ME APIs (JSR-{}).",
                        reason.detail()
                    ));
                }
                "static-vendor-api" | "static-manufacturer-api" => {
                    "Supports the device-specific APIs used by the game."
                }
                code if code.starts_with("archive-") => {
                    "Matches device information in the JAR file name."
                }
                _ => return None,
            };
            Some(explanation.to_owned())
        })
        .unwrap_or_else(|| "Compatibility fallback; no specific device was identified.".to_owned())
}

/// Authoritative launch-time decision and immutable suite identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchPlan {
    pub decision: DeviceDecision,
    pub profile_catalog: String,
    pub midlet: MidletInfo,
    pub jar_sha256: String,
    pub manual_fps_limit: Option<u32>,
    pub automatic_fps_limit: u32,
    pub pointer_events: bool,
}

impl LaunchPlan {
    #[must_use]
    pub fn profile_summary(&self) -> ProfileSummary {
        ProfileSummary::from_decision(&self.decision)
    }
}

/// Performs bounded JAR/JAD and static-evidence inspection on the exact bytes
/// supplied by a platform picker.
///
/// # Errors
/// Returns a controlled diagnostic for an invalid name, JAR, JAD, or static class evidence.
pub fn inspect_import(mut source: ImportSource) -> Result<ImportInspection, EmuError> {
    source.archive_leaf_name = normalized_leaf_name(source.archive_leaf_name.as_deref())?;
    let jar = jar::inspect_bytes(&source.jar_bytes)?;
    let jad = source
        .jad_bytes
        .as_deref()
        .map(jar::parse_jad)
        .transpose()?;
    let evidence =
        ArchiveEvidence::inspect_bytes(source.archive_leaf_name.as_deref(), &source.jar_bytes)?;
    let midlets = midp::suite_midlets(&jar, jad.as_ref());
    if midlets.is_empty() {
        return Err(EmuError::new(
            Category::Api,
            "midlet-selection",
            "suite does not declare a MIDlet",
        ));
    }
    Ok(ImportInspection {
        source,
        jar,
        jad,
        midlets,
        evidence,
    })
}

pub(crate) fn normalized_leaf_name(name: Option<&str>) -> Result<Option<String>, EmuError> {
    let Some(name) = name else {
        return Ok(None);
    };
    let leaf = name
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .ok_or_else(|| {
            EmuError::new(
                Category::Jar,
                "archive-name",
                "archive display name must have a valid Unicode leaf component",
            )
        })?;
    if leaf.len() > MAX_ARCHIVE_LEAF_NAME_BYTES {
        return Err(EmuError::new(
            Category::Jar,
            "archive-name-too-long",
            format!("archive display name exceeds {MAX_ARCHIVE_LEAF_NAME_BYTES} bytes"),
        ));
    }
    Ok(Some(leaf.to_owned()))
}

/// Stable fingerprint used only to validate display caches. Launch decisions
/// remain authoritative even when the fingerprint matches.
#[must_use]
pub fn catalog_fingerprint(profiles: &[DeviceProfile]) -> String {
    let mut digest = Sha256::new();
    for profile in profiles {
        digest.update(profile.schema_version().to_le_bytes());
        digest.update(profile.profile_id().as_bytes());
        digest.update([0]);
        digest.update(profile.profile_version().as_bytes());
        digest.update([0xff]);
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-core/preparation/mod.rs"]
mod tests;
