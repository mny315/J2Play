use super::*;
use crate::settings_actions::SettingsAction;

fn paint(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    time: &mut f64,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    *time += 0.016;
    let mut frame = eframe::Frame::_new_kittest();
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 600.0))),
            time: Some(*time),
            events,
            ..Default::default()
        },
        |ui| {
            app.process_control_editor_preparation(ui.ctx());
            app.process_physical_inputs(ui.ctx(), true, false);
            app.ui(ui, &mut frame);
        },
    )
}

fn visible_label(output: &egui::FullOutput, label: &str) -> Option<Pos2> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text() == label => {
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            shape.clip_rect.contains_rect(rect).then_some(rect.center())
        }
        _ => None,
    })
}

#[test]
fn back_cancel_and_save_keep_the_parent_scroll_for_both_editors_and_settings_pages() {
    for global in [false, true] {
        for physical in [false, true] {
            for action in [
                None,
                Some(SettingsAction::Cancel),
                Some(SettingsAction::Save),
            ] {
                check_return(global, physical, action);
            }
        }
    }
}

fn check_return(global: bool, physical: bool, action: Option<SettingsAction>) {
    let scratch = crate::tests::test_storage::Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    let ctx = egui::Context::default();
    crate::apply_material_theme(&ctx, &app.material_theme);
    let mut time = 0.0;
    paint(&mut app, &ctx, &mut time, vec![]).drop_without_applying_deltas();
    if global {
        app.screen = Screen::AppSettings(app.app_settings.clone().into());
    } else {
        let prepared = frontend_core::inspect_import(frontend_core::ImportSource::new(
            None,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/java-me/conformance.jar"
            ))
            .to_vec(),
            None,
        ))
        .unwrap()
        .select_midlet(2)
        .unwrap();
        let entry = app
            .repository
            .commit_import(&prepared, GameSettings::default())
            .unwrap();
        let id = entry.id().to_owned();
        app.entries.push(entry);
        app.open_settings(&id, true);
    }
    let label = if physical {
        "Configure physical controls…"
    } else {
        "Customize controls…"
    };
    let pos = open_editor_by_touch(&mut app, &ctx, &mut time, label);
    assert!(
        app.motion_page().settings_parent().is_some(),
        "the tap must open the editor"
    );
    for _ in 0..6 {
        paint(&mut app, &ctx, &mut time, vec![]).drop_without_applying_deltas();
    }
    if let Some(action) = action {
        app.apply_settings_action(Some(action));
    } else {
        app.navigate_back();
    }
    for _ in 0..25 {
        let output = paint(&mut app, &ctx, &mut time, vec![]);
        let returned =
            visible_label(&output, label).expect("return must keep the editor button visible");
        assert!(
            (returned.y - pos.y).abs() < 1.0,
            "global={global}, physical={physical}, action={action:?}: {pos:?} -> {returned:?}"
        );
        output.drop_without_applying_deltas();
    }
}

fn open_editor_by_touch(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    time: &mut f64,
    label: &str,
) -> Pos2 {
    let mut button = None;
    for step in 0..16 {
        let mut output = paint(
            app,
            ctx,
            time,
            if step == 0 {
                vec![]
            } else {
                vec![
                    egui::Event::PointerMoved(Pos2::new(200.0, 300.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: Vec2::new(0.0, -140.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            },
        );
        for _ in 0..20 {
            output.drop_without_applying_deltas();
            output = paint(app, ctx, time, vec![]);
        }
        button = visible_label(&output, label);
        output.drop_without_applying_deltas();
        if button.is_some() {
            break;
        }
    }
    let pos = button.expect("scrolling must reveal the editor button");
    for pressed in [true, false] {
        let mut events = vec![
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(1),
                phase: if pressed {
                    egui::TouchPhase::Start
                } else {
                    egui::TouchPhase::End
                },
                pos,
                force: None,
            },
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        if !pressed {
            events.push(egui::Event::PointerGone);
        }
        paint(app, ctx, time, events).drop_without_applying_deltas();
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while app.control_editor_preparing() {
        assert!(
            std::time::Instant::now() < deadline,
            "editor preparation did not finish"
        );
        std::thread::sleep(Duration::from_millis(1));
        paint(app, ctx, time, vec![]).drop_without_applying_deltas();
    }
    pos
}
