use super::*;
use frontend_core::FullscreenMode;
use std::sync::Mutex;

struct FullscreenPlatform {
    calls: Arc<Mutex<Vec<bool>>>,
    fail: bool,
}

impl PlatformBridge for FullscreenPlatform {
    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        UnavailablePlatformBridge.request_document(kind)
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }

    fn set_fullscreen(&mut self, fullscreen: bool) -> Result<(), EmuError> {
        self.calls.lock().unwrap().push(fullscreen);
        if self.fail {
            return Err(EmuError::new(
                Category::Platform,
                "fixture-fullscreen",
                "Fullscreen unavailable",
            ));
        }
        Ok(())
    }
}

fn start_fixture(app: &mut FrontendApp, mode: FullscreenMode) {
    app.startup_splash = None;
    app.session.start("fixture").unwrap();
    app.fullscreen_policy = Some(crate::fullscreen::GameplayFullscreenPolicy::new(mode));
    app.screen = Screen::Gameplay(Box::new(GameplayScreen {
        entry_id: "fixture".into(),
        title: "Fullscreen fixture".into(),
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
}

fn viewport(landscape: bool) -> Rect {
    let size = if landscape {
        Vec2::new(960.0, 453.0)
    } else {
        Vec2::new(453.0, 960.0)
    };
    Rect::from_min_size(Pos2::ZERO, size)
}

fn fullscreen(app: &FrontendApp) -> bool {
    let Screen::Gameplay(gameplay) = &app.screen else {
        panic!("fixture must be in gameplay");
    };
    gameplay.fullscreen
}

#[test]
fn landscape_fullscreen_releases_input_preserves_manual_exit_and_resets_after_stop() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut app = FrontendApp::new(
        root,
        Box::new(FullscreenPlatform {
            calls: calls.clone(),
            fail: false,
        }),
    )
    .unwrap();
    start_fixture(&mut app, FullscreenMode::LandscapeOnly);
    let ids = app.session.active_ids();
    app.sync_gameplay_fullscreen_policy(viewport(false));
    assert!(!fullscreen(&app));
    app.gameplay_transition
        .begin(viewport(false), false, false, Instant::now());
    app.touch_owners
        .insert(7, TouchOwner::VirtualKey(Some(HostAction::Fire)));
    app.sync_gameplay_fullscreen_policy(viewport(true));
    assert!(fullscreen(&app));
    assert!(app.touch_owners.is_empty());
    assert!(app.gameplay_transition.busy());
    assert!(app.fullscreen_help_visible());
    app.acknowledge_fullscreen_help();
    // Back exits fullscreen, and the automatic mode must not undo it next frame.
    let ctx = egui::Context::default();
    ctx.run_ui(
        egui::RawInput {
            events: vec![Event::Key {
                key: Key::BrowserBack,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ui| app.handle_back(ui.ctx(), true),
    )
    .drop_without_applying_deltas();
    assert!(!fullscreen(&app));
    assert!(!app.exit_confirmation);
    app.sync_gameplay_fullscreen_policy(viewport(true));
    app.platform_suspended = true;
    app.sync_gameplay_fullscreen_policy(viewport(true));
    app.platform_suspended = false;
    app.sync_gameplay_fullscreen_policy(viewport(true).shrink(4.0));
    assert!(!fullscreen(&app));
    assert_eq!(*calls.lock().unwrap(), [true, false]);
    app.sync_gameplay_fullscreen_policy(viewport(false));
    app.sync_gameplay_fullscreen_policy(viewport(true));
    assert!(fullscreen(&app));
    assert_eq!(app.session.active_ids(), ids);
    let Screen::Gameplay(gameplay) = &app.screen else {
        unreachable!()
    };
    assert_eq!(gameplay.canvas_dimensions, (240, 320));
    assert_eq!(gameplay.orientation, None);
    assert_eq!(
        app.repository.load_app_settings().unwrap().fullscreen,
        FullscreenMode::Off
    );
    let stop = app.session.stop().unwrap();
    app.sync_gameplay_fullscreen_policy(viewport(false));
    assert!(
        fullscreen(&app),
        "stopping must not apply rotation defaults"
    );
    app.session.apply_event(&SessionEvent {
        session_id: stop.session_id,
        attempt_id: stop.attempt_id,
        kind: SessionEventKind::Stopped,
    });
    app.finish_stopped_session();
    assert!(matches!(app.screen, Screen::Library));
    assert!(app.fullscreen_policy.is_none());
    assert_eq!(*calls.lock().unwrap(), [true, false, true, false]);
    start_fixture(&mut app, FullscreenMode::LandscapeOnly);
    app.sync_gameplay_fullscreen_policy(viewport(true));
    assert!(fullscreen(&app), "next launch restores the saved policy");
}

#[test]
fn on_and_off_defaults_allow_manual_switching_across_rotations() {
    for mode in [FullscreenMode::Off, FullscreenMode::On] {
        let scratch = crate::tests::test_storage::Scratch::new();
        let root = &scratch.0;
        let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
        start_fixture(&mut app, mode);
        app.sync_gameplay_fullscreen_policy(viewport(false));
        assert_eq!(fullscreen(&app), mode == FullscreenMode::On);
        app.toggle_gameplay_fullscreen();
        app.sync_gameplay_fullscreen_policy(viewport(true));
        app.sync_gameplay_fullscreen_policy(viewport(false));
        assert_eq!(fullscreen(&app), mode == FullscreenMode::Off);
    }
}

#[test]
fn fullscreen_failure_is_reported_once_and_invalid_viewports_do_not_consume_the_default() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut app = FrontendApp::new(
        root,
        Box::new(FullscreenPlatform {
            calls: calls.clone(),
            fail: true,
        }),
    )
    .unwrap();
    start_fixture(&mut app, FullscreenMode::On);
    app.sync_gameplay_fullscreen_policy(Rect::NOTHING);
    app.sync_gameplay_fullscreen_policy(Rect::ZERO);
    app.sync_gameplay_fullscreen_policy(Rect::EVERYTHING);
    assert!(calls.lock().unwrap().is_empty());
    app.sync_gameplay_fullscreen_policy(viewport(true));
    assert!(app.display_error.is_some());
    assert!(!fullscreen(&app));
    app.display_error = None;
    app.sync_gameplay_fullscreen_policy(viewport(true));
    assert!(app.display_error.is_none());
    assert_eq!(*calls.lock().unwrap(), [true]);
}

#[test]
fn fullscreen_exit_gesture_uses_a_bounded_top_region() {
    let tall = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(400.0, 1_000.0));
    let tall_region = fullscreen_exit_gesture_rect(tall);
    assert_eq!(tall_region.left_top(), tall.left_top());
    assert!((tall_region.height() - FULLSCREEN_EXIT_GESTURE_MAX_HEIGHT).abs() < f32::EPSILON);

    let medium = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 600.0));
    assert!((fullscreen_exit_gesture_rect(medium).height() - 108.0).abs() < 0.01);
    let short = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 80.0));
    assert!((fullscreen_exit_gesture_rect(short).height() - 80.0).abs() < f32::EPSILON);
}

