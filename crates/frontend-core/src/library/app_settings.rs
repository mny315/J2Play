use super::storage::{atomic_write, read_bounded_file};
use super::{LibraryRepository, io_error, library_error};
use crate::{
    FpsLimit, FullscreenMode, GameSettings, MAX_FPS_LIMIT, MIN_FPS_LIMIT, VibrationSettings,
    VirtualControlLayout,
};
use diagnostics::EmuError;
use serde::{Deserialize, Serialize};
use std::fs;

pub const MIN_UI_SCALE_PERCENT: u16 = 50;
pub const MAX_UI_SCALE_PERCENT: u16 = 150;
const MAX_APP_SETTINGS_BYTES: u64 = 32768;

/// Preferred library presentation; automatic adapts to the frontend's width.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryView {
    #[default]
    Automatic,
    List,
    Tiles,
}

/// App appearance, independent of the host's current light/dark preference.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppTheme {
    #[default]
    System,
    Light,
    Dark,
    Oled,
}

/// System colors when available, or a built-in accent palette.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccentColor {
    #[default]
    System,
    Blue,
    Teal,
    Green,
    Amber,
    Orange,
    Red,
    Pink,
    Purple,
}

/// App-wide defaults, separate from every game's saved overrides.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppSettings {
    pub schema_version: u32,
    pub fps_limit: FpsLimit,
    pub ui_scale_percent: u16,
    #[serde(default)]
    pub language: Option<crate::Language>,
    #[serde(default)]
    pub theme: AppTheme,
    #[serde(default)]
    pub accent_color: AccentColor,
    #[serde(default)]
    pub library_view: LibraryView,
    #[serde(default)]
    pub fullscreen: FullscreenMode,
    #[serde(default)]
    pub physical_bindings: crate::physical_input::PhysicalBindings,
    #[serde(default)]
    pub control_layout: VirtualControlLayout,
    #[serde(default)]
    pub landscape_control_layout: VirtualControlLayout,
    #[serde(default)]
    pub vibration: VibrationSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 12,
            fps_limit: FpsLimit::Automatic,
            ui_scale_percent: 100,
            language: None,
            theme: AppTheme::System,
            accent_color: AccentColor::System,
            library_view: LibraryView::Automatic,
            fullscreen: FullscreenMode::Off,
            physical_bindings: crate::physical_input::PhysicalBindings::default(),
            control_layout: VirtualControlLayout::default(),
            landscape_control_layout: VirtualControlLayout::default(),
            vibration: VibrationSettings::default(),
        }
    }
}

impl AppSettings {
    /// # Errors
    /// Rejects unsupported schemas and values outside the UI/runtime bounds.
    pub fn validate(&self) -> Result<(), EmuError> {
        if self.schema_version != 12 {
            return Err(library_error(
                "app-settings-schema",
                "Unsupported app settings version",
            ));
        }
        if !(MIN_UI_SCALE_PERCENT..=MAX_UI_SCALE_PERCENT).contains(&self.ui_scale_percent) {
            return Err(library_error(
                "app-settings-scale",
                "Interface scale is outside the supported range",
            ));
        }
        if let FpsLimit::Manual { frames_per_second } = self.fps_limit
            && !(MIN_FPS_LIMIT..=MAX_FPS_LIMIT).contains(&frames_per_second)
        {
            return Err(library_error(
                "app-settings-fps",
                "FPS limit is outside the supported range",
            ));
        }
        self.physical_bindings.validate()?;
        self.control_layout.validate()?;
        self.landscape_control_layout.validate()?;
        self.vibration.validate()
    }

    /// Editor draft for the app-wide controls, with the original presentation defaults.
    #[must_use]
    pub fn control_defaults(&self) -> GameSettings {
        GameSettings {
            control_layout: self.control_layout.clone(),
            landscape_control_layout: self.landscape_control_layout.clone(),
            vibration: self.vibration,
            ..GameSettings::default()
        }
    }

