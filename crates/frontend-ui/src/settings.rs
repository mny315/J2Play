use super::{
    DEFAULT_MANUAL_FPS_LIMIT, DataAction, DataActionKind, FpsLimit, FrontendApp, GameScale,
    ImportFlow, MAX_FPS_LIMIT, MAX_SCALE_PERCENT, MAX_VIBRATION_STRENGTH_PERCENT, MIN_FPS_LIMIT,
    MIN_SCALE_PERCENT, RichText, SETTINGS_TOUCH_TARGET_HEIGHT, Screen, SettingsScreen,
    SettingsTarget, VibrationSettings, apply_settings_style, egui, material_app_bar_frame,
    material_card_frame, material_choice_button, material_error_outlined_button,
    material_outlined_button, material_settings_slider_frame, material_supporting_text,
    material_tonal_button, settings_slider_width,
};
use crate::physical_controls::controller_slider;

impl FrontendApp {
    pub(super) fn open_settings(&mut self, entry_id: &str, focus_profile: bool) {
        let Some(entry) = self.entries.iter().find(|entry| entry.id() == entry_id) else {
            return;
        };
        self.screen = Screen::Settings(Box::new(SettingsScreen {
            target: SettingsTarget::Existing {
                entry_id: entry_id.to_owned(),
            },
            draft: entry.settings().clone(),
            focus_profile,
            show_all_profiles: false,
            editor_request: None,
            control_editor: None,
        }));
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn draw_settings(&mut self, ui: &mut egui::Ui) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        apply_settings_style(ui);
        let page_top = ui.next_widget_position();
        material_app_bar_frame(&self.material_theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(tr.text("Game settings")).size(20.0).strong());
        });
        ui.add_space(8.0);

        let Screen::Settings(settings) = &mut self.screen else {
            return;
        };
        let existing_entry_id = match &settings.target {
            SettingsTarget::Existing { entry_id } => Some(entry_id.as_str()),
            SettingsTarget::PendingImport(_) => None,
        };
        let mut requested_data_action = None;
        let mut slider_haptic = false;

        material_card_frame(&self.material_theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(tr.text("Device profile")).size(18.0).strong());
            ui.add_space(4.0);
            let response = crate::profile_picker::draw(
                ui,
                &self.profile_options,
                &mut settings.draft.device_profile,
                settings.show_all_profiles,
            );
            super::focus_ring::track_page_start(
                ui,
                &response,
                12.0,
                page_top,
                &mut settings.focus_profile,
            );
            crate::switch::draw(
                ui,
                &self.material_theme,
                &mut settings.show_all_profiles,
                &tr.text("Show all profiles"),
            );
        });
        ui.add_space(8.0);

        material_card_frame(&self.material_theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(tr.text("Resume game")).size(18.0).strong());
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (value, label) in [
                    (frontend_core::ResumeBehavior::Ask, "Ask every time"),
                    (frontend_core::ResumeBehavior::Always, "Always continue"),
                    (frontend_core::ResumeBehavior::Never, "Start normally"),
                ] {
                    if ui
                        .add(material_choice_button(
                            tr.text(label),
                            settings.draft.resume_behavior == value,
                        ))
                        .clicked()
                    {
                        settings.draft.resume_behavior = value;
                    }
                }
            });
        });
        ui.add_space(8.0);

        material_card_frame(&self.material_theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(tr.text("Game scale")).size(18.0).strong());
            ui.add_space(4.0);
            let automatic_scale = matches!(settings.draft.game_scale, GameScale::AutomaticFit);
            let mut scale_percent = match settings.draft.game_scale {
                GameScale::AutomaticFit => 100,
                GameScale::Manual { percent } => percent,
            };
            let manual_scale = matches!(settings.draft.game_scale, GameScale::Manual { .. });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(material_choice_button(
                        tr.text("Automatic fit"),
                        automatic_scale,
                    ))
                    .clicked()
                {
                    settings.draft.game_scale = GameScale::AutomaticFit;
                    settings.draft.portrait_frame_percent = None;
                }
                if ui
                    .add(material_choice_button(
                        tr.text("Manual scale"),
                        manual_scale,
                    ))
                    .clicked()
                {
                    settings.draft.game_scale = GameScale::Manual {
                        percent: scale_percent,
                    };
                    settings.draft.portrait_frame_percent = None;
                }
            });
            if manual_scale {
                material_settings_slider_frame(&self.material_theme).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let response = ui
                        .scope(|ui| {
                            ui.spacing_mut().slider_width =
                                settings_slider_width(ui.available_width());
                            ui.add(controller_slider(
                                egui::Slider::new(
                                    &mut scale_percent,
                                    MIN_SCALE_PERCENT..=MAX_SCALE_PERCENT,
                                )
                                .suffix("%")
                                .logarithmic(true),
                            ))
                        })
                        .inner;
                    if response.changed() {
                        settings.draft.game_scale = GameScale::Manual {
                            percent: scale_percent,
                        };
                        settings.draft.portrait_frame_percent = None;
                        slider_haptic |= response.dragged();
                    }
                });
            }
        });
        ui.add_space(8.0);

        material_card_frame(&self.material_theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(tr.text("FPS limit")).size(18.0).strong());
            ui.add_space(4.0);
            let automatic_fps = matches!(settings.draft.fps_limit, FpsLimit::Automatic);
            let mut frames_per_second = match settings.draft.fps_limit {
                FpsLimit::Automatic => DEFAULT_MANUAL_FPS_LIMIT,
                FpsLimit::Manual { frames_per_second } => frames_per_second,
            };
            let manual_fps = matches!(settings.draft.fps_limit, FpsLimit::Manual { .. });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(material_choice_button(
                        tr.text("App default"),
                        automatic_fps,
                    ))
                    .clicked()
                {
                    settings.draft.fps_limit = FpsLimit::Automatic;
                }
                if ui
                    .add(material_choice_button(tr.text("Manual limit"), manual_fps))
                    .clicked()
                {
                    settings.draft.fps_limit = FpsLimit::Manual { frames_per_second };
                }
            });
            if manual_fps {
                material_settings_slider_frame(&self.material_theme).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let response = ui
                        .scope(|ui| {
                            ui.spacing_mut().slider_width =
                                settings_slider_width(ui.available_width());
                            ui.add(controller_slider(
                                egui::Slider::new(
                                    &mut frames_per_second,
                                    MIN_FPS_LIMIT..=MAX_FPS_LIMIT,
                                )
                                .suffix(" FPS"),
                            ))
                        })
                        .inner;
                    if response.changed() {
                        settings.draft.fps_limit = FpsLimit::Manual { frames_per_second };
                        slider_haptic |= response.dragged();
                    }
                });
            }
        });

        ui.add_space(8.0);
        settings.draft.fullscreen = draw_fullscreen_card(
            ui,
            &self.material_theme,
            settings.draft.fullscreen,
            Some(self.app_settings.fullscreen),
        );
        ui.add_space(8.0);
        self.app_settings
            .apply_control_defaults(&mut settings.draft);
        let open_controls = draw_controls_card(
            ui,
            &self.material_theme,
            &mut settings.draft.control_layout,
            &mut settings.draft.landscape_control_layout,
            Some(&mut settings.draft.controls_override),
        );

        ui.add_space(8.0);
        let open_physical = super::physical_controls::draw_card(
            ui,
            &self.material_theme,
            Some(&mut settings.draft.physical_bindings),
        );

        if existing_entry_id.is_some() {
            ui.add_space(8.0);
            material_card_frame(&self.material_theme).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(RichText::new(tr.text("Stored data")).size(18.0).strong());
                ui.add_space(4.0);
                let button_width = ui.available_width();
                for (kind, label) in [
                    (DataActionKind::EntryRecord, "Remove library entry…"),
                    (DataActionKind::PrivateArchives, "Delete private JAR/JAD…"),
                    (DataActionKind::RuntimeData, "Delete RMS and saves…"),
                    (DataActionKind::ResumeSave, "Clear save state…"),
                ] {
                    if ui
                        .add_sized(
                            [button_width, SETTINGS_TOUCH_TARGET_HEIGHT],
                            material_error_outlined_button(&self.material_theme, tr.text(label))
                                .wrap(),
                        )
                        .clicked()
                    {
                        requested_data_action = Some(kind);
                    }
                }
            });
        }

        if let (Some(kind), Some(entry_id)) = (requested_data_action, existing_entry_id) {
            self.data_action = Some(DataAction {
                entry_ids: vec![entry_id.to_owned()],
                kind,
            });
        }
        if slider_haptic {
            self.request_slider_haptic();
        }
        if open_controls {
            self.open_control_editor();
        }
        if open_physical {
            self.open_physical_editor();
        }
    }

    pub(super) fn save_settings_screen(&mut self) {
        self.physical_editor = None;
        self.release_all_input();
        if matches!(self.screen, Screen::AppSettings(_)) {
            self.save_app_settings_screen();
            return;
        }
        let Screen::Settings(settings) = std::mem::replace(&mut self.screen, Screen::Library)
        else {
            return;
        };
        let SettingsScreen {
            target,
            draft,
            focus_profile: _,
            show_all_profiles,
            editor_request: _,
            control_editor: _,
        } = *settings;
        match target {
            SettingsTarget::Existing { entry_id } => {
                let Some(index) = self.entries.iter().position(|entry| entry.id() == entry_id)
                else {
                    return;
                };
                if let Err(error) = self
                    .repository
                    .save_settings(&mut self.entries[index], draft.clone())
                {
                    self.screen = Screen::Settings(Box::new(SettingsScreen {
                        target: SettingsTarget::Existing { entry_id },
                        draft,
                        focus_profile: false,
                        show_all_profiles,
                        editor_request: None,
                        control_editor: None,
                    }));
                    self.show_error("Could not save settings", &error);
                }
            }
            SettingsTarget::PendingImport(prepared) => {
                if !self.commit_import(&prepared, draft.clone()) {
                    self.screen = Screen::Settings(Box::new(SettingsScreen {
                        target: SettingsTarget::PendingImport(prepared),
                        draft,
                        focus_profile: false,
                        show_all_profiles,
                        editor_request: None,
                        control_editor: None,
                    }));
                }
            }
        }
    }

    pub(super) fn cancel_settings_screen(&mut self) {
        self.physical_editor = None;
        self.release_all_input();
        let Screen::Settings(settings) = std::mem::replace(&mut self.screen, Screen::Library)
        else {
            return;
        };
        if let SettingsTarget::PendingImport(prepared) = settings.target {
            self.import_flow = Some(ImportFlow::ConfirmProfile(prepared));
        }
    }
}

