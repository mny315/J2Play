//! Finite item fades and rearrangement in content coordinates, independent of scrolling.

use super::{egui, library_view::LibraryLayout, ui_motion::UI_MOTION_SECONDS};
use std::collections::{HashMap, HashSet};

struct Item {
    from: egui::Pos2,
    target: egui::Pos2,
    moved_at: f64,
    appeared_at: Option<f64>,
    seen: bool,
}

impl Item {
    fn position(&self, now: f64) -> egui::Pos2 {
        self.from.lerp(self.target, progress(now, self.moved_at))
    }
}

fn progress(now: f64, started: f64) -> f32 {
    // Both values are bounded UI times, and the result is clamped to one animation.
    #[allow(clippy::cast_possible_truncation)]
    let t = ((now - started) / f64::from(UI_MOTION_SECONDS)).clamp(0.0, 1.0) as f32;
    1.0 - (1.0 - t).powi(3)
}

#[derive(Default)]
pub(super) struct LibraryMotion {
    entries_current: bool,
    known: HashSet<egui::Id>,
    added: HashSet<egui::Id>,
    items: HashMap<egui::Id, Item>,
    layout: Option<LibraryLayout>,
}

impl LibraryMotion {
    pub(super) fn invalidate_entries(&mut self) {
        self.entries_current = false;
    }

    pub(super) fn forget_item(&mut self, id: egui::Id) {
        self.items.remove(&id);
    }

    pub(super) fn begin(&mut self, ids: impl Iterator<Item = egui::Id>, layout: LibraryLayout) {
        // The library bounds the IDs; only visible rows and neighbors retain geometry.
        // Membership changes only on import/removal. Ordinary paints and scrolling
        // must not enumerate or hash all library entries behind virtualized rows.
        self.added.clear();
        if !self.entries_current {
            self.added
                .extend(ids.take(frontend_core::MAX_LIBRARY_ENTRIES));
            self.known.retain(|id| self.added.remove(id));
            self.known.extend(self.added.iter().copied());
            self.entries_current = true;
        }
        if self.layout != Some(layout) {
            self.items.clear();
            self.layout = Some(layout);
        }
        self.items.retain(|id, item| {
            item.seen = false;
            self.known.contains(id)
        });
    }

    pub(super) fn item(
        &mut self,
        id: egui::Id,
        target: egui::Rect,
        visible_index: usize,
        now: f64,
    ) -> (egui::Rect, f32, bool) {
        let added = self.added.contains(&id);
        let item = self.items.entry(id).or_insert_with(|| Item {
            from: target.min,
            target: target.min,
            moved_at: now - f64::from(UI_MOTION_SECONDS),
            // A short, capped stagger on initial display and newly imported entries.
            appeared_at: added
                .then(|| now + f64::from(u8::try_from(visible_index.min(4)).unwrap_or(4)) * 0.025),
            seen: true,
        });
        if item.target != target.min {
            item.from = item.position(now);
            item.target = target.min;
            item.moved_at = now;
        }
        item.seen = true;
        let opacity = item.appeared_at.map_or(1.0, |start| progress(now, start));
        if opacity >= 1.0 {
            item.appeared_at = None;
        }
        let moving = now - item.moved_at < f64::from(UI_MOTION_SECONDS);
        (
            egui::Rect::from_min_size(item.position(now), target.size()),
            0.05 + 0.95 * opacity,
            moving || item.appeared_at.is_some(),
        )
    }

    pub(super) fn finish(&mut self) {
        self.items.retain(|_, item| item.seen);
        self.added.clear();
    }

    pub(super) fn settle(&mut self) {
        self.items.clear();
        self.added.clear();
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/library_motion.rs"]
mod tests;
