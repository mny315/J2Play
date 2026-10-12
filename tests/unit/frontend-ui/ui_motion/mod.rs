use super::*;
use crate::{
    ControlEditorState, Frame, GameScale, GameSettings, GameplayScreen, Pos2, Rect, SettingsScreen,
    SettingsTarget, UnavailablePlatformBridge, Vec2, VibrationSettings, VirtualControlLayout, egui,
};
use eframe::App;
use std::sync::Arc;
use std::time::Duration;

mod actions;
mod return_scroll;

#[test]
fn library_tap_starts_a_real_fixture_with_page_motion() {
    for position in [
        Vec2::new(0.1, 0.5),
        Vec2::new(0.5, 0.3),
        Vec2::new(0.5, 0.7),
    ] {
        launch_fixture_from_tap(position);
    }
}

fn launch_fixture_from_tap(relative_position: Vec2) {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
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
    app.entries.push(entry);
    let ctx = egui::Context::default();
    let mut frame = eframe::Frame::_new_kittest();
    let mut tap_position = None;
    for pressed in [None, None, Some(true), Some(false)] {
        let position = *tap_position.get_or_insert_with(|| {
            ctx.read_response(crate::library_view::launch_id(app.entries[0].id()))
                .map_or(Pos2::ZERO, |response| {
                    response.interact_rect.min + response.interact_rect.size() * relative_position
                })
        });
        if pressed.is_none() {
            tap_position = None;
        }
        let mut events = vec![egui::Event::PointerMoved(position)];
        if let Some(pressed) = pressed {
            events.push(egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            });
        }
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 960.0))),
                events,
                ..Default::default()
            },
            |ui| {
                app.logic(ui.ctx(), &mut frame);
                app.ui(ui, &mut frame);
            },
        )
        .drop_without_applying_deltas();
    }
    assert!(
        matches!(app.screen, Screen::Gameplay(_)),
        "tap must open gameplay"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.latest_frame.is_none() && Instant::now() < deadline {
        app.process_runtime_events(&ctx);
        render(&mut app, &ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        app.latest_frame.is_some(),
        "fixture must present a frame: {:?}",
        app.display_error
    );
    stop_fixture_from_tap(&mut app, &ctx);
    app.runtime.shutdown().unwrap();
}

fn stop_fixture_from_tap(app: &mut FrontendApp, ctx: &egui::Context) {
    let texture = app.game_texture.as_ref().unwrap().id();
    let position = Pos2::new(410.0, 40.0);
    let mut frame = eframe::Frame::_new_kittest();
    for pressed in [None, Some(true), Some(false)] {
        let mut events = vec![egui::Event::PointerMoved(position)];
        if let Some(pressed) = pressed {
            events.push(egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            });
        }
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 960.0))),
                events,
                ..Default::default()
            },
            |ui| app.ui(ui, &mut frame),
        );
        assert!(
            output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture
            )),
            "Stop must still paint the game in the frame of the click"
        );
        output.drop_without_applying_deltas();
    }
    assert_eq!(app.session.state(), crate::SessionState::Stopping);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.session.state() != crate::SessionState::Idle && Instant::now() < deadline {
        app.process_runtime_events(ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(matches!(app.screen, Screen::Library));
    assert!(app.latest_frame.is_none());
}

#[test]
fn every_page_transition_is_short_bounded_and_does_not_restart_on_repaint() {
    let mut motion = UiMotion::default();
    let now = Instant::now();
    assert!(motion.sync(Page::Library, now));
    assert!(motion.started.is_none());
    for page in [
        Page::AppSettings,
        Page::AppControls { landscape: false },
        Page::AppControls { landscape: true },
        Page::AppSettings,
        Page::Library,
        Page::GameSettings,
        Page::GameControls { landscape: false },
        Page::GameSettings,
        Page::Library,
        Page::Gameplay { attempt: None },
        Page::Library,
    ] {
        assert!(motion.sync(page, now));
        let start = motion.opacity(now);
        let middle = motion.opacity(now + Duration::from_millis(80));
        assert!((ENTER_OPACITY..1.0).contains(&start));
        assert!(middle > start && middle < 1.0);
        assert!(!motion.sync(page, now + Duration::from_millis(100)));
        assert_eq!(motion.started, Some(now));
        assert_eq!(
            motion.opacity(now + Duration::from_millis(300)).to_bits(),
            1.0_f32.to_bits()
        );
        assert!(motion.started.is_none());
    }
    motion.sync(Page::GameSettings, now);
    motion.sync(Page::Library, now + Duration::from_millis(20));
    motion.settle();
    assert_eq!(motion.page, Some(Page::Library));
    assert!(motion.started.is_none());
}

fn render(app: &mut FrontendApp, ctx: &egui::Context) {
    render_output(app, ctx).drop_without_applying_deltas();
}

fn render_output(app: &mut FrontendApp, ctx: &egui::Context) -> egui::FullOutput {
    render_events(app, ctx, Vec::new())
}

fn render_events(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut frame = eframe::Frame::_new_kittest();
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 960.0))),
            events,
            ..Default::default()
        },
        |ui| app.ui(ui, &mut frame),
    )
}

