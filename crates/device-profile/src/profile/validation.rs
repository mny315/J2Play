//! Profile schema, capability bounds and evidence validation.

use crate::{Confidence, DeviceProfile, Evidence, LoadError, PointerProfile};
use diagnostics::{Category, EmuError};
use std::collections::HashSet;

impl DeviceProfile {
    pub(super) fn validate(&self) -> Result<(), LoadError> {
        validate_profile_identity(self)?;
        self.validate_composition()?;
        if self.evidence_sources.is_empty() {
            return invalid("at least one evidence source is required");
        }
        if self.device.manufacturer.is_empty() || self.device.model.is_empty() {
            return invalid("manufacturer and model must not be empty");
        }
        let source_ids: HashSet<_> = self
            .evidence_sources
            .iter()
            .map(|source| source.id.as_str())
            .collect();
        if source_ids.len() != self.evidence_sources.len() {
            return invalid("evidence source IDs must be unique");
        }
        for source in &self.evidence_sources {
            if !is_slug(&source.id) || source.title.is_empty() {
                return invalid("evidence source ID/title is invalid");
            }
        }
        self.validate_evidence(&source_ids)?;
        self.validate_base_display()?;
        if self
            .runtime
            .interpreter_instructions_per_second
            .is_some_and(|value| !(1..=1_000_000_000).contains(&value))
        {
            return invalid(
                "runtime interpreter_instructions_per_second must be between 1 and 1000000000",
            );
        }
        if !(1..=1_000).contains(&self.runtime.frames_per_second) {
            return invalid("runtime frames_per_second must be between 1 and 1000");
        }
        self.validate_display_modes()?;
        self.validate_archive_name_hints()?;
        validate_positive(&self.limits.heap_bytes, "heap_bytes")?;
        validate_positive(&self.limits.rms_bytes, "rms_bytes")?;
        validate_positive(&self.limits.jar_bytes, "jar_bytes")?;
        self.validate_m3g()?;
        validate_nonempty_string(&self.device.java_platform, "java_platform")?;
        validate_nonempty_string(&self.java.configuration, "configuration")?;
        validate_nonempty_string(&self.java.profile, "profile")?;
        validate_optional_nonempty_string(&self.device.firmware_revision, "firmware_revision")?;
        validate_optional_nonempty_string(&self.display.framebuffer_format, "framebuffer_format")?;
        validate_optional_nonempty_string(&self.input.soft_keys.canvas_events, "canvas_events")?;
        validate_optional_nonempty_string(
            &self.input.soft_keys.command_mapping,
            "command_mapping",
        )?;
        if let Some(pointer) = &self.input.pointer {
            validate_pointer_profile(pointer)?;
        }
        validate_capability_list(&self.java.vendor_apis, "vendor_apis")?;
        validate_capability_list(&self.java.compatibility_apis, "compatibility_apis")?;
        validate_jsr_list(&self.java.compatibility_jsrs, "compatibility_jsrs")?;
        validate_string_list(&self.media.mmapi_formats, "mmapi_formats")?;
        self.validate_bluetooth()?;
        for (jsr, evidence) in &self.java.jsrs {
            if !is_jsr_id(jsr) || evidence.value() != Some(&true) {
                return invalid("JSR IDs must be numeric slugs and their value must be true");
            }
        }
        let names: HashSet<_> = self.input.canvas_keys.iter().map(|key| key.name).collect();
        let codes: HashSet<_> = self
            .input
            .canvas_keys
            .iter()
            .map(|key| key.key_code)
            .collect();
        if names.len() != self.input.canvas_keys.len()
            || codes.len() != self.input.canvas_keys.len()
        {
            return invalid("Canvas key names and key codes must be unique");
        }
        if self.input.canvas_keys.is_empty() {
            return invalid("at least one Canvas key is required");
        }
        Ok(())
    }

