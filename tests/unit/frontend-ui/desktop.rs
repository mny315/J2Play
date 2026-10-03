use super::*;
use frontend_core::FullscreenMode;
use std::sync::Mutex;

use super::test_storage::Scratch;

struct DesktopPlatform(Arc<Mutex<Vec<bool>>>);

impl PlatformBridge for DesktopPlatform {
    fn orientation_control(&self) -> OrientationControl {
        OrientationControl::Layout
    }

    fn escape_navigates_back(&self) -> bool {
        true
    }

    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        UnavailablePlatformBridge.request_document(kind)
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }

    fn set_fullscreen(&mut self, fullscreen: bool) -> Result<(), EmuError> {
        self.0.lock().unwrap().push(fullscreen);
        Ok(())
    }

    fn set_orientation(&mut self, _: PlatformOrientation) -> Result<(), EmuError> {
        panic!("desktop orientation must not request an OS window rotation")
    }
}

fn fixture(root: &Path, calls: Arc<Mutex<Vec<bool>>>) -> FrontendApp {
    let mut app = FrontendApp::new(root, Box::new(DesktopPlatform(calls))).unwrap();
    app.startup_splash = None;
    app.session.start("fixture").unwrap();
    let mut portrait = VirtualControlLayout::default();
    portrait.fire.visible = false;
    app.screen = Screen::Gameplay(Box::new(GameplayScreen {
        entry_id: "fixture".into(),
        title: "Desktop layout fixture".into(),
        canvas_dimensions: (240, 320),
        pointer_events: false,
        game_scale: GameScale::AutomaticFit,
        portrait_frame_percent: None,
        control_layout: portrait,
        landscape_control_layout: VirtualControlLayout::default(),
        vibration: VibrationSettings::default(),
        orientation: None,
        fast_forward: false,
        fullscreen: false,
    }));
    app
}

fn paint(app: &mut FrontendApp, ctx: &egui::Context, size: Vec2) {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            ..Default::default()
        },
        |ui| app.draw_gameplay(ui),
    )
    .drop_without_applying_deltas();
}

#[test]
fn desktop_resize_selects_saved_layouts_without_restarting() {
    let scratch = Scratch::new();
    let mut app = fixture(&scratch.0, Arc::default());
    let ctx = egui::Context::default();
    let ids = app.session.active_ids();
    for size in [
        Vec2::new(960.0, 600.0),
        Vec2::new(380.0, 720.0),
        Vec2::new(960.0, 600.0),
    ] {
        paint(&mut app, &ctx, size);
        let landscape = size.x > size.y;
        let has_fire = app
            .control_regions
            .iter()
            .any(|(_, key)| *key == crate::TouchOwner::VirtualKey(Some(HostAction::Fire)));
        assert_eq!(has_fire, landscape);
        assert!(app.canvas_rect.is_some());
    }
    assert_eq!(app.host_orientation, PlatformOrientation::Automatic);
    assert!(app.display_error.is_none());
    assert_eq!(app.session.active_ids(), ids);
    let Screen::Gameplay(gameplay) = &app.screen else {
        panic!()
    };
    assert_eq!(gameplay.canvas_dimensions, (240, 320));
    assert_eq!(gameplay.orientation, None);
}

struct NativeFullscreenPlatform {
    events: Arc<Mutex<VecDeque<bool>>>,
    requests: Arc<Mutex<Vec<bool>>>,
}

impl PlatformBridge for NativeFullscreenPlatform {
    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        UnavailablePlatformBridge.request_document(kind)
    }
    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }
    fn set_fullscreen(&mut self, value: bool) -> Result<(), EmuError> {
        self.requests.lock().unwrap().push(value);
        Ok(())
    }
    fn poll_fullscreen(&mut self) -> Option<Result<bool, EmuError>> {
        self.events.lock().unwrap().pop_front().map(Ok)
    }
}

