use super::*;
use frontend_core::physical_input::{GamepadAxis, GamepadButton, PhysicalInputEvent};
use frontend_core::{MAX_UI_SCALE_PERCENT, MIN_UI_SCALE_PERCENT};

fn button(button: GamepadButton, pressed: bool) -> PhysicalInputEvent {
    PhysicalInputEvent::Button {
        device: 1,
        control: PhysicalControl::Gamepad { button },
        pressed,
    }
}

fn value(app: &FrontendApp) -> u16 {
    let Screen::AppSettings(settings) = &app.screen else {
        panic!("expected app settings");
    };
    settings.draft.ui_scale_percent
}

fn paint(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    events: &[PhysicalInputEvent],
    repeat_at: Option<Instant>,
) -> (egui::Response, egui::Response) {
    crate::apply_material_theme(ctx, &app.material_theme);
    let mut events = Some(events);
    let mut responses = None;
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(453.0, 600.0),
            )),
            ..Default::default()
        },
        |ui| {
            if let Some(events) = events.take() {
                for &event in events {
                    app.process_physical_event(ctx, event, false);
                }
                if let Some(now) = repeat_at {
                    app.repeat_slider_input(ctx, now);
                }
            }
            let Screen::AppSettings(settings) = &mut app.screen else {
                panic!("expected app settings");
            };
            ui.label("Interface scale");
            ui.spacing_mut().slider_width = 220.0;
            let slider = ui.add(controller_slider(
                egui::Slider::new(
                    &mut settings.draft.ui_scale_percent,
                    MIN_UI_SCALE_PERCENT..=MAX_UI_SCALE_PERCENT,
                )
                .suffix("%"),
            ));
            let below = ui
                .horizontal(|ui| {
                    ui.add_space(230.0);
                    ui.add(crate::material_tonal_button(&app.material_theme, "Below"))
                })
                .inner;
            app.focus_ring
                .paint(ctx, &app.material_theme, true, ui.clip_rect());
            responses = Some((slider, below));
        },
    )
    .drop_without_applying_deltas();
    responses.unwrap()
}

fn tap(app: &mut FrontendApp, ctx: &egui::Context, key: GamepadButton) {
    paint(app, ctx, &[button(key, true), button(key, false)], None);
}

fn fixture() -> (
    crate::tests::test_storage::Scratch,
    FrontendApp,
    egui::Context,
    egui::Id,
) {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    let ctx = egui::Context::default();
    let id = paint(&mut app, &ctx, &[], None).0.id;
    ctx.memory_mut(|memory| memory.request_focus(id));
    app.focus_ring.navigate();
    paint(&mut app, &ctx, &[], None);
    (scratch, app, ctx, id)
}

#[test]
fn arrows_adjust_rail_and_numeric_field_with_bounds_and_keep_changes_in_the_draft() {
    let (_scratch, mut app, ctx, rail) = fixture();
    tap(&mut app, &ctx, GamepadButton::DpadRight);
    assert_eq!(value(&app), 101);
    assert_eq!(ctx.memory(egui::Memory::focused), Some(rail));
    tap(&mut app, &ctx, GamepadButton::DpadLeft);
    assert_eq!(value(&app), 100);
    assert_eq!(app.app_settings.ui_scale_percent, 100);
    let below = paint(&mut app, &ctx, &[], None).1.id;
    ctx.memory_mut(|memory| memory.request_focus(below));
    paint(&mut app, &ctx, &[], None);
    tap(&mut app, &ctx, GamepadButton::DpadUp);
    let numeric = ctx.memory(egui::Memory::focused).unwrap();
    assert_ne!(numeric, below);
    assert_ne!(
        numeric, rail,
        "Up from the right column must reach the numeric field"
    );
    // No settling paint: the slider registry must recognize both subwidgets.
    tap(&mut app, &ctx, GamepadButton::DpadRight);
    assert_eq!(value(&app), 101);
    assert_eq!(ctx.memory(egui::Memory::focused), Some(numeric));
    tap(&mut app, &ctx, GamepadButton::DpadDown);
    assert_eq!(ctx.memory(egui::Memory::focused), Some(below));
    assert_eq!(value(&app), 101);
    // Commit the numeric editor's focus loss before changing fixture data.
    paint(&mut app, &ctx, &[], None);
    ctx.memory_mut(|memory| memory.request_focus(rail));
    if let Screen::AppSettings(settings) = &mut app.screen {
        settings.draft.ui_scale_percent = 150;
    }
    paint(&mut app, &ctx, &[], None);
    tap(&mut app, &ctx, GamepadButton::DpadRight);
    assert_eq!(value(&app), 150);
    if let Screen::AppSettings(settings) = &mut app.screen {
        settings.draft.ui_scale_percent = 75;
    }
    tap(&mut app, &ctx, GamepadButton::DpadLeft);
    assert_eq!(value(&app), 74);
    if let Screen::AppSettings(settings) = &mut app.screen {
        settings.draft.ui_scale_percent = 50;
    }
    tap(&mut app, &ctx, GamepadButton::DpadLeft);
    assert_eq!(value(&app), 50);
    app.navigate_back();
    assert_eq!(
        app.app_settings.ui_scale_percent, 100,
        "Back must cancel the draft"
    );
    assert_eq!(
        app.repository.load_app_settings().unwrap().ui_scale_percent,
        100
    );
}