    fn validate_composition(&self) -> Result<(), LoadError> {
        let persona = self.composition.persona();
        if !is_slug(persona.lineage()) {
            return invalid("persona lineage must be a lowercase ASCII slug");
        }
        if persona.generation() == 0 {
            return invalid("persona generation must be positive");
        }
        if !is_slug(persona.automatic_host()) {
            return invalid("persona automatic_host must be a lowercase ASCII slug");
        }

        let host = self.composition.host();
        let accepted = host.accepts_lineages();
        if accepted.iter().any(|lineage| !is_slug(lineage)) {
            return invalid("host accepts_lineages must contain lowercase ASCII slugs");
        }
        if accepted.iter().collect::<HashSet<_>>().len() != accepted.len() {
            return invalid("host accepts_lineages must be unique");
        }
        if host.automatic() {
            if host.priority() == 0 || accepted.is_empty() {
                return invalid(
                    "automatic host requires positive priority and at least one accepted lineage",
                );
            }
            let Some(capacities) = host.capacity_limits() else {
                return invalid("automatic host requires capacity_limits");
            };
            for value in [
                capacities.heap_bytes(),
                capacities.rms_bytes(),
                capacities.jar_bytes(),
            ] {
                if !(1..=1_073_741_824).contains(&value) {
                    return invalid("automatic-host capacities must be between 1 byte and 1 GiB");
                }
            }
        } else if host.priority() != 0 || !accepted.is_empty() || host.capacity_limits().is_some() {
            return invalid(
                "manual-only host must not declare priority, accepted lineages, or capacity limits",
            );
        }
        Ok(())
    }

    fn validate_base_display(&self) -> Result<(), LoadError> {
        for (name, value) in [
            ("physical_width", &self.display.physical_width),
            ("physical_height", &self.display.physical_height),
            ("physical_colors", &self.display.physical_colors),
        ] {
            validate_known(value, name)?;
            validate_positive(value, name)?;
        }
        validate_known(&self.display.orientation, "orientation")?;
        validate_positive(
            &self.display.fullscreen_canvas_width,
            "fullscreen_canvas_width",
        )?;
        validate_positive(
            &self.display.fullscreen_canvas_height,
            "fullscreen_canvas_height",
        )?;
        if let Some(dimensions) = self.canvas_dimensions() {
            validate_canvas_dimensions(dimensions)?;
        }
        if let (Some(fullscreen), Some(normal)) = (
            self.canvas_dimensions(),
            self.display.non_fullscreen_drawable_area.value(),
        ) && (normal.width > fullscreen.0 || normal.height > fullscreen.1)
        {
            return invalid("base drawable area must fit inside its full-screen Canvas");
        }
        if let Some(normal) = self.display.non_fullscreen_drawable_area.value()
            && (normal.width == 0 || normal.height == 0)
        {
            return invalid("drawable-area dimensions must be positive");
        }
        if let Some(font_heights) = &self.display.lcd_ui_font_heights {
            let Some(font_heights) = font_heights.value().copied() else {
                return invalid("LCDUI font heights must be known when declared");
            };
            let [small, medium, large] = font_heights.values();
            if font_heights
                .values()
                .into_iter()
                .any(|height| !(1..=64).contains(&height))
            {
                return invalid("LCDUI font heights must be between 1 and 64 pixels");
            }
            if small > medium || medium > large {
                return invalid("LCDUI font heights must be ordered small, medium, large");
            }
        }
        Ok(())
    }