pub(super) const fn vibration_slider_value(vibration: VibrationSettings) -> u8 {
    if vibration.enabled {
        vibration.strength_percent
    } else {
        0
    }
}

pub(super) fn apply_vibration_slider_value(vibration: &mut VibrationSettings, value: u8) {
    let value = value.min(MAX_VIBRATION_STRENGTH_PERCENT);
    vibration.enabled = value > 0;
    if value > 0 {
        vibration.strength_percent = value;
    }
}

/// Shared fullscreen choices, with optional per-game inheritance.
pub(super) fn draw_fullscreen_card(
    ui: &mut egui::Ui,
    theme: &super::MaterialTheme,
    mut selected: Option<frontend_core::FullscreenMode>,
    app_default: Option<frontend_core::FullscreenMode>,
) -> Option<frontend_core::FullscreenMode> {
    use frontend_core::FullscreenMode::{LandscapeOnly, Off, On};
    let tr = crate::i18n::Translator::from_context(ui.ctx());

    let label = |mode| match mode {
        Off => "Off",
        On => "On",
        LandscapeOnly => "Landscape only",
    };
    material_card_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(tr.text("Fullscreen")).size(18.0).strong());
        if let Some(default) = app_default {
            ui.label(material_supporting_text(
                theme,
                format!("{}: {}", tr.text("App default"), tr.text(label(default))),
            ));
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if app_default.is_some()
                && ui
                    .add(material_choice_button(
                        tr.text("Use app default"),
                        selected.is_none(),
                    ))
                    .clicked()
            {
                selected = None;
            }
            for mode in [Off, On, LandscapeOnly] {
                if ui
                    .add(material_choice_button(
                        tr.text(label(mode)),
                        selected == Some(mode),
                    ))
                    .clicked()
                {
                    selected = Some(mode);
                }
            }
        });
    });
    selected
}

