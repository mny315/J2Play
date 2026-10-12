use super::*;
use crate::physical_controls::editor::EditorTab;
use crate::{FrontendApp, Screen};

fn catalog() -> Vec<u16> {
    KeyGroup::ALL
        .into_iter()
        .filter(|group| *group != KeyGroup::Assigned)
        .flat_map(|group| group.usages(&PhysicalBindings::default()))
        .collect()
}

#[test]
fn keyboard_catalog_includes_unassigned_keys_modifiers_and_distinct_number_pad() {
    let mut bindings = PhysicalBindings::default();
    let usages = catalog();
    let unique: std::collections::BTreeSet<_> = usages.iter().copied().collect();
    assert_eq!(usages.len(), 103);
    assert_eq!(unique.len(), usages.len());
    for usage in usages {
        let control = PhysicalControl::Keyboard { usage };
        assert!(control.valid());
        assert!(
            !key_label(usage).starts_with("Key "),
            "missing label for {usage}"
        );
        bindings
            .assign(control, PhysicalAction::Phone(HostAction::Fire))
            .unwrap();
    }
    assert_ne!(
        key_label(30),
        key_label(89),
        "1 and number pad 1 are separate inputs"
    );
    assert_ne!(
        key_label(224),
        key_label(228),
        "left and right Ctrl are separate inputs"
    );
    let unusual = PhysicalControl::Keyboard { usage: 135 };
    bindings.assign(unusual, PhysicalAction::StopGame).unwrap();
    assert!(
        KeyGroup::Assigned.usages(&bindings).contains(&135),
        "existing non-catalog keys remain editable"
    );
    bindings.remove(unusual);
    assert!(!KeyGroup::Assigned.usages(&bindings).contains(&135));
    assert!(KeyGroup::Letters.usages(&bindings).contains(&4));
}

fn frame(app: &mut FrontendApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    use eframe::App;
    crate::apply_material_theme(ctx, &app.material_theme);
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(360.0, 800.0),
            )),
            events,
            ..Default::default()
        },
        |ui| app.ui(ui, &mut eframe::Frame::_new_kittest()),
    )
}

fn text_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing label: {label}"))
}

fn click(app: &mut FrontendApp, ctx: &egui::Context, pos: egui::Pos2) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .drop_without_applying_deltas();
    }
    frame(app, ctx, Vec::new()).drop_without_applying_deltas();
}

#[test]
fn unassigned_keyboard_key_can_be_picked_by_touch_and_saved_only_to_parent_draft() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    let ctx = egui::Context::default();
    for _ in 0..3 {
        frame(&mut app, &ctx, Vec::new()).drop_without_applying_deltas();
    }
    let output = frame(&mut app, &ctx, Vec::new());
    let keyboard = text_center(&output, "Keyboard");
    output.drop_without_applying_deltas();
    click(&mut app, &ctx, keyboard);
    assert_eq!(
        app.physical_editor.as_ref().unwrap().tab,
        EditorTab::Keyboard
    );
    let output = frame(&mut app, &ctx, Vec::new());
    let b = text_center(&output, "B");
    output.drop_without_applying_deltas();
    let control = PhysicalControl::Keyboard { usage: 5 };
    assert_eq!(
        app.physical_editor.as_ref().unwrap().draft.resolve(control),
        None
    );
    click(&mut app, &ctx, b);
    let target = BindingTarget::Control(control);
    assert_eq!(
        app.physical_editor.as_ref().unwrap().picker,
        Some(Picker::Binding(target))
    );
    let output = frame(&mut app, &ctx, Vec::new());
    let fire = text_center(&output, "Fire / OK");
    output.drop_without_applying_deltas();
    click(&mut app, &ctx, fire);
    assert_eq!(
        app.physical_editor.as_ref().unwrap().draft.resolve(control),
        Some(PhysicalAction::Phone(HostAction::Fire))
    );
    app.close_physical_editor(true);
    assert_eq!(app.app_settings.physical_bindings.resolve(control), None);
    assert_eq!(
        app.repository
            .load_app_settings()
            .unwrap()
            .physical_bindings
            .resolve(control),
        None
    );
    app.open_physical_editor();
    assert_eq!(
        app.physical_editor.as_ref().unwrap().draft.resolve(control),
        Some(PhysicalAction::Phone(HostAction::Fire))
    );
    app.close_physical_editor(false);
    app.cancel_settings_screen();
    assert_eq!(app.app_settings.physical_bindings.resolve(control), None);
}

#[test]
fn keyboard_capture_ignores_gamepad_navigation_and_accepts_a_modifier() {
    use frontend_core::physical_input::{GamepadButton, PhysicalInputEvent};
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    let editor = app.physical_editor.as_mut().unwrap();
    editor.tab = EditorTab::Keyboard;
    editor.listening = Some(std::time::Instant::now());
    let ctx = egui::Context::default();
    ctx.input_mut(|input| input.focused = true);
    app.process_physical_event(
        &ctx,
        PhysicalInputEvent::Button {
            device: 1,
            control: PhysicalControl::Gamepad {
                button: GamepadButton::East,
            },
            pressed: true,
        },
        false,
    );
    assert!(app.physical_editor.as_ref().unwrap().listening.is_some());
    let control = PhysicalControl::Keyboard { usage: 229 };
    app.process_physical_event(
        &ctx,
        PhysicalInputEvent::Button {
            device: 2,
            control,
            pressed: true,
        },
        false,
    );
    assert_eq!(
        app.physical_editor.as_ref().unwrap().picker,
        Some(Picker::Binding(BindingTarget::Control(control)))
    );
}

#[test]
fn key_tiles_keep_touch_targets_and_visible_labels_at_phone_ui_scales() {
    let ctx = egui::Context::default();
    let theme = crate::MaterialTheme::fallback(crate::PlatformThemeMode::Dark);
    crate::apply_material_theme(&ctx, &theme);
    for language in frontend_core::Language::ALL {
        crate::i18n::install(&ctx, language);
        for width in [200.0, 280.0, 320.0, 360.0, 480.0, 800.0] {
            for group in KeyGroup::ALL {
                let bindings = PhysicalBindings::default();
                let controls: Vec<_> = group
                    .usages(&bindings)
                    .into_iter()
                    .map(|usage| PhysicalControl::Keyboard { usage })
                    .collect();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 2000.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        draw_grid(ui, &theme, &bindings, &controls);
                    },
                );
                output.textures_delta.clear();
                let tiles: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect)
                            if rect.fill == theme.secondary_container
                                || rect.fill == theme.surface_container =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(tiles.len(), controls.len());
                for (index, rect) in tiles.iter().enumerate() {
                    assert!(
                        rect.width() >= 48.0 && rect.height() >= 48.0,
                        "{width}: {rect:?}"
                    );
                    assert!(rect.right() <= width, "{width}: {rect:?}");
                    for other in &tiles[index + 1..] {
                        assert!(
                            !rect.intersect(*other).is_positive(),
                            "{width} {group:?}: {rect:?} overlaps {other:?}"
                        );
                    }
                }
                for shape in &output.shapes {
                    if let egui::Shape::Text(text) = &shape.shape {
                        let bounds = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(
                            tiles.iter().any(|rect| rect.contains_rect(bounds)),
                            "{language:?}, {width}: {:?} at {bounds:?}",
                            text.galley.job.text
                        );
                    }
                }
                output.drop_without_applying_deltas();
            }
        }
    }
}