    fn validate_display_modes(&self) -> Result<(), LoadError> {
        let (default_mode_id, screen_modes) = match (
            &self.display.default_screen_mode,
            &self.display.screen_modes,
        ) {
            (None, None) => return Ok(()),
            (None, Some(_)) => return invalid("screen_modes require default_screen_mode"),
            (Some(_), None) => return invalid("default_screen_mode requires screen_modes"),
            (Some(default_mode_id), Some(screen_modes)) => {
                (default_mode_id.as_str(), screen_modes.as_slice())
            }
        };
        if screen_modes.is_empty() {
            return invalid("screen_modes must not be empty");
        }

        let mut mode_ids = HashSet::new();
        let mut mode_dimensions = HashSet::new();
        for mode in screen_modes {
            if !is_slug(&mode.id) {
                return invalid("screen mode IDs must be lowercase ASCII slugs");
            }
            if !mode_ids.insert(mode.id.as_str()) {
                return invalid("screen mode IDs must be unique");
            }
            let fullscreen = mode.fullscreen_dimensions();
            validate_canvas_dimensions(fullscreen)?;
            let orientation_independent = (
                fullscreen.0.min(fullscreen.1),
                fullscreen.0.max(fullscreen.1),
            );
            if !mode_dimensions.insert(orientation_independent) {
                return invalid("screen mode dimensions must be unique including rotation");
            }
            if let Some(normal) = mode.non_fullscreen_drawable_area {
                if normal.width == 0 || normal.height == 0 {
                    return invalid("screen mode drawable dimensions must be positive");
                }
                if normal.width > fullscreen.0 || normal.height > fullscreen.1 {
                    return invalid(
                        "screen mode drawable area must fit inside its full-screen Canvas",
                    );
                }
            }
            if let Some(rotated_normal) = mode.rotated_non_fullscreen_drawable_area {
                if rotated_normal.width == 0 || rotated_normal.height == 0 {
                    return invalid("rotated screen mode drawable dimensions must be positive");
                }
                if rotated_normal.width > fullscreen.1 || rotated_normal.height > fullscreen.0 {
                    return invalid(
                        "rotated screen mode drawable area must fit inside its rotated full-screen Canvas",
                    );
                }
            }
        }

        let default_mode = self
            .display
            .screen_modes()
            .iter()
            .find(|mode| mode.id == default_mode_id)
            .ok_or_else(|| {
                EmuError::new(
                    Category::Api,
                    "profile-invalid",
                    "default_screen_mode must reference a declared screen mode",
                )
            })?;
        if self.canvas_dimensions() != Some(default_mode.fullscreen_dimensions())
            || self.non_fullscreen_canvas_dimensions()
                != Some(default_mode.non_fullscreen_dimensions(false))
        {
            return invalid("default screen mode must match the profile's base Canvas dimensions");
        }
        Ok(())
    }

    fn validate_archive_name_hints(&self) -> Result<(), LoadError> {
        let mut models = HashSet::new();
        for hint in self.device.archive_name_hints() {
            let has_model = hint.model.is_some();
            let has_models = !hint.models.is_empty();
            if has_model == has_models {
                return invalid("archive-name hint must declare exactly one of model or models");
            }
            for model in hint.model_names() {
                if !model.bytes().any(|byte| byte.is_ascii_alphanumeric()) {
                    return invalid("archive-name hint model must contain an ASCII token");
                }
                if !models.insert(model.trim().to_ascii_lowercase()) {
                    return invalid("archive-name hint models must be unique");
                }
            }
            let dimensions = hint.fullscreen_dimensions();
            validate_canvas_dimensions(dimensions)?;
            let declared = if self.display.screen_modes().is_empty() {
                self.canvas_dimensions().is_some_and(|base| {
                    dimensions == base || (base.0 != base.1 && dimensions == (base.1, base.0))
                })
            } else {
                self.display
                    .screen_mode_for_dimensions(dimensions)
                    .is_some()
            };
            if !declared {
                return invalid(
                    "archive-name hint Canvas must match a declared screen mode, including rotation",
                );
            }
        }
        Ok(())
    }

    fn validate_bluetooth(&self) -> Result<(), LoadError> {
        if self.java.bluetooth_properties.is_some() && !self.java.supports_jsr("82") {
            return invalid("bluetooth_properties require JSR-82 support");
        }
        if self
            .java
            .bluetooth_properties
            .as_ref()
            .and_then(Evidence::value)
            .is_some_and(|properties| {
                properties
                    .iter()
                    .any(|(name, value)| name.is_empty() || value.is_empty())
            })
        {
            return invalid("bluetooth property names and values must not be empty");
        }
        Ok(())
    }

