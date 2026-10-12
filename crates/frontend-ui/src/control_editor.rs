use super::settings::{apply_vibration_slider_value, vibration_slider_value};

mod navigation;
mod painting;
mod preparation;
mod preview;
mod selection;
use crate::switch;

pub(super) use navigation::ControlNavigation;
pub(crate) use preparation::EditorPreparation;

use super::{
    CONTROL_POSITION_UNITS, ControlSelection, ControlTransform, FrontendApp,
    MAX_CONTROL_CORNER_RADIUS_PERCENT, MAX_CONTROL_SIZE_PERCENT, MAX_VIBRATION_STRENGTH_PERCENT,
    MIN_CONTROL_CORNER_RADIUS_PERCENT, MIN_CONTROL_SIZE_PERCENT, RichText,
    SETTINGS_TOUCH_TARGET_HEIGHT, Screen, VibrationSettings, VirtualControlLayout,
    apply_settings_style, egui, material_app_bar_frame, material_card_frame,
    material_outlined_button, material_settings_slider_frame, material_supporting_text,
    settings_slider_width,
};
use crate::physical_controls::controller_slider;
use preview::ControlPreview;
use selection::{selected_transform, selected_transform_mut, selection_label};

const CONTROL_GRID_UNITS: i16 = 250;

impl FrontendApp {
    pub(super) fn open_control_editor(&mut self) {
        if let Screen::AppSettings(settings) = &mut self.screen {
            settings.control_editor = Some(super::ControlEditorState::new(
                &settings.draft.control_defaults(),
                (240, 320),
            ));
            return;
        }
        let Screen::Settings(settings) = &mut self.screen else {
            return;
        };
        if settings.editor_request.is_some() {
            return;
        }
        let plan = match &settings.target {
            super::SettingsTarget::Existing { entry_id } => {
                let Some(entry) = self.entries.iter().find(|entry| entry.id() == entry_id) else {
                    return;
                };
                match self.editor_preparation.request(
                    self.repository.clone(),
                    entry.clone(),
                    settings.draft.clone(),
                ) {
                    Ok(id) => {
                        settings.editor_request = Some((id, settings.draft.device_profile.clone()));
                    }
                    Err(error) => self.show_error("Could not open control editor", &error),
                }
                return;
            }
            super::SettingsTarget::PendingImport(prepared) => prepared.launch_plan(&settings.draft),
        };
        match plan {
            Ok(plan) => {
                if let Screen::Settings(settings) = &mut self.screen {
                    let mut effective = settings.draft.clone();
                    self.app_settings.apply_control_defaults(&mut effective);
                    settings.control_editor = Some(super::ControlEditorState::new(
                        &effective,
                        plan.profile_summary().canvas_dimensions,
                    ));
                }
            }
            Err(error) => self.show_error("Could not open control editor", &error),
        }
    }

    pub(super) fn control_editor_preparing(&self) -> bool {
        matches!(&self.screen, Screen::Settings(settings) if settings.editor_request.is_some())
    }

    pub(super) fn cancel_control_editor_preparation(&mut self) -> bool {
        if let Screen::Settings(settings) = &mut self.screen
            && settings.editor_request.take().is_some()
        {
            self.editor_preparation.retain(None);
            return true;
        }
        false
    }

    pub(super) fn process_control_editor_preparation(&mut self, ctx: &egui::Context) {
        let current = if let Screen::Settings(settings) = &mut self.screen {
            if settings
                .editor_request
                .as_ref()
                .is_some_and(|(_, profile)| *profile != settings.draft.device_profile)
            {
                settings.editor_request = None;
            }
            settings.editor_request.as_ref().map(|(id, _)| *id)
        } else {
            None
        };
        self.editor_preparation.retain(current);
        let Some((id, result)) = self.editor_preparation.poll(ctx) else {
            return;
        };
        if current != Some(id) {
            return;
        }
        let Screen::Settings(settings) = &mut self.screen else {
            return;
        };
        settings.editor_request = None;
        match result {
            Ok(dimensions) => {
                let mut effective = settings.draft.clone();
                self.app_settings.apply_control_defaults(&mut effective);
                settings.control_editor =
                    Some(super::ControlEditorState::new(&effective, dimensions));
            }
            Err(error) => self.show_error("Could not open control editor", &error),
        }
    }

