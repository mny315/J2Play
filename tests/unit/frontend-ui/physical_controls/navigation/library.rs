use super::*;

fn paint(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    input: Option<GamepadButton>,
) -> Option<egui::Response> {
    let events = input.map_or_else(Vec::new, |button| {
        [true, false]
            .into_iter()
            .map(|pressed| PhysicalInputEvent::Button {
                device: 1,
                control: PhysicalControl::Gamepad { button },
                pressed,
            })
            .collect()
    });
    paint_events(app, ctx, events)
}

fn paint_events(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    mut events: Vec<PhysicalInputEvent>,
) -> Option<egui::Response> {
    let mut focused = None;
    crate::apply_material_theme(ctx, &app.material_theme);
    let mut style = (*ctx.global_style()).clone();
    style.scroll_animation = egui::style::ScrollAnimation::none();
    ctx.set_global_style(style);
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(453.0, 440.0),
            )),
            ..Default::default()
        },
        |ui| {
            for event in events.drain(..) {
                app.process_physical_event(ctx, event, false);
            }
            app.draw_library(ui);
            app.focus_ring
                .paint(ctx, &app.material_theme, true, ui.clip_rect());
            focused = ctx
                .memory(egui::Memory::focused)
                .and_then(|id| ctx.read_response(id));
        },
    )
    .drop_without_applying_deltas();
    focused
}

#[test]
fn a_dpad_reported_as_keys_and_hat_moves_only_one_library_row() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    for index in 1..=3 {
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
        .select_midlet(index)
        .unwrap();
        app.entries.push(
            app.repository
                .commit_import(&prepared, crate::GameSettings::default())
                .unwrap(),
        );
    }
    let ctx = egui::Context::default();
    paint(&mut app, &ctx, None);
    let first = step(&mut app, &ctx, GamepadButton::DpadDown).id;
    let button = |pressed| PhysicalInputEvent::Button {
        device: 1,
        control: PhysicalControl::Gamepad {
            button: GamepadButton::DpadDown,
        },
        pressed,
    };
    let hat = |value| PhysicalInputEvent::Axis {
        device: 1,
        axis: GamepadAxis::HatY,
        value,
    };
    paint_events(&mut app, &ctx, vec![button(true)]);
    let second = paint(&mut app, &ctx, None).unwrap().id;
    assert_ne!(first, second);
    paint_events(&mut app, &ctx, vec![hat(1.0)]);
    assert_eq!(paint(&mut app, &ctx, None).unwrap().id, second);
    paint_events(&mut app, &ctx, vec![button(false), hat(0.0)]);
    paint_events(&mut app, &ctx, vec![hat(1.0)]);
    let third = paint(&mut app, &ctx, None).unwrap().id;
    assert_ne!(second, third);
    assert_ne!(first, third);
}

fn step(app: &mut FrontendApp, ctx: &egui::Context, button: GamepadButton) -> egui::Response {
    paint(app, ctx, Some(button));
    // Directional focus is resolved by egui at the end of the pass. These
    // paints also apply scrolling, but contain no additional physical input.
    for _ in 0..2 {
        paint(app, ctx, None);
    }
    paint(app, ctx, None).expect("one press must leave a visible focus target")
}

#[test]
fn each_direction_moves_one_row_and_left_right_reach_settings_after_scrolling() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    for index in 1..=8 {
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
        .select_midlet(index)
        .unwrap();
        app.entries.push(
            app.repository
                .commit_import(&prepared, crate::GameSettings::default())
                .unwrap(),
        );
    }
    let ctx = egui::Context::default();
    paint(&mut app, &ctx, None);
    let first = step(&mut app, &ctx, GamepadButton::DpadDown);
    assert!(first.rect.width() > 200.0, "{first:?}");
    for button in [
        GamepadButton::DpadUp,
        GamepadButton::DpadLeft,
        GamepadButton::DpadRight,
        GamepadButton::South,
    ] {
        ctx.memory_mut(|memory| memory.surrender_focus(first.id));
        assert_eq!(step(&mut app, &ctx, button).id, first.id);
        assert!(matches!(app.screen, Screen::Library));
    }
    let mut rows = vec![first.id];
    for _ in 1..8 {
        let next = step(&mut app, &ctx, GamepadButton::DpadDown);
        assert!(
            next.rect.width() > 200.0,
            "Down must stay on the launch column: {next:?}"
        );
        assert!(
            !rows.contains(&next.id),
            "each Down must reach the next row"
        );
        assert!(
            next.interact_rect.height() >= 48.0,
            "focus must scroll into view: {next:?}"
        );
        rows.push(next.id);
    }
    for expected in rows.iter().rev().skip(1) {
        assert_eq!(step(&mut app, &ctx, GamepadButton::DpadUp).id, *expected);
    }
    for _ in 1..8 {
        step(&mut app, &ctx, GamepadButton::DpadDown);
    }
    ctx.memory_mut(|memory| memory.surrender_focus(*rows.last().unwrap()));
    let restarted = step(&mut app, &ctx, GamepadButton::DpadDown);
    assert_eq!(restarted.id, first.id);
    assert!((restarted.rect.top() - first.rect.top()).abs() < 1.0);
    let settings = step(&mut app, &ctx, GamepadButton::DpadRight);
    assert!(settings.rect.width() < first.rect.width());
    assert_eq!(step(&mut app, &ctx, GamepadButton::DpadLeft).id, first.id);
    assert_eq!(
        step(&mut app, &ctx, GamepadButton::DpadRight).id,
        settings.id
    );
    paint(&mut app, &ctx, Some(GamepadButton::South));
    assert!(
        matches!(app.screen, Screen::Settings(_)),
        "confirmation must work without waiting for the ring animation"
    );
}
