use super::{
    FrontendApp, MaterialTheme, Rect, SETTINGS_TOUCH_TARGET_HEIGHT, Vec2, apply_settings_style,
    egui, material_outlined_button, material_primary_button, material_text_button,
};

const BUTTON_MIN_WIDTH: f32 = 88.0;
const BUTTON_MAX_WIDTH: f32 = 160.0;
const BUTTON_GAP: f32 = 8.0;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) enum SettingsAction {
    Reset,
    Cancel,
    Save,
}

/// The same geometry drives the panel height, button painting and hit targets.
pub(super) struct SettingsActions {
    pub(super) reset: Option<Rect>,
    pub(super) cancel: Rect,
    pub(super) save: Rect,
}

impl SettingsActions {
    pub(super) fn new(ui: &egui::Ui, available_width: f32, with_reset: bool) -> Self {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        let minimum = ["Reset", "Cancel", "Save"]
            .into_iter()
            .map(|label| {
                ui.painter()
                    .layout_no_wrap(
                        tr.text(label),
                        egui::FontId::proportional(crate::SETTINGS_BODY_TEXT_SIZE),
                        egui::Color32::WHITE,
                    )
                    .size()
                    .x
                    .ceil()
                    + 26.0 // Settings padding plus stroke/rounding allowance.
            })
            .fold(BUTTON_MIN_WIDTH, f32::max);
        let width = available_width.max(0.0);
        let columns = if width >= 3.0 * minimum + 2.0 * BUTTON_GAP {
            3.0
        } else if width >= 2.0 * minimum + BUTTON_GAP {
            2.0
        } else {
            1.0
        };
        // Reserve the Reset column even on settings pages, so Cancel and Save
        // stay in the same place when opening an editor at the same width.
        let button_width =
            ((width - (columns - 1.0) * BUTTON_GAP) / columns).min(BUTTON_MAX_WIDTH.max(minimum));
        let size = Vec2::new(button_width, SETTINGS_TOUCH_TARGET_HEIGHT);
        let row_step = SETTINGS_TOUCH_TARGET_HEIGHT + BUTTON_GAP;
        let cancel_y = if with_reset && columns < 3.0 {
            row_step
        } else {
            0.0
        };
        let save_y = cancel_y + if columns < 2.0 { row_step } else { 0.0 };
        let cancel_x = width
            - button_width
            - if columns >= 2.0 {
                button_width + BUTTON_GAP
            } else {
                0.0
            };
        Self {
            reset: with_reset.then(|| Rect::from_min_size(egui::Pos2::ZERO, size)),
            cancel: Rect::from_min_size(egui::pos2(cancel_x, cancel_y), size),
            save: Rect::from_min_size(egui::pos2(width - button_width, save_y), size),
        }
    }

    pub(super) fn height(&self) -> f32 {
        self.save.bottom()
    }

    fn draw(&self, ui: &mut egui::Ui, theme: &MaterialTheme) -> Option<SettingsAction> {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        apply_settings_style(ui);
        let (area, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), self.height()),
            egui::Sense::hover(),
        );
        let mut clicked = None;
        for (action, rect) in [
            (SettingsAction::Reset, self.reset),
            (SettingsAction::Cancel, Some(self.cancel)),
            (SettingsAction::Save, Some(self.save)),
        ] {
            let Some(rect) = rect else { continue };
            let button = match action {
                SettingsAction::Reset => material_text_button(theme, tr.text("Reset")),
                SettingsAction::Cancel => material_outlined_button(theme, tr.text("Cancel")),
                SettingsAction::Save => material_primary_button(theme, tr.text("Save")),
            };
            let response = ui
                .new_child(
                    egui::UiBuilder::new()
                        .id_salt(action)
                        .max_rect(rect.translate(area.min.to_vec2()))
                        .layout(egui::Layout::centered_and_justified(
                            egui::Direction::TopDown,
                        )),
                )
                .add(button.truncate());
            if response.clicked() {
                clicked = Some(action);
            }
        }
        clicked
    }
}

impl FrontendApp {
    pub(super) fn draw_settings_actions(
        &self,
        ui: &mut egui::Ui,
        actions: &SettingsActions,
    ) -> Option<SettingsAction> {
        actions.draw(ui, &self.material_theme)
    }

    pub(super) fn apply_settings_action(&mut self, action: Option<SettingsAction>) {
        let Some(action) = action else {
            return;
        };
        if self.physical_editor.is_some() {
            match action {
                SettingsAction::Reset => self.reset_physical_editor(),
                SettingsAction::Cancel => self.close_physical_editor(false),
                SettingsAction::Save => self.close_physical_editor(true),
            }
        } else if let Some(editor) = self.screen.control_editor_mut() {
            match action {
                SettingsAction::Reset => editor.reset_layout(),
                SettingsAction::Cancel => self.screen.discard_control_editor(),
                SettingsAction::Save => self.screen.commit_control_editor(),
            }
        } else {
            match action {
                SettingsAction::Reset => {}
                SettingsAction::Cancel => self.cancel_settings_screen(),
                SettingsAction::Save => self.save_settings_screen(),
            }
        }
    }
}