    pub(super) fn draw_control_editor_preparation(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::Translator::from_context(ctx);
        let id = egui::Id::new("control-editor-preparation");
        let response = egui::Modal::new(id)
            .area(egui::Modal::default_area(id).constrain_to(self.safe_content_rect))
            .frame(material_card_frame(&self.material_theme))
            .show(ctx, |ui| {
                ui.set_width((self.safe_content_rect.width() - 64.0).clamp(80.0, 320.0));
                ui.heading(tr.text("Control layout"));
                ui.spinner();
                let cancel = ui.add_sized(
                    [ui.available_width(), SETTINGS_TOUCH_TARGET_HEIGHT],
                    crate::material_text_button(&self.material_theme, tr.text("Cancel")).wrap(),
                );
                cancel.request_focus();
                cancel.clicked()
            });
        if response.inner || response.should_close() {
            self.cancel_control_editor_preparation();
        }
    }

    pub(super) fn draw_control_editor(&mut self, ui: &mut egui::Ui) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        apply_settings_style(ui);
        material_app_bar_frame(&self.material_theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(tr.text("Control layout")).size(20.0).strong());
        });
        ui.add_space(8.0);

        let Some(editor) = self.screen.control_editor_mut() else {
            return;
        };
        preview::orientation_selector(
            ui,
            &self.material_theme,
            self.safe_content_rect.size(),
            self.platform.orientation_control(),
            editor,
        );
        let previous_layout = editor.layout.clone();
        let previous_grid = editor.grid_enabled;
        let preview = ControlPreview::allocate(
            ui,
            self.safe_content_rect.size(),
            self.platform.orientation_control(),
            editor,
        );
        preview.show(ui, &self.material_theme, editor);

        let selected = editor.selected;
        ui.add_space(8.0);
        let mut feedback = draw_control_editor_options(
            ui,
            &self.material_theme,
            &mut editor.layout,
            &mut editor.vibration,
            &mut editor.grid_enabled,
            &mut editor.drag_origin,
            selected,
        );
        if preview.reject_screen_overlap(editor, &previous_layout) {
            feedback.selection_haptic = false;
        }
        if editor.grid_enabled
            && previous_grid
            && editor.vibration.enabled
            && preview.selection_moved(editor, &previous_layout)
        {
            feedback.vibration_strength = Some(editor.vibration.strength_percent);
        }
        // The preview was painted before the drag/options changed the draft.
        // Repaint even when the last slider event is followed by an idle frame.
        if editor.layout != previous_layout || editor.grid_enabled != previous_grid {
            ui.ctx().request_repaint();
        }
        if let Some(strength) = feedback.vibration_strength {
            self.request_strength_haptic(strength);
        } else if feedback.selection_haptic {
            self.request_slider_haptic();
        }
    }
}

fn draw_control_editor_options(
    ui: &mut egui::Ui,
    theme: &super::MaterialTheme,
    layout: &mut VirtualControlLayout,
    vibration: &mut VibrationSettings,
    grid_enabled: &mut bool,
    drag_origin: &mut Option<(ControlTransform, egui::Pos2)>,
    selected: ControlSelection,
) -> EditorOptionsFeedback {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let mut feedback = EditorOptionsFeedback::default();
    material_card_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let transform = selected_transform(layout, selected);
        let visibility = if transform.visible {
            String::new()
        } else {
            format!(" ({})", tr.text("Hidden"))
        };
        ui.label(
            RichText::new(tr.format(
                "Selected: {control}",
                &[(
                    "control",
                    &format!(
                        "{}{visibility}",
                        tr.control(selection_label(layout, selected))
                    ),
                )],
            ))
            .size(18.0)
            .strong(),
        );
        ui.label(material_supporting_text(
            theme,
            format!(
                "X {:+.1}% · Y {:+.1}%",
                f32::from(transform.offset_x) * 100.0 / f32::from(CONTROL_POSITION_UNITS),
                f32::from(transform.offset_y) * 100.0 / f32::from(CONTROL_POSITION_UNITS),
            ),
        ));
        ui.horizontal(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            let width = ((ui.available_width() - spacing) / 2.0).max(0.0);
            let grid_label = if *grid_enabled {
                tr.text("Grid & snap: On")
            } else {
                tr.text("Grid & snap: Off")
            };
            if ui
                .add_sized(
                    [width, SETTINGS_TOUCH_TARGET_HEIGHT],
                    material_outlined_button(theme, grid_label),
                )
                .clicked()
            {
                *grid_enabled = !*grid_enabled;
                *drag_origin = None;
            }
            let visibility_label = if transform.visible {
                tr.text("Hide selected")
            } else {
                tr.text("Show selected")
            };
            if ui
                .add_sized(
                    [width, SETTINGS_TOUCH_TARGET_HEIGHT],
                    material_outlined_button(theme, visibility_label),
                )
                .clicked()
            {
                let selected = selected_transform_mut(layout, selected);
                selected.visible = !selected.visible;
                *drag_origin = None;
            }
        });
        ui.label(RichText::new(tr.text("Size")).strong());
        feedback.selection_haptic |= draw_size_slider(ui, theme, layout, selected);
        ui.label(RichText::new(tr.text("Corner rounding")).strong());
        feedback.selection_haptic |= draw_corner_slider(ui, theme, layout);
        if switch::draw(
            ui,
            theme,
            &mut layout.stick_enabled,
            &tr.text("Use virtual stick"),
        )
        .changed()
        {
            *drag_origin = None;
        }
        switch::draw(
            ui,
            theme,
            &mut layout.two_key_diagonals,
            &tr.text("Two-key diagonals"),
        );
        ui.label(RichText::new(tr.text("Haptic feedback")).strong());
        feedback.vibration_strength = draw_vibration_slider(ui, theme, vibration);
    });
    feedback
}