    fn validate_m3g(&self) -> Result<(), LoadError> {
        let supports_m3g = self.java.supports_jsr("184");
        if supports_m3g != self.m3g.is_some() {
            return invalid("m3g profile must be present exactly when JSR-184 is declared");
        }
        let Some(m3g) = &self.m3g else {
            return Ok(());
        };
        validate_nonempty_string(&m3g.version, "m3g.version")?;
        for (name, value) in [
            ("support_antialiasing", &m3g.properties.support_antialiasing),
            ("support_true_color", &m3g.properties.support_true_color),
            ("support_dithering", &m3g.properties.support_dithering),
            ("support_mipmapping", &m3g.properties.support_mipmapping),
            (
                "support_perspective_correction",
                &m3g.properties.support_perspective_correction,
            ),
            (
                "support_local_camera_lighting",
                &m3g.properties.support_local_camera_lighting,
            ),
        ] {
            validate_known(value, name)?;
        }
        for (name, value) in [
            ("max_lights", &m3g.properties.max_lights),
            ("max_viewport_width", &m3g.properties.max_viewport_width),
            ("max_viewport_height", &m3g.properties.max_viewport_height),
            (
                "max_viewport_dimension",
                &m3g.properties.max_viewport_dimension,
            ),
            (
                "max_texture_dimension",
                &m3g.properties.max_texture_dimension,
            ),
            (
                "max_sprite_crop_dimension",
                &m3g.properties.max_sprite_crop_dimension,
            ),
            (
                "max_transforms_per_vertex",
                &m3g.properties.max_transforms_per_vertex,
            ),
            ("num_texture_units", &m3g.properties.num_texture_units),
        ] {
            validate_known(value, name)?;
            validate_positive(value, name)?;
        }
        validate_positive(&m3g.properties.depth_bits, "depth_bits")?;
        if m3g.budgets.values().contains(&0) {
            return invalid("M3G budgets must be positive");
        }
        let device_texture_dimension = m3g
            .properties
            .max_texture_dimension
            .value()
            .copied()
            .ok_or_else(|| {
                EmuError::new(
                    Category::Api,
                    "profile-invalid",
                    "M3G max_texture_dimension must be known",
                )
            })?;
        if let Some(compatibility_dimension) = m3g.compatibility.max_texture_dimension {
            if compatibility_dimension < device_texture_dimension {
                return invalid(
                    "M3G compatibility texture dimension must not be below the device property",
                );
            }
            let square_pixels =
                u64::from(compatibility_dimension) * u64::from(compatibility_dimension);
            if square_pixels > u64::try_from(m3g.budgets.texture_pixels).unwrap_or(u64::MAX) {
                return invalid(
                    "M3G compatibility texture dimension exceeds the texture pixel budget",
                );
            }
        }
        Ok(())
    }