#[test]
fn compositor_fullscreen_exit_releases_input_and_keeps_the_manual_choice() {
    let scratch = Scratch::new();
    let mut app = fixture(&scratch.0, Arc::default());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let events = Arc::new(Mutex::new(VecDeque::new()));
    app.platform = Box::new(NativeFullscreenPlatform {
        events: events.clone(),
        requests: requests.clone(),
    });
    app.fullscreen_policy = Some(crate::fullscreen::GameplayFullscreenPolicy::new(
        FullscreenMode::LandscapeOnly,
    ));
    let window = Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 600.0));
    let ids = app.session.active_ids();
    app.sync_gameplay_fullscreen_policy(window);
    events.lock().unwrap().push_back(true);
    app.process_platform_fullscreen();
    app.touch_owners
        .insert(1, TouchOwner::VirtualKey(Some(HostAction::Fire)));
    app.canvas_rect = Some(window);
    events.lock().unwrap().push_back(false);
    app.process_platform_fullscreen();
    assert!(app.touch_owners.is_empty());
    assert!(app.canvas_rect.is_none());
    assert!(matches!(&app.screen, Screen::Gameplay(game) if !game.fullscreen));
    app.sync_gameplay_fullscreen_policy(window);
    assert_eq!(
        *requests.lock().unwrap(),
        [true],
        "native exit must not send another command"
    );
    assert_eq!(app.session.active_ids(), ids);
    // A fresh Off launch also exits fullscreen entered from the library.
    app.platform_fullscreen = Some(true);
    app.fullscreen_policy = Some(crate::fullscreen::GameplayFullscreenPolicy::new(
        FullscreenMode::Off,
    ));
    app.sync_gameplay_fullscreen_policy(window);
    assert_eq!(*requests.lock().unwrap(), [true, false]);
}

#[test]
fn desktop_pointer_mapping_follows_actual_canvas_after_layout_resize_and_zoom() {
    let scratch = Scratch::new();
    let mut app = fixture(&scratch.0, Arc::default());
    let ctx = egui::Context::default();
    let (session_id, attempt_id) = app.session.active_ids().unwrap();
    for (canvas_region, expected_point, expected_origin) in [
        (
            platform::LogicalRect {
                x: 0,
                y: 0,
                width: 240,
                height: 320,
            },
            (60, 80),
            Some((0, 0)),
        ),
        (
            platform::LogicalRect {
                x: 12,
                y: 24,
                width: 216,
                height: 272,
            },
            (48, 56),
            None,
        ),
    ] {
        app.accept_frame(
            &ctx,
            Arc::new(
                Frame::with_canvas_region(
                    session_id,
                    attempt_id,
                    240,
                    320,
                    canvas_region,
                    vec![0xff00_0000; 240 * 320].into(),
                )
                .unwrap(),
            ),
        );
        for scale in [0.45, 0.5, 0.65, 0.75, 0.9, 1.0, 1.35, 1.5] {
            ctx.set_zoom_factor(scale);
            for size in [Vec2::new(380.0, 720.0), Vec2::new(1280.0, 800.0)] {
                paint(&mut app, &ctx, size);
                let canvas = app.canvas_rect.unwrap();
                assert!((canvas.aspect_ratio() - 0.75).abs() < 0.001);
                assert_eq!(
                    app.canvas_position(canvas.min + canvas.size() * 0.25),
                    Some(expected_point)
                );
                assert_eq!(app.canvas_position(canvas.min), expected_origin);
                assert_eq!(app.canvas_position(canvas.max), None);
                assert_eq!(app.canvas_position(canvas.min - Vec2::splat(1.0)), None);
            }
        }
    }
}

