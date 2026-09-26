use super::*;

mod connected;
mod library;
mod popup;
mod settings_rows;

fn paint_settings(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    button: Option<GamepadButton>,
) -> (Option<egui::Response>, egui::FullOutput) {
    paint_settings_at_size(app, ctx, button, egui::vec2(453.0, 600.0))
}

fn paint_settings_at_size(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    mut button: Option<GamepadButton>,
    size: egui::Vec2,
) -> (Option<egui::Response>, egui::FullOutput) {
    use eframe::App;
    crate::apply_material_theme(ctx, &app.material_theme);
    let mut style = (*ctx.global_style()).clone();
    style.scroll_animation = egui::style::ScrollAnimation::none();
    ctx.set_global_style(style);
    let mut frame = eframe::Frame::_new_kittest();
    let mut focused = None;
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        },
        |ui| {
            if let Some(button) = button.take() {
                for pressed in [true, false] {
                    app.process_physical_event(
                        ctx,
                        PhysicalInputEvent::Button {
                            device: 1,
                            control: PhysicalControl::Gamepad { button },
                            pressed,
                        },
                        false,
                    );
                }
            }
            app.ui(ui, &mut frame);
            focused = ctx
                .memory(egui::Memory::focused)
                .and_then(|id| ctx.read_response(id));
        },
    );
    (focused, output)
}

fn settings_step(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    button: GamepadButton,
) -> (egui::Response, egui::FullOutput) {
    paint_settings(app, ctx, Some(button))
        .1
        .drop_without_applying_deltas();
    for _ in 0..2 {
        paint_settings(app, ctx, None)
            .1
            .drop_without_applying_deltas();
    }
    let (response, output) = paint_settings(app, ctx, None);
    (response.expect("navigation must keep focus"), output)
}

fn text_is_visible(output: &egui::FullOutput, label: &str) -> bool {
    output.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text)
            if text.galley.job.text == label
                && shape.clip_rect.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size())))
    })
}

pub(super) fn open_settings_page(app: &mut FrontendApp, global: bool) {
    app.screen = if global {
        Screen::AppSettings(app.app_settings.clone().into())
    } else {
        Screen::Settings(Box::new(crate::SettingsScreen {
            target: crate::SettingsTarget::Existing {
                entry_id: "fixture".to_owned(),
            },
            draft: crate::GameSettings::default(),
            focus_profile: false,
            show_all_profiles: false,
            editor_request: None,
            control_editor: None,
        }))
    };
}

fn assert_first_setting(focused: &egui::Response, output: &egui::FullOutput, global: bool) {
    let label = if global { "System" } else { "Automatic" };
    assert!(
        output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == label && focused.rect.contains(text.pos))),
        "focus must start on {label}, got {:?}",
        focused.rect
    );
    assert!(text_is_visible(
        output,
        if global {
            "App settings"
        } else {
            "Game settings"
        }
    ));
}

#[test]
fn first_controller_direction_in_settings_reveals_the_top_control() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    let ctx = egui::Context::default();
    for global in [false, true] {
        for direction in [
            GamepadButton::DpadUp,
            GamepadButton::DpadDown,
            GamepadButton::DpadLeft,
            GamepadButton::DpadRight,
        ] {
            app.screen = Screen::Library;
            paint_settings(&mut app, &ctx, None)
                .1
                .drop_without_applying_deltas();
            app.focus_ring.hide();
            open_settings_page(&mut app, global);
            let (_, output) = paint_settings(&mut app, &ctx, None);
            assert!(
                !output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(rect) if (rect.stroke.width - 2.5).abs() < f32::EPSILON)),
                "opening settings by touch must not reveal the ring"
            );
            output.drop_without_applying_deltas();
            let (first, output) = settings_step(&mut app, &ctx, direction);
            assert_first_setting(&first, &output, global);
            output.drop_without_applying_deltas();
            let (next, output) = settings_step(&mut app, &ctx, GamepadButton::DpadDown);
            assert_ne!(next.id, first.id, "later input must navigate normally");
            output.drop_without_applying_deltas();
        }
    }
}

#[test]
fn entering_settings_focuses_the_top_and_returning_from_an_editor_restores_the_selection() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    let ctx = egui::Context::default();
    app.focus_ring.navigate();
    for global in [false, true] {
        open_settings_page(&mut app, global);
        let (focused, output) = paint_settings(&mut app, &ctx, None);
        assert_first_setting(
            &focused.expect("entry needs an initial focus"),
            &output,
            global,
        );
        output.drop_without_applying_deltas();
        for _ in 0..4 {
            settings_step(&mut app, &ctx, GamepadButton::DpadDown)
                .1
                .drop_without_applying_deltas();
        }
        let previous = ctx.memory(egui::Memory::focused).unwrap();
        app.open_physical_editor();
        paint_settings(&mut app, &ctx, None)
            .1
            .drop_without_applying_deltas();
        app.close_physical_editor(false);
        for _ in 0..3 {
            paint_settings(&mut app, &ctx, None)
                .1
                .drop_without_applying_deltas();
        }
        let (focused, output) = paint_settings(&mut app, &ctx, None);
        assert_eq!(focused.expect("return restores the selection").id, previous);
        output.drop_without_applying_deltas();
    }
}

