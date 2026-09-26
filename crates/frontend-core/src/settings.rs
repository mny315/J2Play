use device_profile::DeviceProfile;
use diagnostics::{Category, EmuError};
use serde::{Deserialize, Serialize};

pub const SETTINGS_SCHEMA_VERSION: u32 = 12;
pub const MAX_PORTRAIT_FRAME_PERCENT: u8 = 115;
pub const MIN_SCALE_PERCENT: u16 = 25;
pub const MAX_SCALE_PERCENT: u16 = 800;
pub const MIN_FPS_LIMIT: u32 = natives::FrameRateControl::MIN_FRAMES_PER_SECOND;
pub const MAX_FPS_LIMIT: u32 = natives::FrameRateControl::MAX_FRAMES_PER_SECOND;
pub const CONTROL_POSITION_UNITS: i16 = 10_000;
pub const MIN_CONTROL_SIZE_PERCENT: u16 = 50;
pub const MAX_CONTROL_SIZE_PERCENT: u16 = 200;
pub const MIN_CONTROL_CORNER_RADIUS_PERCENT: u8 = 0;
pub const MAX_CONTROL_CORNER_RADIUS_PERCENT: u8 = 100;
pub const MIN_VIBRATION_STRENGTH_PERCENT: u8 = 1;
pub const MAX_VIBRATION_STRENGTH_PERCENT: u8 = 100;

/// Per-library-entry device selection. Automatic selection remains authoritative
/// at every launch; a manual choice names one exact built-in profile contract.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProfileChoice {
    #[default]
    Automatic,
    Manual {
        profile_id: String,
    },
}

/// Host-only presentation scale. Percent is fixed-point to keep persistence
/// deterministic and never changes the guest-visible Canvas.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum GameScale {
    #[default]
    AutomaticFit,
    Manual {
        percent: u16,
    },
}

/// Frontend frame-rate policy layered over the automatic runtime ceiling.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum FpsLimit {
    #[default]
    Automatic,
    Manual {
        frames_per_second: u32,
    },
}

/// Host fullscreen at launch, optionally following the host screen orientation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FullscreenMode {
    #[default]
    Off,
    On,
    LandscapeOnly,
}

/// Per-game policy for continuing an automatic checkpoint at launch.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeBehavior {
    #[default]
    Ask,
    Always,
    Never,
}

/// One host-only adjustment relative to the responsive default keypad layout.
///
/// Positions use signed 1/10,000ths of the current controls area, so an
/// unchanged control continues to follow the existing adaptive layout across
/// rotations and window sizes. Size is a percentage of that control's default.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ControlTransform {
    pub visible: bool,
    pub offset_x: i16,
    pub offset_y: i16,
    pub size_percent: u16,
}

impl Default for ControlTransform {
    fn default() -> Self {
        Self {
            visible: true,
            offset_x: 0,
            offset_y: 0,
            size_percent: 100,
        }
    }
}

/// Per-game virtual keypad layout. The direction pad moves and scales as one
/// coherent control; every other visible key remains independently adjustable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VirtualControlLayout {
    /// Whether keypad controls reserve and occupy gameplay space.
    pub visible: bool,
    /// Replace the direction pad with a fixed touch stick using the same keys.
    pub stick_enabled: bool,
    /// Touch direction diagonals hold two numeric keys instead of 1/3/7/9.
    pub two_key_diagonals: bool,
    /// Percentage of the original control corner radius. Zero is rectangular.
    pub corner_radius_percent: u8,
    pub direction_pad: ControlTransform,
    pub fire: ControlTransform,
    pub left_soft_key: ControlTransform,
    pub right_soft_key: ControlTransform,
    pub number_keys: [ControlTransform; 12],
}

impl Default for VirtualControlLayout {
    fn default() -> Self {
        Self {
            visible: true,
            stick_enabled: false,
            two_key_diagonals: false,
            corner_radius_percent: MAX_CONTROL_CORNER_RADIUS_PERCENT,
            direction_pad: ControlTransform::default(),
            fire: ControlTransform::default(),
            left_soft_key: ControlTransform::default(),
            right_soft_key: ControlTransform::default(),
            number_keys: [ControlTransform::default(); 12],
        }
    }
}

