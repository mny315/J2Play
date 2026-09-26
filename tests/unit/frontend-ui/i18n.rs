use super::*;
use std::collections::BTreeSet;

fn placeholders(mut text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    while let Some((literal, rest)) = text.split_once('{') {
        assert!(!literal.contains('}'), "unmatched closing brace: {text}");
        let (key, rest) = rest.split_once('}').expect("unclosed placeholder");
        assert!(
            !key.is_empty()
                && key
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_'),
            "invalid placeholder: {key}"
        );
        names.push(key);
        text = rest;
    }
    assert!(!text.contains('}'), "unmatched closing brace: {text}");
    names.sort_unstable();
    names
}

#[test]
fn all_sixteen_catalogs_have_the_same_keys_and_placeholders() {
    let english = catalog(Language::English);
    assert!(english.len() > 200);
    assert_eq!(Language::ALL.len(), 16);
    for language in Language::ALL {
        let translated = catalog(language);
        assert_eq!(
            english.keys().collect::<Vec<_>>(),
            translated.keys().collect::<Vec<_>>(),
            "{language:?}"
        );
        for (key, value) in translated {
            assert!(!value.trim().is_empty(), "{language:?}: {key}");
            assert_eq!(value.trim(), value, "{language:?}: {key}");
            assert_eq!(
                placeholders(key),
                placeholders(value),
                "{language:?}: {key}"
            );
        }
    }
}

#[test]
fn control_labels_preserve_physical_legends_and_translate_descriptions() {
    for language in Language::ALL {
        let tr = Translator(language);
        for legend in ["Start", "Select", "Start / Options", "Select / Create"] {
            assert_eq!(tr.text(legend), legend, "{language:?}");
            assert_eq!(tr.control(legend), legend, "{language:?}");
        }
        // Home is a translated folder shortcut, but a literal keyboard legend.
        assert_eq!(
            tr.control("Keyboard Home"),
            format!("{}: Home", tr.text("Keyboard")),
            "{language:?}"
        );
        assert_eq!(
            tr.control("Left stick down"),
            format!("{}: {}", tr.text("Left stick"), tr.text("Down")),
            "{language:?}"
        );
    }
}

#[test]
fn formatting_preserves_user_text_and_does_not_expand_it_again() {
    let tr = Translator(Language::Russian);
    assert_eq!(
        tr.format("Game: {title}", &[("title", "{title} 日本語")]),
        "Игра: {title} 日本語"
    );
    assert_eq!(
        Translator(Language::German).format("Open {title}", &[("title", "Save")]),
        "Save öffnen"
    );
}

#[test]
fn language_is_isolated_per_context_and_can_change_without_restart() {
    let russian = egui::Context::default();
    let german = egui::Context::default();
    install(&russian, Language::Russian);
    install(&german, Language::German);
    assert_eq!(Translator::from_context(&russian).text("Save"), "Сохранить");
    assert_eq!(Translator::from_context(&german).text("Save"), "Speichern");
    install(&russian, Language::English);
    assert_eq!(Translator::from_context(&russian).text("Save"), "Save");
}

#[test]
fn bundled_fonts_cover_catalogs_and_native_language_names() {
    let ctx = egui::Context::default();
    install(&ctx, Language::English);
    ctx.begin_pass(egui::RawInput::default());
    ctx.fonts_mut(|fonts| {
        let font = egui::FontId::proportional(18.0);
        for language in Language::ALL {
            let chars: BTreeSet<_> = catalog(language)
                .values()
                .flat_map(|text| text.chars())
                .chain(language.native_name().chars())
                .filter(|ch| !ch.is_control())
                .collect();
            for ch in chars {
                assert!(
                    fonts.has_glyph(&font, ch),
                    "missing glyph {ch:?} for {language:?}"
                );
            }
        }
    });
    ctx.end_pass().textures_delta.clear();
}

#[test]
fn chinese_font_stays_compact_and_covers_names_beyond_the_ui_catalog() {
    let bytes =
        include_bytes!("../../../crates/frontend-ui/assets/fonts/J2PlayChinese-Regular.otf");
    // Embedding the full CJK source previously doubled the Android APK.
    assert!(bytes.len() <= 2 * 1024 * 1024);
    let ctx = egui::Context::default();
    install(&ctx, Language::Chinese);
    ctx.begin_pass(egui::RawInput::default());
    ctx.fonts_mut(|fonts| {
        let font = egui::FontId::proportional(18.0);
        for ch in "龙猫麒麟鲨鱼龟蛇森林海洋春夏秋冬天地玄黄【】！１２３".chars()
        {
            assert!(fonts.has_glyph(&font, ch), "missing name glyph {ch:?}");
        }
    });
    ctx.end_pass().textures_delta.clear();
}

