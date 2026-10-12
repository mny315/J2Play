mod diagram;
mod editor;
mod keyboard;
mod labels;
mod navigation_repeat;
mod picker;
mod selection;
mod sliders;

use super::{Event, FrontendApp, HostAction, Key, Screen, SessionState, egui};
pub(super) use editor::PhysicalEditor;
pub(super) use editor::draw_card;
use frontend_core::physical_input::{
    AxisDirection, GamepadAxis, GamepadButton, PhysicalAction, PhysicalBindings, PhysicalControl,
    PhysicalInputEvent, PhysicalTransition,
};
pub(super) use labels::key_usage;
pub(super) use navigation_repeat::NavigationRepeat;
pub(super) use sliders::{SliderRepeat, controller_slider, press_ui_key};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PhysicalInputConfig {
    pub capture: bool,
    pub gameplay: bool,
    pub text_input: bool,
}

impl FrontendApp {
    pub(super) fn configure_physical_input(&mut self) {
        self.platform.configure_physical_input(
            PhysicalInputConfig {
                capture: self
                    .physical_editor
                    .as_ref()
                    .is_some_and(|editor| editor.listening.is_some()),
                gameplay: matches!(self.screen, Screen::Gameplay(_)) && !self.overlay_active(),
                text_input: self.text_input_active,
            },
            self.physical_editor
                .as_ref()
                .map_or(&self.physical_bindings, |editor| &editor.draft),
        );
    }

    pub(super) fn process_physical_inputs(
        &mut self,
        ctx: &egui::Context,
        fresh: bool,
        allow_game: bool,
    ) {
        self.focus_ring
            .set_controller_connected(self.platform.has_physical_devices());
        if fresh && ctx.input(|input| input.raw.events.contains(&Event::WindowFocused(false))) {
            // Preserve neutral/held edges even when the whole batch is barred.
            // In particular, a release beside FocusLost must not leave a key
            // held until the user's next complete press/release cycle.
            ctx.input(|input| {
                for event in input.raw.events.iter().filter_map(keyboard_input_event) {
                    self.physical_input
                        .update(event, self.physical_bindings.dead_zone_percent);
                }
            });
            self.release_all_input();
            self.focus_ring.hide();
            return;
        }
        if fresh {
            self.process_library_keyboard(ctx);
            self.focus_ring.observe_input(
                ctx,
                self.text_input_active
                    || self
                        .physical_editor
                        .as_ref()
                        .is_some_and(|editor| editor.listening.is_some()),
            );
            if !self.text_input_active
                && !matches!(self.screen, Screen::Library | Screen::Gameplay(_))
                && self
                    .physical_editor
                    .as_ref()
                    .is_none_or(|editor| editor.listening.is_none())
                && !self.control_editor_keyboard(ctx)
            {
                crate::focus_navigation::keyboard(ctx);
            }
        }
        if let Some(editor) = &mut self.physical_editor
            && editor
                .listening
                .is_some_and(|started| started.elapsed() >= Duration::from_secs(15))
        {
            editor.listening = None;
            "No input received. Try again.".clone_into(&mut editor.status);
        }
        // egui owns ordinary keyboard navigation; capture sees physical keys.
        if fresh
            && (!allow_game
                || !matches!(self.screen, Screen::Gameplay(_))
                || self.session.state() != SessionState::Running
                || self.overlay_active())
        {
            if self
                .physical_editor
                .as_ref()
                .is_some_and(|editor| editor.listening.is_some())
            {
                ctx.input_mut(|input| {
                    input
                        .events
                        .retain(|event| !matches!(event, Event::Key { .. } | Event::Text(_)));
                });
            }
            let events: Vec<_> = ctx.input(|input| {
                input
                    .raw
                    .events
                    .iter()
                    .filter_map(keyboard_input_event)
                    .collect()
            });
            for event in events {
                self.process_physical_event(ctx, event, false);
            }
        }
        for _ in 0..256 {
            let Some(event) = self.platform.poll_physical_input() else {
                break;
            };
            self.process_physical_event(ctx, event, allow_game && ctx.input(|input| input.focused));
        }
        if fresh {
            self.repeat_navigation_input(ctx, Instant::now());
            self.repeat_slider_input(ctx, Instant::now());
        }
    }

