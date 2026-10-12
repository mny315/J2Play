use super::*;
use std::collections::HashSet;

fn options() -> Vec<ProfileOption> {
    launch::builtin_device_profiles()
        .unwrap()
        .iter()
        .map(ProfileOption::from_profile)
        .collect()
}

#[test]
fn basic_menu_covers_every_screen_and_input_without_duplicate_choices() {
    let options = options();
    let basic: Vec<_> = options
        .iter()
        .filter(|option| option.basic_name.is_some())
        .collect();
    assert_eq!(
        basic.len(),
        BASIC_PROFILES.len(),
        "every curated ID must exist"
    );
    assert_eq!(basic.len(), 25);
    assert_eq!(
        basic
            .iter()
            .map(|option| option.menu_name(false))
            .collect::<HashSet<_>>()
            .len(),
        basic.len()
    );
    let combinations = |options: &[&ProfileOption]| {
        options
            .iter()
            .map(|option| {
                (
                    option.short_manufacturer.clone(),
                    option.canvas_dimensions,
                    option.touch,
                )
            })
            .collect::<HashSet<_>>()
    };
    assert_eq!(
        combinations(&basic),
        combinations(&options.iter().collect::<Vec<_>>())
    );
    let se_qvga: Vec<_> = basic
        .iter()
        .filter(|option| {
            option.short_manufacturer == "SE"
                && option.canvas_dimensions == Some((240, 320))
                && !option.touch
        })
        .collect();
    assert_eq!(se_qvga.len(), 1);
    assert_eq!(se_qvga[0].profile_id, "se-jp8-late-keypad");
    assert_eq!(
        se_qvga[0].menu_name(false),
        "Sony Ericsson · 240×320 · Keypad phone"
    );
    for family in ["Series 40", "S60", "Asha"] {
        assert!(
            basic
                .iter()
                .any(|option| option.menu_name(false).contains(family))
        );
    }
}

#[test]
fn every_saved_historical_choice_remains_visible_without_replacing_its_id() {
    let options = options();
    for selected in &options {
        let choice = ProfileChoice::Manual {
            profile_id: selected.profile_id.clone(),
        };
        assert!(selected.visible_in_menu(false, &choice));
        assert!(
            options
                .iter()
                .all(|option| option.visible_in_menu(true, &choice))
        );
        for option in &options {
            assert_eq!(
                option.visible_in_menu(false, &choice),
                option.basic_name.is_some() || option.profile_id == selected.profile_id
            );
        }
        if selected.basic_name.is_none() {
            assert_eq!(selected.menu_name(false), selected.display_name);
        }
    }
}

#[test]
fn all_profile_labels_translate_input_consistently_without_changing_keypad_controls() {
    let tr = crate::i18n::Translator(frontend_core::Language::Russian);
    for option in options() {
        for full in [false, true] {
            let label = tr.profile_name(option.menu_name(full));
            assert!(
                label.contains(if option.touch {
                    "Сенсорный"
                } else {
                    "Кнопочный"
                }),
                "{label}"
            );
        }
    }
    assert_eq!(tr.text("Keypad"), "Цифровые клавиши");
}

#[test]
fn profile_input_labels_remain_readable_on_narrow_screens_in_every_language() {
    let options = options();
    let ctx = egui::Context::default();
    for language in frontend_core::Language::ALL {
        crate::i18n::install(&ctx, language);
        for width in [240.0, 320.0] {
            for id in [
                "se-jp8-late-keypad",
                "nokia-featurephone",
                "nokia-s60-touch",
            ] {
                for all in [false, true] {
                    let mut choice = ProfileChoice::Manual {
                        profile_id: id.to_owned(),
                    };
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 600.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            crate::apply_settings_style(ui);
                            draw(ui, &options, &mut choice, all);
                        },
                    );
                    output.textures_delta.clear();
                    let text = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) => Some(text),
                            _ => None,
                        })
                        .expect("selected profile label must be painted");
                    assert!(
                        !text.galley.elided,
                        "{language:?}, {width}, {id}: {:?}",
                        text.galley.job.text
                    );
                    assert!(text.pos.x >= -0.5 && text.pos.x + text.galley.size().x <= width + 0.5);
                    output.drop_without_applying_deltas();
                }
            }
        }
    }
}
