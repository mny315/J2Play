//! Shared selection and group actions for chosen library games.

use super::{FrontendApp, MaterialTheme, egui};
use std::collections::BTreeSet;

const HOLD_SECONDS: f64 = 0.5;
const HOLD_SLOP: f32 = 8.0;

#[derive(Default)]
pub(super) struct LibrarySelection {
    pub active: bool,
    pub ids: BTreeSet<String>,
    hold: Option<Hold>,
    hold_consumed: bool,
    haptic_pending: bool,
}

struct Hold {
    id: egui::Id,
    origin: egui::Pos2,
    started: f64,
}

impl LibrarySelection {
    pub fn clear(&mut self) {
        self.active = false;
        self.ids.clear();
        self.cancel_hold();
        self.haptic_pending = false;
    }

    pub fn cancel_hold(&mut self) {
        self.hold = None;
    }

    pub fn begin(&mut self, ui: &egui::Ui) {
        // Android sends PointerGone in the release frame. Keep the consumed
        // gesture separate from the pending hold so cancellation, movement or
        // disappearing pointer geometry cannot turn its release into a tap.
        if ui.input(|input| {
            input.pointer.primary_pressed()
                || (!input.pointer.primary_down() && !input.pointer.any_released())
        }) {
            self.hold_consumed = false;
        }
        let cancel = !ui.is_enabled()
            || ui.input(|input| {
                !input.focused
                    || input.events.iter().any(|event| {
                        matches!(
                            event,
                            egui::Event::PointerCancelled
                                | egui::Event::PointerGone
                                | egui::Event::WindowFocused(false)
                        )
                    })
                    || (!input.pointer.primary_down() && !input.pointer.any_released())
                    || self.hold.as_ref().is_some_and(|hold| {
                        input
                            .pointer
                            .interact_pos()
                            .is_none_or(|pos| pos.distance(hold.origin) > HOLD_SLOP)
                    })
            });
        if cancel {
            self.cancel_hold();
        }
    }

    pub fn toggle(&mut self, id: &str) {
        self.active = true;
        self.haptic_pending = true;
        if !self.ids.remove(id) {
            self.ids.insert(id.to_owned());
        }
    }

    pub fn take_haptic(&mut self) -> bool {
        std::mem::take(&mut self.haptic_pending)
    }

    /// Returns a launch only for an ordinary click outside selection mode.
    /// Track primary-pointer holds too: some touchscreens report mouse events.
    pub fn interact(&mut self, response: &egui::Response, id: &str, available: bool) -> bool {
        if !response.enabled() || self.hold_consumed {
            return false;
        }
        let ctx = &response.ctx;
        let (pressed, down, pos, now) = ctx.input(|input| {
            (
                input.pointer.primary_pressed(),
                input.pointer.primary_down(),
                input.pointer.interact_pos(),
                input.time,
            )
        });
        if available && pressed && response.is_pointer_button_down_on() {
            self.hold = pos.map(|origin| Hold {
                id: response.id,
                origin,
                started: now,
            });
        }
        let mut held = false;
        if let Some(hold) = &mut self.hold
            && hold.id == response.id
            && down
        {
            let remaining = HOLD_SECONDS - (now - hold.started);
            if remaining <= 0.0 {
                self.hold_consumed = true;
                held = true;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(remaining));
            }
        }
        if held || (available && response.clicked_by(egui::PointerButton::Secondary)) {
            self.toggle(id);
            ctx.request_repaint();
            return false;
        }
        if response.clicked() {
            if self.active {
                self.toggle(id);
                ctx.request_repaint();
            } else {
                return true;
            }
        }
        false
    }
}

pub(super) fn selection_button(
    ui: &mut egui::Ui,
    theme: &MaterialTheme,
    selected: bool,
    description: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        super::material_tonal_button(theme, "").min_size(egui::vec2(48.0, 48.0)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            selected,
            description,
        )
    });
    let rect = egui::Rect::from_center_size(response.rect.center(), egui::Vec2::splat(22.0));
    let color = if selected {
        theme.primary
    } else {
        theme.on_surface_variant
    };
    ui.painter().rect_stroke(
        rect,
        5.0,
        egui::Stroke::new(2.0, color),
        egui::StrokeKind::Inside,
    );
    if selected {
        ui.painter().line_segment(
            [
                rect.min + egui::vec2(5.0, 11.0),
                rect.min + egui::vec2(9.0, 15.0),
            ],
            egui::Stroke::new(2.0, color),
        );
        ui.painter().line_segment(
            [
                rect.min + egui::vec2(9.0, 15.0),
                rect.min + egui::vec2(17.0, 7.0),
            ],
            egui::Stroke::new(2.0, color),
        );
    }
    response.on_hover_text(description)
}

impl FrontendApp {
    pub(super) fn flush_library_selection_haptic(&mut self, ui: &egui::Ui) {
        if self.library_selection.take_haptic()
            && ui.is_enabled()
            && !self.platform_suspended
            && ui.input(|input| input.focused)
            && self.app_settings.vibration.enabled
        {
            self.request_strength_haptic(self.app_settings.vibration.strength_percent);
        }
    }

    pub(super) fn draw_library_selection_bar(&mut self, ui: &mut egui::Ui) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        let mut actions = Vec::new();
        ui.scope(|ui| {
            super::apply_settings_style(ui);
            ui.label(tr.format(
                "Selected: {count}",
                &[("count", &self.library_selection.ids.len().to_string())],
            ));
            ui.horizontal_wrapped(|ui| {
                let selected = !self.library_selection.ids.is_empty();
                let move_games = ui
                    .add_enabled(
                        selected && self.library_folders.available,
                        super::material_primary_button(&self.material_theme, tr.text("Move")),
                    )
                    .on_hover_text(tr.text("Move to folder"));
                let delete_games = ui.add_enabled(
                    selected,
                    super::material_error_outlined_button(
                        &self.material_theme,
                        tr.text("Delete games"),
                    )
                    .wrap(),
                );
                let cancel = ui
                    .add_sized(
                        [48.0, 48.0],
                        super::material_tonal_button(&self.material_theme, "×"),
                    )
                    .on_hover_text(tr.text("Cancel"));
                cancel.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        cancel.enabled(),
                        tr.text("Cancel"),
                    )
                });
                for action in [&move_games, &delete_games, &cancel] {
                    if action.enabled() {
                        actions.push(action.id);
                    }
                }
                if cancel.clicked() {
                    self.library_selection.clear();
                } else if delete_games.clicked() {
                    self.open_delete_selected_games();
                } else if move_games.clicked() {
                    self.open_move_to_folder();
                }
            });
        });
        ui.ctx().data_mut(|data| {
            data.insert_temp(egui::Id::new(super::library_folders::FOLDER_FOCUS), actions)
        });
    }

    pub(super) fn open_delete_selected_games(&mut self) {
        let entry_ids: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| self.library_selection.ids.contains(entry.id()))
            .map(|entry| entry.id().to_owned())
            .collect();
        if !entry_ids.is_empty() {
            self.data_action = Some(super::DataAction {
                entry_ids,
                kind: super::DataActionKind::Game,
            });
        }
    }

    pub(super) fn library_selection_strength(&self, ui: &egui::Ui, id: &str) -> f32 {
        ui.ctx().animate_bool_with_time(
            egui::Id::new(("library-selected", id)),
            self.library_selection.ids.contains(id),
            super::ui_motion::UI_MOTION_SECONDS,
        )
    }
}
