use super::{game_info::GameInfo, library_error};
use crate::{GameSettings, ProfileChoice, ProfileSummary};
use diagnostics::{Category, EmuError};
use serde::{Deserialize, Serialize};

pub(super) const ENTRY_SCHEMA_VERSION: u32 = 1;
pub(super) const MAX_ENTRY_METADATA_BYTES: u64 = 256 * 1024;
pub(super) const MAX_TITLE_BYTES: usize = 512;
const MAX_CLASS_NAME_BYTES: usize = 1024;
const MAX_ICON_NAME_BYTES: usize = 4096;
const MAX_ARCHIVE_NAME_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CachedAutomaticProfile {
    pub(super) catalog_fingerprint: String,
    pub(super) target_profile_id: String,
    pub(super) runtime_profile_id: String,
    pub(super) canvas_width: u32,
    pub(super) canvas_height: u32,
}

impl CachedAutomaticProfile {
    pub(super) fn from_summary(catalog_fingerprint: String, summary: ProfileSummary) -> Self {
        Self {
            catalog_fingerprint,
            target_profile_id: summary.target_profile_id,
            runtime_profile_id: summary.runtime_profile_id,
            canvas_width: summary.canvas_dimensions.0,
            canvas_height: summary.canvas_dimensions.1,
        }
    }
}

/// One independently recoverable library metadata record. Private archive and
/// runtime data ownership are intentionally represented by separate paths.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryEntry {
    pub(super) schema_version: u32,
    pub(super) id: String,
    pub(super) title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) folder_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) game_info: Option<GameInfo>,
    pub(super) midlet_index: u32,
    pub(super) midlet_class: String,
    pub(super) icon_resource: Option<String>,
    pub(super) jar_sha256: String,
    pub(super) archive_leaf_name: Option<String>,
    pub(super) private_jar: String,
    pub(super) private_jad: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) jad_sha256: Option<String>,
    pub(super) settings: GameSettings,
    pub(super) cached_automatic_profile: CachedAutomaticProfile,
}

impl LibraryEntry {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn vendor(&self) -> Option<&str> {
        self.game_info.as_ref()?.vendor.as_deref()
    }

    #[must_use]
    pub fn release_year(&self) -> Option<u16> {
        self.game_info.as_ref()?.release_year
    }

    #[must_use]
    pub const fn midlet_index(&self) -> u32 {
        self.midlet_index
    }

    #[must_use]
    pub fn midlet_class(&self) -> &str {
        &self.midlet_class
    }

    #[must_use]
    pub fn icon_resource(&self) -> Option<&str> {
        self.icon_resource.as_deref()
    }

    #[must_use]
    pub fn jar_sha256(&self) -> &str {
        &self.jar_sha256
    }

    #[must_use]
    pub const fn settings(&self) -> &GameSettings {
        &self.settings
    }

    #[must_use]
    pub fn effective_profile_id(&self, current_catalog_fingerprint: &str) -> Option<&str> {
        match &self.settings.device_profile {
            ProfileChoice::Automatic
                if self.cached_automatic_profile.catalog_fingerprint
                    == current_catalog_fingerprint =>
            {
                Some(&self.cached_automatic_profile.target_profile_id)
            }
            ProfileChoice::Automatic => None,
            ProfileChoice::Manual { profile_id } => Some(profile_id),
        }
    }

    /// Display-only Canvas selected for Automatic. A manual choice or stale
    /// catalog must not reuse the dimensions of a previous automatic decision.
    #[must_use]
    pub fn automatic_canvas_dimensions(
        &self,
        current_catalog_fingerprint: &str,
    ) -> Option<(u32, u32)> {
        (matches!(self.settings.device_profile, ProfileChoice::Automatic)
            && self.cached_automatic_profile.catalog_fingerprint == current_catalog_fingerprint)
            .then_some((
                self.cached_automatic_profile.canvas_width,
                self.cached_automatic_profile.canvas_height,
            ))
    }

    pub(super) fn validate(&self) -> Result<(), EmuError> {
        let profiles = launch::builtin_device_profiles()?;
        self.validate_with_profiles(profiles, &crate::catalog_fingerprint(profiles))
    }

