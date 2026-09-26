use crate::{
    MaterialTheme, PlatformTheme, RichText, Vec2, egui, material_card_frame, material_choice_button,
};
use frontend_core::{AccentColor, AppTheme};

pub(super) fn draw_card(
    ui: &mut egui::Ui,
    theme: &MaterialTheme,
    preference: &mut AppTheme,
    accent: &mut AccentColor,
) {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    material_card_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(tr.text("Appearance")).size(18.0).strong());
        ui.add_space(4.0);
        ui.label(RichText::new(tr.text("Theme")).strong());
        ui.horizontal_wrapped(|ui| {
            for (value, label) in [
                (AppTheme::System, "System"),
                (AppTheme::Light, "Light"),
                (AppTheme::Dark, "Dark"),
                (AppTheme::Oled, "OLED"),
            ] {
                if ui
                    .add(material_choice_button(tr.text(label), *preference == value))
                    .clicked()
                {
                    *preference = value;
                }
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new(tr.text("Accent color")).strong());
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(material_choice_button(
                    tr.text("System"),
                    *accent == AccentColor::System,
                ))
                .clicked()
            {
                *accent = AccentColor::System;
            }
            for (value, label) in [
                (AccentColor::Blue, "Blue"),
                (AccentColor::Teal, "Teal"),
                (AccentColor::Green, "Green"),
                (AccentColor::Amber, "Amber"),
                (AccentColor::Orange, "Orange"),
                (AccentColor::Red, "Red"),
                (AccentColor::Pink, "Pink"),
                (AccentColor::Purple, "Purple"),
            ] {
                if accent_button(ui, theme, value, label, *accent == value).clicked() {
                    *accent = value;
                }
            }
        });
    });
}

fn accent_button(
    ui: &mut egui::Ui,
    theme: &MaterialTheme,
    accent: AccentColor,
    label: &str,
    selected: bool,
) -> egui::Response {
    let palette = MaterialTheme::resolve(
        PlatformTheme {
            mode: theme.mode,
            light_colors: None,
            dark_colors: None,
        },
        AppTheme::System,
        accent,
    );
    let swatch = ui.make_persistent_id(("accent-swatch", label));
    let response = egui::Button::new((
        egui::Atom::custom(swatch, Vec2::splat(12.0)),
        RichText::new(crate::i18n::Translator::from_context(ui.ctx()).text(label)).strong(),
    ))
    .selected(selected)
    .corner_radius(24)
    .min_size(Vec2::new(0.0, 48.0))
    .atom_ui(ui);
    if let Some(rect) = response.rect(swatch) {
        ui.painter()
            .circle_filled(rect.center(), 6.0, palette.primary);
    }
    crate::focus_ring::track(ui, &response.response, 24.0);
    response.response
}
