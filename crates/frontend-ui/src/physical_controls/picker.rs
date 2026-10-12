use super::editor::PhysicalEditor;
use super::labels::action_label;
use super::selection::{ActionPage, BindingTarget, ControlGroup, Picker};
use super::{FrontendApp, HostAction, PhysicalAction, egui};
use crate::{
    SETTINGS_TOUCH_TARGET_HEIGHT, apply_settings_style, material_choice_button,
    material_outlined_button, material_primary_button, material_text_button,
};

fn action_cells(page: ActionPage) -> &'static [Option<(&'static str, PhysicalAction)>] {
    use HostAction as H;
    use PhysicalAction::{DebugOverlay, FastForward, Fullscreen, Menu, Phone, StopGame};
    match page {
        ActionPage::Phone => &[
            Some(("L soft", Phone(H::SoftLeft))),
            Some(("Up", Phone(H::Up))),
            Some(("R soft", Phone(H::SoftRight))),
            Some(("Left", Phone(H::Left))),
            Some(("Fire / OK", Phone(H::Fire))),
            Some(("Right", Phone(H::Right))),
            Some(("Back", Phone(H::Back))),
            Some(("Down", Phone(H::Down))),
            Some(("Clear", Phone(H::Clear))),
            Some(("Game A", Phone(H::GameA))),
            None,
            Some(("Game B", Phone(H::GameB))),
        ],
        ActionPage::Keypad => &[
            Some(("1", Phone(H::Num1))),
            Some(("2", Phone(H::Num2))),
            Some(("3", Phone(H::Num3))),
            Some(("4", Phone(H::Num4))),
            Some(("5", Phone(H::Num5))),
            Some(("6", Phone(H::Num6))),
            Some(("7", Phone(H::Num7))),
            Some(("8", Phone(H::Num8))),
            Some(("9", Phone(H::Num9))),
            Some(("*", Phone(H::Star))),
            Some(("0", Phone(H::Num0))),
            Some(("#", Phone(H::Pound))),
        ],
        ActionPage::App => &[
            Some(("App menu / Back", Menu)),
            Some(("Fullscreen", Fullscreen)),
            Some(("Fast-forward", FastForward)),
            Some(("Debug overlay", DebugOverlay)),
            Some(("Stop game", StopGame)),
        ],
    }
}

impl PhysicalEditor {
    fn draw_picker_footer(&mut self, ui: &mut egui::Ui, theme: &crate::MaterialTheme, focus: bool) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        ui.horizontal(|ui| {
            let removable =
                self.pending.is_none() && matches!(self.picker, Some(Picker::Binding(_)));
            let width = if removable {
                (ui.available_width() - ui.spacing().item_spacing.x) * 0.5
            } else {
                ui.available_width()
            };
            if removable && let Some(Picker::Binding(target)) = self.picker {
                let has_binding = target.actions(&self.draft).next().is_some();
                if ui
                    .add_enabled_ui(has_binding, |ui| {
                        ui.add_sized(
                            [width, SETTINGS_TOUCH_TARGET_HEIGHT],
                            material_outlined_button(theme, tr.text("Remove")).wrap(),
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.remove(target);
                }
            }
            let response = ui.add_sized(
                [width, SETTINGS_TOUCH_TARGET_HEIGHT],
                material_text_button(theme, tr.text("Cancel")).wrap(),
            );
            if focus && self.listening.is_some() {
                response.request_focus();
            }
            if response.clicked() {
                self.cancel_capture();
            }
        });
    }

    fn draw_group(&mut self, ui: &mut egui::Ui, group: ControlGroup, focus: bool) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        let width = (ui.available_width() - ui.spacing().item_spacing.x * 2.0) / 3.0;
        let controls = group.controls();
        egui::Grid::new(("physical-control-directions", group))
            .num_columns(3)
            .show(ui, |ui| {
                for cell in 0..9 {
                    if let Some((_, label, Some(control))) =
                        controls.iter().find(|(index, _, _)| *index == cell)
                    {
                        let target = BindingTarget::from_control(*control);
                        let response = ui.add_sized(
                            [width, 56.0],
                            material_choice_button(tr.text(*label), false).wrap(),
                        );
                        if focus && cell == 1 {
                            response.request_focus();
                        }
                        if response.clicked() {
                            self.select(Picker::Binding(target));
                        }
                    } else {
                        ui.allocate_space(egui::vec2(width, 56.0));
                    }
                    if cell % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
    }

    fn draw_actions(
        &mut self,
        ui: &mut egui::Ui,
        target: BindingTarget,
        focus: bool,
        max_height: f32,
    ) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        ui.horizontal(|ui| {
            let width = (ui.available_width() - ui.spacing().item_spacing.x * 2.0) / 3.0;
            for (page, label) in [
                (ActionPage::Phone, "Phone"),
                (ActionPage::Keypad, "Keypad"),
                (ActionPage::App, "App"),
            ] {
                if ui
                    .add_sized(
                        [width, SETTINGS_TOUCH_TARGET_HEIGHT],
                        material_choice_button(tr.text(label), self.page == page).wrap(),
                    )
                    .clicked()
                {
                    self.page = page;
                }
            }
        });
        ui.add_space(8.0);
        let cells = action_cells(self.page);
        let columns: u8 = if self.page == ActionPage::App { 2 } else { 3 };
        let actions = target.actions(&self.draft);
        let initial = cells
            .iter()
            .flatten()
            .find(|(_, action)| actions.clone().any(|bound| bound == *action))
            .or_else(|| cells.iter().flatten().next())
            .map(|(_, action)| *action);
        let mut selected = None;
        crate::scrolling::vertical()
            .id_salt(("physical-action-scroll", target, self.page))
            .max_height(max_height)
            .show(ui, |ui| {
                let width = (ui.available_width()
                    - ui.spacing().item_spacing.x * f32::from(columns - 1))
                    / f32::from(columns);
                egui::Grid::new(("physical-action-grid", target, self.page))
                    .num_columns(usize::from(columns))
                    .show(ui, |ui| {
                        for (index, cell) in cells.iter().enumerate() {
                            if let Some((label, action)) = cell {
                                let response = ui.add_sized(
                                    [width, 52.0],
                                    material_choice_button(
                                        tr.text(*label),
                                        actions.clone().any(|bound| bound == *action),
                                    )
                                    .wrap(),
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        ui.is_enabled(),
                                        tr.text(action_label(*action)),
                                    )
                                });
                                if focus && Some(*action) == initial {
                                    response.request_focus();
                                }
                                if response.clicked() {
                                    selected = Some(*action);
                                }
                            } else {
                                ui.allocate_space(egui::vec2(width, 52.0));
                            }
                            if index % usize::from(columns) == usize::from(columns - 1) {
                                ui.end_row();
                            }
                        }
                    });
            });
        if let Some(action) = selected {
            self.choose_action(target, action);
        }
    }
}