#[test]
fn controller_opens_the_diagram_picker_assigns_and_returns_to_the_same_control() {
    use super::super::selection::{BindingTarget, Picker};
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    let ctx = egui::Context::default();
    let (initial, output) = paint_settings(&mut app, &ctx, None);
    let initial = initial.unwrap().id;
    assert!(text_is_visible(&output, "L2 / LT"));
    assert!(
        !output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Rect(rect) if (rect.stroke.width - 2.5).abs() < f32::EPSILON)),
        "opening the editor must not show a focus ring"
    );
    output.drop_without_applying_deltas();
    let (revealed, output) = settings_step(&mut app, &ctx, GamepadButton::DpadDown);
    assert_eq!(
        revealed.id, initial,
        "the first navigation press reveals the initial control"
    );
    output.drop_without_applying_deltas();
    let (_, output) = settings_step(&mut app, &ctx, GamepadButton::South);
    let target = BindingTarget::Trigger { right: false };
    assert_eq!(
        app.physical_editor.as_ref().unwrap().picker,
        Some(Picker::Binding(target))
    );
    output.drop_without_applying_deltas();
    let (first_action, output) = paint_settings(&mut app, &ctx, None);
    assert_ne!(
        first_action.unwrap().id,
        initial,
        "focus must enter the picker"
    );
    output.drop_without_applying_deltas();
    // The first unassigned target starts on L soft. Right reaches Up.
    settings_step(&mut app, &ctx, GamepadButton::DpadRight)
        .1
        .drop_without_applying_deltas();
    settings_step(&mut app, &ctx, GamepadButton::South)
        .1
        .drop_without_applying_deltas();
    let editor = app.physical_editor.as_ref().unwrap();
    assert!(!editor.capturing());
    for control in target.controls() {
        assert_eq!(
            editor.draft.resolve(control),
            Some(PhysicalAction::Phone(HostAction::Up))
        );
    }
    let (returned, output) = paint_settings(&mut app, &ctx, None);
    assert_eq!(returned.unwrap().id, initial);
    output.drop_without_applying_deltas();
    settings_step(&mut app, &ctx, GamepadButton::South)
        .1
        .drop_without_applying_deltas();
    paint_settings(&mut app, &ctx, Some(GamepadButton::East))
        .1
        .drop_without_applying_deltas();
    assert!(app.physical_editor.is_some());
    assert!(!app.physical_editor.as_ref().unwrap().capturing());
}

#[test]
fn returning_to_first_game_setting_reveals_the_page_title_and_profile() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    app.screen = Screen::Settings(Box::new(crate::SettingsScreen {
        target: crate::SettingsTarget::Existing {
            entry_id: "fixture".to_owned(),
        },
        draft: crate::GameSettings::default(),
        focus_profile: true,
        show_all_profiles: false,
        editor_request: None,
        control_editor: None,
    }));
    let ctx = egui::Context::default();
    let (first, output) = paint_settings(&mut app, &ctx, None);
    let first = first.unwrap().id;
    output.drop_without_applying_deltas();
    for _ in 0..5 {
        let (_, output) = settings_step(&mut app, &ctx, GamepadButton::DpadDown);
        output.drop_without_applying_deltas();
    }
    let (_, output) = paint_settings(&mut app, &ctx, None);
    assert!(!text_is_visible(&output, "Game settings"));
    output.drop_without_applying_deltas();
    let mut returned = false;
    for _ in 0..16 {
        let (focused, output) = settings_step(&mut app, &ctx, GamepadButton::DpadUp);
        returned = focused.id == first;
        if returned {
            for label in ["Game settings", "Device profile"] {
                assert!(text_is_visible(&output, label), "missing context: {label}");
            }
        }
        output.drop_without_applying_deltas();
        if returned {
            break;
        }
    }
    assert!(returned, "Up must reach the first setting");
}

#[test]
fn down_does_not_select_a_slightly_offset_button_in_the_same_row() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.focus_ring.navigate();
    let ctx = egui::Context::default();
    let mut ids = Vec::new();
    for frame in 0..4 {
        ctx.run_ui(egui::RawInput::default(), |ui| {
            if frame == 1 {
                for pressed in [true, false] {
                    app.process_physical_event(
                        &ctx,
                        PhysicalInputEvent::Button {
                            device: 1,
                            control: PhysicalControl::Gamepad {
                                button: GamepadButton::DpadDown,
                            },
                            pressed,
                        },
                        false,
                    );
                }
            }
            ids.clear();
            for (index, position) in [
                egui::pos2(100.0, 20.0),
                egui::pos2(195.0, 22.0),
                egui::pos2(100.0, 240.0),
            ]
            .into_iter()
            .enumerate()
            {
                let response = ui.put(
                    egui::Rect::from_min_size(position, egui::vec2(80.0, 48.0)),
                    crate::material_outlined_button(&app.material_theme, index.to_string()),
                );
                if frame == 0 && index == 0 {
                    response.request_focus();
                }
                ids.push(response.id);
            }
        })
        .drop_without_applying_deltas();
    }
    assert_eq!(
        ctx.memory(egui::Memory::focused),
        Some(ids[2]),
        "Down must not drift sideways within the current row"
    );
}

fn assert_focused_label(focused: &egui::Response, output: &egui::FullOutput, label: &str) {
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == label
            && focused.rect.contains(text.pos)
            && shape.clip_rect.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))
    )), "focus must show {label}: {:?}", focused.rect);
}
