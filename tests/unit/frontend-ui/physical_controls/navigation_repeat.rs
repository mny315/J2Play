use super::*;
use eframe::App;
use frontend_core::physical_input::{GamepadAxis, GamepadButton, PhysicalInputEvent};

fn button(button: GamepadButton, pressed: bool) -> PhysicalInputEvent {
    PhysicalInputEvent::Button {
        device: 1,
        control: PhysicalControl::Gamepad { button },
        pressed,
    }
}

fn axis(axis: GamepadAxis, value: f32) -> PhysicalInputEvent {
    PhysicalInputEvent::Axis {
        device: 1,
        axis,
        value,
    }
}

fn paint(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    events: &[PhysicalInputEvent],
    mut repeat_at: Option<Instant>,
) -> Option<egui::Response> {
    crate::apply_material_theme(ctx, &app.material_theme);
    let mut style = (*ctx.global_style()).clone();
    style.scroll_animation = egui::style::ScrollAnimation::none();
    ctx.set_global_style(style);
    let mut frame = eframe::Frame::_new_kittest();
    let mut events = Some(events);
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(453.0, 440.0),
            )),
            ..Default::default()
        },
        |ui| {
            if let Some(events) = events.take() {
                for &event in events {
                    app.process_physical_event(ctx, event, false);
                }
            }
            if let Some(now) = repeat_at.take() {
                app.repeat_navigation_input(ctx, now);
            }
            app.ui(ui, &mut frame);
        },
    )
    .drop_without_applying_deltas();
    ctx.memory(egui::Memory::focused)
        .and_then(|id| ctx.read_response(id))
}

fn settle(app: &mut FrontendApp, ctx: &egui::Context) -> egui::Response {
    for _ in 0..2 {
        paint(app, ctx, &[], None);
    }
    paint(app, ctx, &[], None).expect("navigation keeps a visible focus")
}

fn fixture() -> (
    crate::tests::test_storage::Scratch,
    FrontendApp,
    egui::Context,
) {
    let scratch = crate::tests::test_storage::Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
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
    paint(&mut app, &ctx, &[], None);
    paint(
        &mut app,
        &ctx,
        &[button(GamepadButton::DpadDown, true)],
        None,
    );
    settle(&mut app, &ctx);
    (scratch, app, ctx)
}

#[test]
fn held_direction_scrolls_the_library_after_a_delay_without_catch_up_bursts() {
    let (_scratch, mut app, ctx) = fixture();
    let first = settle(&mut app, &ctx).id;
    let next = app.navigation_repeat.as_ref().unwrap().next;
    paint(
        &mut app,
        &ctx,
        &[],
        Some(next.checked_sub(Duration::from_millis(1)).unwrap()),
    );
    assert_eq!(settle(&mut app, &ctx).id, first);
    let mut rows = vec![first];
    for step in 0..7 {
        let now = next + Duration::from_secs(step * 10);
        paint(&mut app, &ctx, &[], Some(now));
        let focused = settle(&mut app, &ctx);
        assert!(
            !rows.contains(&focused.id),
            "each repeat reaches one new row"
        );
        assert!(
            focused.interact_rect.height() >= 48.0,
            "scroll focus into view"
        );
        rows.push(focused.id);
        paint(&mut app, &ctx, &[], Some(now));
        assert_eq!(settle(&mut app, &ctx).id, focused.id, "no repeat burst");
    }
    // At the bottom, holding Down stays on the last row instead of wrapping.
    paint(&mut app, &ctx, &[], Some(next + Duration::from_secs(80)));
    assert_eq!(settle(&mut app, &ctx).id, rows[7]);
    paint(
        &mut app,
        &ctx,
        &[
            button(GamepadButton::DpadDown, false),
            axis(GamepadAxis::LeftY, -1.0),
        ],
        None,
    );
    assert_eq!(settle(&mut app, &ctx).id, rows[6]);
    let next = app.navigation_repeat.as_ref().unwrap().next;
    paint(&mut app, &ctx, &[], Some(next));
    assert_eq!(settle(&mut app, &ctx).id, rows[5]);
    paint(&mut app, &ctx, &[axis(GamepadAxis::LeftY, 0.0)], None);
    paint(&mut app, &ctx, &[], Some(next + REPEAT_INTERVAL));
    assert_eq!(settle(&mut app, &ctx).id, rows[5]);
    assert!(app.navigation_repeat.is_none());
}