#[derive(Default)]
struct EditorOptionsFeedback {
    selection_haptic: bool,
    vibration_strength: Option<u8>,
}

fn draw_size_slider(
    ui: &mut egui::Ui,
    theme: &super::MaterialTheme,
    layout: &mut VirtualControlLayout,
    selected: ControlSelection,
) -> bool {
    let mut size_percent = selected_transform(layout, selected).size_percent;
    let mut dragged_change = false;
    material_settings_slider_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().slider_width = settings_slider_width(ui.available_width());
                ui.add(controller_slider(
                    egui::Slider::new(
                        &mut size_percent,
                        MIN_CONTROL_SIZE_PERCENT..=MAX_CONTROL_SIZE_PERCENT,
                    )
                    .suffix("%"),
                ))
            })
            .inner;
        if response.changed() {
            selected_transform_mut(layout, selected).size_percent = size_percent;
            dragged_change = response.dragged();
        }
    });
    dragged_change
}

fn draw_corner_slider(
    ui: &mut egui::Ui,
    theme: &super::MaterialTheme,
    layout: &mut VirtualControlLayout,
) -> bool {
    let mut dragged_change = false;
    material_settings_slider_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().slider_width = settings_slider_width(ui.available_width());
                ui.add(controller_slider(
                    egui::Slider::new(
                        &mut layout.corner_radius_percent,
                        MIN_CONTROL_CORNER_RADIUS_PERCENT..=MAX_CONTROL_CORNER_RADIUS_PERCENT,
                    )
                    .suffix("%"),
                ))
            })
            .inner;
        dragged_change = response.changed() && response.dragged();
    });
    dragged_change
}

fn draw_vibration_slider(
    ui: &mut egui::Ui,
    theme: &super::MaterialTheme,
    vibration: &mut VibrationSettings,
) -> Option<u8> {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let mut strength = vibration_slider_value(*vibration);
    let mut haptic = None;
    material_settings_slider_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().slider_width = settings_slider_width(ui.available_width());
                ui.add(controller_slider(
                    egui::Slider::new(&mut strength, 0..=MAX_VIBRATION_STRENGTH_PERCENT)
                        .custom_formatter(|value, _| {
                            if value <= 0.0 {
                                tr.text("Off")
                            } else {
                                format!("{value:.0}%")
                            }
                        })
                        .custom_parser(|value| {
                            let value = value.trim();
                            if value.eq_ignore_ascii_case(&tr.text("Off")) {
                                Some(0.0)
                            } else {
                                value.trim_end_matches('%').parse().ok()
                            }
                        }),
                ))
            })
            .inner;
        if response.changed() {
            apply_vibration_slider_value(vibration, strength);
            if strength > 0 && response.dragged() {
                haptic = Some(strength);
            }
        }
    });
    haptic
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/control_editor/mod.rs"]
mod tests;
