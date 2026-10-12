use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostAction {
    Up,
    Down,
    Left,
    Right,
    Fire,
    SoftLeft,
    SoftRight,
    Back,
    Clear,
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
    Star,
    Pound,
    GameA,
    GameB,
    Menu,
    Quit,
    Screenshot,
    FastForward,
    DebugOverlay,
}

impl HostAction {
    #[must_use]
    pub const fn is_emulator_action(self) -> bool {
        matches!(
            self,
            Self::Menu | Self::Quit | Self::Screenshot | Self::FastForward | Self::DebugOverlay
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyKind {
    Pressed,
    Released,
    Repeated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InputEvent {
    pub tick: u64,
    pub kind: KeyKind,
    pub action: HostAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointerKind {
    PointerPressed,
    PointerReleased,
    PointerDragged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PointerEvent {
    pub tick: u64,
    pub kind: PointerKind,
    pub x: i32,
    pub y: i32,
}

/// Separates guest-visible handset capabilities from normalized host pointer
/// delivery. A desktop mouse or frontend touch event is explicit user input,
/// so passthrough is enabled by default without changing what MIDP reports for
/// the emulated device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerInputPolicy {
    device_events: bool,
    device_motion_events: bool,
    host_passthrough: bool,
}

impl PointerInputPolicy {
    #[must_use]
    pub const fn new(device_events: bool, device_motion_events: bool) -> Self {
        Self {
            device_events,
            device_motion_events,
            host_passthrough: true,
        }
    }

    /// Returns the value exposed by `Canvas.hasPointerEvents()`.
    #[must_use]
    pub const fn device_events(self) -> bool {
        self.device_events
    }

    /// Returns the value exposed by `Canvas.hasPointerMotionEvents()`.
    #[must_use]
    pub const fn device_motion_events(self) -> bool {
        self.device_motion_events
    }

    /// Selects strict device-only delivery for conformance frontends or host
    /// passthrough for interactive frontends.
    #[must_use]
    pub const fn with_host_passthrough(mut self, enabled: bool) -> Self {
        self.host_passthrough = enabled;
        self
    }

    /// Returns whether one normalized frontend pointer event is delivered.
    #[must_use]
    pub const fn allows(self, kind: PointerKind) -> bool {
        if self.host_passthrough {
            return true;
        }
        match kind {
            PointerKind::PointerPressed | PointerKind::PointerReleased => self.device_events,
            PointerKind::PointerDragged => self.device_events && self.device_motion_events,
        }
    }
}

impl Default for PointerInputPolicy {
    fn default() -> Self {
        Self::new(false, false)
    }
}

/// A frontend text-edit operation, kept separate from phone key codes so
/// committed desktop/Android text is delivered only to an active MIDP editor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextInputKind {
    Commit,
    DeleteBackward,
    DeleteForward,
}

/// One UTF-16 code unit or editing command committed by a host text service.
/// Android and Linux text services produce the same backend-neutral event.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextInputEvent {
    pub tick: u64,
    pub kind: TextInputKind,
    pub code_unit: u16,
}

/// Converts one frontend/IME commit into Java UTF-16 events.
pub fn committed_text_events(tick: u64, text: &str) -> impl Iterator<Item = TextInputEvent> + '_ {
    text.encode_utf16().map(move |code_unit| TextInputEvent {
        tick,
        kind: TextInputKind::Commit,
        code_unit,
    })
}

pub struct MidpKeyEvent {
    pub kind: KeyKind,
    /// Raw device code exposed to application `Canvas` callbacks.
    pub key_code: i32,
    /// Stable emulator-internal code used only by the built-in LCDUI widgets.
    ///
    /// This must never replace `key_code` in application callbacks: profiles
    /// intentionally expose different raw codes for the same physical action.
    pub lcdui_key_code: i32,
    pub game_action: Option<i32>,
    pub state: u32,
}

pub const GAME_UP: i32 = 1;
pub const GAME_LEFT: i32 = 2;
pub const GAME_RIGHT: i32 = 5;
pub const GAME_DOWN: i32 = 6;
pub const GAME_FIRE: i32 = 8;
pub const GAME_A: i32 = 9;
pub const GAME_B: i32 = 10;
pub const GAME_C: i32 = 11;
pub const GAME_D: i32 = 12;

#[must_use]
pub const fn lcdui_key_code(action: HostAction) -> Option<i32> {
    Some(match action {
        HostAction::Up => -1,
        HostAction::Down => -2,
        HostAction::Left => -3,
        HostAction::Right => -4,
        HostAction::Fire => -5,
        HostAction::SoftLeft => -6,
        HostAction::SoftRight => -7,
        HostAction::Clear => -8,
        HostAction::Back => -11,
        HostAction::GameA => -13,
        HostAction::GameB => -14,
        HostAction::Star => 42,
        HostAction::Pound => 35,
        HostAction::Num0 => 48,
        HostAction::Num1 => 49,
        HostAction::Num2 => 50,
        HostAction::Num3 => 51,
        HostAction::Num4 => 52,
        HostAction::Num5 => 53,
        HostAction::Num6 => 54,
        HostAction::Num7 => 55,
        HostAction::Num8 => 56,
        HostAction::Num9 => 57,
        HostAction::Menu
        | HostAction::Quit
        | HostAction::Screenshot
        | HostAction::FastForward
        | HostAction::DebugOverlay => {
            return None;
        }
    })
}

#[must_use]
pub const fn game_action(action: HostAction) -> Option<i32> {
    match action {
        HostAction::Up | HostAction::Num2 => Some(GAME_UP),
        HostAction::Down | HostAction::Num8 => Some(GAME_DOWN),
        HostAction::Left | HostAction::Num4 => Some(GAME_LEFT),
        HostAction::Right | HostAction::Num6 => Some(GAME_RIGHT),
        HostAction::Fire | HostAction::Num5 => Some(GAME_FIRE),
        HostAction::GameA | HostAction::Num7 => Some(GAME_A),
        HostAction::GameB | HostAction::Num9 => Some(GAME_B),
        HostAction::Star => Some(GAME_C),
        HostAction::Pound => Some(GAME_D),
        _ => None,
    }
}

pub(crate) const fn state_bit(game_action: Option<i32>) -> u32 {
    match game_action {
        Some(value) if value > 0 && value < 32 => 1 << value,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceKey {
    pub key_code: i32,
    pub game_action: Option<i32>,
}

/// Device-side mapping applied after host inputs have been normalized to actions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceInputMap(BTreeMap<HostAction, DeviceKey>);

impl DeviceInputMap {
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = (HostAction, DeviceKey)>) -> Self {
        Self(entries.into_iter().collect())
    }

    #[must_use]
    pub fn key(&self, action: HostAction) -> Option<DeviceKey> {
        self.0.get(&action).copied()
    }
}

impl Default for DeviceInputMap {
    fn default() -> Self {
        let entries = [
            HostAction::Up,
            HostAction::Down,
            HostAction::Left,
            HostAction::Right,
            HostAction::Fire,
            HostAction::SoftLeft,
            HostAction::SoftRight,
            HostAction::Clear,
            HostAction::Back,
            HostAction::GameA,
            HostAction::GameB,
            HostAction::Star,
            HostAction::Pound,
            HostAction::Num0,
            HostAction::Num1,
            HostAction::Num2,
            HostAction::Num3,
            HostAction::Num4,
            HostAction::Num5,
            HostAction::Num6,
            HostAction::Num7,
            HostAction::Num8,
            HostAction::Num9,
        ]
        .into_iter()
        .filter_map(|action| {
            Some((
                action,
                DeviceKey {
                    key_code: lcdui_key_code(action)?,
                    game_action: game_action(action),
                },
            ))
        });
        Self::new(entries)
    }
}

#[derive(Clone)]
pub struct InputState {
    mapping: DeviceInputMap,
    pressed: BTreeMap<HostAction, usize>,
    state: u32,
}

impl Default for InputState {
    fn default() -> Self {
        Self::new(DeviceInputMap::default())
    }
}

impl InputState {
    #[must_use]
    pub fn new(mapping: DeviceInputMap) -> Self {
        Self {
            mapping,
            pressed: BTreeMap::new(),
            state: 0,
        }
    }
    #[must_use]
    pub const fn game_state(&self) -> u32 {
        self.state
    }
    pub fn apply(&mut self, event: InputEvent) -> Option<MidpKeyEvent> {
        let device_key = self.mapping.key(event.action)?;
        let lcdui_key_code = lcdui_key_code(event.action)?;
        match event.kind {
            KeyKind::Pressed => {
                let count = self.pressed.entry(event.action).or_default();
                *count = count.saturating_add(1);
                if *count != 1 {
                    return None;
                }
                self.state |= state_bit(device_key.game_action);
            }
            KeyKind::Released => {
                let count = self.pressed.get_mut(&event.action)?;
                *count -= 1;
                if *count != 0 {
                    return None;
                }
                self.remove_pressed_action(event.action);
            }
            KeyKind::Repeated => {
                if !self.pressed.contains_key(&event.action) {
                    return None;
                }
            }
        }
        Some(MidpKeyEvent {
            kind: event.kind,
            key_code: device_key.key_code,
            lcdui_key_code,
            game_action: device_key.game_action,
            state: self.state,
        })
    }

    fn remove_pressed_action(&mut self, action: HostAction) {
        self.pressed.remove(&action);
        // Distinct device keys, such as Up and Num2, may hold the same game
        // action. Recompute from the bounded set of keys that remain down.
        self.state = self
            .pressed
            .keys()
            .filter_map(|action| self.mapping.key(*action))
            .fold(0, |state, key| state | state_bit(key.game_action));
    }

    /// Plans release callbacks without changing live input during a checkpoint.
    #[must_use]
    pub fn release_events(&self) -> Vec<MidpKeyEvent> {
        let mut events = Vec::with_capacity(self.pressed.len());
        let mut remaining_state = 0;
        // Traverse backwards to accumulate keys that remain after each release,
        // including distinct keys mapped to the same game action.
        for &action in self.pressed.keys().rev() {
            let Some(device_key) = self.mapping.key(action) else {
                continue;
            };
            if let Some(lcdui_key_code) = lcdui_key_code(action) {
                events.push(MidpKeyEvent {
                    kind: KeyKind::Released,
                    key_code: device_key.key_code,
                    lcdui_key_code,
                    game_action: device_key.game_action,
                    state: remaining_state,
                });
            }
            remaining_state |= state_bit(device_key.game_action);
        }
        events.reverse();
        events
    }

    pub fn focus_lost(&mut self) -> Vec<MidpKeyEvent> {
        let events = self.release_events();
        self.pressed.clear();
        self.state = 0;
        events
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/input.rs"]
mod tests;