/// Shared entry point for the global and per-game layout editor.
pub(super) fn draw_controls_card(
    ui: &mut egui::Ui,
    theme: &super::MaterialTheme,
    portrait: &mut super::VirtualControlLayout,
    landscape: &mut super::VirtualControlLayout,
    mut game_override: Option<&mut bool>,
) -> bool {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let mut open = false;
    material_card_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(tr.text("Virtual controls"))
                .size(18.0)
                .strong(),
        );
        ui.add_space(4.0);
        ui.label(if game_override.as_deref() == Some(&false) {
            tr.text("Current layout: Global")
        } else if !portrait.visible && !landscape.visible {
            tr.text("Current layout: Hidden")
        } else if portrait.is_default() && landscape.is_default() {
            tr.text("Current layout: Default")
        } else {
            tr.text("Current layout: Custom")
        });
        open = ui
            .add_sized(
                [ui.available_width(), SETTINGS_TOUCH_TARGET_HEIGHT],
                material_tonal_button(theme, tr.text("Customize controls…")),
            )
            .clicked();
        let visibility_label = if portrait.visible {
            tr.text("Hide controls during gameplay")
        } else {
            tr.text("Show controls during gameplay")
        };
        if ui
            .add_sized(
                [ui.available_width(), SETTINGS_TOUCH_TARGET_HEIGHT],
                material_outlined_button(theme, visibility_label),
            )
            .clicked()
        {
            portrait.visible = !portrait.visible;
            landscape.visible = portrait.visible;
            if let Some(value) = game_override.as_deref_mut() {
                *value = true;
            }
        }
        if let Some(value) = game_override
            && ui
                .add_sized(
                    [ui.available_width(), SETTINGS_TOUCH_TARGET_HEIGHT],
                    material_choice_button(tr.text("Use global controls"), !*value),
                )
                .clicked()
        {
            *value = false;
        }
    });
    open
}