    pub(super) fn process_physical_event(
        &mut self,
        ctx: &egui::Context,
        event: PhysicalInputEvent,
        allow_game: bool,
    ) {
        let dead_zone = self
            .physical_editor
            .as_ref()
            .map_or(self.physical_bindings.dead_zone_percent, |editor| {
                editor.draft.dead_zone_percent
            });
        let changes = self.physical_input.update(event, dead_zone);
        let accepts_input = !self.platform_suspended && ctx.input(|input| input.focused);
        if matches!(event, PhysicalInputEvent::Disconnected { .. }) {
            self.focus_ring.hide();
            if let Some(editor) = self.screen.control_editor_mut() {
                editor.navigation.moving = None;
            }
        }
        if matches!(event, PhysicalInputEvent::Reset) {
            self.release_all_input();
            return;
        }
        for change in changes {
            self.release_navigation_input(change);
            self.release_slider_input(change);
            if self.capture_physical_control(change, accepts_input) {
                continue;
            }
            let gameplay = matches!(self.screen, Screen::Gameplay(_)) && !self.overlay_active();
            if !gameplay {
                if change.pressed && accepts_input {
                    self.navigate_with_controller(ctx, change);
                }
                continue;
            }
            // Host actions remain available during loading, user pause and
            // layout transitions, just like their gameplay toolbar buttons.
            let host_action = matches!(
                self.physical_bindings.resolve(change.control),
                Some(
                    PhysicalAction::Menu
                        | PhysicalAction::Fullscreen
                        | PhysicalAction::DebugOverlay
                        | PhysicalAction::StopGame
                )
            ) && !matches!(
                self.session.state(),
                SessionState::Idle | SessionState::Stopping
            );
            let enabled = accepts_input
                && (host_action || (allow_game && self.session.state() == SessionState::Running));
            if let Some((action, pressed)) =
                self.physical_input
                    .resolve(change, &self.physical_bindings, enabled)
            {
                self.apply_physical_action(action, pressed);
            }
        }
    }

    pub(super) fn apply_physical_action(&mut self, action: PhysicalAction, pressed: bool) {
        match action {
            PhysicalAction::Phone(action) => {
                if let Some(event) =
                    super::session_input::physical_key_event(&self.touch_owners, action, pressed)
                {
                    self.submit_input(event);
                }
            }
            PhysicalAction::Menu if pressed => self.navigate_back(),
            PhysicalAction::Fullscreen if pressed => self.toggle_gameplay_fullscreen(),
            PhysicalAction::FastForward if pressed => self.toggle_fast_forward(),
            PhysicalAction::DebugOverlay if pressed => self.toggle_runtime_debug(),
            PhysicalAction::StopGame if pressed => self.stop_active_session(),
            _ => {}
        }
    }

    fn capture_physical_control(&mut self, change: PhysicalTransition, enabled: bool) -> bool {
        let Some(editor) = &mut self.physical_editor else {
            return false;
        };
        if editor
            .quiet_until
            .is_some_and(|until| Instant::now() < until)
        {
            return true;
        }
        let Some(_) = editor.listening else {
            return false;
        };
        let matches_capture = editor.tab != editor::EditorTab::Keyboard
            || matches!(change.control, PhysicalControl::Keyboard { .. });
        if change.pressed && enabled && matches_capture && self.display_error.is_none() {
            editor.listening = None;
            editor.quiet_until = Some(Instant::now() + Duration::from_millis(250));
            editor.select(selection::Picker::Binding(
                selection::BindingTarget::from_control(change.control),
            ));
            self.physical_input.release_actions();
        }
        true
    }

