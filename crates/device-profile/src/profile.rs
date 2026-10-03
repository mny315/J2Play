//! Bounded profile loading and runtime persona/host resolution.

use super::{
    Device, DeviceProfile, DisplayProfile, EvidenceSource, InputProfile, JavaProfile, Limits,
    LoadError, M3gProfile, Media, ProfileComposition, ProfileOverrides, ProfileStatus,
    ResolvedProfile, RuntimeProfile, ScreenMode, ScreenModeSelection,
};
use diagnostics::{Category, EmuError};
use std::fs::File;
use std::io::Read;
use std::path::Path;

mod dimensions;
mod validation;

pub use dimensions::dimension_pairs;
use validation::{invalid, validate_canvas_dimensions};

impl DeviceProfile {
    /// Maximum accepted encoded JSON document size.
    pub const MAX_JSON_BYTES: usize = 1024 * 1024;

    /// Loads and validates a UTF-8 JSON profile from a filesystem path.
    ///
    /// # Errors
    ///
    /// Returns a categorized [`EmuError`] for I/O, JSON or validation failures.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, LoadError> {
        let file = File::open(path).map_err(|error| {
            EmuError::with_source(
                Category::Api,
                "profile-open",
                "cannot read device profile",
                error,
            )
        })?;
        Self::from_reader(file)
    }

    /// Loads and validates a JSON profile from a reader.
    ///
    /// # Errors
    ///
    /// Returns a categorized [`EmuError`] for malformed, oversized or invalid input.
    pub fn from_reader(mut reader: impl Read) -> Result<Self, LoadError> {
        let limit = u64::try_from(Self::MAX_JSON_BYTES).map_or(u64::MAX, |size| size + 1);
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                EmuError::with_source(
                    Category::Api,
                    "profile-read",
                    "cannot read device profile",
                    error,
                )
            })?;
        if bytes.len() > Self::MAX_JSON_BYTES {
            return Err(EmuError::new(
                Category::Api,
                "profile-too-large",
                format!(
                    "device profile is {} bytes; limit is {}",
                    bytes.len(),
                    Self::MAX_JSON_BYTES
                ),
            ));
        }
        let profile: Self = serde_json::from_slice(&bytes).map_err(|error| {
            EmuError::with_source(
                Category::Api,
                "profile-json",
                "invalid device profile JSON",
                error,
            )
        })?;
        profile.validate()?;
        Ok(profile)
    }

    /// Returns the schema version. Version 2 is the only accepted value.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the stable handset-family identifier shared by related presets.
    #[must_use]
    pub fn family_id(&self) -> &str {
        self.family_id.as_deref().unwrap_or(&self.profile_id)
    }
    /// Returns the stable device profile identifier.
    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
    /// Returns the profile data version.
    #[must_use]
    pub fn profile_version(&self) -> &str {
        &self.profile_version
    }
    /// Returns whether the profile is research-only or hardware-verified.
    #[must_use]
    pub const fn status(&self) -> ProfileStatus {
        self.status
    }
    /// Returns the declarative persona/host split for launch composition.
    #[must_use]
    pub const fn composition(&self) -> &ProfileComposition {
        &self.composition
    }
    /// Returns the source catalog referenced by evidence objects.
    #[must_use]
    pub fn evidence_sources(&self) -> &[EvidenceSource] {
        &self.evidence_sources
    }
    /// Returns device identity and platform properties.
    #[must_use]
    pub const fn device(&self) -> &Device {
        &self.device
    }
    /// Returns physical display and Canvas dimensions.
    #[must_use]
    pub const fn display(&self) -> &DisplayProfile {
        &self.display
    }
    /// Returns host-side runtime performance limits for this handset.
    #[must_use]
    pub const fn runtime(&self) -> RuntimeProfile {
        self.runtime
    }
    /// Returns CLDC, MIDP, JSR and system-property information.
    #[must_use]
    pub const fn java(&self) -> &JavaProfile {
        &self.java
    }
    /// Returns the Canvas key map and soft-key behavior.
    #[must_use]
    pub const fn input(&self) -> &InputProfile {
        &self.input
    }
    /// Returns measured or unknown resource limits.
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }
    /// Returns measured or unknown media capabilities.
    #[must_use]
    pub const fn media(&self) -> &Media {
        &self.media
    }
    /// Returns JSR-184 capabilities and suite-scoped implementation budgets.
    #[must_use]
    pub const fn m3g(&self) -> Option<&M3gProfile> {
        self.m3g.as_ref()
    }

    /// Concrete full-screen Canvas dimensions required to run this profile.
    #[must_use]
    pub fn canvas_dimensions(&self) -> Option<(u32, u32)> {
        Some((
            *self.display.fullscreen_canvas_width.value()?,
            *self.display.fullscreen_canvas_height.value()?,
        ))
    }

    /// Concrete drawable dimensions exposed by a Canvas before it requests
    /// full-screen mode. Profiles without a separately measured normal area
    /// fall back to their full-screen Canvas dimensions.
    #[must_use]
    pub fn non_fullscreen_canvas_dimensions(&self) -> Option<(u32, u32)> {
        self.display
            .non_fullscreen_drawable_area
            .value()
            .map(|dimensions| (dimensions.width(), dimensions.height()))
            .or_else(|| self.canvas_dimensions())
    }

    /// Resolves runtime-selectable family capabilities without mutating the
    /// evidence-bearing device preset.
    ///
    /// # Errors
    /// Returns a profile diagnostic when an override is not a declared family
    /// screen mode, exceeds implementation bounds, or advertises pointer motion
    /// without pointer events.
    pub fn resolve(&self, overrides: ProfileOverrides) -> Result<ResolvedProfile<'_>, LoadError> {
        let screen_mode_selection = if overrides.canvas_dimensions.is_some() {
            ScreenModeSelection::ExplicitOverride
        } else {
            ScreenModeSelection::ProfileDefault
        };
        self.resolve_selected(self, overrides, screen_mode_selection, false)
    }

    /// Resolves a runtime profile and uses unambiguous screen dimensions found
    /// in merged manifest/JAD values when no explicit override was supplied.
    /// Only dimensions declared by this profile family are considered.
    ///
    /// # Errors
    /// Returns the same diagnostics as [`Self::resolve`].
    pub fn resolve_with_suite_properties<'value>(
        &self,
        overrides: ProfileOverrides,
        property_values: impl IntoIterator<Item = &'value str>,
    ) -> Result<ResolvedProfile<'_>, LoadError> {
        if overrides.canvas_dimensions.is_some() {
            return self.resolve(overrides);
        }
        if let Some((width, height)) = self.infer_canvas_dimensions(property_values) {
            return self.resolve_selected(
                self,
                overrides.with_canvas_dimensions(width, height),
                ScreenModeSelection::SuiteMetadata,
                false,
            );
        }
        self.resolve_selected(self, overrides, ScreenModeSelection::ProfileDefault, false)
    }

    /// Resolves a concrete Canvas variant selected by a frontend-neutral
    /// launch decision. This keeps the selection origin observable without
    /// changing the evidence-bearing profile.
    ///
    /// # Errors
    /// Returns the same diagnostics as [`Self::resolve`] when the selected
    /// dimensions are not a declared mode of this profile family.
    pub fn resolve_selected_canvas(
        &self,
        overrides: ProfileOverrides,
        canvas_dimensions: (u32, u32),
        selection: ScreenModeSelection,
    ) -> Result<ResolvedProfile<'_>, LoadError> {
        self.resolve_selected(
            self,
            overrides.with_canvas_dimensions(canvas_dimensions.0, canvas_dimensions.1),
            selection,
            false,
        )
    }

    /// Resolves this profile as the guest-visible target persona while using a
    /// separately validated runtime host. Screen and pointer semantics always
    /// come from the persona; the host does not need to declare its geometry.
    ///
    /// # Errors
    /// Returns the same diagnostics as [`Self::resolve_selected_canvas`].
    pub fn resolve_selected_canvas_with_host<'profile>(
        &'profile self,
        runtime_host: &'profile Self,
        overrides: ProfileOverrides,
        canvas_dimensions: (u32, u32),
        selection: ScreenModeSelection,
    ) -> Result<ResolvedProfile<'profile>, LoadError> {
        self.resolve_selected(
            runtime_host,
            overrides.with_canvas_dimensions(canvas_dimensions.0, canvas_dimensions.1),
            selection,
            true,
        )
    }

    fn infer_canvas_dimensions<'value>(
        &self,
        property_values: impl IntoIterator<Item = &'value str>,
    ) -> Option<(u32, u32)> {
        let mut inferred = None;
        for value in property_values {
            for dimensions in dimension_pairs(value) {
                if self
                    .display
                    .screen_mode_for_dimensions(dimensions)
                    .is_none()
                {
                    continue;
                }
                match inferred {
                    Some(previous) if previous != dimensions => return None,
                    Some(_) => {}
                    None => inferred = Some(dimensions),
                }
            }
        }
        inferred
    }

    fn resolve_selected<'profile>(
        &'profile self,
        runtime_host: &'profile Self,
        overrides: ProfileOverrides,
        screen_mode_selection: ScreenModeSelection,
        uses_automatic_host: bool,
    ) -> Result<ResolvedProfile<'profile>, LoadError> {
        let base_canvas_dimensions = self.canvas_dimensions().ok_or_else(|| {
            EmuError::new(
                Category::Api,
                "profile-canvas-size",
                "device profile must define full-screen Canvas dimensions before it can run",
            )
        })?;
        let base_non_fullscreen_dimensions = self
            .non_fullscreen_canvas_dimensions()
            .unwrap_or(base_canvas_dimensions);
        let (canvas_dimensions, non_fullscreen_canvas_dimensions, screen_mode_id) = if let Some(
            canvas_dimensions,
        ) =
            overrides.canvas_dimensions
        {
            if self.display.screen_modes().is_empty() {
                let non_fullscreen_dimensions = self.resolve_uncatalogued_canvas_override(
                    base_canvas_dimensions,
                    base_non_fullscreen_dimensions,
                    canvas_dimensions,
                )?;
                (canvas_dimensions, non_fullscreen_dimensions, None)
            } else {
                let (mode, transposed) = self
                        .display
                        .screen_mode_for_dimensions(canvas_dimensions)
                        .ok_or_else(|| {
                            let declared_modes = self
                                .display
                                .screen_modes()
                                .iter()
                                .map(ScreenMode::id)
                                .collect::<Vec<_>>()
                                .join(", ");
                            EmuError::new(
                                Category::Api,
                                "profile-canvas-size",
                                format!(
                                    "Canvas override is not a declared screen mode for the selected profile family; available modes: {declared_modes} (rotation allowed)"
                                ),
                            )
                        })?;
                (
                    canvas_dimensions,
                    mode.non_fullscreen_dimensions(transposed),
                    Some(mode.id()),
                )
            }
        } else {
            (
                base_canvas_dimensions,
                base_non_fullscreen_dimensions,
                self.display.default_screen_mode.as_deref(),
            )
        };
        validate_canvas_dimensions(canvas_dimensions)?;
        let pointer_events = overrides.pointer_events.unwrap_or_else(|| {
            self.input
                .pointer
                .as_ref()
                .and_then(|pointer| pointer.events.value().copied())
                .unwrap_or(false)
        });
        let pointer_motion_events = overrides.pointer_motion_events.unwrap_or_else(|| {
            self.input
                .pointer
                .as_ref()
                .and_then(|pointer| pointer.motion_events.value().copied())
                .unwrap_or(false)
        });
        if pointer_motion_events && !pointer_events {
            return invalid("pointer motion requires pointer events");
        }
        Ok(ResolvedProfile {
            persona: self,
            runtime_host,
            uses_automatic_host,
            canvas_dimensions,
            non_fullscreen_canvas_dimensions,
            screen_mode_id,
            screen_mode_selection,
            pointer_events,
            pointer_motion_events,
        })
    }

    fn resolve_uncatalogued_canvas_override(
        &self,
        base_canvas: (u32, u32),
        base_non_fullscreen: (u32, u32),
        canvas: (u32, u32),
    ) -> Result<(u32, u32), LoadError> {
        let (base_width, base_height) = base_canvas;
        let (width, height) = canvas;
        let is_base_orientation = canvas == base_canvas
            || (base_width != base_height && canvas == (base_height, base_width));
        if self.family_id.is_some() && !is_base_orientation {
            return Err(EmuError::new(
                Category::Api,
                "profile-canvas-size",
                "Canvas override is not the base screen mode or its rotation for the selected profile family",
            ));
        }
        let within_family_bounds = width.min(height) <= base_width.min(base_height)
            && width.max(height) <= base_width.max(base_height);
        if !within_family_bounds {
            return Err(EmuError::new(
                Category::Api,
                "profile-canvas-size",
                "Canvas override exceeds the selected profile family bounds",
            ));
        }
        if canvas == base_canvas {
            Ok(base_non_fullscreen)
        } else if base_width != base_height && canvas == (base_height, base_width) {
            Ok((base_non_fullscreen.1, base_non_fullscreen.0))
        } else {
            Ok(canvas)
        }
    }
}