#[test]
fn fullscreen_exit_gesture_recognizes_each_new_pair_of_taps() {
    let region = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 120.0));
    let position = Pos2::new(200.0, 60.0);
    let click = || {
        [
            Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    let started = Instant::now();
    let mut gesture = FullscreenExitGesture::default();

    assert!(!gesture.process(&click(), region, started));
    assert!(gesture.process(&click(), region, started + Duration::from_millis(100)));
    assert!(!gesture.process(&click(), region, started + Duration::from_millis(200)));
    assert!(gesture.process(&click(), region, started + Duration::from_millis(300)));
}

#[test]
fn fullscreen_help_appears_once_and_stays_below_the_gesture_outline() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    app.screen = Screen::Gameplay(Box::new(GameplayScreen {
        entry_id: "fixture".into(),
        title: "Fullscreen fixture".into(),
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
    app.set_gameplay_fullscreen(true).unwrap();
    assert!(app.fullscreen_help_visible());
    let ctx = egui::Context::default();
    for size in [
        Vec2::new(453.0, 960.0),
        Vec2::new(960.0, 400.0),
        Vec2::new(320.0, 480.0),
    ] {
        let viewport = Rect::from_min_size(Pos2::ZERO, size);
        app.safe_content_rect = Rect::from_min_max(Pos2::new(12.0, 24.0), viewport.max);
        for _ in 0..3 {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(viewport),
                    ..Default::default()
                },
                |ui| app.draw_fullscreen_help(ui.ctx()),
            )
            .drop_without_applying_deltas();
        }
        let help_title = crate::i18n::Translator::from_context(&ctx).text("Fullscreen controls");
        let help = ctx
            .memory(|memory| memory.area_rect(egui::Id::new(Some(help_title.as_str()))))
            .expect("fullscreen help must be laid out");
        let gesture = fullscreen_exit_gesture_rect(app.safe_content_rect);
        assert!(help.top() >= gesture.bottom() + FULLSCREEN_HELP_INSET - 1.0);
        assert!(app.safe_content_rect.contains_rect(help), "{help:?}");
    }
    app.acknowledge_fullscreen_help();
    assert!(!app.fullscreen_help_visible());
    assert!(app.repository.fullscreen_help_acknowledged().unwrap());
    app.set_gameplay_fullscreen(false).unwrap();
    app.set_gameplay_fullscreen(true).unwrap();
    assert!(!app.fullscreen_help_visible());
    drop(app);
    let app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    assert!(matches!(
        app.fullscreen_help,
        FullscreenHelpState::Acknowledged
    ));
}
