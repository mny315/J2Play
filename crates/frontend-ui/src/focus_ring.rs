use super::{MaterialTheme, egui};
use std::time::Instant;

const TARGET_ID: &str = "material-focus-target";
const PAINTED_ID: &str = "material-painted-focus";
pub(super) const OUTSET: i8 = 4;

pub(super) fn restore_without_scroll(ctx: &egui::Context, id: egui::Id) {
    ctx.memory_mut(|memory| memory.request_focus(id));
    // The saved viewport already contains this control. Treat it as the
    // previous selection so track() does not issue a new scroll request.
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(PAINTED_ID), id));
}

#[derive(Clone, Copy)]
struct FocusTarget {
    id: egui::Id,
    rect: egui::Rect,
    clip: egui::Rect,
    layer: egui::LayerId,
    radius: f32,
    pass: u64,
}

/// Register only geometry from the current paint pass, never an outgoing page.
/// Returns whether selection or viewport changes need contextual scroll anchors.
pub(super) fn track(ui: &egui::Ui, response: &egui::Response, radius: f32) -> bool {
    super::focus_navigation::register(ui, response);
    if !response.has_focus() || !response.enabled() {
        return false;
    }
    let previous = ui
        .ctx()
        .data(|data| data.get_temp::<egui::Id>(egui::Id::new(PAINTED_ID)));
    let resized = ui.ctx().data(|data| {
        data.get_temp::<FocusTarget>(egui::Id::new(TARGET_ID))
            .is_some_and(|target| {
                target.id == response.id
                    && target.layer == response.layer_id
                    && target.clip.size() != ui.clip_rect().size()
            })
    });
    let reveal = previous != Some(response.id) || resized;
    if reveal {
        ui.scroll_to_rect(response.rect.expand(f32::from(OUTSET)), None);
    }
    let target = FocusTarget {
        id: response.id,
        rect: response.rect,
        clip: ui.clip_rect(),
        layer: response.layer_id,
        radius,
        pass: ui.ctx().cumulative_pass_nr(),
    };
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::new(TARGET_ID), target));
    reveal
}

/// Anchor a settings page above its footer, without revealing the ring on touch.
pub(super) fn track_page_start(
    ui: &egui::Ui,
    response: &egui::Response,
    radius: f32,
    page_top: egui::Pos2,
    focus_requested: &mut bool,
) {
    if response.enabled() && std::mem::take(focus_requested) {
        response.request_focus();
    }
    if track(ui, response, radius) {
        ui.scroll_to_rect(
            egui::Rect::from_min_max(page_top, response.rect.max),
            Some(egui::Align::TOP),
        );
    }
}

/// Retains the native button's sizing, accessibility and activation behavior.
pub(super) struct FocusButton(egui::Button<'static>);

impl From<egui::Button<'static>> for FocusButton {
    fn from(button: egui::Button<'static>) -> Self {
        Self(button)
    }
}

impl FocusButton {
    pub(super) fn min_size(mut self, size: egui::Vec2) -> Self {
        self.0 = self.0.min_size(size);
        self
    }

    pub(super) fn wrap(mut self) -> Self {
        self.0 = self.0.wrap();
        self
    }

    pub(super) fn truncate(mut self) -> Self {
        self.0 = self.0.truncate();
        self
    }
}

impl egui::Widget for FocusButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let response = self.0.ui(ui);
        track(ui, &response, 24.0);
        response
    }
}

#[derive(Default)]
pub(super) struct FocusRing {
    navigation_visible: bool,
    controller_connected: bool,
    target: Option<FocusTarget>,
    origin: Option<(egui::Rect, f32)>,
    motion_clip: Option<egui::Rect>,
    started: Option<Instant>,
    entering: bool,
}

impl FocusRing {
    pub(super) fn set_controller_connected(&mut self, connected: bool) {
        self.controller_connected = connected;
    }

    pub(super) fn controller_connected(&self) -> bool {
        self.controller_connected
    }

    pub(super) fn restore_controller_focus(&self, ctx: &egui::Context) {
        if self.controller_connected
            && ctx.input(|input| input.focused)
            && ctx.memory(egui::Memory::focused).is_none()
        {
            super::focus_navigation::restore_focus(ctx, self.target.map(|target| target.id));
        }
    }

    pub(super) fn hide(&mut self) {
        self.navigation_visible = false;
    }