    fn validate_evidence(&self, source_ids: &HashSet<&str>) -> Result<(), LoadError> {
        macro_rules! validate {
            ($($item:expr),+ $(,)?) => {
                $(EvidenceRef::from($item).validate(source_ids)?;)+
            };
        }
        validate!(&self.device.firmware_revision, &self.device.java_platform);
        for hint in self.device.archive_name_hints() {
            EvidenceRef {
                known: true,
                confidence: hint.confidence,
                sources: &hint.sources,
            }
            .validate(source_ids)?;
        }
        validate!(
            &self.display.physical_width,
            &self.display.physical_height,
            &self.display.orientation,
            &self.display.physical_colors,
            &self.display.fullscreen_canvas_width,
            &self.display.fullscreen_canvas_height,
            &self.display.non_fullscreen_drawable_area,
            &self.display.framebuffer_format
        );
        if let Some(font_heights) = &self.display.lcd_ui_font_heights {
            validate!(font_heights);
        }
        for mode in self.display.screen_modes() {
            EvidenceRef {
                known: true,
                confidence: mode.confidence,
                sources: &mode.sources,
            }
            .validate(source_ids)?;
        }
        validate!(
            &self.java.configuration,
            &self.java.profile,
            &self.java.compatibility_jsrs,
            &self.java.vendor_apis,
            &self.java.compatibility_apis,
            &self.java.system_properties
        );
        if let Some(properties) = &self.java.bluetooth_properties {
            validate!(properties);
        }
        for item in self.java.jsrs.values() {
            validate!(item);
        }
        validate!(
            &self.input.soft_keys.fullscreen_canvas_available,
            &self.input.soft_keys.canvas_events,
            &self.input.soft_keys.command_mapping,
            &self.limits.heap_bytes,
            &self.limits.rms_bytes,
            &self.limits.jar_bytes,
            &self.media.mmapi_formats
        );
        if let Some(pointer) = &self.input.pointer {
            validate!(&pointer.events, &pointer.motion_events);
        }
        if let Some(m3g) = &self.m3g {
            validate!(
                &m3g.version,
                &m3g.properties.support_antialiasing,
                &m3g.properties.support_true_color,
                &m3g.properties.support_dithering,
                &m3g.properties.support_mipmapping,
                &m3g.properties.support_perspective_correction,
                &m3g.properties.support_local_camera_lighting,
                &m3g.properties.max_lights,
                &m3g.properties.max_viewport_width,
                &m3g.properties.max_viewport_height,
                &m3g.properties.max_viewport_dimension,
                &m3g.properties.max_texture_dimension,
                &m3g.properties.max_sprite_crop_dimension,
                &m3g.properties.max_transforms_per_vertex,
                &m3g.properties.num_texture_units,
                &m3g.properties.depth_bits,
                &m3g.properties.color_format
            );
        }
        for key in &self.input.canvas_keys {
            EvidenceRef {
                known: true,
                confidence: key.confidence,
                sources: &key.sources,
            }
            .validate(source_ids)?;
        }
        Ok(())
    }
}

pub(super) fn invalid<T>(message: &str) -> Result<T, LoadError> {
    Err(EmuError::new(Category::Api, "profile-invalid", message))
}

pub(super) fn validate_canvas_dimensions((width, height): (u32, u32)) -> Result<(), LoadError> {
    if width == 0
        || height == 0
        || width > 4_096
        || height > 4_096
        || u64::from(width) * u64::from(height) > 4_194_304
    {
        return invalid("full-screen Canvas dimensions exceed implementation bounds");
    }
    Ok(())
}

fn validate_profile_identity(profile: &DeviceProfile) -> Result<(), LoadError> {
    if profile.schema_version != 2 {
        return invalid("schema_version must be 2");
    }
    if profile
        .family_id
        .as_deref()
        .is_some_and(|value| !is_slug(value))
    {
        return invalid("family_id must be a lowercase ASCII slug");
    }
    if !is_slug(&profile.profile_id) {
        return invalid("profile_id must be a lowercase ASCII slug");
    }
    if !is_semver_triplet(&profile.profile_version) {
        return invalid("profile_version must contain three numeric components");
    }
    Ok(())
}

fn validate_pointer_profile(pointer: &PointerProfile) -> Result<(), LoadError> {
    validate_known(&pointer.events, "pointer events")?;
    validate_known(&pointer.motion_events, "pointer motion events")?;
    if pointer.motion_events.value() == Some(&true) && pointer.events.value() != Some(&true) {
        return invalid("pointer motion requires pointer events");
    }
    Ok(())
}