impl FrontendApp {
    pub(crate) fn draw_physical_capture(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::Translator::from_context(ctx);
        if self.overlay_active() {
            return;
        }
        let Some(editor) = &mut self.physical_editor else {
            return;
        };
        if !editor.capturing() {
            if editor.picker_open {
                if let Some(origin) = editor.origin_focus.take() {
                    ctx.memory_mut(|memory| memory.request_focus(origin));
                }
                editor.picker_open = false;
            }
            return;
        }
        if !editor.picker_open {
            editor.origin_focus = ctx.memory(egui::Memory::focused);
            editor.picker_open = true;
        }
        let focus = std::mem::take(&mut editor.focus_picker);
        let theme = &self.material_theme;
        let id = egui::Id::new("physical-input-capture");
        let response = egui::Modal::new(id)
            .area(
                egui::Modal::default_area(id)
                    .constrain_to(self.safe_content_rect)
                    .fade_in(true),
            )
            .frame(crate::material_card_frame(theme))
            .show(ctx, |ui| {
                apply_settings_style(ui);
                ui.set_width((self.safe_content_rect.width() - 64.0).clamp(160.0, 420.0));
                let max_height = (self.safe_content_rect.height() - 280.0).clamp(56.0, 280.0);
                if editor.listening.is_some() {
                    ui.heading(tr.text(editor.tab.capture_label()));
                    ui.label(tr.text("Waiting up to 15 seconds…"));
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                } else if let Some((target, action)) = editor.pending {
                    ui.heading(tr.text("Reassign control?"));
                    ui.label(tr.control(&target.label()));
                    ui.label(format!(
                        "{} - {}",
                        tr.control(&target.assignment(&editor.draft)),
                        tr.text(action_label(action))
                    ));
                    let response = ui.add_sized(
                        [ui.available_width(), SETTINGS_TOUCH_TARGET_HEIGHT],
                        material_primary_button(theme, tr.text("Reassign")),
                    );
                    if focus {
                        response.request_focus();
                    }
                    if response.clicked() {
                        editor.assign(target, action);
                    }
                } else if let Some(picker) = editor.picker {
                    ui.heading(tr.control(&picker.label()));
                    match picker {
                        Picker::Group(group) => {
                            crate::scrolling::vertical()
                                .id_salt(("physical-group-scroll", group))
                                .max_height(max_height + 56.0)
                                .show(ui, |ui| editor.draw_group(ui, group, focus));
                        }
                        Picker::Binding(target) => {
                            ui.label(tr.format(
                                "Assigned: {action}",
                                &[("action", &tr.control(&target.assignment(&editor.draft)))],
                            ));
                            editor.draw_actions(ui, target, focus, max_height);
                            if !editor.status.is_empty() {
                                ui.label(tr.error(&editor.status));
                            }
                        }
                    }
                }
                ui.add_space(8.0);
                editor.draw_picker_footer(ui, theme, focus);
            });
        if response.should_close() {
            editor.cancel_capture();
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/physical_controls/picker.rs"]
mod tests;