#[test]
fn arabic_wraps_in_logical_order_and_places_numbers_left_to_right() {
    let ctx = egui::Context::default();
    install(&ctx, Language::Arabic);
    ctx.begin_pass(egui::RawInput::default());
    ctx.fonts_mut(|fonts| {
        let text = "إعدادات اللعبة 123 J2Play";
        let galley = fonts.layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(18.0),
            egui::Color32::WHITE,
        );
        assert_eq!(galley.rows[0].text(), text);
        let row = &galley.rows[0];
        let glyphs = &row.glyphs;
        assert!(glyphs[0].pos.x > glyphs[1].pos.x);
        let one = glyphs.iter().position(|glyph| glyph.chr == '1').unwrap();
        assert!(glyphs[one].pos.x < glyphs[one + 1].pos.x);
        assert!(glyphs[one + 1].pos.x < glyphs[one + 2].pos.x);
        let wrapped = fonts.layout(
            text.repeat(4),
            egui::FontId::proportional(18.0),
            egui::Color32::WHITE,
            150.0,
        );
        assert!(wrapped.rows.len() > 1);
        let recovered: String = wrapped.rows.iter().map(|row| row.text()).collect();
        assert_eq!(recovered, text.repeat(4));
        assert!(
            wrapped
                .rows
                .iter()
                .all(|row| row.glyphs.iter().all(|glyph| glyph.pos.x.is_finite()))
        );
    });
    ctx.end_pass().textures_delta.clear();
}

#[test]
fn complex_script_catalogs_keep_source_text_and_finite_geometry() {
    let ctx = egui::Context::default();
    install(&ctx, Language::English);
    ctx.begin_pass(egui::RawInput::default());
    ctx.fonts_mut(|fonts| {
        for language in [
            Language::Arabic,
            Language::Hindi,
            Language::Thai,
            Language::Chinese,
        ] {
            for text in catalog(language).values() {
                let galley = fonts.layout(
                    text.clone(),
                    egui::FontId::proportional(17.0),
                    egui::Color32::WHITE,
                    180.0,
                );
                assert_eq!(galley.job.text, *text, "{language:?}");
                assert!(!galley.rows.is_empty(), "{language:?}: {text}");
                assert!(galley.rect.is_finite(), "{language:?}: {text}");
            }
        }
    });
    ctx.end_pass().textures_delta.clear();
}

#[test]
fn language_menu_is_rasterized_before_opening_and_rewarmed_after_scale_changes() {
    let ctx = egui::Context::default();
    for (zoom_factor, text_factor) in [
        (0.9, 1.0),
        (0.9, 0.85),
        (1.0, 1.0),
        (1.25, 1.0),
        (1.25, 0.85),
        (1.25, 1.5),
    ] {
        ctx.set_zoom_factor(zoom_factor);
        let mut points = [0.0; crate::PlatformTextScale::SAMPLE_COUNT];
        for (size, point) in (1_u16..).zip(&mut points) {
            *point = f32::from(size) * text_factor;
        }
        let scale = crate::PlatformTextScale::from_points(points).unwrap();
        for _ in 0..Language::ALL.len() + 2 {
            install_scaled(&ctx, Language::English, scale);
            ctx.run_ui(egui::RawInput::default(), |ui| warm_language_menu(ui.ctx()))
                .drop_without_applying_deltas();
        }
        ctx.begin_pass(egui::RawInput::default());
        ctx.fonts_mut(|fonts| {
            for language in Language::ALL {
                fonts.layout_no_wrap(
                    language.native_name().to_owned(),
                    egui::FontId::proportional(crate::SETTINGS_BODY_TEXT_SIZE),
                    egui::Color32::WHITE,
                );
            }
        });
        let mut output = ctx.end_pass();
        let pending = output.textures_delta.set.len();
        output.textures_delta.clear();
        assert_eq!(
            pending, 0,
            "opening at zoom {zoom_factor}, text scale {text_factor} must reuse the font atlas"
        );
    }
}

#[test]
fn translated_library_settings_captions_fit_the_buttons() {
    let ctx = egui::Context::default();
    let theme = crate::MaterialTheme::fallback(crate::PlatformThemeMode::Light);
    crate::apply_material_theme(&ctx, &theme);
    for language in Language::ALL {
        install(&ctx, language);
        let label = Translator(language).text("Settings");
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let width = crate::library_view::settings_button_width(ui, &label);
            crate::library_view::settings_button(
                ui,
                "fixture",
                "Fixture",
                egui::vec2(width, 48.0),
                &label,
                &theme,
            );
        });
        output.textures_delta.clear();
        let galley = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => Some(&text.galley),
                _ => None,
            })
            .expect("settings caption must be painted");
        assert!(!galley.elided, "{language:?}: {label}");
    }
}

#[path = "i18n/layout.rs"]
mod layout;