struct EvidenceRef<'a> {
    known: bool,
    confidence: Confidence,
    sources: &'a [String],
}
impl<'a, T> From<&'a Evidence<T>> for EvidenceRef<'a> {
    fn from(value: &'a Evidence<T>) -> Self {
        Self {
            known: value.value.is_some(),
            confidence: value.confidence,
            sources: &value.sources,
        }
    }
}
impl EvidenceRef<'_> {
    fn validate(&self, known_sources: &HashSet<&str>) -> Result<(), LoadError> {
        if (self.confidence == Confidence::Unknown) == self.known {
            return invalid(
                "unknown evidence must have null value; known evidence must have a value",
            );
        }
        if self.known && self.sources.is_empty() {
            return invalid("known evidence must reference a source");
        }
        if !self.known && !self.sources.is_empty() {
            return invalid("unknown evidence cannot reference sources");
        }
        if self
            .sources
            .iter()
            .any(|source| !known_sources.contains(source.as_str()))
        {
            return invalid("evidence references an unknown source ID");
        }
        if self.sources.len() > 1 {
            let unique: HashSet<_> = self.sources.iter().collect();
            if unique.len() != self.sources.len() {
                return invalid("evidence source references must be unique");
            }
        }
        Ok(())
    }
}

fn is_slug(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn is_semver_triplet(value: &str) -> bool {
    let mut parts = value.split('.');
    (0..3).all(|_| {
        parts
            .next()
            .is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    }) && parts.next().is_none()
}

fn is_jsr_id(value: &str) -> bool {
    let mut parts = value.splitn(2, '-');
    let number = parts.next().unwrap_or_default();
    if number.is_empty()
        || number.starts_with('0')
        || !number.bytes().all(|byte| byte.is_ascii_digit())
    {
        return false;
    }
    parts.next().is_none_or(|suffix| {
        !suffix.is_empty()
            && suffix.as_bytes()[0].is_ascii_lowercase()
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    })
}

fn validate_positive<T>(evidence: &Evidence<T>, name: &str) -> Result<(), LoadError>
where
    T: Copy + PartialEq + From<u8>,
{
    if evidence.value().is_some_and(|value| *value == T::from(0)) {
        return invalid(&format!("{name} must be positive"));
    }
    Ok(())
}

fn validate_known<T>(evidence: &Evidence<T>, name: &str) -> Result<(), LoadError> {
    if evidence.value().is_none() {
        return invalid(&format!("{name} must be known"));
    }
    Ok(())
}

fn validate_nonempty_string(evidence: &Evidence<String>, name: &str) -> Result<(), LoadError> {
    if evidence.value().is_none_or(String::is_empty) {
        return invalid(&format!("{name} must be a non-empty known string"));
    }
    Ok(())
}

fn validate_optional_nonempty_string(
    evidence: &Evidence<String>,
    name: &str,
) -> Result<(), LoadError> {
    if evidence.value().is_some_and(String::is_empty) {
        return invalid(&format!("{name} must not be empty when known"));
    }
    Ok(())
}

fn validate_string_list(evidence: &Evidence<Vec<String>>, name: &str) -> Result<(), LoadError> {
    if let Some(values) = evidence.value() {
        let unique: HashSet<_> = values.iter().collect();
        if values.iter().any(String::is_empty) || unique.len() != values.len() {
            return invalid(&format!("{name} must contain unique non-empty strings"));
        }
    }
    Ok(())
}

fn validate_capability_list(evidence: &Evidence<Vec<String>>, name: &str) -> Result<(), LoadError> {
    validate_string_list(evidence, name)?;
    if evidence
        .value()
        .is_some_and(|values| values.iter().any(|value| !is_slug(value)))
    {
        return invalid(&format!("{name} must contain lowercase capability slugs"));
    }
    Ok(())
}

fn validate_jsr_list(evidence: &Evidence<Vec<String>>, name: &str) -> Result<(), LoadError> {
    validate_string_list(evidence, name)?;
    if evidence
        .value()
        .is_some_and(|values| values.iter().any(|value| !is_jsr_id(value)))
    {
        return invalid(&format!("{name} must contain numeric JSR slugs"));
    }
    Ok(())
}