#[test]
fn duplicate_hat_report_keeps_one_timer_until_both_sources_release() {
    let (_scratch, mut app, ctx) = fixture();
    let first = settle(&mut app, &ctx).id;
    let next = app.navigation_repeat.as_ref().unwrap().next;
    paint(&mut app, &ctx, &[axis(GamepadAxis::HatY, 1.0)], None);
    assert_eq!(settle(&mut app, &ctx).id, first);
    assert_eq!(app.navigation_repeat.as_ref().unwrap().next, next);
    paint(
        &mut app,
        &ctx,
        &[button(GamepadButton::DpadDown, false)],
        Some(next),
    );
    let second = settle(&mut app, &ctx).id;
    assert_ne!(first, second);
    paint(&mut app, &ctx, &[axis(GamepadAxis::HatY, 0.0)], None);
    paint(&mut app, &ctx, &[], Some(next + REPEAT_INTERVAL));
    assert_eq!(settle(&mut app, &ctx).id, second);
    assert!(app.navigation_repeat.is_none());
}

#[test]
fn disconnect_barrier_focus_loss_suspend_and_page_changes_cancel_held_navigation() {
    for cancel in 0..6 {
        let (_scratch, mut app, ctx) = fixture();
        let first = settle(&mut app, &ctx).id;
        let next = app.navigation_repeat.as_ref().unwrap().next;
        match cancel {
            0 => app.process_physical_event(
                &ctx,
                PhysicalInputEvent::Disconnected { device: 1 },
                false,
            ),
            1 => app.release_all_input(),
            2 => ctx.input_mut(|input| input.focused = false),
            3 => app.platform_suspended = true,
            4 => app.screen = Screen::AppSettings(app.app_settings.clone().into()),
            5 => app.exit_confirmation = true,
            _ => unreachable!(),
        }
        app.repeat_navigation_input(&ctx, next);
        assert!(app.navigation_repeat.is_none(), "cancellation {cancel}");
        assert_eq!(ctx.memory(egui::Memory::focused), Some(first));
        if cancel == 1 {
            app.process_physical_event(&ctx, button(GamepadButton::DpadDown, true), false);
            assert!(
                app.navigation_repeat.is_none(),
                "held input needs a release"
            );
        }
    }
}

#[test]
fn held_navigation_scrolls_popup_options_and_stops_when_the_popup_closes() {
    let (_scratch, mut app, ctx) = fixture();
    app.release_all_input();
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    let owner = settle(&mut app, &ctx).id;
    paint(
        &mut app,
        &ctx,
        &[
            button(GamepadButton::DpadDown, false),
            button(GamepadButton::South, true),
            button(GamepadButton::South, false),
        ],
        None,
    );
    let first = settle(&mut app, &ctx);
    assert_ne!(first.id, owner);
    paint(
        &mut app,
        &ctx,
        &[button(GamepadButton::DpadDown, true)],
        None,
    );
    let mut previous = settle(&mut app, &ctx).id;
    let next = app.navigation_repeat.as_ref().unwrap().next;
    for step in 0..10 {
        paint(&mut app, &ctx, &[], Some(next + REPEAT_INTERVAL * step));
        let focused = settle(&mut app, &ctx);
        assert_ne!(focused.id, previous);
        assert_eq!(focused.layer_id, first.layer_id);
        assert!(focused.interact_rect.height() >= 40.0);
        previous = focused.id;
    }
    assert!(crate::focus_navigation::close_popup(&ctx));
    paint(&mut app, &ctx, &[], Some(next + Duration::from_secs(5)));
    assert!(app.navigation_repeat.is_none());
    assert_eq!(settle(&mut app, &ctx).id, owner);
    assert_eq!(
        app.app_settings.language, None,
        "focus does not select a language"
    );
}
