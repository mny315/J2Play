use super::{
    EmuError, Event, FULLSCREEN_DOUBLE_TAP_MAX_DISTANCE, FULLSCREEN_DOUBLE_TAP_MAX_GAP,
    FULLSCREEN_TAP_MAX_DURATION, FULLSCREEN_TAP_MAX_TRAVEL, FrontendApp, FullscreenHelpState,
    Instant, Pos2, Rect, Screen, SessionState, egui, fullscreen_exit_gesture_rect,
};
use frontend_core::FullscreenMode;

pub(super) struct GameplayFullscreenPolicy {
    mode: FullscreenMode,
    landscape: Option<bool>,
}

impl GameplayFullscreenPolicy {
    pub(super) const fn new(mode: FullscreenMode) -> Self {
        Self {
            mode,
            landscape: None,
        }
    }

    fn update(&mut self, landscape: bool) -> Option<bool> {
        let previous = self.landscape.replace(landscape);
        if previous.is_some()
            && (self.mode != FullscreenMode::LandscapeOnly || previous == Some(landscape))
        {
            return None;
        }
        Some(match self.mode {
            FullscreenMode::Off => false,
            FullscreenMode::On => true,
            FullscreenMode::LandscapeOnly => landscape,
        })
    }
}

impl FrontendApp {
    pub(super) fn toggle_gameplay_fullscreen(&mut self) {
        let Some(fullscreen) = (match &self.screen {
            Screen::Gameplay(gameplay) => Some(!gameplay.fullscreen),
            Screen::Library | Screen::AppSettings(_) | Screen::Settings(_) => None,
        }) else {
            return;
        };
        if let Err(error) = self.set_gameplay_fullscreen(fullscreen) {
            self.show_error("Could not change fullscreen mode", &error);
        }
    }

    pub(super) fn set_gameplay_fullscreen(&mut self, fullscreen: bool) -> Result<(), EmuError> {
        let Some(current) = (match &self.screen {
            Screen::Gameplay(gameplay) => Some(gameplay.fullscreen),
            Screen::Library | Screen::AppSettings(_) | Screen::Settings(_) => None,
        }) else {
            return Ok(());
        };
        if current == fullscreen
            && self
                .platform_fullscreen
                .is_none_or(|actual| actual == fullscreen)
        {
            return Ok(());
        }

        self.clear_gameplay_input_geometry();
        self.fullscreen_exit_gesture.reset();
        let result = self.platform.set_fullscreen(fullscreen);
        if result.is_ok() {
            self.gameplay_transition.request();
        }
        if (result.is_ok() || !fullscreen)
            && let Screen::Gameplay(gameplay) = &mut self.screen
        {
            gameplay.fullscreen = fullscreen;
        }
        if result.is_ok() && fullscreen && self.fullscreen_help == FullscreenHelpState::Pending {
            self.fullscreen_help = FullscreenHelpState::Visible;
        }
        result
    }

    pub(super) fn fullscreen_help_visible(&self) -> bool {
        self.fullscreen_help == FullscreenHelpState::Visible
            && matches!(&self.screen, Screen::Gameplay(gameplay) if gameplay.fullscreen)
    }

    pub(super) fn acknowledge_fullscreen_help(&mut self) {
        self.fullscreen_help = FullscreenHelpState::Acknowledged;
        self.fullscreen_exit_gesture.reset();
        if let Err(error) = self.repository.acknowledge_fullscreen_help() {
            self.show_error("Could not save fullscreen help", &error);
        }
    }

    pub(super) fn process_fullscreen_exit_gesture(&mut self, ctx: &egui::Context) -> bool {
        if self.overlay_active()
            || !matches!(&self.screen, Screen::Gameplay(gameplay) if gameplay.fullscreen)
        {
            self.fullscreen_exit_gesture.reset();
            return false;
        }
        let exit = ctx.input(|input| {
            self.fullscreen_exit_gesture.process(
                &input.raw.events,
                fullscreen_exit_gesture_rect(self.safe_content_rect),
                Instant::now(),
            )
        });
        if !exit {
            return false;
        }
        if let Err(error) = self.set_gameplay_fullscreen(false) {
            self.show_error("Could not exit fullscreen mode", &error);
        }
        true
    }

