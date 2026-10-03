use super::{
    DEFAULT_MANUAL_FPS_LIMIT, FpsLimit, FrontendApp, MAX_FPS_LIMIT, MIN_FPS_LIMIT, RichText,
    Screen, apply_settings_style, egui, material_app_bar_frame, material_card_frame,
    material_choice_button, material_settings_slider_frame, settings_slider_width,
};
use crate::physical_controls::controller_slider;
use frontend_core::{LibraryView, MAX_UI_SCALE_PERCENT, MIN_UI_SCALE_PERCENT};

mod appearance;

impl FrontendApp {
    pub(super) fn draw_app_settings(&mut self, ui: &mut egui::Ui) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        apply_settings_style(ui);
        let page_top = ui.next_widget_position();
        material_app_bar_frame(&self.material_theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(tr.text("App settings")).size(20.0).strong());
        });
        ui.add_space(8.0);
        let Screen::AppSettings(settings) = &mut self.screen else {
            return;
        };
        let draft = &mut settings.draft;
        let mut haptic = false;
        draw_language_card(
            ui,
            &self.material_theme,
            &mut draft.language,
            page_top,
            &mut settings.focus_first,
        );
        ui.add_space(8.0);
        material_card_frame(&self.material_theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(tr.text("FPS limit")).size(18.0).strong());
            ui.add_space(4.0);
            let automatic = draft.fps_limit == FpsLimit::Automatic;
            ui.horizontal_wrapped(|ui| {
                let response = ui.add(material_choice_button(tr.text("Device profile"), automatic));
                if response.clicked() {
                    draft.fps_limit = FpsLimit::Automatic;
                }
                if ui
                    .add(material_choice_button(tr.text("Fixed limit"), !automatic))
                    .clicked()
                    && automatic
                {
                    draft.fps_limit = FpsLimit::Manual {
                        frames_per_second: DEFAULT_MANUAL_FPS_LIMIT,
                    };
                }
            });
            if let FpsLimit::Manual { frames_per_second } = &mut draft.fps_limit {
                material_settings_slider_frame(&self.material_theme).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.spacing_mut().slider_width = settings_slider_width(ui.available_width());
                    let response = ui.add(controller_slider(
                        egui::Slider::new(frames_per_second, MIN_FPS_LIMIT..=MAX_FPS_LIMIT)
                            .suffix(" FPS"),
                    ));
                    haptic |= response.changed() && response.dragged();
                });
            }
        });
        ui.add_space(8.0);
        draft.fullscreen = super::settings::draw_fullscreen_card(
            ui,
            &self.material_theme,
            Some(draft.fullscreen),
            None,
        )
        .unwrap_or_default();
        ui.add_space(8.0);
        appearance::draw_card(
            ui,
            &self.material_theme,
            &mut draft.theme,
            &mut draft.accent_color,
        );
        ui.add_space(8.0);
        haptic |= draw_interface_scale_card(ui, &self.material_theme, &mut draft.ui_scale_percent);
        ui.add_space(8.0);
        draw_library_view_card(ui, &self.material_theme, &mut draft.library_view);
        ui.add_space(8.0);
        let open_controls = super::settings::draw_controls_card(
            ui,
            &self.material_theme,
            &mut draft.control_layout,
            &mut draft.landscape_control_layout,
            None,
        );
        ui.add_space(8.0);
        let open_physical = super::physical_controls::draw_card(ui, &self.material_theme, None);
        if open_controls {
            self.open_control_editor();
        }
        if haptic {
            self.request_slider_haptic();
        }
        if open_physical {
            self.open_physical_editor();
        }
    }

    pub(super) fn save_app_settings_screen(&mut self) {
        let Screen::AppSettings(settings) = &self.screen else {
            return;
        };
        let draft = &settings.draft;
        match self.repository.save_app_settings(draft) {
            Ok(()) => {
                self.app_settings = draft.clone();
                self.screen = Screen::Library;
            }
            Err(error) => self.show_error("Could not save app settings", &error),
        }
    }
}

fn draw_interface_scale_card(
    ui: &mut egui::Ui,
    theme: &super::MaterialTheme,
    percent: &mut u16,
) -> bool {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let mut haptic = false;
    material_card_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(tr.text("Interface scale"))
                .size(18.0)
                .strong(),
        );
        ui.add_space(4.0);
        material_settings_slider_frame(theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().slider_width = settings_slider_width(ui.available_width());
            let response = ui.add(controller_slider(
                egui::Slider::new(percent, MIN_UI_SCALE_PERCENT..=MAX_UI_SCALE_PERCENT).suffix("%"),
            ));
            haptic |= response.changed() && response.dragged();
        });
        if ui
            .add(material_choice_button(
                tr.text("System default (100%)"),
                *percent == 100,
            ))
            .clicked()
        {
            *percent = 100;
        }
    });
    haptic
}

fn draw_library_view_card(
    ui: &mut egui::Ui,
    theme: &super::MaterialTheme,
    draft: &mut LibraryView,
) {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    material_card_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(tr.text("Library view")).size(18.0).strong());
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for (view, label) in [
                (LibraryView::Automatic, "Auto"),
                (LibraryView::List, "List"),
                (LibraryView::Tiles, "Tiles"),
            ] {
                if ui
                    .add(material_choice_button(tr.text(label), *draft == view))
                    .clicked()
                {
                    *draft = view;
                }
            }
        });
    });
}

fn draw_language_card(
    ui: &mut egui::Ui,
    theme: &super::MaterialTheme,
    selected: &mut Option<frontend_core::Language>,
    page_top: egui::Pos2,
    focus_first: &mut bool,
) {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    material_card_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(tr.text("Language")).size(18.0).strong());
        let id = ui.make_persistent_id(egui::IdSalt::new("language"));
        let combo = egui::ComboBox::from_id_salt("language")
            .selected_text(selected.map_or_else(
                || tr.text("System"),
                |language| language.native_name().to_owned(),
            ))
            .width(ui.available_width())
            .height(super::PROFILE_MENU_MAX_HEIGHT)
            .show_ui(ui, |ui| {
                let mut menu = crate::focus_navigation::ComboMenu::begin(ui, id);
                menu.selectable_value(ui, selected, None, &tr.text("System"));
                for language in frontend_core::Language::ALL {
                    menu.selectable_value(ui, selected, Some(language), language.native_name());
                }
                menu.finish(ui);
            });
        super::focus_ring::track_page_start(ui, &combo.response, 12.0, page_top, focus_first);
    });
}