impl VirtualControlLayout {
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub(crate) fn validate(&self) -> Result<(), EmuError> {
        if !(MIN_CONTROL_CORNER_RADIUS_PERCENT..=MAX_CONTROL_CORNER_RADIUS_PERCENT)
            .contains(&self.corner_radius_percent)
        {
            return Err(settings_error(
                "settings-control-corner-radius",
                format!(
                    "control corner radius must be between {MIN_CONTROL_CORNER_RADIUS_PERCENT}% and {MAX_CONTROL_CORNER_RADIUS_PERCENT}%"
                ),
            ));
        }
        validate_control_transform("direction pad", self.direction_pad)?;
        validate_control_transform("fire key", self.fire)?;
        validate_control_transform("left soft key", self.left_soft_key)?;
        validate_control_transform("right soft key", self.right_soft_key)?;
        for (index, transform) in self.number_keys.iter().copied().enumerate() {
            validate_control_transform(format_args!("number key {index}"), transform)?;
        }
        Ok(())
    }
}

/// Per-game policy for both touch-control feedback and guest-requested
/// vibration. One percentage scales a guest-provided intensity when present.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VibrationSettings {
    pub enabled: bool,
    pub strength_percent: u8,
}

impl Default for VibrationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            strength_percent: MAX_VIBRATION_STRENGTH_PERCENT,
        }
    }
}

impl VibrationSettings {
    pub(crate) fn validate(self) -> Result<(), EmuError> {
        if !(MIN_VIBRATION_STRENGTH_PERCENT..=MAX_VIBRATION_STRENGTH_PERCENT)
            .contains(&self.strength_percent)
        {
            return Err(settings_error(
                "settings-vibration-strength",
                format!(
                    "vibration strength must be between {MIN_VIBRATION_STRENGTH_PERCENT}% and {MAX_VIBRATION_STRENGTH_PERCENT}%"
                ),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub(crate) fn apply_to(self, request: natives::VibrationRequest) -> natives::VibrationRequest {
        use natives::VibrationRequest;

        if request == VibrationRequest::Stop || !self.enabled {
            return VibrationRequest::Stop;
        }
        if self.strength_percent == MAX_VIBRATION_STRENGTH_PERCENT {
            return request;
        }
        let scaled_level = |level: Option<u8>| {
            let requested = u16::from(level.unwrap_or(MAX_VIBRATION_STRENGTH_PERCENT).clamp(
                MIN_VIBRATION_STRENGTH_PERCENT,
                MAX_VIBRATION_STRENGTH_PERCENT,
            ));
            let strength = u16::from(self.strength_percent);
            let scaled = (requested * strength).div_ceil(100);
            Some(
                u8::try_from(scaled)
                    .unwrap_or(MAX_VIBRATION_STRENGTH_PERCENT)
                    .clamp(
                        MIN_VIBRATION_STRENGTH_PERCENT,
                        MAX_VIBRATION_STRENGTH_PERCENT,
                    ),
            )
        };
        match request {
            VibrationRequest::Continuous { level } => VibrationRequest::Continuous {
                level: scaled_level(level),
            },
            VibrationRequest::Timed {
                duration_millis,
                level,
            } => VibrationRequest::Timed {
                duration_millis,
                level: scaled_level(level),
            },
            VibrationRequest::Stop => VibrationRequest::Stop,
        }
    }
}

/// Versioned settings persisted independently for each library entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameSettings {
    pub schema_version: u32,
    #[serde(default)]
    pub resume_behavior: ResumeBehavior,
    pub device_profile: ProfileChoice,
    pub game_scale: GameScale,
    /// Portrait keypad gameplay only: percentage of the reference fitted frame.
    #[serde(default)]
    pub portrait_frame_percent: Option<u8>,
    pub fps_limit: FpsLimit,
    /// None inherits the app-wide fullscreen mode.
    #[serde(default)]
    pub fullscreen: Option<FullscreenMode>,
    /// None inherits the global physical bindings, independently of touch controls.
    #[serde(default)]
    pub physical_bindings: Option<crate::physical_input::PhysicalBindings>,
    /// Explicit per-game controls (including feedback) override app defaults.
    #[serde(default)]
    pub controls_override: bool,
    #[serde(default)]
    pub control_layout: VirtualControlLayout,
    #[serde(default)]
    pub landscape_control_layout: VirtualControlLayout,
    #[serde(default)]
    pub vibration: VibrationSettings,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            resume_behavior: ResumeBehavior::Ask,
            device_profile: ProfileChoice::Automatic,
            game_scale: GameScale::AutomaticFit,
            portrait_frame_percent: None,
            fps_limit: FpsLimit::Automatic,
            fullscreen: None,
            physical_bindings: None,
            controls_override: false,
            control_layout: VirtualControlLayout::default(),
            landscape_control_layout: VirtualControlLayout::default(),
            vibration: VibrationSettings::default(),
        }
    }
}

impl GameSettings {
    /// Validates the schema, bounds, and any exact built-in profile reference.
    ///
    /// # Errors
    /// Returns a stable frontend diagnostic for unsupported or invalid values.
    pub fn validate(&self, profiles: &[DeviceProfile]) -> Result<(), EmuError> {
        if self.schema_version != SETTINGS_SCHEMA_VERSION {
            return Err(settings_error(
                "settings-schema",
                format!(
                    "unsupported game settings schema {}; expected {SETTINGS_SCHEMA_VERSION}",
                    self.schema_version
                ),
            ));
        }
        if let ProfileChoice::Manual { profile_id } = &self.device_profile
            && !profiles
                .iter()
                .any(|profile| profile.profile_id() == profile_id)
        {
            return Err(settings_error(
                "settings-profile",
                format!("unknown built-in device profile {profile_id}"),
            ));
        }
        if let GameScale::Manual { percent } = self.game_scale
            && !(MIN_SCALE_PERCENT..=MAX_SCALE_PERCENT).contains(&percent)
        {
            return Err(settings_error(
                "settings-scale",
                format!(
                    "manual game scale must be between {MIN_SCALE_PERCENT}% and {MAX_SCALE_PERCENT}%"
                ),
            ));
        }
        if self
            .portrait_frame_percent
            .is_some_and(|percent| !(25..=MAX_PORTRAIT_FRAME_PERCENT).contains(&percent))
        {
            return Err(settings_error(
                "settings-portrait-frame",
                "portrait frame must be between 25% and 115%",
            ));
        }
        if let FpsLimit::Manual { frames_per_second } = self.fps_limit
            && !(MIN_FPS_LIMIT..=MAX_FPS_LIMIT).contains(&frames_per_second)
        {
            return Err(settings_error(
                "settings-fps",
                format!("manual FPS limit must be between {MIN_FPS_LIMIT} and {MAX_FPS_LIMIT}"),
            ));
        }
        self.control_layout.validate()?;
        self.landscape_control_layout.validate()?;
        if let Some(bindings) = &self.physical_bindings {
            bindings.validate()?;
        }
        self.vibration.validate()
    }

