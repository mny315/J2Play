use super::{
    Duration, Event, FrontendApp, Instant, Key, PhysicalControl, PhysicalTransition, Screen, egui,
};
use std::collections::BTreeSet;

const SLIDERS_ID: &str = "controller-sliders";
const MAX_SLIDERS: usize = 32;
const MAX_REPEAT_SOURCES: usize = 16;
const REPEAT_DELAY: Duration = Duration::from_millis(400);
const REPEAT_INTERVAL: Duration = Duration::from_millis(60);

#[derive(Clone, Copy)]
struct SliderTarget {
    id: egui::Id,
    rect: egui::Rect,
    layer: egui::LayerId,
}

#[derive(Clone, Default)]
struct SliderTargets {
    pass: u64,
    targets: Vec<SliderTarget>,
}

struct ControllerSlider<'a>(egui::Slider<'a>);

pub(crate) fn controller_slider(slider: egui::Slider<'_>) -> impl egui::Widget + '_ {
    // All product sliders use integer percentages or frame rates.
    ControllerSlider(slider.step_by(1.0))
}

impl egui::Widget for ControllerSlider<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let response = self.0.ui(ui);
        crate::focus_ring::track(ui, &response, 12.0);
        let pass = ui.ctx().cumulative_pass_nr();
        ui.ctx().data_mut(|data| {
            let sliders = data.get_temp_mut_or_default::<SliderTargets>(egui::Id::new(SLIDERS_ID));
            if sliders.pass != pass {
                sliders.pass = pass;
                sliders.targets.clear();
            }
            if response.enabled() && sliders.targets.len() < MAX_SLIDERS {
                sliders.targets.push(SliderTarget {
                    id: response.id,
                    rect: response.rect,
                    layer: response.layer_id,
                });
            }
        });
        response
    }
}

pub(super) fn focused_slider(ctx: &egui::Context) -> Option<(egui::Id, bool)> {
    let id = ctx.memory(egui::Memory::focused)?;
    let response = ctx.read_response(id)?;
    if !response.enabled() || !ctx.memory(|memory| memory.allows_interaction(response.layer_id)) {
        return None;
    }
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|data| {
        let sliders = data.get_temp_mut_or_default::<SliderTargets>(egui::Id::new(SLIDERS_ID));
        // Logic consumes the preceding paint; old pages must not accept edits.
        if pass.saturating_sub(sliders.pass) > 1 {
            return None;
        }
        sliders
            .targets
            .iter()
            .find(|target| {
                target.layer == response.layer_id
                    && (target.id == id
                        // A slider's numeric field has its own focus ID. Both
                        // parts belong to the same adjustment, even immediately
                        // after directional focus changes at the end of a pass.
                        || (response.sense.senses_drag()
                            && target.rect.contains_rect(response.rect)))
            })
            // Product sliders are horizontal, with the numeric field after
            // the rail. egui edits that field with Up/Down, reserving Left/Right
            // for its text cursor; controller adjustment keeps one convention.
            .map(|target| (id, response.rect.left() > target.rect.left() + 1.0))
    })
}

pub(crate) struct SliderRepeat {
    focus: egui::Id,
    key: Key,
    sources: BTreeSet<(u32, PhysicalControl)>,
    next: Instant,
}

pub(crate) fn press_ui_key(ctx: &egui::Context, key: Key) {
    ctx.input_mut(|input| {
        for pressed in [true, false] {
            input.events.push(Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
    });
    ctx.request_repaint();
}

impl FrontendApp {
    pub(super) fn adjust_slider_with_controller(
        &mut self,
        ctx: &egui::Context,
        change: PhysicalTransition,
        key: Key,
    ) -> bool {
        if !matches!(self.screen, Screen::Settings(_) | Screen::AppSettings(_))
            || self.overlay_active()
        {
            return false;
        }
        let Some((focus, numeric)) = focused_slider(ctx) else {
            return false;
        };
        let key = match (key, numeric) {
            (Key::ArrowLeft, true) => Key::ArrowDown,
            (Key::ArrowRight, true) => Key::ArrowUp,
            _ => key,
        };
        ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        let source = (change.device, change.control);
        if let Some(repeat) = &mut self.slider_repeat
            && repeat.focus == focus
            && repeat.key == key
        {
            if repeat.sources.len() < MAX_REPEAT_SOURCES {
                repeat.sources.insert(source);
            }
        } else {
            self.slider_repeat = Some(SliderRepeat {
                focus,
                key,
                sources: BTreeSet::from([source]),
                next: Instant::now() + REPEAT_DELAY,
            });
            press_ui_key(ctx, key);
        }
        ctx.request_repaint_after(REPEAT_DELAY);
        true
    }

    pub(super) fn release_slider_input(&mut self, change: PhysicalTransition) {
        if !change.pressed
            && let Some(repeat) = &mut self.slider_repeat
        {
            repeat.sources.remove(&(change.device, change.control));
            if repeat.sources.is_empty() {
                self.slider_repeat = None;
            }
        }
    }

    pub(super) fn repeat_slider_input(&mut self, ctx: &egui::Context, now: Instant) {
        let enabled = !self.platform_suspended
            && !self.overlay_active()
            && matches!(self.screen, Screen::Settings(_) | Screen::AppSettings(_))
            && ctx.input(|input| input.focused && !input.pointer.any_down());
        let Some(repeat) = &mut self.slider_repeat else {
            return;
        };
        let focused = focused_slider(ctx).map(|(id, _)| id);
        if !enabled || focused != Some(repeat.focus) {
            self.slider_repeat = None;
            return;
        }
        if now >= repeat.next {
            // At most one step per paint; a delayed UI never catches up in a burst.
            press_ui_key(ctx, repeat.key);
            repeat.next = now + REPEAT_INTERVAL;
        }
        ctx.request_repaint_after(repeat.next.saturating_duration_since(now));
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/physical_controls/sliders.rs"]
mod tests;
