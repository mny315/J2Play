use super::{Duration, FrontendApp, Instant, PhysicalControl, PhysicalTransition, Screen, egui};
use crate::ui_motion::Page;
use std::collections::BTreeSet;

const REPEAT_DELAY: Duration = Duration::from_millis(400);
const REPEAT_INTERVAL: Duration = Duration::from_millis(60);
const MAX_REPEAT_SOURCES: usize = 16;

pub(crate) struct NavigationRepeat {
    direction: egui::FocusDirection,
    sources: BTreeSet<(u32, PhysicalControl)>,
    next: Instant,
    page: Page,
    overlay: bool,
    popup: Option<egui::LayerId>,
}

impl FrontendApp {
    pub(super) fn start_navigation_repeat(
        &mut self,
        ctx: &egui::Context,
        change: PhysicalTransition,
        direction: egui::FocusDirection,
    ) {
        self.navigation_repeat = Some(NavigationRepeat {
            direction,
            sources: BTreeSet::from([(change.device, change.control)]),
            next: Instant::now() + REPEAT_DELAY,
            page: self.motion_page(),
            overlay: self.overlay_active(),
            popup: crate::focus_navigation::active_layer(ctx),
        });
        ctx.request_repaint_after(REPEAT_DELAY);
    }

    pub(super) fn add_navigation_repeat_source(
        &mut self,
        change: PhysicalTransition,
        direction: egui::FocusDirection,
    ) {
        if let Some(repeat) = &mut self.navigation_repeat
            && repeat.direction == direction
            && repeat.sources.len() < MAX_REPEAT_SOURCES
        {
            repeat.sources.insert((change.device, change.control));
        }
    }

    pub(super) fn release_navigation_input(&mut self, change: PhysicalTransition) {
        if !change.pressed
            && let Some(repeat) = &mut self.navigation_repeat
        {
            repeat.sources.remove(&(change.device, change.control));
            if repeat.sources.is_empty() {
                self.navigation_repeat = None;
            }
        }
    }

    pub(super) fn move_controller_focus(
        &mut self,
        ctx: &egui::Context,
        direction: egui::FocusDirection,
    ) {
        if !self.move_library_focus(ctx, direction)
            && !crate::focus_navigation::move_focus(ctx, direction)
        {
            ctx.memory_mut(|memory| {
                // Cardinal navigation needs an initial anchor.
                memory.move_focus(if memory.focused().is_some() {
                    direction
                } else {
                    egui::FocusDirection::Next
                });
            });
        }
        ctx.request_repaint();
    }

    pub(super) fn repeat_navigation_input(&mut self, ctx: &egui::Context, now: Instant) {
        let Some(repeat) = &self.navigation_repeat else {
            return;
        };
        let enabled = !self.platform_suspended
            && (!matches!(self.screen, Screen::Gameplay(_)) || self.overlay_active())
            && !self.text_input_active
            && self.physical_editor.as_ref().is_none_or(|editor| {
                editor.listening.is_none() && editor.quiet_until.is_none_or(|until| now >= until)
            })
            && ctx.input(|input| {
                input.focused
                    && !input.pointer.any_down()
                    && !input.pointer.any_pressed()
                    && !input
                        .events
                        .iter()
                        .any(|event| matches!(event, egui::Event::Key { pressed: true, .. }))
            });
        if !enabled
            || repeat.page != self.motion_page()
            || repeat.overlay != self.overlay_active()
            || repeat.popup != crate::focus_navigation::active_layer(ctx)
        {
            self.navigation_repeat = None;
            return;
        }
        let Some(repeat) = &mut self.navigation_repeat else {
            return;
        };
        let direction = repeat.direction;
        let due = now >= repeat.next;
        if due {
            // One step per fresh frame; a stalled UI never catches up in a burst.
            repeat.next = now + REPEAT_INTERVAL;
        }
        ctx.request_repaint_after(repeat.next.saturating_duration_since(now));
        if due {
            self.move_controller_focus(ctx, direction);
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/physical_controls/navigation_repeat.rs"]
mod tests;
