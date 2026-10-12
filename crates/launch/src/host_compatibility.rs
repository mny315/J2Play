//! Capability-superset validation for target-persona runtime hosts.

use device_profile::DeviceProfile;

pub(crate) fn is_compatible_host(host: &DeviceProfile, target: &DeviceProfile) -> bool {
    let host_policy = host.composition().host();
    let Some(capacities) = host_policy.capacity_limits() else {
        return false;
    };
    host.device()
        .manufacturer()
        .eq_ignore_ascii_case(target.device().manufacturer())
        && host_policy.automatic()
        && host_policy
            .accepts_lineages()
            .iter()
            .any(|lineage| lineage == target.composition().persona().lineage())
        && java_runtime_at_least(host, target)
        && supported_jsrs(target).all(|jsr| host.java().supports_jsr(jsr))
        && supported_vendor_apis(target).all(|api| host.java().supports_vendor_api(api))
        && media_formats_at_least(host, target)
        && m3g_at_least(host, target)
        && declared_limit_at_least(
            Some(capacities.heap_bytes()),
            target.limits().heap_bytes().value().copied(),
        )
        && declared_limit_at_least(
            Some(capacities.rms_bytes()),
            target.limits().rms_bytes().value().copied(),
        )
        && declared_limit_at_least(
            Some(capacities.jar_bytes()),
            target.limits().jar_bytes().value().copied(),
        )
}

fn media_formats_at_least(host: &DeviceProfile, target: &DeviceProfile) -> bool {
    let Some(target_formats) = target.media().mmapi_formats().value() else {
        return true;
    };
    host.media()
        .mmapi_formats()
        .value()
        .is_some_and(|host_formats| {
            target_formats
                .iter()
                .all(|format| host_formats.contains(format))
        })
}

fn m3g_at_least(host: &DeviceProfile, target: &DeviceProfile) -> bool {
    let Some(target) = target.m3g() else {
        return true;
    };
    let Some(host) = host.m3g() else {
        return false;
    };
    let version_at_least = match (host.version().value(), target.version().value()) {
        (Some(host), Some(target)) => match (decimal_version(host), decimal_version(target)) {
            (Some(host), Some(target)) => host >= target,
            _ => host == target,
        },
        (_, None) => true,
        (None, Some(_)) => false,
    };
    let host_properties = host.properties();
    let target_properties = target.properties();
    let boolean_properties_at_least = [
        (
            host_properties.support_antialiasing(),
            target_properties.support_antialiasing(),
        ),
        (
            host_properties.support_true_color(),
            target_properties.support_true_color(),
        ),
        (
            host_properties.support_dithering(),
            target_properties.support_dithering(),
        ),
        (
            host_properties.support_mipmapping(),
            target_properties.support_mipmapping(),
        ),
        (
            host_properties.support_perspective_correction(),
            target_properties.support_perspective_correction(),
        ),
        (
            host_properties.support_local_camera_lighting(),
            target_properties.support_local_camera_lighting(),
        ),
    ]
    .into_iter()
    .all(|(host, target)| {
        !target.value().copied().unwrap_or(false) || host.value().copied().unwrap_or(false)
    });
    let numeric_properties_at_least = [
        (host_properties.max_lights(), target_properties.max_lights()),
        (
            host_properties.max_viewport_width(),
            target_properties.max_viewport_width(),
        ),
        (
            host_properties.max_viewport_height(),
            target_properties.max_viewport_height(),
        ),
        (
            host_properties.max_viewport_dimension(),
            target_properties.max_viewport_dimension(),
        ),
        (
            host_properties.max_sprite_crop_dimension(),
            target_properties.max_sprite_crop_dimension(),
        ),
        (
            host_properties.max_transforms_per_vertex(),
            target_properties.max_transforms_per_vertex(),
        ),
        (
            host_properties.num_texture_units(),
            target_properties.num_texture_units(),
        ),
        (host_properties.depth_bits(), target_properties.depth_bits()),
    ]
    .into_iter()
    .all(|(host, target)| declared_limit_at_least(host.value(), target.value()));
    let host_texture_dimension = host
        .compatibility()
        .max_texture_dimension()
        .or_else(|| host_properties.max_texture_dimension().value().copied());
    let target_texture_dimension = target
        .compatibility()
        .max_texture_dimension()
        .or_else(|| target_properties.max_texture_dimension().value().copied());
    version_at_least
        && boolean_properties_at_least
        && numeric_properties_at_least
        && declared_limit_at_least(host_texture_dimension, target_texture_dimension)
        && m3g_budgets_at_least(host.budgets(), target.budgets())
}

fn m3g_budgets_at_least(
    host: &device_profile::M3gBudgets,
    target: &device_profile::M3gBudgets,
) -> bool {
    host.native_bytes() >= target.native_bytes()
        && host.objects() >= target.objects()
        && host.file_bytes() >= target.file_bytes()
        && host.decompressed_bytes() >= target.decompressed_bytes()
        && host.sections() >= target.sections()
        && host.vertices() >= target.vertices()
        && host.triangles_per_render() >= target.triangles_per_render()
        && host.texture_pixels() >= target.texture_pixels()
        && host.keyframes() >= target.keyframes()
        && host.graph_depth() >= target.graph_depth()
        && host.fragments_per_render() >= target.fragments_per_render()
}

fn decimal_version(value: &str) -> Option<(u16, u16)> {
    let (major, minor) = value.split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

fn declared_limit_at_least<T: Ord>(host: Option<T>, target: Option<T>) -> bool {
    target.is_none_or(|target| host.is_some_and(|host| host >= target))
}

fn java_runtime_at_least(host: &DeviceProfile, target: &DeviceProfile) -> bool {
    java_component_at_least(
        host.java().configuration().value(),
        target.java().configuration().value(),
        "CLDC-",
    ) && java_component_at_least(
        host.java().profile().value(),
        target.java().profile().value(),
        "MIDP-",
    )
}

fn java_component_at_least(host: Option<&String>, target: Option<&String>, prefix: &str) -> bool {
    match (
        host.and_then(|value| java_component_version(value, prefix)),
        target.and_then(|value| java_component_version(value, prefix)),
    ) {
        (Some(host), Some(target)) => host >= target,
        _ => host == target,
    }
}

fn java_component_version(value: &str, prefix: &str) -> Option<(u16, u16)> {
    decimal_version(value.strip_prefix(prefix)?)
}

fn supported_jsrs(profile: &DeviceProfile) -> impl Iterator<Item = &str> {
    profile
        .java()
        .jsrs()
        .iter()
        .filter_map(|(jsr, evidence)| (evidence.value() == Some(&true)).then_some(jsr.as_str()))
        .chain(
            profile
                .java()
                .compatibility_jsrs()
                .value()
                .into_iter()
                .flatten()
                .map(String::as_str),
        )
}

fn supported_vendor_apis(profile: &DeviceProfile) -> impl Iterator<Item = &str> {
    [
        profile.java().vendor_apis(),
        profile.java().compatibility_apis(),
    ]
    .into_iter()
    .filter_map(device_profile::Evidence::value)
    .flatten()
    .map(String::as_str)
}