    /// Resolve host controls without changing the saved entry or guest profile.
    pub fn apply_control_defaults(&self, game: &mut GameSettings) {
        if !game.controls_override {
            game.control_layout = self.control_layout.clone();
            game.landscape_control_layout = self.landscape_control_layout.clone();
            game.vibration = self.vibration;
        }
    }

    /// A game's explicit fullscreen mode takes priority, including Off.
    #[must_use]
    pub fn effective_fullscreen(&self, game_mode: Option<FullscreenMode>) -> FullscreenMode {
        game_mode.unwrap_or(self.fullscreen)
    }

    /// Explicit game overrides win; automatic games inherit the app default.
    #[must_use]
    pub const fn effective_manual_fps_limit(&self, game_limit: FpsLimit) -> Option<u32> {
        match game_limit {
            FpsLimit::Manual { frames_per_second } => Some(frames_per_second),
            FpsLimit::Automatic => match self.fps_limit {
                FpsLimit::Automatic => None,
                FpsLimit::Manual { frames_per_second } => Some(frames_per_second),
            },
        }
    }
}

impl LibraryRepository {
    /// Sets the shell's first-run keypad visibility without changing saved settings.
    #[must_use]
    pub fn with_default_virtual_controls_visible(mut self, visible: bool) -> Self {
        self.default_virtual_controls_visible = visible;
        self
    }

    /// Defaults for this shell when app settings are absent or cannot be loaded.
    #[must_use]
    pub fn default_app_settings(&self) -> AppSettings {
        let layout = VirtualControlLayout {
            visible: self.default_virtual_controls_visible,
            ..VirtualControlLayout::default()
        };
        AppSettings {
            control_layout: layout.clone(),
            landscape_control_layout: layout,
            ..AppSettings::default()
        }
    }

    /// Missing settings use explicit first-run defaults.
    ///
    /// # Errors
    /// Reports malformed, oversized, unsupported, or unreadable settings.
    pub fn load_app_settings(&self) -> Result<AppSettings, EmuError> {
        let path = self.root.join("frontend-state").join("app-settings.json");
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => {}
            Ok(_) => {
                return Err(library_error(
                    "app-settings-read",
                    "App settings are not a regular file",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(self.default_app_settings());
            }
            Err(error) => {
                return Err(io_error(
                    "app-settings-read",
                    "Cannot read app settings",
                    error,
                ));
            }
        }
        let bytes = read_bounded_file(&path, MAX_APP_SETTINGS_BYTES, "app-settings-read")?;
        let mut settings: AppSettings = serde_json::from_slice(&bytes)
            .map_err(|_| library_error("app-settings-json", "App settings contain invalid JSON"))?;
        settings.physical_bindings.normalize_dpad_bindings();
        if matches!(settings.schema_version, 1..=8) {
            settings
                .physical_bindings
                .normalize_legacy_defaults(settings.schema_version);
            settings.schema_version = 9;
        }
        if matches!(settings.schema_version, 9..=10) {
            settings.schema_version = 11;
        }
        if settings.schema_version == 11 {
            settings
                .physical_bindings
                .normalize_legacy_keyboard_defaults();
            settings.schema_version = 12;
        }
        settings.validate()?;
        Ok(settings)
    }

    /// # Errors
    /// Reports invalid settings or an unsuccessful atomic write.
    pub fn save_app_settings(&self, settings: &AppSettings) -> Result<(), EmuError> {
        settings.validate()?;
        let bytes = serde_json::to_vec(settings)
            .map_err(|_| library_error("app-settings-json", "Cannot encode app settings"))?;
        if bytes.len() as u64 > MAX_APP_SETTINGS_BYTES {
            return Err(library_error(
                "app-settings-size",
                "App settings exceed the size limit",
            ));
        }
        atomic_write(
            &self.root.join("frontend-state").join("app-settings.json"),
            &bytes,
        )
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-core/library/app_settings/mod.rs"]
mod tests;
