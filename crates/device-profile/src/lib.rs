//! Typed loading and access to version 2 `J2Play` device profiles.

use diagnostics::EmuError;
use serde::Deserialize;
use std::collections::BTreeMap;

mod display;
mod input;
mod m3g;
mod profile;

pub use display::{Dimensions, DisplayProfile, LcdUiFontHeights, Orientation, ScreenMode};
pub use input::{CanvasKey, GameAction, InputProfile, KeyName, PointerProfile, SoftKeys};
pub use m3g::{M3gBudgets, M3gCompatibility, M3gProfile, M3gProperties};
pub use profile::dimension_pairs;

/// Shared project error returned by profile loading and validation.
pub type LoadError = EmuError;

/// Profile capability implied by a Java class name.
///
/// Keeping this mapping beside the profile model prevents launch selection and
/// bootstrap visibility from drifting apart as optional APIs are added.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum JavaCapability {
    /// A standard Java ME optional package.
    Jsr(&'static str),
    /// A manufacturer or compatibility API surface.
    VendorApi(&'static str),
}

/// Returns the optional profile capability required by a Java class.
#[must_use]
pub fn java_capability_for_class(name: &str) -> Option<JavaCapability> {
    if name.starts_with("com/mascotcapsule/micro3d/") || name.starts_with("com/hi/micro3d/") {
        Some(JavaCapability::VendorApi("mascot-capsule-micro3d-v3"))
    } else if name.starts_with("com/siemens/mp/color_game/") {
        Some(JavaCapability::VendorApi("siemens-color-game"))
    } else if name.starts_with("com/siemens/mp/game/") {
        Some(JavaCapability::VendorApi("siemens-game"))
    } else if name.starts_with("com/siemens/mp/") {
        Some(JavaCapability::VendorApi("siemens-extension"))
    } else if name.starts_with("com/nokia/mid/ui/") {
        Some(JavaCapability::VendorApi("nokia-ui"))
    } else if name.starts_with("com/nokia/mid/sound/") {
        Some(JavaCapability::VendorApi("nokia-sound"))
    } else if name == "com/samsung/util/AudioClip"
        || name.starts_with("com/samsung/util/AudioClip$")
    {
        Some(JavaCapability::VendorApi("samsung-audioclip"))
    } else if name.starts_with("zh/system/") {
        Some(JavaCapability::VendorApi("zh-system"))
    } else if matches!(
        name,
        "javax/microedition/sensor/SensorInfo" | "javax/microedition/sensor/SensorManager"
    ) {
        Some(JavaCapability::VendorApi("sensor-probe"))
    } else if name.starts_with("javax/microedition/m3g/") {
        Some(JavaCapability::Jsr("184"))
    } else if name.starts_with("javax/microedition/khronos/") || name.starts_with("java/nio/") {
        Some(JavaCapability::Jsr("239"))
    } else if name.starts_with("javax/bluetooth/") {
        Some(JavaCapability::Jsr("82"))
    } else if name.starts_with("javax/microedition/io/file/") {
        Some(JavaCapability::Jsr("75"))
    } else if name.starts_with("javax/microedition/sensor/") {
        Some(JavaCapability::Jsr("256"))
    } else if name.starts_with("javax/wireless/messaging/") {
        Some(JavaCapability::Jsr("120"))
    } else if name.starts_with("javax/microedition/media/") {
        Some(JavaCapability::Jsr("135"))
    } else {
        None
    }
}

/// Confidence assigned to a researched property.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Explicitly stated by a source or measured by a probe.
    Confirmed,
    /// Derived indirectly from sourced information.
    Inferred,
    /// Not currently known; the associated value must be absent.
    Unknown,
}

/// A value together with its confidence and evidence references.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Evidence<T> {
    value: Option<T>,
    confidence: Confidence,
    sources: Vec<String>,
}

impl<T> Evidence<T> {
    /// Returns the value, or `None` when the property is explicitly unknown.
    #[must_use]
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Returns the evidence confidence.
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Returns identifiers from [`DeviceProfile::evidence_sources`].
    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }
}

/// Declarative identity and automatic runtime route of one device persona.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PersonaComposition {
    lineage: String,
    generation: u16,
    automatic_host: String,
}

impl PersonaComposition {
    /// Stable compatibility lineage used for explicit host admission.
    #[must_use]
    pub fn lineage(&self) -> &str {
        &self.lineage
    }

    /// Monotonic generation within the declared lineage.
    #[must_use]
    pub const fn generation(&self) -> u16 {
        self.generation
    }

