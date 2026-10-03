//! Physical controls are host inputs, independent of the emulated phone profile.

mod state;
pub use state::{PhysicalInputEvent, PhysicalInputState, PhysicalTransition};

use diagnostics::{Category, EmuError};
use platform::HostAction;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

// Leave room for a full keyboard alongside controller and hardware bindings.
pub const MAX_PHYSICAL_BINDINGS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    LeftShoulder,
    RightShoulder,
    LeftTrigger,
    RightTrigger,
    LeftStick,
    RightStick,
    Start,
    Select,
    Extra(u8),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GamepadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    HatX,
    HatY,
    LeftTrigger,
    RightTrigger,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AxisDirection {
    Negative,
    Positive,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PhysicalControl {
    /// USB HID keyboard usage, normalized by the shell (not a native device ID).
    Keyboard {
        usage: u16,
    },
    Gamepad {
        button: GamepadButton,
    },
    Axis {
        axis: GamepadAxis,
        direction: AxisDirection,
    },
    /// Additional Android hardware keys, including optional volume bindings.
    AndroidKey {
        code: u16,
    },
}

impl PhysicalControl {
    /// A D-pad may arrive as hat axes, key events, or both. Its binding is the
    /// same regardless of the native reporting format; held sources stay separate.
    const fn binding_control(self) -> Self {
        use AxisDirection::{Negative, Positive};
        use GamepadButton as B;
        let button = match self {
            Self::Axis {
                axis: GamepadAxis::HatX,
                direction: Negative,
            } => B::DpadLeft,
            Self::Axis {
                axis: GamepadAxis::HatX,
                direction: Positive,
            } => B::DpadRight,
            Self::Axis {
                axis: GamepadAxis::HatY,
                direction: Negative,
            } => B::DpadUp,
            Self::Axis {
                axis: GamepadAxis::HatY,
                direction: Positive,
            } => B::DpadDown,
            _ => return self,
        };
        Self::Gamepad { button }
    }

    #[must_use]
    pub const fn valid(self) -> bool {
        match self {
            Self::Keyboard { usage } => usage >= 4 && usage <= 231,
            Self::Gamepad {
                button: GamepadButton::Extra(number),
            } => number >= 1 && number <= 16,
            Self::Axis {
                axis: GamepadAxis::LeftTrigger | GamepadAxis::RightTrigger,
                direction: AxisDirection::Negative,
            } => false,
            // Home, power, system Back and app switching stay with the OS.
            Self::AndroidKey { code } => {
                code > 0 && code <= 512 && !matches!(code, 3 | 4 | 26 | 187 | 219 | 223 | 224)
            }
            Self::Gamepad { .. } | Self::Axis { .. } => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhysicalAction {
    Phone(HostAction),
    Menu,
    Fullscreen,
    FastForward,
    DebugOverlay,
    StopGame,
}

impl PhysicalAction {
    #[must_use]
    pub const fn valid(self) -> bool {
        match self {
            Self::Phone(action) => !action.is_emulator_action(),
            _ => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalBinding {
    pub control: PhysicalControl,
    pub action: PhysicalAction,
}

/// A complete mapping. An empty list intentionally disables physical gameplay input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalBindings {
    pub bindings: Vec<PhysicalBinding>,
    /// Stick/trigger activation threshold; release uses a lower threshold to avoid chatter.
    pub dead_zone_percent: u8,
}

impl PhysicalBindings {
    /// Upgrade only the complete, unmodified default for the saved schema. Custom
    /// layouts (including an intentionally empty one) keep their assignments.
    pub(crate) fn normalize_legacy_defaults(&mut self, schema_version: u32) {
        use GamepadButton as B;
        use HostAction as H;
        use PhysicalAction::Phone;
        let mut original = Self::default();
        original
            .bindings
            .retain(|binding| !matches!(binding.control, PhysicalControl::Keyboard { .. }));
        original
            .bindings
            .splice(0..0, Self::keyboard_defaults(true));
        original.bindings.retain(|binding| {
            !matches!(
                binding.control,
                PhysicalControl::Gamepad {
                    button: B::LeftStick | B::RightStick
                }
            )
        });
        for binding in &mut original.bindings {
            binding.action = match binding.control {
                PhysicalControl::Gamepad { button: B::South } => Phone(H::Fire),
                PhysicalControl::Gamepad { button: B::East } => Phone(H::SoftRight),
                PhysicalControl::Gamepad { button: B::West } => Phone(if schema_version <= 6 {
                    H::Num1
                } else {
                    H::Num0
                }),
                PhysicalControl::Gamepad { button: B::North } => Phone(if schema_version <= 6 {
                    H::Num3
                } else {
                    H::Pound
                }),
                _ => binding.action,
            };
        }
        if *self == original {
            *self = Self::default();
        }
    }

    /// Upgrade the untouched keyboard independently of controller assignments.
    /// Explicit game overrides never call this app-settings migration.
    pub(crate) fn normalize_legacy_keyboard_defaults(&mut self) {
        let original = Self::keyboard_defaults(true);
        let keyboard_count = self
            .bindings
            .iter()
            .filter(|binding| matches!(binding.control, PhysicalControl::Keyboard { .. }))
            .count();
        if keyboard_count != original.len()
            || !original
                .iter()
                .all(|binding| self.bindings.contains(binding))
        {
            return;
        }
        let updated = Self::keyboard_defaults(false);
        if self.bindings.len() - keyboard_count + updated.len() > MAX_PHYSICAL_BINDINGS {
            return;
        }
        self.bindings
            .retain(|binding| !matches!(binding.control, PhysicalControl::Keyboard { .. }));
        self.bindings.splice(0..0, updated);
    }

    pub(crate) fn normalize_dpad_bindings(&mut self) -> bool {
        let mut seen = BTreeSet::new();
        if self.bindings.len() > MAX_PHYSICAL_BINDINGS
            || self
                .bindings
                .iter()
                .any(|binding| !seen.insert(binding.control))
            || !self
                .bindings
                .iter()
                .any(|binding| binding.control != binding.control.binding_control())
        {
            return false;
        }
        // Earlier settings could bind the hat and key reports separately.
        // Assign appends, so the most recent assignment wins for each direction.
        seen.clear();
        self.bindings.reverse();
        self.bindings.retain_mut(|binding| {
            binding.control = binding.control.binding_control();
            seen.insert(binding.control)
        });
        self.bindings.reverse();
        true
    }

    /// # Errors
    /// Rejects duplicate inputs, unsupported actions and values outside fixed bounds.
    pub fn validate(&self) -> Result<(), EmuError> {
        let mut controls = BTreeSet::new();
        if self.bindings.len() > MAX_PHYSICAL_BINDINGS
            || !(10..=80).contains(&self.dead_zone_percent)
            || self.bindings.iter().any(|binding| {
                !binding.control.valid()
                    || !binding.action.valid()
                    || !controls.insert(binding.control.binding_control())
            })
        {
            return Err(EmuError::new(
                Category::Platform,
                "settings-physical-input",
                "Physical controls contain duplicate or unsupported bindings, or exceed their limits",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn resolve(&self, control: PhysicalControl) -> Option<PhysicalAction> {
        self.bindings
            .iter()
            .find(|binding| binding.control.binding_control() == control.binding_control())
            .map(|binding| binding.action)
    }

    /// Remove a binding regardless of whether the D-pad reports keys or axes.
    pub fn remove(&mut self, control: PhysicalControl) {
        self.bindings
            .retain(|binding| binding.control.binding_control() != control.binding_control());
    }

    /// Assigning an already bound input moves it to the selected action.
    /// # Errors
    /// Leaves the mapping unchanged if the assignment is invalid or at capacity.
    pub fn assign(
        &mut self,
        control: PhysicalControl,
        action: PhysicalAction,
    ) -> Result<(), EmuError> {
        let control = control.binding_control();
        let mut updated = self.clone();
        updated
            .bindings
            .retain(|binding| binding.control.binding_control() != control);
        updated.bindings.push(PhysicalBinding { control, action });
        updated.validate()?;
        *self = updated;
        Ok(())
    }
}

impl PhysicalBindings {
    fn keyboard_defaults(legacy: bool) -> Vec<PhysicalBinding> {
        use HostAction as H;
        use PhysicalAction::{Menu, Phone};
        // Keep the original keyboard available for saved-settings migrations.
        let mut bindings = Vec::new();
        for (usage, action) in [
            (82, Phone(H::Up)),
            (81, Phone(H::Down)),
            (80, Phone(H::Left)),
            (79, Phone(H::Right)),
            (40, Phone(H::Fire)),
            (44, Phone(if legacy { H::Fire } else { H::Num5 })),
            (58, Phone(H::SoftLeft)),
            (59, Phone(H::SoftRight)),
            (60, Phone(H::Back)),
            (42, Phone(H::Clear)),
            (39, Phone(H::Num0)),
            (30, Phone(H::Num1)),
            (31, Phone(H::Num2)),
            (32, Phone(H::Num3)),
            (33, Phone(H::Num4)),
            (34, Phone(H::Num5)),
            (35, Phone(H::Num6)),
            (36, Phone(H::Num7)),
            (37, Phone(H::Num8)),
            (38, Phone(H::Num9)),
            (41, Menu),
            (68, PhysicalAction::Fullscreen),
            (67, PhysicalAction::DebugOverlay),
        ] {
            bindings.push(PhysicalBinding {
                control: PhysicalControl::Keyboard { usage },
                action,
            });
        }
        if !legacy {
            for (usage, action) in [
                (26, H::Up),       // W
                (4, H::Left),      // A
                (22, H::Down),     // S
                (7, H::Right),     // D
                (29, H::Num5),     // Z
                (27, H::Num0),     // X
                (6, H::Star),      // C
                (25, H::Pound),    // V
                (20, H::SoftLeft), // Q
                (8, H::SoftRight), // E
                (98, H::Num0),     // NumPad 0
                (89, H::Num1),     // NumPad 1
                (90, H::Num2),     // NumPad 2
                (91, H::Num3),     // NumPad 3
                (92, H::Num4),     // NumPad 4
                (93, H::Num5),     // NumPad 5
                (94, H::Num6),     // NumPad 6
                (95, H::Num7),     // NumPad 7
                (96, H::Num8),     // NumPad 8
                (97, H::Num9),     // NumPad 9
            ] {
                bindings.push(PhysicalBinding {
                    control: PhysicalControl::Keyboard { usage },
                    action: Phone(action),
                });
            }
        }
        bindings
    }
}

impl Default for PhysicalBindings {
    fn default() -> Self {
        use GamepadButton as B;
        use HostAction as H;
        use PhysicalAction::{Menu, Phone};
        let mut bindings = Self::keyboard_defaults(false);
        for (button, action) in [
            (B::DpadUp, Phone(H::Up)),
            (B::DpadDown, Phone(H::Down)),
            (B::DpadLeft, Phone(H::Left)),
            (B::DpadRight, Phone(H::Right)),
            (B::South, Phone(H::Num5)),
            (B::East, Phone(H::Star)),
            (B::West, Phone(H::Pound)),
            (B::North, Phone(H::Num0)),
            (B::LeftShoulder, Phone(H::SoftLeft)),
            (B::RightShoulder, Phone(H::SoftRight)),
            (B::LeftStick, PhysicalAction::DebugOverlay),
            (B::RightStick, PhysicalAction::StopGame),
            (B::Start, Menu),
            (B::Select, Phone(H::Num0)),
        ] {
            bindings.push(PhysicalBinding {
                control: PhysicalControl::Gamepad { button },
                action,
            });
        }
        for (axis, negative, positive) in [
            (GamepadAxis::LeftX, H::Left, H::Right),
            (GamepadAxis::LeftY, H::Up, H::Down),
        ] {
            for (direction, action) in [
                (AxisDirection::Negative, negative),
                (AxisDirection::Positive, positive),
            ] {
                bindings.push(PhysicalBinding {
                    control: PhysicalControl::Axis { axis, direction },
                    action: Phone(action),
                });
            }
        }
        Self {
            bindings,
            dead_zone_percent: 35,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-core/physical_input/mod.rs"]
mod tests;
