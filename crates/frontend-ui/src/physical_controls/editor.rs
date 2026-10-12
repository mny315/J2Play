use super::keyboard::KeyGroup;
use super::selection::{ActionPage, BindingTarget, Picker};
use super::{
    FrontendApp, GamepadButton, Instant, PhysicalAction, PhysicalBindings, PhysicalControl, Screen,
    egui,
};
use crate::{
    RichText, SETTINGS_TOUCH_TARGET_HEIGHT, apply_settings_style, material_app_bar_frame,
    material_card_frame, material_choice_button, material_outlined_button,
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(super) enum EditorTab {
    #[default]
    Gamepad,
    Keyboard,
    Extra,
}

impl EditorTab {
    pub(super) const fn capture_label(self) -> &'static str {
        match self {
            Self::Keyboard => "Press a key",
            Self::Gamepad | Self::Extra => "Press a button",
        }
    }
}

pub(crate) struct PhysicalEditor {
    pub(super) diagram: super::diagram::Diagram,
    pub(super) draft: PhysicalBindings,
    pub(super) listening: Option<Instant>,
    pub(super) picker: Option<Picker>,
    pub(super) pending: Option<(BindingTarget, PhysicalAction)>,
    pub(super) page: ActionPage,
    pub(super) tab: EditorTab,
    pub(super) key_group: KeyGroup,
    pub(super) focus_picker: bool,
    pub(super) focus_diagram: bool,
    pub(super) picker_open: bool,
    pub(super) origin_focus: Option<egui::Id>,
    pub(super) status: String,
    pub(super) quiet_until: Option<Instant>,
}

impl PhysicalEditor {
    pub(super) fn select(&mut self, picker: Picker) {
        self.listening = None;
        self.pending = None;
        self.picker = Some(picker);
        self.focus_picker = true;
        self.status.clear();
        if let Picker::Binding(target) = picker {
            self.page = target
                .actions(&self.draft)
                .next()
                .map_or(ActionPage::Phone, ActionPage::for_action);
        }
    }

    pub(super) fn choose_action(&mut self, target: BindingTarget, action: PhysicalAction) {
        if target.actions(&self.draft).any(|old| old != action) {
            self.pending = Some((target, action));
            self.focus_picker = true;
        } else {
            self.assign(target, action);
        }
    }

    pub(super) fn assign(&mut self, target: BindingTarget, action: PhysicalAction) {
        let mut draft = self.draft.clone();
        for control in target.controls() {
            if let Err(error) = draft.assign(control, action) {
                error.message().clone_into(&mut self.status);
                self.pending = None;
                return;
            }
        }
        self.draft = draft;
        self.status.clear();
        self.cancel_capture();
    }

    pub(super) fn remove(&mut self, target: BindingTarget) {
        for control in target.controls() {
            self.draft.remove(control);
        }
        self.status.clear();
        self.cancel_capture();
    }

    pub(crate) fn cancel_capture(&mut self) {
        self.listening = None;
        self.picker = None;
        self.pending = None;
        self.focus_picker = false;
    }

    pub(crate) fn capturing(&self) -> bool {
        self.listening.is_some() || self.picker.is_some() || self.pending.is_some()
    }

    fn draw_tabs(&mut self, ui: &mut egui::Ui) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        ui.horizontal_wrapped(|ui| {
            let tabs = [
                (EditorTab::Gamepad, "Gamepad"),
                (EditorTab::Keyboard, "Keyboard"),
                (EditorTab::Extra, "Extra"),
            ]
            .map(|(tab, label)| (tab, tr.text(label)));
            let minimum = tabs
                .iter()
                .map(|(_, label)| {
                    ui.painter()
                        .layout_no_wrap(
                            label.clone(),
                            egui::TextStyle::Button.resolve(ui.style()),
                            egui::Color32::WHITE,
                        )
                        .size()
                        .x
                        .ceil()
                        + ui.spacing().button_padding.x * 2.0
                })
                .fold(0.0, f32::max);
            let width = ((ui.available_width() - ui.spacing().item_spacing.x * 2.0) / 3.0)
                .max(minimum)
                .min(ui.available_width());
            for (tab, label) in tabs {
                if ui
                    .add_sized(
                        [width, SETTINGS_TOUCH_TARGET_HEIGHT],
                        material_choice_button(label, self.tab == tab).wrap(),
                    )
                    .clicked()
                {
                    self.tab = tab;
                    self.status.clear();
                }
            }
        });
    }

    fn draw_gamepad(&mut self, ui: &mut egui::Ui, theme: &crate::MaterialTheme) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        let focus = std::mem::take(&mut self.focus_diagram);
        if let Some(picker) = self.diagram.draw(ui, theme, &self.draft, focus) {
            self.select(picker);
        }
        material_card_frame(theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(tr.text("Stick / trigger dead zone"));
            ui.spacing_mut().slider_width = crate::settings_slider_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add(super::controller_slider(
                    egui::Slider::new(&mut self.draft.dead_zone_percent, 10..=80).show_value(false),
                ));
                ui.label(format!("{}%", self.draft.dead_zone_percent));
            });
        });
    }
}

