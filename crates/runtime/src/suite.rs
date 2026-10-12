use diagnostics::{Category, EmuError};
use runtime_bootstrap::{class_requests_jsr239, profile_supports_suite_bootstrap};
use std::collections::{BTreeMap, HashSet};

pub fn install_rust_bootstrap<S: std::hash::BuildHasher>(
    program: &mut vm::Program,
    limits: &vm::Limits,
    resolved_profile: &device_profile::ResolvedProfile<'_>,
    requested_compatibility_jsrs: &HashSet<String, S>,
    replace_bundled_nokia_full_canvas: bool,
) -> Result<(), EmuError> {
    let profile = resolved_profile.persona();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display_modes(
        resolved_profile.canvas_dimensions(),
        resolved_profile.non_fullscreen_canvas_dimensions(),
    ) {
        let replaces_bundled_adapter = replace_bundled_nokia_full_canvas
            && entry.class.class_name(entry.class.this_class)
                == Some("com/nokia/mid/ui/FullCanvas");
        if replaces_bundled_adapter
            || profile_supports_suite_bootstrap(
                profile,
                entry.requirement,
                requested_compatibility_jsrs,
            )
        {
            program.add_class(&entry.class, limits)?;
        }
    }
    Ok(())
}

pub fn suite_bundles_nokia_full_canvas_adapter(
    resources: &[jar::ClassResource],
) -> Result<bool, EmuError> {
    for resource in resources {
        if resource.name == "com/nokia/mid/ui/FullCanvas.class"
            && runtime_bootstrap::is_bundled_nokia_full_canvas_adapter(&parse_class_resource(
                resource,
            )?)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Finds suite-requested APIs that the selected persona permits conditionally.
/// Profiles without optional JSR-239 do not need this extra class parse; the
/// installation pass still validates every class before adding it to the VM.
pub fn suite_requested_compatibility_jsrs(
    resources: &[jar::ClassResource],
    profile: &device_profile::DeviceProfile,
    cancelled: impl Fn() -> bool,
) -> Result<Option<HashSet<String>>, EmuError> {
    if !profile
        .java()
        .compatibility_jsrs()
        .value()
        .is_some_and(|jsrs| jsrs.iter().any(|jsr| jsr == "239"))
    {
        return Ok((!cancelled()).then(HashSet::new));
    }
    let mut requested = HashSet::new();
    for resource in resources {
        if cancelled() {
            return Ok(None);
        }
        if class_requests_jsr239(&parse_class_resource(resource)?) {
            requested.insert("239".to_owned());
            break;
        }
    }
    Ok(Some(requested))
}

fn parse_class_resource(resource: &jar::ClassResource) -> Result<classfile::ClassFile, EmuError> {
    classfile::parse(&resource.bytes).map_err(|error| {
        EmuError::with_source(
            Category::ClassLoading,
            "class-parse",
            format!("cannot parse {}: {error}", resource.name),
            error,
        )
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/runtime/suite.rs"]
mod tests;

pub fn install_suite_classes(
    program: &mut vm::Program,
    limits: &vm::Limits,
    resources: Vec<jar::ClassResource>,
    cancelled: impl Fn() -> bool,
) -> Result<bool, EmuError> {
    for resource in resources {
        if cancelled() {
            return Ok(false);
        }
        let parsed = parse_class_resource(&resource)?;
        let class_name = parsed
            .class_name(parsed.this_class)
            .ok_or_else(|| EmuError::new(Category::ClassLoading, "invalid-class", resource.name))?;
        if !program.contains_class(class_name) {
            program.add_class(&parsed, limits)?;
        }
    }
    Ok(true)
}

#[must_use]
pub fn profile_supports_m3g(profile: &device_profile::DeviceProfile) -> bool {
    profile.java().supports_jsr("184")
}

#[must_use]
pub fn profile_supports_micro3d(profile: &device_profile::DeviceProfile) -> bool {
    profile
        .java()
        .supports_vendor_api("mascot-capsule-micro3d-v3")
}

pub fn apply_profile_limits(
    limits: &mut vm::Limits,
    profile: &device_profile::ResolvedProfile<'_>,
) -> Result<(), EmuError> {
    let persona = profile.persona();
    let runtime_host = profile.runtime_host();
    let (width, height) = profile.canvas_dimensions();
    let (normal_width, normal_height) = profile.non_fullscreen_canvas_dimensions();
    limits.lcd_width = width;
    limits.lcd_height = height;
    limits.lcd_normal_width = normal_width;
    limits.lcd_normal_height = normal_height;
    limits.lcd_fullscreen_available = persona
        .input()
        .soft_keys()
        .fullscreen_canvas_available()
        .value()
        .copied()
        .unwrap_or(true);
    let host_heap_bytes = if profile.uses_automatic_host() {
        runtime_host
            .composition()
            .host()
            .capacity_limits()
            .map(device_profile::HostCapacityLimits::heap_bytes)
    } else {
        persona.limits().heap_bytes().value().copied()
    };
    if let Some(heap_bytes) = host_heap_bytes {
        limits.max_heap_bytes = usize::try_from(heap_bytes).map_err(|_| {
            EmuError::new(
                Category::Api,
                "profile-heap-limit",
                "runtime-host heap limit exceeds host address space",
            )
        })?;
    }
    if let Some(persona_m3g) = persona.m3g() {
        let host_m3g = runtime_host.m3g().ok_or_else(|| {
            EmuError::new(
                Category::Api,
                "profile-m3g-host",
                "runtime host cannot implement the persona's JSR-184 contract",
            )
        })?;
        apply_m3g_profile_limits(limits, persona_m3g, host_m3g)?;
    }
    Ok(())
}

fn apply_m3g_profile_limits(
    limits: &mut vm::Limits,
    persona: &device_profile::M3gProfile,
    runtime_host: &device_profile::M3gProfile,
) -> Result<(), EmuError> {
    let budgets = *runtime_host.budgets();
    limits.m3g_arena = m3g::ArenaLimits {
        objects: budgets.objects(),
        bytes: budgets.native_bytes(),
    };
    limits.m3g_loader = m3g::LoaderLimits {
        file_bytes: budgets.file_bytes(),
        decompressed_bytes: budgets.decompressed_bytes(),
        sections: budgets.sections(),
        objects: budgets.objects(),
        object_bytes: budgets.file_bytes(),
    };
    apply_m3g_properties(limits, persona, runtime_host)?;
    limits.m3g_render.triangles = u64::try_from(budgets.triangles_per_render()).unwrap_or(u64::MAX);
    limits.m3g_render.fragments = u64::try_from(budgets.fragments_per_render()).unwrap_or(u64::MAX);
    limits.m3g_vertices = budgets.vertices();
    limits.m3g_texture_pixels = budgets.texture_pixels();
    limits.m3g_keyframes = budgets.keyframes();
    limits.m3g_graph_depth = budgets.graph_depth();
    limits.micro3d_objects = budgets.objects();
    limits.micro3d_bytes = budgets.native_bytes();
    limits.micro3d_loader = micro3d::LoaderLimits {
        file_bytes: budgets.file_bytes(),
        vertices: budgets.vertices(),
        faces: budgets.triangles_per_render(),
        bones: budgets.graph_depth(),
        actions: budgets.objects(),
        keyframes: budgets.keyframes(),
        texture_pixels: budgets.texture_pixels(),
        decoded_bytes: budgets.native_bytes(),
    };
    Ok(())
}

fn required_m3g_property<T: Copy>(
    property: &device_profile::Evidence<T>,
    name: &str,
) -> Result<T, EmuError> {
    property.value().copied().ok_or_else(|| {
        EmuError::new(
            Category::Api,
            "profile-m3g-property",
            format!("M3G property {name} must be known before enabling JSR-184"),
        )
    })
}

fn apply_m3g_properties(
    limits: &mut vm::Limits,
    persona: &device_profile::M3gProfile,
    runtime_host: &device_profile::M3gProfile,
) -> Result<(), EmuError> {
    let properties = persona.properties();
    limits.m3g_support_antialiasing =
        required_m3g_property(properties.support_antialiasing(), "supportAntialiasing")?;
    limits.m3g_support_true_color =
        required_m3g_property(properties.support_true_color(), "supportTrueColor")?;
    limits.m3g_support_dithering =
        required_m3g_property(properties.support_dithering(), "supportDithering")?;
    limits.m3g_support_mipmapping =
        required_m3g_property(properties.support_mipmapping(), "supportMipmapping")?;
    limits.m3g_support_perspective_correction = required_m3g_property(
        properties.support_perspective_correction(),
        "supportPerspectiveCorrection",
    )?;
    limits.m3g_support_local_camera_lighting = required_m3g_property(
        properties.support_local_camera_lighting(),
        "supportLocalCameraLighting",
    )?;
    let max_viewport_dimension =
        required_m3g_property(properties.max_viewport_dimension(), "maxViewportDimension")?;
    let advertised_width =
        required_m3g_property(properties.max_viewport_width(), "maxViewportWidth")?;
    let advertised_height =
        required_m3g_property(properties.max_viewport_height(), "maxViewportHeight")?;
    let max_lights = usize::try_from(required_m3g_property(properties.max_lights(), "maxLights")?)
        .map_err(|_| {
            EmuError::new(
                Category::Api,
                "profile-m3g-limit",
                "M3G light limit exceeds host address space",
            )
        })?;
    let texture_units = usize::try_from(required_m3g_property(
        properties.num_texture_units(),
        "numTextureUnits",
    )?)
    .map_err(|_| {
        EmuError::new(
            Category::Api,
            "profile-m3g-limit",
            "M3G texture-unit limit exceeds host address space",
        )
    })?;
    if max_lights > m3g::MAX_LIGHTS || texture_units > m3g::MAX_TEXTURE_UNITS {
        return Err(EmuError::new(
            Category::Api,
            "profile-m3g-limit",
            "device profile advertises capabilities beyond the software backend",
        ));
    }
    limits.m3g_render.max_viewport_width = advertised_width.min(max_viewport_dimension);
    limits.m3g_render.max_viewport_height = advertised_height.min(max_viewport_dimension);
    limits.m3g_max_lights = max_lights;
    limits.m3g_max_viewport_width = advertised_width;
    limits.m3g_max_viewport_height = advertised_height;
    limits.m3g_max_viewport_dimension = max_viewport_dimension;
    limits.m3g_num_texture_units = texture_units;
    let texture_dimension =
        required_m3g_property(properties.max_texture_dimension(), "maxTextureDimension")?;
    limits.m3g_max_texture_dimension = texture_dimension;
    limits.m3g_max_sprite_crop_dimension = required_m3g_property(
        properties.max_sprite_crop_dimension(),
        "maxSpriteCropDimension",
    )?;
    limits.m3g_max_transforms_per_vertex = required_m3g_property(
        properties.max_transforms_per_vertex(),
        "maxTransformsPerVertex",
    )?;
    limits.m3g_compatibility_max_texture_dimension = runtime_host
        .compatibility()
        .max_texture_dimension()
        .or_else(|| {
            runtime_host
                .properties()
                .max_texture_dimension()
                .value()
                .copied()
        })
        .unwrap_or(texture_dimension);
    Ok(())
}

pub fn profile_display_colors(profile: &device_profile::DeviceProfile) -> Result<i32, EmuError> {
    let colors = profile
        .display()
        .physical_colors()
        .value()
        .copied()
        .ok_or_else(|| {
            EmuError::new(
                Category::Api,
                "profile-display-colors",
                "device profile does not define physical colors",
            )
        })?;
    i32::try_from(colors).map_err(|_| {
        EmuError::new(
            Category::Api,
            "profile-display-colors",
            "device profile physical color count exceeds Java int",
        )
    })
}

pub fn profile_lcd_ui_font_heights(
    profile: &device_profile::DeviceProfile,
) -> Option<device_profile::LcdUiFontHeights> {
    profile
        .display()
        .lcd_ui_font_heights()
        .and_then(device_profile::Evidence::value)
        .copied()
}

#[must_use]
pub fn profile_input_map(profile: &device_profile::DeviceProfile) -> platform::DeviceInputMap {
    let entries = profile.input().canvas_keys().iter().map(|key| {
        let action = match key.name() {
            device_profile::KeyName::Up => platform::HostAction::Up,
            device_profile::KeyName::Down => platform::HostAction::Down,
            device_profile::KeyName::Left => platform::HostAction::Left,
            device_profile::KeyName::Right => platform::HostAction::Right,
            device_profile::KeyName::Fire => platform::HostAction::Fire,
            device_profile::KeyName::SoftLeft => platform::HostAction::SoftLeft,
            device_profile::KeyName::SoftRight => platform::HostAction::SoftRight,
            device_profile::KeyName::Clear => platform::HostAction::Clear,
            device_profile::KeyName::Back | device_profile::KeyName::End => {
                platform::HostAction::Back
            }
            device_profile::KeyName::GameButtonA => platform::HostAction::GameA,
            device_profile::KeyName::GameButtonB => platform::HostAction::GameB,
            device_profile::KeyName::Star => platform::HostAction::Star,
            device_profile::KeyName::Pound => platform::HostAction::Pound,
            device_profile::KeyName::Num0 => platform::HostAction::Num0,
            device_profile::KeyName::Num1 => platform::HostAction::Num1,
            device_profile::KeyName::Num2 => platform::HostAction::Num2,
            device_profile::KeyName::Num3 => platform::HostAction::Num3,
            device_profile::KeyName::Num4 => platform::HostAction::Num4,
            device_profile::KeyName::Num5 => platform::HostAction::Num5,
            device_profile::KeyName::Num6 => platform::HostAction::Num6,
            device_profile::KeyName::Num7 => platform::HostAction::Num7,
            device_profile::KeyName::Num8 => platform::HostAction::Num8,
            device_profile::KeyName::Num9 => platform::HostAction::Num9,
        };
        (
            action,
            platform::DeviceKey {
                key_code: key.key_code(),
                game_action: key.game_action().map(device_profile::GameAction::code),
            },
        )
    });
    platform::DeviceInputMap::new(entries)
}

pub fn profile_canvas_dimensions(
    profile: &device_profile::DeviceProfile,
) -> Result<(u32, u32), EmuError> {
    profile.canvas_dimensions().ok_or_else(|| {
        EmuError::new(
            Category::Api,
            "profile-display-unknown",
            "device profile must define full-screen Canvas dimensions before it can run",
        )
    })
}

pub fn profile_non_fullscreen_canvas_dimensions(
    profile: &device_profile::DeviceProfile,
) -> Result<(u32, u32), EmuError> {
    let normal = profile.non_fullscreen_canvas_dimensions().ok_or_else(|| {
        EmuError::new(
            Category::Api,
            "profile-display-unknown",
            "device profile must define ordinary Canvas dimensions before it can run",
        )
    })?;
    let fullscreen = profile_canvas_dimensions(profile)?;
    if normal.0 > fullscreen.0 || normal.1 > fullscreen.1 {
        return Err(EmuError::new(
            Category::Api,
            "profile-display-size",
            "ordinary Canvas dimensions must fit inside the full-screen Canvas",
        ));
    }
    Ok(normal)
}

pub struct CanvasInputProfile {
    by_key_code: BTreeMap<i32, (&'static str, Option<i32>)>,
    preferred_key_code: BTreeMap<i32, i32>,
}

impl CanvasInputProfile {
    #[must_use]
    pub fn from_profile(profile: &device_profile::DeviceProfile) -> Self {
        let mut by_key_code = BTreeMap::new();
        let mut preferred_key_code = BTreeMap::new();
        for key in profile.input().canvas_keys() {
            let game_action = key.game_action().map(device_profile::GameAction::code);
            by_key_code.insert(key.key_code(), (key.name().as_str(), game_action));
            if let Some(game_action) = game_action {
                preferred_key_code
                    .entry(game_action)
                    .or_insert(key.key_code());
            }
        }
        Self {
            by_key_code,
            preferred_key_code,
        }
    }

    /// Known keys without a game action return zero; unknown keys return `None`.
    #[must_use]
    pub fn game_action(&self, key_code: i32) -> Option<i32> {
        Some(self.by_key_code.get(&key_code)?.1.unwrap_or(0))
    }

    #[must_use]
    pub fn key_code(&self, game_action: i32) -> Option<i32> {
        self.preferred_key_code.get(&game_action).copied()
    }

    #[must_use]
    pub fn key_name(&self, key_code: i32) -> Option<&str> {
        self.by_key_code.get(&key_code).map(|(name, _)| *name)
    }
}

/// Register the native implementations visible through the selected persona.
pub fn register_profile_natives(
    program: &mut vm::Program,
    profile: &device_profile::DeviceProfile,
) -> Result<(), EmuError> {
    cldc::register_core_natives(program.native_registry_mut())?;
    midp::register_natives(program.native_registry_mut())?;
    rms::register_natives(program.native_registry_mut())?;
    mmapi::register_natives(program.native_registry_mut())?;
    gcf::register_natives(program.native_registry_mut())?;
    if profile.java().supports_jsr("82") {
        bluetooth::register_natives(program.native_registry_mut())?;
    }
    if profile_supports_m3g(profile) {
        m3g::register_natives(program.native_registry_mut())?;
    }
    if profile_supports_micro3d(profile) {
        micro3d::register_natives(program.native_registry_mut())?;
    }
    Ok(())
}