    pub(super) fn process_platform_fullscreen(&mut self) {
        for _ in 0..super::MAX_PLATFORM_EVENTS_PER_TICK {
            match self.platform.poll_fullscreen() {
                Some(Ok(fullscreen)) => {
                    self.platform_fullscreen = Some(fullscreen);
                    let Screen::Gameplay(gameplay) = &self.screen else {
                        continue;
                    };
                    if gameplay.fullscreen == fullscreen {
                        continue;
                    }
                    self.clear_gameplay_input_geometry();
                    self.fullscreen_exit_gesture.reset();
                    self.gameplay_transition.request();
                    if let Screen::Gameplay(gameplay) = &mut self.screen {
                        gameplay.fullscreen = fullscreen;
                    }
                    if fullscreen && self.fullscreen_help == super::FullscreenHelpState::Pending {
                        self.fullscreen_help = super::FullscreenHelpState::Visible;
                    }
                }
                Some(Err(error)) => self.show_error("Could not change fullscreen mode", &error),
                None => break,
            }
        }
    }

    pub(super) fn sync_gameplay_fullscreen_policy(&mut self, viewport: Rect) {
        if self.platform_suspended
            || !matches!(self.screen, Screen::Gameplay(_))
            || matches!(
                self.session.state(),
                SessionState::Idle | SessionState::Stopping | SessionState::Failed
            )
            || !viewport.is_finite()
            || viewport.width() <= 0.0
            || viewport.height() <= 0.0
        {
            return;
        }
        // Use the whole host viewport: system bars and the IME must not
        // masquerade as a rotation or undo the user's manual fullscreen exit.
        let Some(fullscreen) = self
            .fullscreen_policy
            .as_mut()
            .and_then(|policy| policy.update(viewport.width() > viewport.height()))
        else {
            return;
        };
        // The orientation is consumed even on failure, so an unsupported host
        // produces one error per change instead of retrying on every frame.
        if let Err(error) = self.set_gameplay_fullscreen(fullscreen) {
            self.show_error("Could not change fullscreen mode", &error);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FullscreenTapPress {
    started_at: Instant,
    origin: Pos2,
    valid: bool,
}

#[derive(Clone, Copy, Debug)]
struct FullscreenTap {
    completed_at: Instant,
    position: Pos2,
}

#[derive(Default)]
pub(super) struct FullscreenExitGesture {
    press: Option<FullscreenTapPress>,
    previous_tap: Option<FullscreenTap>,
}

impl FullscreenExitGesture {
    pub(super) fn reset(&mut self) {
        self.press = None;
        self.previous_tap = None;
    }

    pub(super) fn process(&mut self, events: &[Event], region: Rect, now: Instant) -> bool {
        if self.previous_tap.is_some_and(|tap| {
            now.saturating_duration_since(tap.completed_at) > FULLSCREEN_DOUBLE_TAP_MAX_GAP
        }) {
            self.previous_tap = None;
        }

        for event in events {
            match event {
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => {
                    if region.contains(*pos) {
                        self.press = Some(FullscreenTapPress {
                            started_at: now,
                            origin: *pos,
                            valid: true,
                        });
                    } else {
                        self.press = None;
                        self.previous_tap = None;
                    }
                }
                Event::PointerMoved(position) => {
                    if let Some(press) = &mut self.press
                        && press.origin.distance(*position) > FULLSCREEN_TAP_MAX_TRAVEL
                    {
                        press.valid = false;
                    }
                }
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => {
                    let Some(press) = self.press.take() else {
                        continue;
                    };
                    let valid = press.valid
                        && now.saturating_duration_since(press.started_at)
                            <= FULLSCREEN_TAP_MAX_DURATION
                        && press.origin.distance(*pos) <= FULLSCREEN_TAP_MAX_TRAVEL
                        && region.contains(*pos);
                    if !valid {
                        self.previous_tap = None;
                        continue;
                    }
                    if self.previous_tap.take().is_some_and(|tap| {
                        now.saturating_duration_since(tap.completed_at)
                            <= FULLSCREEN_DOUBLE_TAP_MAX_GAP
                            && tap.position.distance(*pos) <= FULLSCREEN_DOUBLE_TAP_MAX_DISTANCE
                    }) {
                        self.reset();
                        return true;
                    }
                    self.previous_tap = Some(FullscreenTap {
                        completed_at: now,
                        position: *pos,
                    });
                }
                Event::WindowFocused(false) | Event::PointerCancelled => self.reset(),
                Event::PointerGone => self.press = None,
                _ => {}
            }
        }
        false
    }
}