#[test]
fn held_adjustment_repeats_without_bursts_and_stops_on_release_disconnect_or_barrier() {
    let (_scratch, mut app, ctx, rail) = fixture();
    paint(
        &mut app,
        &ctx,
        &[button(GamepadButton::DpadRight, true)],
        None,
    );
    assert_eq!(value(&app), 101);
    let next = app.slider_repeat.as_ref().unwrap().next;
    paint(
        &mut app,
        &ctx,
        &[],
        next.checked_sub(Duration::from_millis(1)),
    );
    assert_eq!(value(&app), 101, "holding has an initial delay");
    let hat = |value| PhysicalInputEvent::Axis {
        device: 1,
        axis: GamepadAxis::HatX,
        value,
    };
    paint(&mut app, &ctx, &[hat(1.0)], None);
    assert_eq!(value(&app), 101, "duplicate d-pad reports must coalesce");
    paint(&mut app, &ctx, &[], Some(next));
    assert_eq!(value(&app), 102);
    paint(
        &mut app,
        &ctx,
        &[button(GamepadButton::DpadRight, false)],
        None,
    );
    paint(&mut app, &ctx, &[], Some(next + Duration::from_secs(5)));
    assert_eq!(value(&app), 103, "a late paint performs only one repeat");
    paint(
        &mut app,
        &ctx,
        &[PhysicalInputEvent::Disconnected { device: 1 }],
        None,
    );
    assert!(app.slider_repeat.is_none());
    paint(&mut app, &ctx, &[], Some(next + Duration::from_secs(10)));
    assert_eq!(value(&app), 103);
    tap(&mut app, &ctx, GamepadButton::DpadLeft);
    assert_eq!(
        value(&app),
        103,
        "after reconnect, first reveal the focused slider"
    );
    paint(
        &mut app,
        &ctx,
        &[button(GamepadButton::DpadLeft, true)],
        None,
    );
    assert_eq!(value(&app), 102);
    let below = paint(&mut app, &ctx, &[], None).1.id;
    ctx.memory_mut(|memory| memory.request_focus(below));
    paint(&mut app, &ctx, &[], Some(next + Duration::from_secs(15)));
    assert!(app.slider_repeat.is_none());
    assert_eq!(value(&app), 102, "focus changes stop repeat");
    paint(
        &mut app,
        &ctx,
        &[button(GamepadButton::DpadLeft, false)],
        None,
    );
    ctx.memory_mut(|memory| memory.request_focus(rail));
    paint(&mut app, &ctx, &[], None);
    paint(&mut app, &ctx, &[hat(1.0)], None);
    assert_eq!(value(&app), 103);
    app.release_all_input();
    assert!(app.slider_repeat.is_none());
    paint(
        &mut app,
        &ctx,
        &[hat(1.0)],
        Some(next + Duration::from_secs(20)),
    );
    assert_eq!(
        value(&app),
        103,
        "held input cannot restart after a barrier"
    );
    paint(&mut app, &ctx, &[hat(0.0)], None);
    paint(&mut app, &ctx, &[hat(1.0)], None);
    assert_eq!(value(&app), 104);
    app.platform_suspended = true;
    paint(&mut app, &ctx, &[], Some(next + Duration::from_secs(25)));
    assert!(app.slider_repeat.is_none());
    assert_eq!(value(&app), 104);
}

#[test]
fn dead_zone_value_is_read_only_and_slider_drag_never_requests_ime() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    let ctx = egui::Context::default();
    crate::apply_material_theme(&ctx, &app.material_theme);
    let frame = |app: &mut FrontendApp, events: Vec<Event>| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(453.0, 1200.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.draw_physical_editor(ui),
        );
        output.textures_delta.clear();
        output
    };
    let original = app
        .physical_editor
        .as_ref()
        .unwrap()
        .draft
        .dead_zone_percent;
    frame(&mut app, vec![]).drop_without_applying_deltas();
    let output = frame(&mut app, vec![]);
    let percent = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.text().trim_end_matches('%') == original.to_string() =>
            {
                Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
            }
            _ => None,
        })
        .expect("the current dead zone percentage is visible");
    let pointer = |pos, pressed| {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    for events in [
        pointer(percent, true),
        pointer(percent, false),
        vec![],
        vec![Event::Text("80".to_owned())],
    ] {
        let output = frame(&mut app, events);
        assert!(output.platform_output.ime.is_none());
        assert_eq!(
            app.physical_editor
                .as_ref()
                .unwrap()
                .draft
                .dead_zone_percent,
            original,
            "the percentage label cannot be edited"
        );
    }
    let rail = ctx.data(|data| {
        data.get_temp::<SliderTargets>(egui::Id::new(SLIDERS_ID))
            .unwrap()
            .targets[0]
            .rect
    });
    let start = rail.center();
    let end = rail.lerp_inside(egui::vec2(0.75, 0.5));
    for events in [
        pointer(start, true),
        vec![Event::PointerMoved(end)],
        pointer(end, false),
        vec![],
    ] {
        assert!(frame(&mut app, events).platform_output.ime.is_none());
    }
    assert!(
        app.physical_editor
            .as_ref()
            .unwrap()
            .draft
            .dead_zone_percent
            > original
    );
    assert_eq!(
        app.app_settings.physical_bindings.dead_zone_percent,
        original
    );
}
