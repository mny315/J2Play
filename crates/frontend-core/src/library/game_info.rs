use super::storage::read_bounded_file_cancellable;
use super::{EmuError, LibraryEntry, LibraryRepository, MAX_PRIVATE_JAR_BYTES, library_error};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_VENDOR_BYTES: usize = 256;
const YEAR_RANGE: std::ops::RangeInclusive<u16> = 1900..=2099;

/// Optional display data from explicit suite properties. ZIP/build timestamps,
/// archive names and copyright ranges cannot establish a game's release year.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GameInfo {
    pub(super) vendor: Option<String>,
    pub(super) release_year: Option<u16>,
}

/// Validated display metadata tied to the exact entry and descriptor it came from.
/// Background readers return this value; the UI applies it to the current entry.
#[derive(Debug)]
pub struct RecoveredGameInfo {
    entry_id: String,
    jad_sha256: Option<String>,
    info: GameInfo,
}

impl GameInfo {
    pub(super) fn from_properties(properties: &BTreeMap<String, String>) -> Self {
        let vendor = properties
            .get("midlet-vendor")
            .and_then(|value| vendor_text(value));
        let release_year = ["midlet-release-year", "midlet-year", "release-year"]
            .into_iter()
            .find_map(|key| properties.get(key))
            .and_then(|value| {
                let value = value.trim();
                (value.len() == 4 && value.bytes().all(|byte| byte.is_ascii_digit()))
                    .then(|| value.parse::<u16>().ok())
                    .flatten()
                    .filter(|year| YEAR_RANGE.contains(year))
            });
        Self {
            vendor,
            release_year,
        }
    }

    pub(super) fn validate(&self) -> Result<(), EmuError> {
        if self.vendor.as_ref().is_some_and(|vendor| {
            vendor.is_empty()
                || vendor.len() > MAX_VENDOR_BYTES
                || vendor.chars().any(char::is_control)
        }) || self
            .release_year
            .is_some_and(|year| !YEAR_RANGE.contains(&year))
        {
            return Err(library_error(
                "library-game-info",
                "game information exceeds its text or year bounds",
            ));
        }
        Ok(())
    }
}

fn vendor_text(value: &str) -> Option<String> {
    let mut text = String::new();
    let mut space = false;
    for character in value.chars() {
        if character.is_whitespace() {
            space = !text.is_empty();
        } else if !character.is_control() {
            if text.len() + usize::from(space) + character.len_utf8() > MAX_VENDOR_BYTES {
                break;
            }
            if space {
                text.push(' ');
            }
            text.push(character);
            space = false;
        }
    }
    (!text.is_empty()).then_some(text)
}

impl LibraryRepository {
    /// Reads missing legacy display information without modifying library records.
    /// Run on a background worker for visible entries. Already cached information
    /// needs no filesystem access and returns `None`.
    ///
    /// # Errors
    /// Returns a controlled archive, integrity or storage diagnostic. The entry
    /// remains usable when its optional information cannot be recovered.
    pub fn read_game_info(
        &self,
        entry: &LibraryEntry,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Option<RecoveredGameInfo>, EmuError> {
        if entry.game_info.is_some() {
            return Ok(None);
        }
        entry.validate()?;
        let bytes = read_bounded_file_cancellable(
            &self.private_jar_path(entry),
            MAX_PRIVATE_JAR_BYTES,
            "library-game-info-read",
            &mut cancelled,
        )?;
        let jar = jar::inspect_bytes(&bytes)?;
        drop(bytes);
        if cancelled() {
            return Err(library_error(
                "library-game-info-read",
                "Metadata reading was interrupted",
            ));
        }
        if jar.sha256 != entry.jar_sha256 {
            return Err(library_error(
                "library-jar-integrity",
                "private JAR digest no longer matches library metadata",
            ));
        }
        let jad = self
            .read_private_jad(entry)?
            .as_deref()
            .map(jar::parse_jad)
            .transpose()?;
        let suite = midp::describe_suite(&jar, jad.as_ref(), Some(entry.midlet_index))?;
        Ok(Some(RecoveredGameInfo {
            entry_id: entry.id.clone(),
            jad_sha256: entry.jad_sha256.clone(),
            info: GameInfo::from_properties(&suite.properties),
        }))
    }

    /// Saves recovered information into the current stored entry, preserving newer metadata.
    /// Returns `false` for a stale result or an entry whose information is already cached.
    ///
    /// # Errors
    /// Returns a controlled storage diagnostic; failure leaves the entry unchanged.
    pub fn cache_game_info(
        &self,
        entry: &mut LibraryEntry,
        recovered: RecoveredGameInfo,
    ) -> Result<bool, EmuError> {
        if entry.game_info.is_some()
            || entry.id != recovered.entry_id
            || entry.jad_sha256 != recovered.jad_sha256
        {
            return Ok(false);
        }
        let mut updated = self.load_entry(entry.id())?;
        if updated.game_info.is_some() || updated.jad_sha256 != recovered.jad_sha256 {
            *entry = updated;
            return Ok(false);
        }
        updated.game_info = Some(recovered.info);
        self.write_entry(&updated)?;
        *entry = updated;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-core/library/game_info.rs"]
mod tests;
