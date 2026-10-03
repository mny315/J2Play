use super::*;
use std::collections::BTreeSet;

#[test]
fn down_from_profile_and_full_width_buttons_keeps_the_left_column() {
    for width in [360.0, 453.0, 800.0] {
        let scratch = crate::tests::test_storage::Scratch::new();
        let root = &scratch.0;
        let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
        app.startup_splash = None;
        app.focus_ring.navigate();
        let size = egui::vec2(width, 600.0);
        for global in [false, true] {
            let ctx = egui::Context::default();
            open_settings_page(&mut app, global);
            step_at_size(&mut app, &ctx, size, None)
                .1
                .drop_without_applying_deltas();
            let (focused, output) =
                step_at_size(&mut app, &ctx, size, Some(GamepadButton::DpadDown));
            assert_focused_label(
                &focused,
                &output,
                if global {
                    "Device profile"
                } else {
                    "Show all profiles"
                },
            );
            output.drop_without_applying_deltas();
        }
    }
}

fn step_at_size(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    size: egui::Vec2,
    button: Option<GamepadButton>,
) -> (egui::Response, egui::FullOutput) {
    paint_settings_at_size(app, ctx, button, size)
        .1
        .drop_without_applying_deltas();
    for _ in 0..3 {
        paint_settings_at_size(app, ctx, None, size)
            .1
            .drop_without_applying_deltas();
    }
    let (response, output) = paint_settings_at_size(app, ctx, None, size);
    (response.expect("settings must retain focus"), output)
}

fn collect_label(
    focused: &egui::Response,
    output: egui::FullOutput,
    labels: &mut BTreeSet<String>,
) {
    for shape in &output.shapes {
        if let egui::Shape::Text(text) = &shape.shape
            && focused.rect.contains(text.pos)
        {
            labels.insert(text.galley.job.text.clone());
        }
    }
    output.drop_without_applying_deltas();
}

#[test]
fn game_and_app_settings_visit_small_choices_and_keep_horizontal_steps_in_the_row() {
    for global in [false, true] {
        for width in [360.0, 453.0, 800.0] {
            let scratch = crate::tests::test_storage::Scratch::new();
            let root = &scratch.0;
            let mut app =
                FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
            app.startup_splash = None;
            app.focus_ring.navigate();
            open_settings_page(&mut app, global);
            let ctx = egui::Context::default();
            let size = egui::vec2(width, 480.0);
            let (mut focused, output) = step_at_size(&mut app, &ctx, size, None);
            let mut labels = BTreeSet::new();
            collect_label(&focused, output, &mut labels);
            for _ in 0..48 {
                if !focused.sense.senses_drag() {
                    // Explore each row from left to right, then go to the next.
                    for button in [GamepadButton::DpadLeft, GamepadButton::DpadRight] {
                        for _ in 0..12 {
                            let (next, output) = step_at_size(&mut app, &ctx, size, Some(button));
                            collect_label(&next, output, &mut labels);
                            assert!(
                                (next.rect.center().y - focused.rect.center().y).abs() < 2.0,
                                "{button:?} left its row in global={global}, width={width}: {labels:?}"
                            );
                            if next.id == focused.id {
                                break;
                            }
                            focused = next;
                        }
                    }
                }
                let (next, output) =
                    step_at_size(&mut app, &ctx, size, Some(GamepadButton::DpadDown));
                collect_label(&next, output, &mut labels);
                if next.id == focused.id {
                    break;
                }
                focused = next;
            }
            let expected: &[&str] = if global {
                &[
                    "Device profile",
                    "Fixed limit",
                    "Off",
                    "On",
                    "Landscape only",
                    "System",
                    "Light",
                    "Dark",
                    "OLED",
                    "Blue",
                    "Teal",
                    "Green",
                    "Amber",
                    "Orange",
                    "Red",
                    "Pink",
                    "Purple",
                    "System default (100%)",
                    "Auto",
                    "List",
                    "Tiles",
                ]
            } else {
                &[
                    "Automatic",
                    "Show all profiles",
                    "Ask every time",
                    "Always continue",
                    "Start normally",
                    "Automatic fit",
                    "Manual scale",
                    "App default",
                    "Manual limit",
                    "Use app default",
                    "Off",
                    "On",
                    "Landscape only",
                    "Use global controls",
                ]
            };
            for label in expected.iter().copied().chain([
                "Customize controls…",
                "Configure physical controls…",
                "Cancel",
                "Save",
            ]) {
                let label = crate::i18n::Translator::from_context(&ctx).text(label);
                assert!(
                    labels.contains(label.as_str()),
                    "missing {label} in global={global}, width={width}: {labels:?}"
                );
            }
        }
    }
}

fn profile_text_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == label => Some(text.pos),
            _ => None,
        })
        .expect("profile must be visible")
}

#[test]
fn focusing_a_visible_profile_does_not_move_its_text_or_adjacent_rows() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    app.focus_ring.navigate();
    open_settings_page(&mut app, false);
    let ctx = egui::Context::default();
    paint_settings(&mut app, &ctx, None)
        .1
        .drop_without_applying_deltas();
    let (_, before) = settings_step(&mut app, &ctx, GamepadButton::South);
    let options: Vec<_> = app
        .profile_options
        .iter()
        .filter(|option| option.visible_in_menu(false, &crate::ProfileChoice::Automatic))
        .collect();
    let tr = crate::i18n::Translator(frontend_core::Language::English);
    let first = tr.profile_name(options[0].menu_name(false));
    let second = tr.profile_name(options[1].menu_name(false));
    let positions = [
        profile_text_position(&before, &first),
        profile_text_position(&before, &second),
    ];
    before.drop_without_applying_deltas();
    let (_, focused) = settings_step(&mut app, &ctx, GamepadButton::DpadDown);
    assert_eq!(
        positions,
        [
            profile_text_position(&focused, &first),
            profile_text_position(&focused, &second)
        ]
    );
    focused.drop_without_applying_deltas();
    let (_, unfocused) = settings_step(&mut app, &ctx, GamepadButton::DpadUp);
    assert_eq!(
        positions,
        [
            profile_text_position(&unfocused, &first),
            profile_text_position(&unfocused, &second)
        ]
    );
    unfocused.drop_without_applying_deltas();
}