    /// Canonical runtime host selected for automatic launches of this persona.
    #[must_use]
    pub fn automatic_host(&self) -> &str {
        &self.automatic_host
    }
}

/// Bounded implementation capacities supplied by an automatic runtime host.
/// They are deliberately separate from evidence-bearing handset limits.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostCapacityLimits {
    #[serde(rename = "heap_bytes")]
    heap: u64,
    #[serde(rename = "rms_bytes")]
    rms: u64,
    #[serde(rename = "jar_bytes")]
    jar: u64,
}

impl HostCapacityLimits {
    #[must_use]
    pub const fn heap_bytes(self) -> u64 {
        self.heap
    }

    #[must_use]
    pub const fn rms_bytes(self) -> u64 {
        self.rms
    }

    #[must_use]
    pub const fn jar_bytes(self) -> u64 {
        self.jar
    }
}

/// Automatic-host policy attached to an evidence-bearing preset.
///
/// Every preset remains usable as an exact manual host. Only presets with
/// `automatic = true` may be selected through a persona's automatic route.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostComposition {
    automatic: bool,
    #[serde(default)]
    priority: u16,
    #[serde(default)]
    accepts_lineages: Vec<String>,
    #[serde(default)]
    capacity_limits: Option<HostCapacityLimits>,
}

impl HostComposition {
    #[must_use]
    pub const fn automatic(&self) -> bool {
        self.automatic
    }

    #[must_use]
    pub const fn priority(&self) -> u16 {
        self.priority
    }

    #[must_use]
    pub fn accepts_lineages(&self) -> &[String] {
        &self.accepts_lineages
    }

    #[must_use]
    pub const fn capacity_limits(&self) -> Option<HostCapacityLimits> {
        self.capacity_limits
    }
}

/// Explicit split between the target persona and its possible runtime-host
/// role. Runtime routes are catalog data, never filename or platform-string
/// heuristics.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileComposition {
    persona: PersonaComposition,
    host: HostComposition,
}

impl ProfileComposition {
    #[must_use]
    pub const fn persona(&self) -> &PersonaComposition {
        &self.persona
    }

    #[must_use]
    pub const fn host(&self) -> &HostComposition {
        &self.host
    }
}

/// Complete, validated `DeviceProfile` schema version 2.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceProfile {
    schema_version: u32,
    #[serde(default)]
    family_id: Option<String>,
    profile_id: String,
    profile_version: String,
    status: ProfileStatus,
    composition: ProfileComposition,
    evidence_sources: Vec<EvidenceSource>,
    device: Device,
    display: DisplayProfile,
    #[serde(default)]
    runtime: RuntimeProfile,
    java: JavaProfile,
    input: InputProfile,
    limits: Limits,
    media: Media,
    m3g: Option<M3gProfile>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileStatus {
    Research,
    Verified,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSource {
    id: String,
    kind: SourceKind,
    title: String,
    revision: Option<String>,
    url: Option<String>,
}
impl EvidenceSource {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub const fn kind(&self) -> SourceKind {
        self.kind
    }
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
    #[must_use]
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    OfficialDocument,
    TechnicalPublication,
    HardwareProbe,
    SdkProbe,
    CompatibilityAudit,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    manufacturer: String,
    model: String,
    firmware_revision: Evidence<String>,
    java_platform: Evidence<String>,
    #[serde(default)]
    archive_name_hints: Vec<ArchiveNameHint>,
}
impl Device {
    #[must_use]
    pub fn manufacturer(&self) -> &str {
        &self.manufacturer
    }
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }
    #[must_use]
    pub const fn firmware_revision(&self) -> &Evidence<String> {
        &self.firmware_revision
    }
    #[must_use]
    pub const fn java_platform(&self) -> &Evidence<String> {
        &self.java_platform
    }
    /// Evidence-backed device-model hints accepted from an archive leaf name.
    ///
    /// These hints only help launch selection; they do not alter the base
    /// identity or capabilities declared by the profile.
    #[must_use]
    pub fn archive_name_hints(&self) -> &[ArchiveNameHint] {
        &self.archive_name_hints
    }
}

/// A weak distribution-name hint mapping a device model to a declared Canvas.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveNameHint {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    models: Vec<String>,
    fullscreen_canvas: Dimensions,
    confidence: Confidence,
    sources: Vec<String>,
}