impl FrontendApp {
    pub(crate) fn open_physical_editor(&mut self) {
        let draft = match &self.screen {
            Screen::AppSettings(settings) => settings.draft.physical_bindings.clone(),
            Screen::Settings(settings) => settings
                .draft
                .physical_bindings
                .as_ref()
                .unwrap_or(&self.app_settings.physical_bindings)
                .clone(),
            _ => return,
        };
        self.release_all_input();
        self.physical_editor = Some(PhysicalEditor {
            diagram: super::diagram::Diagram::default(),
            draft,
            listening: None,
            picker: None,
            pending: None,
            page: ActionPage::Phone,
            tab: EditorTab::Gamepad,
            key_group: KeyGroup::default(),
            focus_picker: false,
            focus_diagram: true,
            picker_open: false,
            origin_focus: None,
            quiet_until: None,
            status: String::new(),
        });
    }

    pub(crate) fn close_physical_editor(&mut self, save: bool) {
        let Some(editor) = self.physical_editor.take() else {
            return;
        };
        if save {
            match &mut self.screen {
                Screen::AppSettings(settings) => settings.draft.physical_bindings = editor.draft,
                Screen::Settings(settings) => settings.draft.physical_bindings = Some(editor.draft),
                _ => {}
            }
        }
        self.release_all_input();
    }

    pub(crate) fn draw_physical_editor(&mut self, ui: &mut egui::Ui) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        apply_settings_style(ui);
        let devices = self.platform.physical_devices();
        let Some(editor) = &mut self.physical_editor else {
            return;
        };
        let theme = &self.material_theme;
        material_app_bar_frame(theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(tr.text("Physical controls"))
                    .size(20.0)
                    .strong(),
            );
        });
        ui.add_space(8.0);
        if !devices.is_empty() {
            ui.label(crate::material_supporting_text(
                theme,
                tr.format("Connected: {devices}", &[("devices", &devices.join(", "))]),
            ));
        }
        let previous_tab = editor.tab;
        let previous_group = editor.key_group;
        editor.draw_tabs(ui);
        ui.add_space(8.0);
        let capture = ui
            .add_sized(
                [ui.available_width(), SETTINGS_TOUCH_TARGET_HEIGHT],
                material_outlined_button(
                    theme,
                    format!("{}…", tr.text(editor.tab.capture_label())),
                ),
            )
            .clicked();
        ui.add_space(8.0);
        match editor.tab {
            EditorTab::Gamepad => editor.draw_gamepad(ui, theme),
            EditorTab::Keyboard => {
                if let Some(picker) =
                    super::keyboard::draw(ui, theme, &editor.draft, &mut editor.key_group)
                {
                    editor.select(picker);
                }
            }
            EditorTab::Extra => {
                let controls = extra_controls(&editor.draft);
                if let Some(picker) =
                    super::keyboard::draw_grid(ui, theme, &editor.draft, &controls)
                {
                    editor.select(picker);
                }
            }
        }
        if !editor.status.is_empty() {
            ui.label(tr.error(&editor.status));
        }
        let page_changed = previous_tab != editor.tab || previous_group != editor.key_group;
        if page_changed {
            editor.focus_diagram = false;
            self.release_all_input();
        }
        if capture {
            self.release_all_input();
            if let Some(editor) = &mut self.physical_editor {
                editor.listening = Some(Instant::now());
                editor.focus_picker = true;
                editor.status.clear();
            }
        }
        self.configure_physical_input();
    }

    pub(crate) fn reset_physical_editor(&mut self) {
        if let Some(editor) = &mut self.physical_editor {
            editor.draft = PhysicalBindings::default();
            editor.cancel_capture();
            editor.status.clear();
        }
    }
}

pub(crate) fn draw_card(
    ui: &mut egui::Ui,
    theme: &crate::MaterialTheme,
    game: Option<&mut Option<PhysicalBindings>>,
) -> bool {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let mut open = false;
    material_card_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(tr.text("Physical controls"))
                .size(18.0)
                .strong(),
        );

        if let Some(game) = game {
            ui.label(if game.is_none() {
                tr.text("Current bindings: Global")
            } else {
                tr.text("Current bindings: This game")
            });
            if ui
                .add_enabled(
                    game.is_some(),
                    material_outlined_button(theme, tr.text("Use global bindings")),
                )
                .clicked()
            {
                *game = None;
            }
        }
        open = ui
            .add_sized(
                [ui.available_width(), SETTINGS_TOUCH_TARGET_HEIGHT],
                material_outlined_button(theme, tr.text("Configure physical controls…")),
            )
            .clicked();
    });
    open
}

fn extra_controls(bindings: &PhysicalBindings) -> Vec<PhysicalControl> {
    let mut controls = vec![
        PhysicalControl::AndroidKey { code: 24 },
        PhysicalControl::AndroidKey { code: 25 },
    ];
    controls.extend((1..=16).map(|number| PhysicalControl::Gamepad {
        button: GamepadButton::Extra(number),
    }));
    for binding in &bindings.bindings {
        if matches!(binding.control, PhysicalControl::AndroidKey { .. })
            && !controls.contains(&binding.control)
        {
            controls.push(binding.control);
        }
    }
    controls
}
