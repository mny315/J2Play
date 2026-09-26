//! Embedded device-profile catalog shared by every frontend.

use crate::archive_name::separator_tokens;
use crate::host_compatibility::is_compatible_host;
use device_profile::DeviceProfile;
use diagnostics::{Category, EmuError};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::sync::OnceLock;

static PARSED_CATALOG: OnceLock<Vec<DeviceProfile>> = OnceLock::new();

macro_rules! embedded_profiles {
    ($(($id:literal, $path:literal)),+ $(,)?) => {
        const BUILTIN_PROFILES: &[(&str, &[u8])] = &[
            $(($id, include_bytes!(concat!("../../../profiles/", $path)))),+
        ];
    };
}

// Order is observable only as the final fallback between otherwise identical
// evidence scores. Keep compatibility defaults ahead of exact historical
// personae; model, descriptor, API and screen evidence still outrank it.
embedded_profiles!(
    ("se-featurephone", "sony-ericsson/featurephone.json"),
    ("nokia-featurephone", "nokia/featurephone.json"),
    // MIDP 2.0 is the safer unresolved old Series 40 compatibility default.
    ("nokia-s40-v2-keypad", "nokia/s40-v2-keypad.json"),
    ("nokia-s40-v1-keypad", "nokia/s40-v1-keypad.json"),
    ("nokia-s40-touch", "nokia/s40-touch.json"),
    ("nokia-asha-touch", "nokia/asha-touch.json"),
    // Likewise prefer S60 2nd Edition for an unresolved 176x208 target.
    ("nokia-s60-v2-keypad", "nokia/s60-v2-keypad.json"),
    ("nokia-s60-v1-keypad", "nokia/s60-v1-keypad.json"),
    ("nokia-s60-keypad", "nokia/s60-keypad.json"),
    ("nokia-s60-touch", "nokia/s60-touch.json"),
    ("siemens-featurephone", "siemens/featurephone.json"),
    (
        "benq-siemens-featurephone",
        "benq-siemens/featurephone.json"
    ),
    ("siemens-sgold-keypad", "siemens/sgold-keypad.json"),
    (
        "siemens-midp1-color-keypad",
        "siemens/midp1-color-keypad.json"
    ),
    (
        "siemens-midp1-basic-keypad",
        "siemens/midp1-basic-keypad.json"
    ),
    ("siemens-sxg75-keypad", "siemens/sxg75-keypad.json"),
    ("benq-siemens-ef81-keypad", "benq-siemens/ef81-keypad.json"),
    // Broad Samsung automatic hosts precede exact contract variants.
    ("samsung-featurephone", "samsung/featurephone.json"),
    ("samsung-touch", "samsung/touch.json"),
    ("samsung-midp2-keypad", "samsung/midp2-keypad.json"),
    (
        "samsung-midp2-cldc10-keypad",
        "samsung/midp2-cldc10-keypad.json"
    ),
    ("samsung-midp1-keypad", "samsung/midp1-keypad.json"),
    ("samsung-midp20-touch", "samsung/midp20-touch.json"),
    ("samsung-midp21-touch", "samsung/midp21-touch.json"),
    ("samsung-f480-touch", "samsung/f480-touch.json"),
    ("samsung-midp20-m3g-touch", "samsung/midp20-m3g-touch.json"),
    ("samsung-wvga-touch", "samsung/wvga-touch.json"),
    // Exact Sony Ericsson generations follow established fallback profiles.
    ("se-jp1-keypad", "sony-ericsson/jp1-keypad.json"),
    ("se-jp2-keypad", "sony-ericsson/jp2-keypad.json"),
    ("se-jp3-keypad", "sony-ericsson/jp3-keypad.json"),
    ("se-jp4-keypad", "sony-ericsson/jp4-keypad.json"),
    ("se-jp5-keypad", "sony-ericsson/jp5-keypad.json"),
    ("se-jp6-keypad", "sony-ericsson/jp6-keypad.json"),
    (
        "se-jp6-no-bluetooth-keypad",
        "sony-ericsson/jp6-no-bluetooth-keypad.json"
    ),
    ("se-jp6-touch", "sony-ericsson/jp6-touch.json"),
    ("se-jp7-basic-keypad", "sony-ericsson/jp7-basic-keypad.json"),
    ("se-jp7-media-keypad", "sony-ericsson/jp7-media-keypad.json"),
    (
        "se-jp7-no-bluetooth-keypad",
        "sony-ericsson/jp7-no-bluetooth-keypad.json"
    ),
    ("se-jp8-keypad", "sony-ericsson/jp8-keypad.json"),
    ("se-jp8-late-keypad", "sony-ericsson/jp8-late-keypad.json"),
    ("se-jp8-touch", "sony-ericsson/jp8-touch.json"),
    ("se-entry-keypad", "sony-ericsson/entry-keypad.json"),
    ("se-entry-3d-keypad", "sony-ericsson/entry-3d-keypad.json"),
);

