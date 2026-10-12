use super::{Rect, Vec2, VirtualControlGeometry};
use std::time::{Duration, Instant};

const DURATION: Duration = Duration::from_millis(240);
const REQUEST_TIMEOUT: Duration = Duration::from_millis(600);

#[derive(Clone, Copy)]
struct Presentation {
    viewport: Rect,
    landscape: bool,
    fullscreen: bool,
    canvas: Option<Rect>,
    controls: Option<VirtualControlGeometry>,
    panel: Option<Rect>,
    toolbar: f32,
}

/// Only bounded host geometry is retained, never guest frames or input events.
#[derive(Default)]
pub(super) struct GameplayTransition {
    last: Option<Presentation>,
    from: Option<Presentation>,
    requested: Option<Instant>,
    started: Option<Instant>,
    progress: f32,
}

impl GameplayTransition {
    pub(super) fn request(&mut self) {
        if self.last.is_some() {
            self.requested = Some(Instant::now());
        }
    }

    pub(super) fn busy(&self) -> bool {
        self.requested.is_some() || self.started.is_some()
    }

    pub(super) fn begin(
        &mut self,
        viewport: Rect,
        fullscreen: bool,
        landscape: bool,
        now: Instant,
    ) -> bool {
        let changed = self.last.is_some_and(|last| {
            last.fullscreen != fullscreen
                || last.landscape != landscape
                || (last.viewport.width() > last.viewport.height())
                    != (viewport.width() > viewport.height())
        });
        let start = self.requested.is_some() && changed;
        if start {
            self.from = self.last.map(|mut last| {
                if (last.viewport.width() > last.viewport.height())
                    != (viewport.width() > viewport.height())
                {
                    last.remap(viewport);
                }
                last
            });
            self.started = Some(now);
            self.requested = None;
        } else if self
            .requested
            .is_some_and(|at| now.saturating_duration_since(at) >= REQUEST_TIMEOUT)
        {
            self.requested = None;
        }
        self.progress = self.started.map_or(1.0, |at| {
            let t = (now.saturating_duration_since(at).as_secs_f32() / DURATION.as_secs_f32())
                .clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        });
        if self.progress >= 1.0 {
            self.started = None;
            self.from = None;
        }
        let target_toolbar = if fullscreen { 0.0 } else { 1.0 };
        let toolbar = self.from.map_or(target_toolbar, |from| {
            egui_lerp(from.toolbar, target_toolbar, self.progress)
        });
        self.last = Some(Presentation {
            viewport,
            landscape,
            fullscreen,
            canvas: None,
            controls: None,
            panel: None,
            toolbar,
        });
        start
    }

    pub(super) fn toolbar(&self) -> f32 {
        self.last.map_or(1.0, |last| last.toolbar)
    }

    pub(super) fn canvas(&mut self, target: Rect) -> Rect {
        let rect = self
            .from
            .and_then(|from| from.canvas)
            .filter(|_| target.is_positive())
            .map_or(target, |from| {
                // Interpolate one dimension so the live guest frame is never stretched.
                let height = egui_lerp(from.height(), target.height(), self.progress);
                let size = Vec2::new(height * target.aspect_ratio(), height);
                Rect::from_center_size(from.center().lerp(target.center(), self.progress), size)
            });
        let rect = if self.started.is_some() {
            self.last.map_or(rect, |last| fit_rect(rect, last.viewport))
        } else {
            target
        };
        if let Some(last) = &mut self.last {
            last.canvas = Some(rect);
        }
        rect
    }

    pub(super) fn panel(&mut self, target: Option<Rect>) -> Option<Rect> {
        let panel = match (self.from.and_then(|from| from.panel), target) {
            (Some(from), Some(target)) => Some(from.lerp_towards(&target, self.progress)),
            (_, target) => target,
        };
        if let Some(last) = &mut self.last {
            last.panel = panel;
        }
        panel
    }

    pub(super) fn controls(&mut self, target: VirtualControlGeometry) -> VirtualControlGeometry {
        let geometry = self
            .from
            .and_then(|from| from.controls)
            .map_or(target, |from| blend_controls(from, target, self.progress));
        if let Some(last) = &mut self.last {
            last.controls = Some(geometry);
        }
        geometry
    }
}

fn egui_lerp(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

fn fit_rect(rect: Rect, viewport: Rect) -> Rect {
    if !rect.is_positive() || !viewport.is_positive() {
        return Rect::from_min_size(viewport.min, Vec2::ZERO);
    }
    let scale = (viewport.width() / rect.width())
        .min(viewport.height() / rect.height())
        .min(1.0);
    let size = rect.size() * scale.max(0.0);
    let minimum = viewport.min + size / 2.0;
    let maximum = (viewport.max - size / 2.0).max(minimum);
    let center = rect.center().clamp(minimum, maximum);
    Rect::from_center_size(center, size)
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/gameplay_transition/mod.rs"]
mod tests;

impl Presentation {
    fn remap(&mut self, viewport: Rect) {
        let scale = (viewport.width() / self.viewport.width().max(1.0))
            .min(viewport.height() / self.viewport.height().max(1.0));
        let point = |p| viewport.center() + (p - self.viewport.center()) * scale;
        let rect = |r: Rect| Rect::from_center_size(point(r.center()), r.size() * scale);
        self.canvas = self.canvas.map(rect);
        self.panel = self.panel.map(rect);
        if let Some(controls) = &mut self.controls {
            controls.direction_pad_center = point(controls.direction_pad_center);
            controls.direction_button_size *= scale;
            controls.fire_center = point(controls.fire_center);
            controls.fire_size *= scale;
            controls.left_soft_key_center = point(controls.left_soft_key_center);
            controls.left_soft_key_size *= scale;
            controls.right_soft_key_center = point(controls.right_soft_key_center);
            controls.right_soft_key_size *= scale;
            for (center, size) in &mut controls.number_keys {
                *center = point(*center);
                *size *= scale;
            }
        }
        self.viewport = viewport;
    }
}

fn blend_controls(
    from: VirtualControlGeometry,
    to: VirtualControlGeometry,
    t: f32,
) -> VirtualControlGeometry {
    VirtualControlGeometry {
        direction_pad_center: from.direction_pad_center.lerp(to.direction_pad_center, t),
        direction_button_size: egui_lerp(from.direction_button_size, to.direction_button_size, t),
        fire_center: from.fire_center.lerp(to.fire_center, t),
        fire_size: egui_lerp(from.fire_size, to.fire_size, t),
        left_soft_key_center: from.left_soft_key_center.lerp(to.left_soft_key_center, t),
        left_soft_key_size: egui_lerp(from.left_soft_key_size, to.left_soft_key_size, t),
        right_soft_key_center: from.right_soft_key_center.lerp(to.right_soft_key_center, t),
        right_soft_key_size: egui_lerp(from.right_soft_key_size, to.right_soft_key_size, t),
        number_keys: std::array::from_fn(|i| {
            (
                from.number_keys[i].0.lerp(to.number_keys[i].0, t),
                egui_lerp(from.number_keys[i].1, to.number_keys[i].1, t),
            )
        }),
        corner_radius_percent: to.corner_radius_percent,
    }
}

impl super::FrontendApp {
    pub(super) fn begin_gameplay_transition(&mut self, ui: &super::egui::Ui, fullscreen: bool) {
        self.gameplay_transition.begin(
            ui.max_rect(),
            fullscreen,
            ui.available_width() > ui.available_height(),
            super::Instant::now(),
        );
        if self.gameplay_transition.busy() && !self.platform_suspended {
            ui.ctx().request_repaint();
        }
    }
}