#[test]
fn logic_only_ticks_do_not_consume_the_visible_fade() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    let ctx = egui::Context::default();
    render(&mut app, &ctx);
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    let mut frame = eframe::Frame::_new_kittest();
    let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| app.logic(ctx, &mut frame));
    assert_eq!(app.ui_motion.page, Some(Page::Library));
    assert!(app.ui_motion.started.is_none());
    let title_alpha = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "App settings" => {
                    Some(text.fallback_color.a())
                }
                _ => None,
            })
            .expect("settings title must be painted")
    };
    let entering = render_output(&mut app, &ctx);
    let alpha = title_alpha(&entering);
    assert!(alpha > 0 && alpha < 128, "entry alpha was {alpha}");
    entering.drop_without_applying_deltas();
    app.ui_motion.settle();
    let settled = render_output(&mut app, &ctx);
    assert_eq!(title_alpha(&settled), 255);
    settled.drop_without_applying_deltas();
}

#[test]
fn settings_and_control_editor_navigation_share_motion_without_resetting_drafts() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    let ctx = egui::Context::default();
    render(&mut app, &ctx);
    for global in [true, false] {
        app.screen = if global {
            Screen::AppSettings(app.app_settings.clone().into())
        } else {
            Screen::Settings(Box::new(SettingsScreen {
                target: SettingsTarget::Existing {
                    entry_id: "fixture".into(),
                },
                draft: GameSettings::default(),
                focus_profile: false,
                show_all_profiles: false,
                editor_request: None,
                control_editor: None,
            }))
        };
        render(&mut app, &ctx);
        assert!(app.ui_motion.started.is_some());
        let editor = ControlEditorState::new(&GameSettings::default(), (240, 320));
        match &mut app.screen {
            Screen::AppSettings(settings) => settings.control_editor = Some(editor),
            Screen::Settings(settings) => settings.control_editor = Some(editor),
            _ => panic!(),
        }
        render(&mut app, &ctx);
        let started = app.ui_motion.started;
        app.screen
            .control_editor_mut()
            .unwrap()
            .layout
            .fire
            .offset_x = 123;
        render(&mut app, &ctx);
        assert_eq!(app.ui_motion.started, started);
        assert_eq!(
            app.screen
                .control_editor_mut()
                .unwrap()
                .layout
                .fire
                .offset_x,
            123
        );
        app.screen.discard_control_editor();
        render(&mut app, &ctx);
        assert!(app.ui_motion.started.is_some());
        app.screen = Screen::Library;
        render(&mut app, &ctx);
        assert_eq!(app.ui_motion.page, Some(Page::Library));
    }
}

#[test]
fn game_entry_and_exit_fade_without_first_frame_flash_or_moving_canvas() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    let ctx = egui::Context::default();
    render(&mut app, &ctx);
    let command = app.session.start("fixture").unwrap();
    app.screen = Screen::Gameplay(Box::new(GameplayScreen {
        entry_id: "fixture".into(),
        title: "Motion fixture".into(),
        canvas_dimensions: (240, 320),
        pointer_events: false,
        game_scale: GameScale::AutomaticFit,
        portrait_frame_percent: None,
        control_layout: VirtualControlLayout::default(),
        landscape_control_layout: VirtualControlLayout::default(),
        vibration: VibrationSettings::default(),
        orientation: None,
        fast_forward: false,
        fullscreen: false,
    }));
    let loading = render_output(&mut app, &ctx);
    assert!(loading.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "Starting game…"
    )));
    assert!(
        !loading.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Rect(rect) if Some(rect.rect) == app.canvas_rect
                && rect.fill.a() > 0
                && rect.fill.r() == 0 && rect.fill.g() == 0 && rect.fill.b() == 0
        )),
        "loading must not paint a fake black guest framebuffer"
    );
    loading.drop_without_applying_deltas();
    assert!(app.ui_motion.started.is_some());
    let entry_started = app.ui_motion.started;
    let canvas = app.canvas_rect.unwrap();
    let controls = app.control_regions.clone();
    let frame = |pixel| {
        Arc::new(
            Frame::new(
                command.session_id,
                command.attempt_id,
                240,
                320,
                vec![pixel; 240 * 320].into(),
            )
            .unwrap(),
        )
    };
    app.accept_frame(&ctx, frame(0xff00_ff00));
    render(&mut app, &ctx);
    let started = app.ui_motion.started;
    assert!(started.is_some());
    assert_eq!(
        started, entry_started,
        "first guest frame must not restart the page fade"
    );
    app.accept_frame(&ctx, frame(0xffff_0000));
    render(&mut app, &ctx);
    assert_eq!(app.ui_motion.started, started);
    assert_eq!(app.canvas_rect, Some(canvas));
    assert_eq!(app.control_regions, controls);
    assert_eq!(app.latest_frame.as_ref().unwrap().pixels[0], 0xffff_0000);
    assert_eq!(
        app.session.active_ids(),
        Some((command.session_id, command.attempt_id))
    );
    app.platform_suspended = true;
    app.sync_ui_motion(&ctx);
    assert!(app.ui_motion.started.is_none());
    app.platform_suspended = false;
    app.finish_stopped_session();
    render(&mut app, &ctx);
    assert_eq!(app.ui_motion.page, Some(Page::Library));
    assert!(app.ui_motion.started.is_some());
    assert!(app.latest_frame.is_none());
}