/// Borrows the validated, immutable device catalog shared by every frontend.
///
/// # Errors
/// Returns a profile validation diagnostic if an embedded profile is invalid.
pub fn builtin_device_profiles() -> Result<&'static [DeviceProfile], EmuError> {
    if let Some(profiles) = PARSED_CATALOG.get() {
        return Ok(profiles);
    }
    let profiles: Vec<DeviceProfile> = BUILTIN_PROFILES
        .iter()
        .map(|(_, bytes)| DeviceProfile::from_reader(*bytes))
        .collect::<Result<_, _>>()?;
    // Embedded bytes cannot change during the process. All callers can borrow
    // the validated catalog without duplicating its strings and collections.
    // Concurrent first callers can parse the same catalog; only one is retained.
    Ok(PARSED_CATALOG.get_or_init(|| profiles))
}

/// Loads one embedded profile by its stable ID.
///
/// # Errors
/// Returns a profile validation diagnostic if the embedded profile is invalid.
pub fn builtin_device_profile(name: &OsStr) -> Result<Option<DeviceProfile>, EmuError> {
    BUILTIN_PROFILES
        .iter()
        .find_map(|(candidate, bytes)| (name == OsStr::new(candidate)).then_some(*bytes))
        .map(DeviceProfile::from_reader)
        .transpose()
}

pub(crate) fn validate_catalog(
    profiles: &[DeviceProfile],
    require_automatic_routes: bool,
) -> Result<(), EmuError> {
    if profiles.is_empty() {
        return Err(catalog_error("at least one device profile is required"));
    }
    let profiles_by_id = profiles
        .iter()
        .map(|profile| (profile.profile_id(), profile))
        .collect::<HashMap<_, _>>();
    if profiles_by_id.len() != profiles.len() {
        return Err(catalog_error(
            "device profile IDs in the launch catalog must be unique",
        ));
    }
    if require_automatic_routes {
        let mut used_automatic_hosts = HashSet::new();
        for target in profiles {
            let host_id = target.composition().persona().automatic_host();
            let host = profiles_by_id.get(host_id).ok_or_else(|| {
                catalog_error(format!(
                    "target persona {} references missing automatic host {host_id}",
                    target.profile_id()
                ))
            })?;
            if !host.composition().host().automatic() {
                return Err(catalog_error(format!(
                    "target persona {} references manual-only host {host_id}",
                    target.profile_id()
                )));
            }
            if !is_compatible_host(host, target) {
                return Err(catalog_error(format!(
                    "automatic host {host_id} cannot implement target persona {}",
                    target.profile_id()
                )));
            }
            used_automatic_hosts.insert(host_id);
        }
        if let Some(unused) = profiles.iter().find(|profile| {
            profile.composition().host().automatic()
                && !used_automatic_hosts.contains(profile.profile_id())
        }) {
            return Err(catalog_error(format!(
                "automatic host {} is not referenced by any target persona",
                unused.profile_id()
            )));
        }
    }
    let mut models = HashSet::new();
    let mut compact_models = HashSet::new();
    for profile in profiles {
        for hint in profile.device().archive_name_hints() {
            for model in hint.model_names() {
                let model_tokens = separator_tokens(model);
                let normalized = model_tokens.join(" ");
                if !models.insert(normalized) {
                    return Err(catalog_error(format!(
                        "device model {model} is declared more than once in the launch catalog"
                    )));
                }
                if !compact_models.insert(model_tokens.concat()) {
                    return Err(catalog_error(format!(
                        "device model {model} is ambiguous when archive-name separators are removed"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn catalog_error(message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Api, "device-selection-catalog", message)
}