fn key(app: &mut FrontendApp, ctx: &egui::Context, key: Key) {
    for pressed in [true, false] {
        ctx.run_ui(
            egui::RawInput {
                events: vec![Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| app.process_physical_inputs(ui.ctx(), true, true),
        )
        .drop_without_applying_deltas();
    }
}

#[test]
fn desktop_escape_closes_one_editor_or_page_and_f11_works_while_loading_and_paused() {
    let scratch = Scratch::new();
    let mut app = fixture(&scratch.0, Arc::default());
    let ctx = egui::Context::default();
    for started in [false, true] {
        if started {
            let (session_id, attempt_id) = app.session.active_ids().unwrap();
            app.session.apply_event(&SessionEvent {
                session_id,
                attempt_id,
                kind: SessionEventKind::Started,
            });
            app.session.set_paused(PauseReason::User, true).unwrap();
        }
        key(&mut app, &ctx, Key::F11);
        assert!(matches!(&app.screen, Screen::Gameplay(game) if game.fullscreen));
        // The first Escape dismisses fullscreen help. Subsequent ones leave
        // fullscreen, open the exit decision, and dismiss that decision.
        if app.fullscreen_help_visible() {
            key(&mut app, &ctx, Key::Escape);
        }
        key(&mut app, &ctx, Key::Escape);
        assert!(matches!(&app.screen, Screen::Gameplay(game) if !game.fullscreen));
        assert!(!app.exit_confirmation);
        key(&mut app, &ctx, Key::Escape);
        assert!(app.exit_confirmation);
        key(&mut app, &ctx, Key::Escape);
        assert!(!app.exit_confirmation);
    }
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_control_editor();
    app.screen
        .control_editor_mut()
        .unwrap()
        .layout
        .fire
        .offset_x = 750;
    key(&mut app, &ctx, Key::Escape);
    assert!(app.screen.control_editor_mut().is_none());
    assert!(matches!(&app.screen, Screen::AppSettings(settings)
        if settings.draft.control_layout.fire.offset_x == 0));
    key(&mut app, &ctx, Key::Escape);
    assert!(matches!(app.screen, Screen::Library));
    assert!(!app.exit_confirmation);
}

#[test]
fn desktop_landscape_editor_keeps_selection_and_both_drafts_across_window_changes() {
    let scratch = Scratch::new();
    let mut app = fixture(&scratch.0, Arc::default());
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_control_editor();
    app.screen
        .control_editor_mut()
        .unwrap()
        .layout
        .fire
        .offset_x = 750;
    let ctx = egui::Context::default();
    let mut landscape = Pos2::ZERO;
    for (index, size) in [Vec2::new(380.0, 720.0); 3].into_iter().enumerate() {
        let events = if index == 2 {
            vec![
                Event::PointerMoved(landscape),
                Event::PointerButton {
                    pos: landscape,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                Event::PointerButton {
                    pos: landscape,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        } else {
            Vec::new()
        };
        app.safe_content_rect = Rect::from_min_size(Pos2::ZERO, size);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(app.safe_content_rect),
                events,
                ..Default::default()
            },
            |ui| app.draw_control_editor(ui),
        );
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == "Landscape"
            {
                landscape = text.visual_bounding_rect().center();
            }
        }
        output.drop_without_applying_deltas();
    }
    let editor = app.screen.control_editor_mut().unwrap();
    assert!(editor.landscape);
    assert!(!editor.landscape_requested);
    editor.layout.fire.size_percent = 140;
    for size in [Vec2::new(1280.0, 800.0), Vec2::new(320.0, 720.0)] {
        app.safe_content_rect = Rect::from_min_size(Pos2::ZERO, size);
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(app.safe_content_rect),
                ..Default::default()
            },
            |ui| app.draw_control_editor(ui),
        )
        .drop_without_applying_deltas();
        let editor = app.screen.control_editor_mut().unwrap();
        assert!(editor.landscape);
        assert_eq!(editor.layout.fire.size_percent, 140);
        assert_eq!(editor.other_layout.fire.offset_x, 750);
    }
    app.screen.commit_control_editor();
    assert_eq!(
        app.repository.load_app_settings().unwrap(),
        frontend_core::AppSettings::default()
    );
    app.save_settings_screen();
    let saved = app.repository.load_app_settings().unwrap();
    assert_eq!(saved.control_layout.fire.offset_x, 750);
    assert_eq!(saved.landscape_control_layout.fire.size_percent, 140);
}

#[test]
fn native_input_is_not_replayed_by_layout_or_hidden_window_logic() {
    let ctx = egui::Context::default();
    let mut last_time = None;
    let mut accepted = 0;
    let input = egui::RawInput {
        time: Some(1.0),
        events: vec![Event::Text("Я".into())],
        ..Default::default()
    };
    let output = ctx.run_ui(input, |ui| {
        accepted += usize::from(crate::eframe_app::take_native_input(
            ui.ctx(),
            &mut last_time,
        ));
        if ui.ctx().current_pass_index() == 0 {
            ui.ctx().request_discard("exercise second layout pass");
        }
    });
    assert_eq!(output.platform_output.num_completed_passes, 2);
    output.drop_without_applying_deltas();
    assert_eq!(accepted, 1);
    let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| {
        accepted += usize::from(crate::eframe_app::take_native_input(ctx, &mut last_time));
    });
    assert_eq!(accepted, 1);
    ctx.run_ui(
        egui::RawInput {
            time: Some(2.0),
            ..Default::default()
        },
        |ui| {
            accepted += usize::from(crate::eframe_app::take_native_input(
                ui.ctx(),
                &mut last_time,
            ));
        },
    )
    .drop_without_applying_deltas();
    assert_eq!(accepted, 2);
}