    /// Upgrades previous on-disk settings schemas. New fields have serde
    /// defaults which exactly preserve their controls and vibration policy.
    pub(crate) fn migrate_to_current(&mut self) -> Result<bool, EmuError> {
        let physical_changed = self
            .physical_bindings
            .as_mut()
            .is_some_and(crate::physical_input::PhysicalBindings::normalize_dpad_bindings);
        match self.schema_version {
            SETTINGS_SCHEMA_VERSION => Ok(physical_changed),
            8..=11 => {
                self.schema_version = SETTINGS_SCHEMA_VERSION;
                Ok(true)
            }
            1..=4 => {
                // Earlier versions used the portrait adjustments in both orientations.
                self.landscape_control_layout = self.control_layout.clone();
                self.migrate_controls_override();
                self.schema_version = SETTINGS_SCHEMA_VERSION;
                Ok(true)
            }
            5..=7 => {
                self.migrate_controls_override();
                self.schema_version = SETTINGS_SCHEMA_VERSION;
                Ok(true)
            }
            version => Err(settings_error(
                "settings-schema",
                format!(
                    "unsupported game settings schema {version}; expected {SETTINGS_SCHEMA_VERSION}"
                ),
            )),
        }
    }

    fn migrate_controls_override(&mut self) {
        self.controls_override = !self.control_layout.is_default()
            || !self.landscape_control_layout.is_default()
            || self.vibration != VibrationSettings::default();
    }

    #[must_use]
    pub const fn manual_fps_limit(&self) -> Option<u32> {
        match self.fps_limit {
            FpsLimit::Automatic => None,
            FpsLimit::Manual { frames_per_second } => Some(frames_per_second),
        }
    }
}

fn validate_control_transform(
    label: impl std::fmt::Display,
    transform: ControlTransform,
) -> Result<(), EmuError> {
    if !(-CONTROL_POSITION_UNITS..=CONTROL_POSITION_UNITS).contains(&transform.offset_x)
        || !(-CONTROL_POSITION_UNITS..=CONTROL_POSITION_UNITS).contains(&transform.offset_y)
    {
        return Err(settings_error(
            "settings-control-position",
            format!("{label} position is outside the bounded controls area"),
        ));
    }
    if !(MIN_CONTROL_SIZE_PERCENT..=MAX_CONTROL_SIZE_PERCENT).contains(&transform.size_percent) {
        return Err(settings_error(
            "settings-control-size",
            format!(
                "{label} size must be between {MIN_CONTROL_SIZE_PERCENT}% and {MAX_CONTROL_SIZE_PERCENT}%"
            ),
        ));
    }
    Ok(())
}

fn settings_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Platform, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-core/settings/mod.rs"]
mod tests;
