use super::*;

#[path = "../../support/text_languages.rs"]
mod language_samples;

#[test]
fn every_guest_language_choice_has_a_runtime_text_fixture() {
    let choices = LOCALES
        .iter()
        .map(|&(code, _)| code)
        .collect::<std::collections::BTreeSet<_>>();
    let samples = language_samples::LANGUAGE_SAMPLES
        .iter()
        .map(|&(code, _, _)| code)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(choices, samples);
}

#[test]
fn guest_language_choices_are_valid_distinct_and_readable() {
    let mut codes = std::collections::BTreeSet::new();
    let ctx = egui::Context::default();
    crate::i18n::install(&ctx, frontend_core::Language::English);
    ctx.begin_pass(egui::RawInput::default());
    ctx.fonts_mut(|fonts| {
        let font = egui::FontId::proportional(18.0);
        let mut missing = std::collections::BTreeSet::new();
        for &(code, name) in LOCALES {
            device_profile::validate_guest_locale(code).unwrap();
            assert!(codes.insert(code));
            for ch in name.chars() {
                if !fonts.has_glyph(&font, ch) {
                    missing.insert((code, ch));
                }
            }
        }
        assert!(missing.is_empty(), "missing glyphs: {missing:?}");
    });
    ctx.end_pass().textures_delta.clear();
    assert!(codes.contains("he-IL"));
    assert!(codes.contains("ru-RU"));
    assert!(codes.len() > frontend_core::Language::ALL.len());
}

#[test]
fn explicit_and_custom_guest_languages_fit_narrow_screens() {
    let ctx = egui::Context::default();
    let theme = crate::MaterialTheme::fallback(crate::PlatformThemeMode::Light);
    crate::apply_material_theme(&ctx, &theme);
    for language in frontend_core::Language::ALL {
        crate::i18n::install(&ctx, language);
        for width in [240.0, 320.0] {
            for code in ["he-IL", "ru-RU", "iw-IL", "pt-BR"] {
                let mut selected = Some(code.to_owned());
                let viewport =
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 600.0));
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(viewport),
                        ..Default::default()
                    },
                    |ui| {
                        crate::apply_settings_style(ui);
                        draw(ui, &theme, &mut selected);
                    },
                );
                for shape in &output.shapes {
                    if let egui::Shape::Text(text) = &shape.shape {
                        let bounds = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(
                            viewport.expand(0.5).contains_rect(bounds),
                            "{language:?}, {width}, {code}: {bounds:?}"
                        );
                        assert!(!text.galley.elided);
                    }
                }
                output.drop_without_applying_deltas();
            }
        }
    }
}