impl ArchiveNameHint {
    /// Device model spellings whose separator-delimited tokens may occur in an
    /// archive leaf name. A grouped hint keeps models which share one Canvas
    /// and evidence source compact without weakening per-model matching.
    pub fn model_names(&self) -> impl Iterator<Item = &str> {
        self.model
            .iter()
            .map(String::as_str)
            .chain(self.models.iter().map(String::as_str))
    }

    /// Guest-visible full-screen Canvas selected for this model hint.
    #[must_use]
    pub const fn fullscreen_canvas(&self) -> Dimensions {
        self.fullscreen_canvas
    }

    /// Confidence assigned to this model-to-screen mapping.
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Evidence source identifiers for this mapping.
    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    const fn fullscreen_dimensions(&self) -> (u32, u32) {
        (self.fullscreen_canvas.width, self.fullscreen_canvas.height)
    }
}

/// Host-side performance model selected by a device profile.
///
/// These are emulator scheduling parameters rather than guest-visible device
/// properties.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProfile {
    interpreter_instructions_per_second: Option<u64>,
    frames_per_second: u32,
}

impl Default for RuntimeProfile {
    fn default() -> Self {
        Self {
            interpreter_instructions_per_second: None,
            frames_per_second: 60,
        }
    }
}

impl RuntimeProfile {
    /// Sustainable interpreted guest-bytecode rate in real-time mode.
    #[must_use]
    pub const fn interpreter_instructions_per_second(self) -> Option<u64> {
        self.interpreter_instructions_per_second
    }

    /// Automatic host-side presentation ceiling used unless the user selects
    /// a bounded manual override.
    #[must_use]
    pub const fn frames_per_second(self) -> u32 {
        self.frames_per_second
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JavaProfile {
    configuration: Evidence<String>,
    profile: Evidence<String>,
    jsrs: BTreeMap<String, Evidence<bool>>,
    compatibility_jsrs: Evidence<Vec<String>>,
    vendor_apis: Evidence<Vec<String>>,
    compatibility_apis: Evidence<Vec<String>>,
    system_properties: Evidence<BTreeMap<String, String>>,
    #[serde(default)]
    bluetooth_properties: Option<Evidence<BTreeMap<String, String>>>,
}
impl JavaProfile {
    #[must_use]
    pub const fn configuration(&self) -> &Evidence<String> {
        &self.configuration
    }
    #[must_use]
    pub const fn profile(&self) -> &Evidence<String> {
        &self.profile
    }
    #[must_use]
    pub const fn jsrs(&self) -> &BTreeMap<String, Evidence<bool>> {
        &self.jsrs
    }
    /// Returns standard packages explicitly enabled for cross-vendor compatibility.
    #[must_use]
    pub const fn compatibility_jsrs(&self) -> &Evidence<Vec<String>> {
        &self.compatibility_jsrs
    }
    #[must_use]
    pub const fn vendor_apis(&self) -> &Evidence<Vec<String>> {
        &self.vendor_apis
    }
    /// Returns cross-vendor compatibility surfaces explicitly enabled for this profile.
    #[must_use]
    pub const fn compatibility_apis(&self) -> &Evidence<Vec<String>> {
        &self.compatibility_apis
    }
    /// Returns whether the selected handset exposes a named vendor API.
    #[must_use]
    pub fn supports_vendor_api(&self, api: &str) -> bool {
        [&self.vendor_apis, &self.compatibility_apis]
            .into_iter()
            .filter_map(Evidence::value)
            .flatten()
            .any(|candidate| candidate == api)
    }
    /// Returns whether the selected handset exposes a standard JSR.
    #[must_use]
    pub fn supports_jsr(&self, jsr: &str) -> bool {
        self.jsrs.get(jsr).and_then(Evidence::value) == Some(&true)
            || self
                .compatibility_jsrs
                .value()
                .is_some_and(|jsrs| jsrs.iter().any(|candidate| candidate == jsr))
    }
    #[must_use]
    pub const fn system_properties(&self) -> &Evidence<BTreeMap<String, String>> {
        &self.system_properties
    }

    /// Returns the evidence-bearing, case-sensitive JSR-82 property table.
    #[must_use]
    pub const fn bluetooth_properties(&self) -> Option<&Evidence<BTreeMap<String, String>>> {
        self.bluetooth_properties.as_ref()
    }
}

/// Runtime-selectable family capabilities layered over an evidence-bearing
/// preset. The base profile remains immutable so compatibility keys and
/// hardware claims cannot silently change.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProfileOverrides {
    canvas_dimensions: Option<(u32, u32)>,
    pointer_events: Option<bool>,
    pointer_motion_events: Option<bool>,
}

impl ProfileOverrides {
    /// Overrides the logical full-screen Canvas dimensions.
    #[must_use]
    pub const fn with_canvas_dimensions(mut self, width: u32, height: u32) -> Self {
        self.canvas_dimensions = Some((width, height));
        self
    }