    fn navigate_with_controller(&mut self, ctx: &egui::Context, change: PhysicalTransition) {
        use GamepadButton as B;
        use egui::FocusDirection;
        let control = change.control;
        let direction = navigation_direction(control);
        if let Some(direction) = direction {
            let adjustment = match direction {
                FocusDirection::Left => Some(Key::ArrowLeft),
                FocusDirection::Right => Some(Key::ArrowRight),
                _ => None,
            };
            if self.physical_input.other_report_held(change) {
                // Repeats wait for both reports to release, without taking a
                // second step or restarting the initial delay.
                if self.slider_repeat.is_some()
                    && let Some(key) = adjustment
                {
                    self.adjust_slider_with_controller(ctx, change, key);
                }
                self.add_navigation_repeat_source(change, direction);
                return;
            }
            self.navigation_repeat = None;
            if self.control_editor_move_direction(ctx, direction) {
                self.focus_ring.navigate();
                self.slider_repeat = None;
                return;
            }
            let first = self.focus_ring.navigate();
            if first && matches!(self.screen, Screen::Library) && !self.entries.is_empty() {
                ctx.memory_mut(|memory| {
                    if let Some(id) = memory.focused() {
                        memory.surrender_focus(id);
                    }
                });
            } else if first && ctx.memory(egui::Memory::focused).is_some() {
                self.slider_repeat = None;
                if adjustment.is_none() || sliders::focused_slider(ctx).is_none() {
                    self.start_navigation_repeat(ctx, change, direction);
                }
                ctx.request_repaint();
                return;
            }
            if let Some(key) = adjustment
                && self.adjust_slider_with_controller(ctx, change, key)
            {
                return;
            }
            self.slider_repeat = None;
            self.start_navigation_repeat(ctx, change, direction);
            self.move_controller_focus(ctx, direction);
        } else if control == (PhysicalControl::Gamepad { button: B::South }) {
            self.focus_ring.navigate();
            self.navigation_repeat = None;
            self.slider_repeat = None;
            if ctx.memory(egui::Memory::focused).is_none()
                && self.move_library_focus(ctx, FocusDirection::None)
            {
                ctx.request_repaint();
                return;
            }
            sliders::press_ui_key(ctx, Key::Enter);
        } else if matches!(
            control,
            PhysicalControl::Gamepad {
                button: B::East | B::Start
            }
        ) || ((self.platform.escape_navigates_back()
            || self
                .screen
                .control_editor_mut()
                .is_some_and(|editor| editor.navigation.moving.is_some()))
            && control == PhysicalControl::Keyboard { usage: 41 })
        {
            // Prevent egui from applying this same Escape to the page or modal
            // exposed by Back later in this paint pass.
            if matches!(control, PhysicalControl::Keyboard { .. }) {
                ctx.input_mut(|input| {
                    input.events.retain(|event| {
                        !matches!(
                            event,
                            Event::Key {
                                key: Key::Escape,
                                ..
                            }
                        )
                    });
                });
            }
            self.navigation_repeat = None;
            self.slider_repeat = None;
            if !crate::focus_navigation::close_popup(ctx) {
                self.navigate_back();
            }
        }
    }
}

pub(super) fn keyboard_input_event(event: &Event) -> Option<PhysicalInputEvent> {
    let Event::Key {
        key,
        physical_key,
        pressed,
        repeat: false,
        ..
    } = *event
    else {
        return None;
    };
    Some(PhysicalInputEvent::Button {
        device: u32::MAX,
        control: PhysicalControl::Keyboard {
            usage: key_usage(physical_key.unwrap_or(key))?,
        },
        pressed,
    })
}

fn navigation_direction(control: PhysicalControl) -> Option<egui::FocusDirection> {
    use GamepadButton as B;
    use egui::FocusDirection;
    match control {
        PhysicalControl::Gamepad { button: B::DpadUp }
        | PhysicalControl::Axis {
            axis: GamepadAxis::HatY | GamepadAxis::LeftY,
            direction: AxisDirection::Negative,
        } => Some(FocusDirection::Up),
        PhysicalControl::Gamepad {
            button: B::DpadDown,
        }
        | PhysicalControl::Axis {
            axis: GamepadAxis::HatY | GamepadAxis::LeftY,
            direction: AxisDirection::Positive,
        } => Some(FocusDirection::Down),
        PhysicalControl::Gamepad {
            button: B::DpadLeft,
        }
        | PhysicalControl::Axis {
            axis: GamepadAxis::HatX | GamepadAxis::LeftX,
            direction: AxisDirection::Negative,
        } => Some(FocusDirection::Left),
        PhysicalControl::Gamepad {
            button: B::DpadRight,
        }
        | PhysicalControl::Axis {
            axis: GamepadAxis::HatX | GamepadAxis::LeftX,
            direction: AxisDirection::Positive,
        } => Some(FocusDirection::Right),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/physical_controls/mod.rs"]
mod tests;
