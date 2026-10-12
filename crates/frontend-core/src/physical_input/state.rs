use super::{AxisDirection, GamepadAxis, PhysicalAction, PhysicalBindings, PhysicalControl};
use std::collections::{BTreeMap, BTreeSet};

const MAX_HELD_CONTROLS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicalInputEvent {
    Button {
        device: u32,
        control: PhysicalControl,
        pressed: bool,
    },
    /// Signed axes use [-1, 1], triggers [0, 1]; Y increases downward.
    Axis {
        device: u32,
        axis: GamepadAxis,
        value: f32,
    },
    Disconnected {
        device: u32,
    },
    /// Lost transport events are an input barrier, never a dropped release.
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalTransition {
    pub device: u32,
    pub control: PhysicalControl,
    pub pressed: bool,
}

#[derive(Default)]
pub struct PhysicalInputState {
    held: BTreeSet<(u32, PhysicalControl)>,
    active: BTreeMap<(u32, PhysicalControl), PhysicalAction>,
}

impl PhysicalInputState {
    /// Whether any physical source still holds this resolved action.
    #[must_use]
    pub fn action_active(&self, action: PhysicalAction) -> bool {
        self.active.values().any(|held| *held == action)
    }

    /// A D-pad can report the same held direction as both a key and a hat axis.
    /// Keep their releases separate, but let menus avoid duplicate navigation.
    #[must_use]
    pub fn other_report_held(&self, change: PhysicalTransition) -> bool {
        self.held.iter().any(|(device, control)| {
            *device == change.device
                && *control != change.control
                && control.binding_control() == change.control.binding_control()
        })
    }

    /// Converts axes with hysteresis and deduplicates presses per physical source.
    pub fn update(
        &mut self,
        event: PhysicalInputEvent,
        dead_zone_percent: u8,
    ) -> Vec<PhysicalTransition> {
        let mut changes = Vec::new();
        match event {
            PhysicalInputEvent::Button {
                device,
                control,
                pressed,
            } => {
                self.button(device, control, pressed, &mut changes);
            }
            PhysicalInputEvent::Axis {
                device,
                axis,
                value,
            } => {
                let value = if value.is_finite() {
                    value.clamp(-1.0, 1.0)
                } else {
                    0.0
                };
                let press = f32::from(dead_zone_percent.clamp(10, 80)) / 100.0;
                for direction in [AxisDirection::Negative, AxisDirection::Positive] {
                    let control = PhysicalControl::Axis { axis, direction };
                    let magnitude = if direction == AxisDirection::Negative {
                        -value
                    } else {
                        value
                    };
                    let threshold = if self.held.contains(&(device, control)) {
                        press * 0.7
                    } else {
                        press
                    };
                    self.button(device, control, magnitude > threshold, &mut changes);
                }
            }
            PhysicalInputEvent::Disconnected { device } => {
                changes.extend(self.held.extract_if(.., |(id, _)| *id == device).map(
                    |(device, control)| PhysicalTransition {
                        device,
                        control,
                        pressed: false,
                    },
                ));
            }
            PhysicalInputEvent::Reset => {
                for (device, control) in std::mem::take(&mut self.held) {
                    changes.push(PhysicalTransition {
                        device,
                        control,
                        pressed: false,
                    });
                }
            }
        }
        changes
    }

    fn button(
        &mut self,
        device: u32,
        control: PhysicalControl,
        pressed: bool,
        changes: &mut Vec<PhysicalTransition>,
    ) {
        if !control.valid() {
            return;
        }
        let transitioned = if pressed {
            self.held.len() < MAX_HELD_CONTROLS && self.held.insert((device, control))
        } else {
            self.held.remove(&(device, control))
        };
        if transitioned {
            changes.push(PhysicalTransition {
                device,
                control,
                pressed,
            });
        }
    }

    /// Returns only first-press/last-release edges for an action, even across devices.
    /// Releases retain the action chosen at press time, including after a remap.
    pub fn resolve(
        &mut self,
        change: PhysicalTransition,
        mapping: &PhysicalBindings,
        enabled: bool,
    ) -> Option<(PhysicalAction, bool)> {
        let action = (change.pressed && enabled)
            .then(|| mapping.resolve(change.control))
            .flatten();
        self.resolve_action(change, action)
    }

    /// Shares ownership for a context-specific action, such as an editor key.
    /// `None` disables new presses; releases keep the action selected at press time.
    pub fn resolve_action(
        &mut self,
        change: PhysicalTransition,
        action: Option<PhysicalAction>,
    ) -> Option<(PhysicalAction, bool)> {
        let source = (change.device, change.control);
        if change.pressed {
            let action = action?;
            let already_active = self.action_active(action);
            self.active.insert(source, action);
            (!already_active).then_some((action, true))
        } else {
            let action = self.active.remove(&source)?;
            (!self.action_active(action)).then_some((action, false))
        }
    }

    /// The caller releases guest input atomically; held controls require neutral
    /// before they can press again after pause, remap, navigation or focus loss.
    pub fn release_actions(&mut self) {
        self.active.clear();
    }
}
