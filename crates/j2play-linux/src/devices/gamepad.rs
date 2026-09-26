use crate::{input::Input, operation::error};
use frontend_core::{
    PlatformLifecycleSignal, VibrationRequest,
    physical_input::{GamepadAxis, GamepadButton, PhysicalControl, PhysicalInputEvent},
};
use sdl2::{
    controller::{Axis, Button, GameController},
    event::Event,
};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub(super) struct Gamepads {
    subsystem: Option<sdl2::GameControllerSubsystem>,
    controllers: BTreeMap<u32, GameController>,
    rumbling: bool,
    continuous: Option<(u16, Instant)>,
    rumble_failed: bool,
}

impl Gamepads {
    pub(super) fn new(sdl: &sdl2::Sdl, input: &Input) -> Self {
        let subsystem = if let Ok(subsystem) = sdl.game_controller() {
            Some(subsystem)
        } else {
            input.error(error(
                "linux-gamepad-unavailable",
                "Controller input is unavailable. Check device access permissions.",
            ));
            None
        };
        let mut result = Self {
            subsystem,
            controllers: BTreeMap::new(),
            rumbling: false,
            continuous: None,
            rumble_failed: false,
        };
        if let Some(subsystem) = &result.subsystem {
            for index in 0..subsystem.num_joysticks().unwrap_or(0).min(64) {
                result.open(index, input);
            }
        }
        result
    }

    fn open(&mut self, index: u32, input: &Input) {
        let Some(subsystem) = &self.subsystem else {
            return;
        };
        if self.controllers.len() >= 8 || !subsystem.is_game_controller(index) {
            return;
        }
        match subsystem.open(index) {
            Ok(controller) => {
                let id = controller.instance_id();
                if self.controllers.contains_key(&id) {
                    return;
                }
                let name = controller
                    .name()
                    .chars()
                    .filter(|ch| !ch.is_control())
                    .take(128)
                    .collect();
                input.device_changed(id, Some(name));
                self.controllers.insert(id, controller);
                self.rumble_failed = false;
            }
            Err(_) => input.error(error(
                "linux-gamepad-open",
                "A controller could not be opened. Check device access permissions.",
            )),
        }
    }

    pub(super) fn event(
        &mut self,
        event: &Event,
        input: &Input,
        lifecycle: &PlatformLifecycleSignal,
    ) {
        match *event {
            Event::ControllerDeviceAdded { which, .. } => self.open(which, input),
            Event::ControllerDeviceRemoved { which, .. } => {
                self.controllers.remove(&which);
                input.device_changed(which, None);
            }
            Event::ControllerDeviceRemapped { which, .. } => {
                if let Some(controller) = self.controllers.get(&which) {
                    let name = controller
                        .name()
                        .chars()
                        .filter(|ch| !ch.is_control())
                        .take(128)
                        .collect();
                    input.device_changed(which, Some(name));
                }
            }
            Event::ControllerButtonDown { which, button, .. }
            | Event::ControllerButtonUp { which, button, .. }
                if self.controllers.contains_key(&which) =>
            {
                if let Some(button) = button_mapping(button) {
                    input.event(
                        PhysicalInputEvent::Button {
                            device: which,
                            control: PhysicalControl::Gamepad { button },
                            pressed: matches!(event, Event::ControllerButtonDown { .. }),
                        },
                        lifecycle,
                    );
                }
            }
            Event::ControllerAxisMotion {
                which, axis, value, ..
            } if self.controllers.contains_key(&which) => {
                let (axis, value) = axis_mapping(axis, value);
                input.event(
                    PhysicalInputEvent::Axis {
                        device: which,
                        axis,
                        value,
                    },
                    lifecycle,
                );
            }
            _ => {}
        }
    }

    pub(super) fn can_rumble(&self) -> bool {
        !self.rumble_failed && self.controllers.values().any(GameController::has_rumble)
    }

    pub(super) fn rumble(&mut self, request: VibrationRequest, input: &Input) {
        let (strength, millis) = match request {
            VibrationRequest::Stop if !self.rumbling => return,
            VibrationRequest::Stop => {
                self.continuous = None;
                (0, 0)
            }
            VibrationRequest::Continuous { level } => {
                let strength = strength(level);
                self.continuous = Some((strength, Instant::now()));
                (strength, 1000)
            }
            VibrationRequest::Timed {
                duration_millis,
                level,
            } => {
                self.continuous = None;
                (
                    strength(level),
                    u32::try_from(duration_millis.min(60_000)).unwrap_or(60_000),
                )
            }
        };
        self.rumbling = strength > 0;
        self.apply(strength, millis, input);
    }

    fn apply(&mut self, strength: u16, millis: u32, input: &Input) {
        for controller in self
            .controllers
            .values_mut()
            .filter(|controller| controller.has_rumble())
        {
            if controller.set_rumble(strength, strength, millis).is_err() && !self.rumble_failed {
                self.rumble_failed = true;
                input.error(error(
                    "linux-rumble-failed",
                    "Controller vibration failed and has been disabled.",
                ));
            }
        }
    }

    pub(super) fn refresh_rumble(&mut self, input: &Input) {
        if let Some((strength, renewed)) = self.continuous
            && renewed.elapsed() >= Duration::from_millis(500)
        {
            self.apply(strength, 1000, input);
            self.continuous = Some((strength, Instant::now()));
        }
    }
}

fn strength(level: Option<u8>) -> u16 {
    u16::try_from(u32::from(level.unwrap_or(100).min(100)) * u32::from(u16::MAX) / 100)
        .unwrap_or(u16::MAX)
}

fn button_mapping(button: Button) -> Option<GamepadButton> {
    use GamepadButton as B;
    Some(match button {
        Button::A => B::South,
        Button::B => B::East,
        Button::X => B::West,
        Button::Y => B::North,
        Button::DPadUp => B::DpadUp,
        Button::DPadDown => B::DpadDown,
        Button::DPadLeft => B::DpadLeft,
        Button::DPadRight => B::DpadRight,
        Button::LeftShoulder => B::LeftShoulder,
        Button::RightShoulder => B::RightShoulder,
        Button::LeftStick => B::LeftStick,
        Button::RightStick => B::RightStick,
        Button::Start => B::Start,
        Button::Back => B::Select,
        Button::Misc1 => B::Extra(1),
        Button::Paddle1 => B::Extra(2),
        Button::Paddle2 => B::Extra(3),
        Button::Paddle3 => B::Extra(4),
        Button::Paddle4 => B::Extra(5),
        Button::Touchpad => B::Extra(6),
        Button::Guide => return None,
    })
}

fn axis_mapping(axis: Axis, value: i16) -> (GamepadAxis, f32) {
    use GamepadAxis as A;
    let axis = match axis {
        Axis::LeftX => A::LeftX,
        Axis::LeftY => A::LeftY,
        Axis::RightX => A::RightX,
        Axis::RightY => A::RightY,
        Axis::TriggerLeft => A::LeftTrigger,
        Axis::TriggerRight => A::RightTrigger,
    };
    let value = f32::from(value) / if value < 0 { 32768.0 } else { 32767.0 };
    (
        axis,
        if matches!(axis, A::LeftTrigger | A::RightTrigger) {
            value.max(0.0)
        } else {
            value
        },
    )
}

#[cfg(test)]
#[path = "../../../../tests/unit/j2play-linux/devices/gamepad.rs"]
mod tests;