    pub(super) fn navigate(&mut self) -> bool {
        let first = !self.navigation_visible && !self.controller_connected;
        if first {
            *self = Self {
                navigation_visible: true,
                controller_connected: self.controller_connected,
                ..Self::default()
            };
        }
        self.navigation_visible = true;
        first
    }

    pub(super) fn observe_input(&mut self, ctx: &egui::Context, capturing: bool) {
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::PointerButton { pressed: true, .. }
                    | egui::Event::Touch {
                        phase: egui::TouchPhase::Start,
                        ..
                    }
                    | egui::Event::WindowFocused(false) => self.hide(),
                    egui::Event::Key {
                        key:
                            egui::Key::Tab
                            | egui::Key::ArrowUp
                            | egui::Key::ArrowDown
                            | egui::Key::ArrowLeft
                            | egui::Key::ArrowRight
                            | egui::Key::Enter
                            | egui::Key::Space,
                        pressed: true,
                        ..
                    } if !capturing => {
                        self.navigate();
                    }
                    _ => {}
                }
            }
        });
    }

    fn sample(&self, now: Instant) -> Option<(egui::Rect, f32, f32)> {
        let target = self.target?;
        let progress = self.started.map_or(1.0, |started| {
            (now.saturating_duration_since(started).as_secs_f32()
                / super::ui_motion::UI_MOTION_SECONDS)
                .clamp(0.0, 1.0)
        });
        let eased = 1.0 - (1.0 - progress).powi(3);
        let (origin, radius) = self.origin.unwrap_or((target.rect, target.radius));
        Some((
            origin.lerp_towards(&target.rect, eased),
            egui::lerp(radius..=target.radius, eased),
            if self.entering { eased } else { 1.0 },
        ))
    }

    fn sync(&mut self, target: Option<FocusTarget>, now: Instant) {
        let Some(target) = target else {
            *self = Self {
                navigation_visible: self.navigation_visible,
                controller_connected: self.controller_connected,
                ..Self::default()
            };
            return;
        };
        if self
            .target
            .is_none_or(|old| old.id != target.id || old.layer != target.layer)
        {
            self.motion_clip = self
                .target
                .filter(|old| old.layer == target.layer)
                .map(|old| self.motion_clip.unwrap_or(old.clip).union(target.clip));
            let previous = self
                .target
                .filter(|old| old.layer == target.layer)
                .and_then(|_| self.sample(now));
            self.origin = previous.map(|(rect, radius, _)| (rect, radius));
            self.entering = previous.is_none();
            self.started = Some(now);
        }
        // Scrolling and resizing update geometry without restarting the motion.
        self.target = Some(target);
        if self.started.is_some_and(|started| {
            now.saturating_duration_since(started).as_secs_f32()
                >= super::ui_motion::UI_MOTION_SECONDS
        }) {
            self.started = None;
            self.origin = None;
            self.motion_clip = None;
        }
    }

    pub(super) fn paint(
        &mut self,
        ctx: &egui::Context,
        theme: &MaterialTheme,
        active: bool,
        safe_rect: egui::Rect,
    ) {
        let target = ctx
            .data(|data| data.get_temp::<FocusTarget>(egui::Id::new(TARGET_ID)))
            .filter(|target| {
                active
                    && ctx.input(|input| input.focused)
                    && target.pass == ctx.cumulative_pass_nr()
                    && ctx.memory(|memory| {
                        memory.has_focus(target.id) && memory.allows_interaction(target.layer)
                    })
            });
        let now = Instant::now();
        self.sync(target, now);
        let Some(target) = target else {
            ctx.data_mut(|data| data.remove::<egui::Id>(egui::Id::new(PAINTED_ID)));
            return;
        };
        ctx.data_mut(|data| data.insert_temp(egui::Id::new(PAINTED_ID), target.id));
        if !self.navigation_visible && !self.controller_connected {
            return;
        }
        if let Some((rect, radius, opacity)) = self.sample(now) {
            let outset = f32::from(OUTSET);
            let painter = ctx
                .layer_painter(target.layer)
                .with_clip_rect(self.motion_clip.unwrap_or(target.clip).intersect(safe_rect));
            // The gap remains visible even on filled primary buttons.
            for (width, color) in [(4.5, theme.background), (2.5, theme.primary)] {
                painter.rect_stroke(
                    rect.expand(outset),
                    radius + outset,
                    egui::Stroke::new(width, color.gamma_multiply(opacity)),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if self.started.is_some() {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/focus_ring/mod.rs"]
mod tests;