    pub(super) fn validate_with_profiles(
        &self,
        profiles: &[device_profile::DeviceProfile],
        catalog_fingerprint: &str,
    ) -> Result<(), EmuError> {
        if self.schema_version != ENTRY_SCHEMA_VERSION {
            return Err(library_error(
                "library-entry-schema",
                format!(
                    "unsupported library entry schema {}; expected {ENTRY_SCHEMA_VERSION}",
                    self.schema_version
                ),
            ));
        }
        validate_entry_id(&self.id)?;
        bounded_nonempty(&self.title, MAX_TITLE_BYTES, "library-title")?;
        if let Some(info) = &self.game_info {
            info.validate()?;
        }
        bounded_nonempty(
            &self.midlet_class,
            MAX_CLASS_NAME_BYTES,
            "library-midlet-class",
        )?;
        if self.midlet_index == 0 {
            return Err(library_error(
                "library-midlet-index",
                "library MIDlet index must be positive",
            ));
        }
        if self.jar_sha256.len() != 64
            || !self.jar_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(library_error(
                "library-jar-digest",
                "library JAR digest must be 64 hexadecimal characters",
            ));
        }
        if self.id != entry_id(&self.jar_sha256, self.midlet_index) {
            return Err(library_error(
                "library-entry-id",
                "library entry identifier does not match its JAR and MIDlet identity",
            ));
        }
        optional_bounded(
            self.archive_leaf_name.as_deref(),
            MAX_ARCHIVE_NAME_BYTES,
            "library-archive-name",
        )?;
        optional_bounded(
            self.icon_resource.as_deref(),
            MAX_ICON_NAME_BYTES,
            "library-icon-name",
        )?;
        self.validate_cached_profile()?;
        self.validate_private_archive_names()?;
        self.validate_descriptor_digest()?;
        self.validate_current_cached_profile(profiles, catalog_fingerprint)?;
        self.settings.validate(profiles)
    }

    fn validate_cached_profile(&self) -> Result<(), EmuError> {
        if self.cached_automatic_profile.catalog_fingerprint.len() != 64
            || !self
                .cached_automatic_profile
                .catalog_fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(library_error(
                "library-profile-cache",
                "cached profile catalog fingerprint must be hexadecimal",
            ));
        }
        bounded_nonempty(
            &self.cached_automatic_profile.target_profile_id,
            128,
            "library-profile-cache",
        )?;
        bounded_nonempty(
            &self.cached_automatic_profile.runtime_profile_id,
            128,
            "library-profile-cache",
        )?;
        let cached_pixels = self
            .cached_automatic_profile
            .canvas_width
            .checked_mul(self.cached_automatic_profile.canvas_height);
        if cached_pixels.is_none_or(|pixels| pixels == 0 || pixels > 16 * 1024 * 1024) {
            return Err(library_error(
                "library-profile-cache",
                "cached profile Canvas dimensions are invalid",
            ));
        }
        Ok(())
    }

    fn validate_private_archive_names(&self) -> Result<(), EmuError> {
        let canonical_archive = format!("{}.jar", self.id);
        if self.private_jar != canonical_archive {
            return Err(library_error(
                "library-private-jar",
                "library private JAR filename is not canonical",
            ));
        }
        let canonical_descriptor = format!("{}.jad", self.id);
        let descriptor_slot_zero = descriptor_slot_name(&self.id, 0);
        let descriptor_slot_one = descriptor_slot_name(&self.id, 1);
        if self.private_jad.as_deref().is_some_and(|private_jad| {
            private_jad != canonical_descriptor
                && private_jad != descriptor_slot_zero
                && private_jad != descriptor_slot_one
        }) {
            return Err(library_error(
                "library-private-jad",
                "library private JAD filename is not a bounded canonical slot",
            ));
        }
        Ok(())
    }

    fn validate_descriptor_digest(&self) -> Result<(), EmuError> {
        if self.private_jad.is_none() && self.jad_sha256.is_some() {
            return Err(library_error(
                "library-jad-digest",
                "library JAD digest cannot be present without a private JAD filename",
            ));
        }
        let legacy_descriptor = format!("{}.jad", self.id);
        if self
            .private_jad
            .as_deref()
            .is_some_and(|name| name != legacy_descriptor.as_str())
            && self.jad_sha256.is_none()
        {
            return Err(library_error(
                "library-jad-digest",
                "slotted private JAD metadata must include its integrity digest",
            ));
        }
        if let Some(digest) = &self.jad_sha256
            && (digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(library_error(
                "library-jad-digest",
                "library JAD digest must be 64 hexadecimal characters",
            ));
        }
        Ok(())
    }

    fn validate_current_cached_profile(
        &self,
        profiles: &[device_profile::DeviceProfile],
        catalog_fingerprint: &str,
    ) -> Result<(), EmuError> {
        if self.cached_automatic_profile.catalog_fingerprint == catalog_fingerprint
            && [
                &self.cached_automatic_profile.target_profile_id,
                &self.cached_automatic_profile.runtime_profile_id,
            ]
            .into_iter()
            .any(|cached_id| {
                !profiles
                    .iter()
                    .any(|profile| profile.profile_id() == cached_id)
            })
        {
            return Err(library_error(
                "library-profile-cache",
                "current cached profile identifier is not in the built-in catalog",
            ));
        }
        Ok(())
    }
    pub(super) fn encode(&self) -> Result<Vec<u8>, EmuError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            EmuError::with_source(
                Category::Platform,
                "library-entry-json",
                "cannot serialize library entry",
                error,
            )
        })?;
        if bytes.len() as u64 > MAX_ENTRY_METADATA_BYTES {
            return Err(library_error(
                "library-entry-too-large",
                "serialized library entry exceeds 256 KiB",
            ));
        }
        Ok(bytes)
    }
}

pub(super) fn entry_id(sha256: &str, midlet_index: u32) -> String {
    format!("{sha256}-{midlet_index}")
}

pub(super) fn descriptor_slot_name(entry_id: &str, slot: usize) -> String {
    debug_assert!(slot < 2);
    format!("{entry_id}.{slot}.jad")
}

pub(super) fn validate_entry_id(id: &str) -> Result<(), EmuError> {
    let valid = id.len() <= 80
        && !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(library_error(
            "library-entry-id",
            "library entry identifier is not canonical",
        ))
    }
}

fn bounded_nonempty(value: &str, maximum: usize, code: &'static str) -> Result<(), EmuError> {
    if !value.is_empty() && value.len() <= maximum {
        Ok(())
    } else {
        Err(library_error(
            code,
            "library metadata string is empty or oversized",
        ))
    }
}

fn optional_bounded(
    value: Option<&str>,
    maximum: usize,
    code: &'static str,
) -> Result<(), EmuError> {
    value.map_or(Ok(()), |value| bounded_nonempty(value, maximum, code))
}
