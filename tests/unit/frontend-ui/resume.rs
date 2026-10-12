use super::*;
use frontend_core::ResumeBehavior;

fn imported_app(root: &std::path::Path) -> FrontendApp {
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let prepared = frontend_core::inspect_import(ImportSource::new(
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
    app
}

fn fixture(root: &std::path::Path) -> FrontendApp {
    let mut app = imported_app(root);
    let entry = &app.entries[0];
    app.session.start(entry.id()).unwrap();
    app.screen = Screen::Gameplay(Box::new(GameplayScreen {
        entry_id: entry.id().into(),
        title: entry.title().into(),
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
    app.host_request = Some(
        HostRequest::new(
            1,
            HostRequestKind::ResumeGame {
                title: "Fixture".into(),
                can_resume: true,
                detail: "A saved game is available.".into(),
            },
        )
        .unwrap(),
    );
    app
}

#[test]
fn stop_write_failure_stays_visible_after_returning_to_the_library() {
    let scratch = test_storage::Scratch::new();
    let mut app = imported_app(&scratch.0);
    let id = app.entries[0].id().to_owned();
    let ctx = egui::Context::default();
    app.open_game(&id);
    wait_for(&mut app, &ctx, |app| app.latest_frame.is_some());
    assert!(app.display_error.is_none());

    let path = scratch
        .0
        .join("library/runtime/resume")
        .join(id)
        .join("automatic.save");
    std::fs::create_dir_all(&path).unwrap();
    app.stop_active_session();
    wait_for(&mut app, &ctx, |app| {
        app.session.state() == SessionState::Idle
    });
    assert!(matches!(app.screen, Screen::Library));
    let error = app
        .display_error
        .as_ref()
        .expect("Stop must show its save error");
    assert!(
        error
            .user_message
            .contains("Could not save the game state before stopping")
    );
    assert!(path.is_dir(), "a failed save must not remove existing data");
    app.runtime.shutdown().unwrap();
}

fn wait_for(app: &mut FrontendApp, ctx: &egui::Context, ready: impl Fn(&FrontendApp) -> bool) {
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    while !ready(app) && Instant::now() < deadline {
        app.process_runtime_events(ctx);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(ready(app), "{:?}", app.display_error);
}

#[test]
fn closing_the_app_does_not_create_a_stop_checkpoint() {
    for native_exit in [false, true] {
        let scratch = test_storage::Scratch::new();
        let mut app = imported_app(&scratch.0);
        let id = app.entries[0].id().to_owned();
        let ctx = egui::Context::default();
        app.open_game(&id);
        wait_for(&mut app, &ctx, |app| app.latest_frame.is_some());
        assert!(app.display_error.is_none());
        if native_exit {
            eframe::App::on_exit(&mut app);
        } else {
            app.confirm_exit(&ctx);
            assert!(app.exit_requested, "{:?}", app.display_error);
        }
        app.runtime.shutdown().unwrap();
        assert!(
            !scratch
                .0
                .join("library/runtime/resume")
                .join(id)
                .join("automatic.save")
                .exists(),
            "closing the app created a Stop checkpoint; native_exit={native_exit}"
        );
    }
}

#[test]
fn resume_choice_is_remembered_only_when_checked_and_can_be_changed_later() {
    for (continue_game, remember, expected) in [
        (true, false, ResumeBehavior::Ask),
        (false, false, ResumeBehavior::Ask),
        (true, true, ResumeBehavior::Always),
        (false, true, ResumeBehavior::Never),
    ] {
        let scratch = test_storage::Scratch::new();
        let mut app = fixture(&scratch.0);
        app.resolve_resume_request(999, continue_game, true);
        assert_eq!(
            app.entries[0].settings().resume_behavior,
            ResumeBehavior::Ask
        );
        app.resolve_resume_request(1, continue_game, remember);
        assert!(app.host_request.is_none());
        assert_eq!(app.entries[0].settings().resume_behavior, expected);
        assert_eq!(
            app.repository.load().unwrap().entries[0]
                .settings()
                .resume_behavior,
            expected
        );
        app.runtime.shutdown().unwrap();
    }
}

#[test]
fn resume_preference_write_failure_keeps_the_question_and_previous_settings() {
    let scratch = test_storage::Scratch::new();
    let mut app = fixture(&scratch.0);
    let metadata = scratch
        .0
        .join("library/entries")
        .join(format!("{}.json", app.entries[0].id()));
    let previous = metadata.with_extension("original");
    std::fs::rename(&metadata, &previous).unwrap();
    std::fs::create_dir(&metadata).unwrap();
    app.resolve_resume_request(1, true, true);
    assert!(app.display_error.is_some());
    assert!(app.host_request.is_some());
    assert_eq!(
        app.entries[0].settings().resume_behavior,
        ResumeBehavior::Ask
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn resume_dialog_exposes_continue_restart_cancel_and_remember_on_a_narrow_screen() {
    let scratch = test_storage::Scratch::new();
    let mut app = fixture(&scratch.0);
    app.safe_content_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 640.0));
    let ctx = egui::Context::default();
    for _ in 0..2 {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(app.safe_content_rect),
                ..Default::default()
            },
            |ui| app.draw_host_request(ui.ctx()),
        )
        .drop_without_applying_deltas();
    }
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(app.safe_content_rect),
            ..Default::default()
        },
        |ui| app.draw_host_request(ui.ctx()),
    );
    for label in [
        "Continue",
        "Start normally",
        "Cancel",
        "Remember my choice for this game",
    ] {
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == label)), "missing {label}");
    }
    output.drop_without_applying_deltas();
    app.runtime.shutdown().unwrap();
}