    /// Overrides MIDP pointer support. Motion implies press/release support.
    #[must_use]
    pub const fn with_pointer(mut self, events: bool, motion_events: bool) -> Self {
        self.pointer_events = Some(events);
        self.pointer_motion_events = Some(motion_events);
        self
    }
}

/// Origin of the logical screen mode selected for a runtime session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScreenModeSelection {
    /// The caller explicitly selected a mode.
    ExplicitOverride,
    /// One unambiguous declared mode was found in manifest/JAD values.
    SuiteMetadata,
    /// A weak distribution filename hint selected a declared mode.
    ArchiveMetadata,
    /// No stronger signal was available, so the profile default was used.
    ProfileDefault,
}

impl ScreenModeSelection {
    /// Stable diagnostic label suitable for frontend status output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitOverride => "explicit-override",
            Self::SuiteMetadata => "suite-metadata",
            Self::ArchiveMetadata => "archive-metadata",
            Self::ProfileDefault => "profile-default",
        }
    }
}

/// Fully resolved composition of one guest-visible persona and one runtime
/// host. Display, input, identity and API visibility belong to the persona;
/// execution tuning and host resource policy belong to the runtime host.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedProfile<'a> {
    persona: &'a DeviceProfile,
    runtime_host: &'a DeviceProfile,
    uses_automatic_host: bool,
    canvas_dimensions: (u32, u32),
    non_fullscreen_canvas_dimensions: (u32, u32),
    screen_mode_id: Option<&'a str>,
    screen_mode_selection: ScreenModeSelection,
    pointer_events: bool,
    pointer_motion_events: bool,
}

impl ResolvedProfile<'_> {
    /// Returns the immutable evidence-bearing target persona.
    #[must_use]
    pub const fn persona(&self) -> &DeviceProfile {
        self.persona
    }

    /// Returns the independently selected runtime host.
    #[must_use]
    pub const fn runtime_host(&self) -> &DeviceProfile {
        self.runtime_host
    }

    /// Returns whether catalog policy selected the runtime host automatically.
    /// An automatic self-route is still distinct from an exact manual launch.
    #[must_use]
    pub const fn uses_automatic_host(&self) -> bool {
        self.uses_automatic_host
    }

    /// Returns the runtime Canvas dimensions after overrides.
    #[must_use]
    pub const fn canvas_dimensions(&self) -> (u32, u32) {
        self.canvas_dimensions
    }

    /// Returns the ordinary Canvas drawable area for the resolved screen mode.
    #[must_use]
    pub const fn non_fullscreen_canvas_dimensions(&self) -> (u32, u32) {
        self.non_fullscreen_canvas_dimensions
    }

    /// Returns the selected evidence-backed screen mode, when the profile has a catalog.
    #[must_use]
    pub const fn screen_mode_id(&self) -> Option<&str> {
        self.screen_mode_id
    }

    /// Returns why this runtime screen mode was selected.
    #[must_use]
    pub const fn screen_mode_selection(&self) -> ScreenModeSelection {
        self.screen_mode_selection
    }

    /// Returns whether pointer press/release callbacks are exposed.
    #[must_use]
    pub const fn pointer_events(&self) -> bool {
        self.pointer_events
    }

    /// Returns whether pointer drag callbacks are exposed.
    #[must_use]
    pub const fn pointer_motion_events(&self) -> bool {
        self.pointer_motion_events
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // Unit suffix is part of the JSON schema.
pub struct Limits {
    heap_bytes: Evidence<u64>,
    rms_bytes: Evidence<u64>,
    jar_bytes: Evidence<u64>,
}
impl Limits {
    #[must_use]
    pub const fn heap_bytes(&self) -> &Evidence<u64> {
        &self.heap_bytes
    }
    #[must_use]
    pub const fn rms_bytes(&self) -> &Evidence<u64> {
        &self.rms_bytes
    }
    #[must_use]
    pub const fn jar_bytes(&self) -> &Evidence<u64> {
        &self.jar_bytes
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Media {
    mmapi_formats: Evidence<Vec<String>>,
}

impl Media {
    #[must_use]
    pub const fn mmapi_formats(&self) -> &Evidence<Vec<String>> {
        &self.mmapi_formats
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/device-profile/mod.rs"]
mod tests;
