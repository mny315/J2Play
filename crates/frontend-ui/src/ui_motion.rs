use super::{FrontendApp, Screen};
use std::time::Instant;

pub(super) const UI_MOTION_SECONDS: f32 = 0.24;
const ENTER_OPACITY: f32 = 0.05;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Page {
    Library,
    AppSettings,
    GameSettings,
    PhysicalControls {
        global: bool,
    },
    AppControls {
        landscape: bool,
    },
    GameControls {
        landscape: bool,
    },
    Gameplay {
        attempt: Option<(frontend_core::SessionId, frontend_core::AttemptId)>,
    },
}

impl Page {
    fn settings_parent(self) -> Option<Self> {
        match self {
            Self::AppControls { .. } | Self::PhysicalControls { global: true } => {
                Some(Self::AppSettings)
            }
            Self::GameControls { .. } | Self::PhysicalControls { global: false } => {
                Some(Self::GameSettings)
            }
            _ => None,
        }
    }
}

/// A single finite fade for the currently displayed page. No outgoing screen,
/// guest texture, settings draft or navigation action is retained or replayed.
#[derive(Default)]
pub(super) struct UiMotion {
    page: Option<Page>,
    started: Option<Instant>,
    settings_focus: Option<super::egui::Id>,
}

impl UiMotion {
    fn sync(&mut self, page: Page, now: Instant) -> bool {
        if self.page == Some(page) {
            return false;
        }
        self.started = self.page.map(|_| now);
        self.page = Some(page);
        true
    }

    fn opacity(&mut self, now: Instant) -> f32 {
        let Some(started) = self.started else {
            return 1.0;
        };
        let t = (now.saturating_duration_since(started).as_secs_f32() / UI_MOTION_SECONDS)
            .clamp(0.0, 1.0);
        if t >= 1.0 {
            self.settle();
            return 1.0;
        }
        // Keep the middle of the fade visible even on a fast navigation tap.
        // Ease both ends instead of making most of the page opaque immediately.
        let eased = t * t * (3.0 - 2.0 * t);
        ENTER_OPACITY + (1.0 - ENTER_OPACITY) * eased
    }

    pub(super) fn settle(&mut self) {
        self.started = None;
    }
}

impl FrontendApp {
    pub(super) fn motion_page(&self) -> Page {
        if self.physical_editor.is_some() {
            return Page::PhysicalControls {
                global: matches!(self.screen, Screen::AppSettings(_)),
            };
        }
        match &self.screen {
            Screen::Library => Page::Library,
            Screen::AppSettings(settings) => {
                settings
                    .control_editor
                    .as_ref()
                    .map_or(Page::AppSettings, |editor| Page::AppControls {
                        landscape: editor.landscape,
                    })
            }
            Screen::Settings(settings) => {
                settings
                    .control_editor
                    .as_ref()
                    .map_or(Page::GameSettings, |editor| Page::GameControls {
                        landscape: editor.landscape,
                    })
            }
            Screen::Gameplay(_) => Page::Gameplay {
                attempt: self.session.active_ids(),
            },
        }
    }

    pub(super) fn sync_ui_motion(&mut self, ctx: &super::egui::Context) -> f32 {
        let now = Instant::now();
        // Start only when this page is actually painted. eframe also runs logic
        // without painting; that must not consume a transition before it is seen.
        // A visual fade must not issue commands to the emulator worker.
        let page = self.motion_page();
        let previous = self.ui_motion.page;
        if self.ui_motion.sync(page, now) {
            if page.settings_parent().is_some() && page.settings_parent() == previous {
                self.ui_motion.settings_focus = ctx.memory(super::egui::Memory::focused);
            } else if previous.and_then(Page::settings_parent) == Some(page) {
                // The parent keeps its own scroll state. Restore navigation to
                // its previous control without scrolling it to the page header.
                if let Some(id) = self.ui_motion.settings_focus.take() {
                    super::focus_ring::restore_without_scroll(ctx, id);
                } else if let Some(id) = ctx.memory(super::egui::Memory::focused) {
                    ctx.memory_mut(|memory| memory.surrender_focus(id));
                }
            } else if matches!(page, Page::AppSettings | Page::GameSettings) {
                self.ui_motion.settings_focus = None;
                self.screen.focus_first_setting();
            }
        }
        if self.platform_suspended {
            self.ui_motion.settle();
        }
        let opacity = self.ui_motion.opacity(now);
        if self.ui_motion.started.is_some() {
            ctx.request_repaint();
        }
        opacity
    }

    pub(super) fn repaint_changed_page(&self, ctx: &super::egui::Context) {
        if !self.platform_suspended && self.ui_motion.page != Some(self.motion_page()) {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/ui_motion/mod.rs"]
mod tests;
