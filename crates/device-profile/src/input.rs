//! Guest-visible keypad mappings, soft keys and pointer capabilities.

use serde::Deserialize;

use crate::{Confidence, Evidence};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KeyName {
    Up,
    Down,
    Left,
    Right,
    Fire,
    SoftLeft,
    SoftRight,
    Clear,
    Back,
    End,
    GameButtonA,
    GameButtonB,
    Star,
    Pound,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
}

impl KeyName {
    /// Stable profile spelling used by diagnostics and Java `Canvas`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
            Self::Fire => "FIRE",
            Self::SoftLeft => "SOFT_LEFT",
            Self::SoftRight => "SOFT_RIGHT",
            Self::Clear => "CLEAR",
            Self::Back => "BACK",
            Self::End => "END",
            Self::GameButtonA => "GAME_BUTTON_A",
            Self::GameButtonB => "GAME_BUTTON_B",
            Self::Star => "STAR",
            Self::Pound => "POUND",
            Self::Num0 => "NUM0",
            Self::Num1 => "NUM1",
            Self::Num2 => "NUM2",
            Self::Num3 => "NUM3",
            Self::Num4 => "NUM4",
            Self::Num5 => "NUM5",
            Self::Num6 => "NUM6",
            Self::Num7 => "NUM7",
            Self::Num8 => "NUM8",
            Self::Num9 => "NUM9",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GameAction {
    Up,
    Down,
    Left,
    Right,
    Fire,
    GameA,
    GameB,
    GameC,
    GameD,
}

impl GameAction {
    /// MIDP `Canvas` game-action integer.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::Up => 1,
            Self::Left => 2,
            Self::Right => 5,
            Self::Down => 6,
            Self::Fire => 8,
            Self::GameA => 9,
            Self::GameB => 10,
            Self::GameC => 11,
            Self::GameD => 12,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasKey {
    pub(super) name: KeyName,
    pub(super) key_code: i32,
    pub(super) game_action: Option<GameAction>,
    pub(super) confidence: Confidence,
    pub(super) sources: Vec<String>,
}
impl CanvasKey {
    #[must_use]
    pub const fn name(&self) -> KeyName {
        self.name
    }
    #[must_use]
    pub const fn key_code(&self) -> i32 {
        self.key_code
    }
    #[must_use]
    pub const fn game_action(&self) -> Option<GameAction> {
        self.game_action
    }
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }
    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoftKeys {
    pub(super) fullscreen_canvas_available: Evidence<bool>,
    pub(super) canvas_events: Evidence<String>,
    pub(super) command_mapping: Evidence<String>,
}
impl SoftKeys {
    #[must_use]
    pub const fn fullscreen_canvas_available(&self) -> &Evidence<bool> {
        &self.fullscreen_canvas_available
    }
    #[must_use]
    pub const fn canvas_events(&self) -> &Evidence<String> {
        &self.canvas_events
    }
    #[must_use]
    pub const fn command_mapping(&self) -> &Evidence<String> {
        &self.command_mapping
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputProfile {
    pub(super) canvas_keys: Vec<CanvasKey>,
    pub(super) soft_keys: SoftKeys,
    #[serde(default)]
    pub(super) pointer: Option<PointerProfile>,
}
impl InputProfile {
    #[must_use]
    pub fn canvas_keys(&self) -> &[CanvasKey] {
        &self.canvas_keys
    }
    #[must_use]
    pub const fn soft_keys(&self) -> &SoftKeys {
        &self.soft_keys
    }
    /// Returns pointer/touch event capabilities exposed to MIDP Canvas.
    #[must_use]
    pub const fn pointer(&self) -> Option<&PointerProfile> {
        self.pointer.as_ref()
    }
}

/// MIDP pointer capabilities for a device preset.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointerProfile {
    pub(super) events: Evidence<bool>,
    pub(super) motion_events: Evidence<bool>,
}

impl PointerProfile {
    /// Whether `pointerPressed` and `pointerReleased` are available.
    #[must_use]
    pub const fn events(&self) -> &Evidence<bool> {
        &self.events
    }

    /// Whether `pointerDragged` is available.
    #[must_use]
    pub const fn motion_events(&self) -> &Evidence<bool> {
        &self.motion_events
    }
}
