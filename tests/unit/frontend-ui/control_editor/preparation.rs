use super::*;
use crate::tests::test_storage::Scratch;
use crate::{FrontendApp, Screen, UnavailablePlatformBridge};
use frontend_core::{GameSettings, ImportSource, ProfileChoice, inspect_import};
use std::sync::mpsc;

fn app() -> (Scratch, FrontendApp) {
    let scratch = Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
    let prepared = inspect_import(ImportSource::new(
        Some("fixture.jar".into()),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/java-me/conformance.jar"
        ))
        .to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    assert!(app.commit_import(&prepared, GameSettings::default()));
    let id = app.entries[0].id().to_owned();
    app.open_settings(&id, false);
    (scratch, app)
}

fn finish(app: &mut FrontendApp, ctx: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        app.process_control_editor_preparation(ctx);
        if app.editor_preparation.running.is_none() && app.editor_preparation.queued.is_none() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "preview preparation did not finish"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn existing_editor_prepares_in_background_using_the_current_draft_profile() {
    let (_scratch, mut app) = app();
    let ctx = egui::Context::default();
    let prepared = app.repository.prepare_launch(&app.entries[0]).unwrap();
    let automatic = prepared
        .launch_plan(&GameSettings::default())
        .unwrap()
        .profile_summary()
        .canvas_dimensions;
    let manual = launch::builtin_device_profiles()
        .unwrap()
        .iter()
        .find(|profile| {
            profile
                .canvas_dimensions()
                .is_some_and(|dimensions| dimensions != automatic)
        })
        .unwrap();
    for profile in [
        ProfileChoice::Manual {
            profile_id: manual.profile_id().into(),
        },
        ProfileChoice::Automatic,
    ] {
        let Screen::Settings(settings) = &mut app.screen else {
            panic!("settings")
        };
        settings.control_editor = None;
        settings.draft.device_profile = profile;
        settings.draft.controls_override = true;
        settings.draft.control_layout.fire.offset_x = 750;
        let expected = prepared
            .launch_plan(&settings.draft)
            .unwrap()
            .profile_summary()
            .canvas_dimensions;
        app.open_control_editor();
        assert!(app.control_editor_preparing());
        assert!(app.screen.control_editor_mut().is_none());
        assert!(
            app.editor_preparation.running.is_none(),
            "opening only queues the read"
        );
        finish(&mut app, &ctx);
        assert!(app.display_error.is_none());
        let editor = app.screen.control_editor_mut().unwrap();
        assert_eq!(editor.canvas_dimensions, expected);
        assert_eq!(editor.layout.fire.offset_x, 750);
        assert_eq!(
            app.repository.load().unwrap().entries[0].settings(),
            &GameSettings::default()
        );
    }
}

#[test]
fn back_cancels_without_losing_the_draft_and_a_late_result_cannot_reopen_it() {
    let (_scratch, mut app) = app();
    let ctx = egui::Context::default();
    let (started, started_rx) = mpsc::sync_channel(1);
    let (release, release_rx) = mpsc::sync_channel(1);
    let id = app
        .editor_preparation
        .request_with(Box::new(move |_| {
            started.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            Ok((123, 456))
        }))
        .unwrap();
    let Screen::Settings(settings) = &mut app.screen else {
        panic!("settings")
    };
    settings.draft.control_layout.fire.offset_x = 750;
    settings.editor_request = Some((id, settings.draft.device_profile.clone()));
    app.process_control_editor_preparation(&ctx);
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    app.navigate_back();
    let Screen::Settings(settings) = &app.screen else {
        panic!("parent draft must remain")
    };
    assert_eq!(settings.draft.control_layout.fire.offset_x, 750);
    assert!(settings.editor_request.is_none());
    let entry_id = app.entries[0].id().to_owned();
    app.open_settings(&entry_id, false);
    release.send(()).unwrap();
    finish(&mut app, &ctx);
    assert!(app.screen.control_editor_mut().is_none());
    assert!(app.display_error.is_none());
}

#[test]
fn preparation_retains_one_running_job_and_only_the_latest_queued_request() {
    let mut loader = EditorPreparation::default();
    let ctx = egui::Context::default();
    let (started, started_rx) = mpsc::sync_channel(1);
    let (release, release_rx) = mpsc::sync_channel(1);
    loader
        .request_with(Box::new(move |cancelled| {
            started.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            assert!(cancelled.load(Ordering::Acquire));
            Ok((1, 1))
        }))
        .unwrap();
    assert!(loader.poll(&ctx).is_none());
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    for _ in 0..50 {
        loader
            .request_with(Box::new(|_| panic!("replaced request must not run")))
            .unwrap();
    }
    let latest = loader.request_with(Box::new(|_| Ok((240, 320)))).unwrap();
    assert!(loader.poll(&ctx).is_none());
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some((id, result)) = loader.poll(&ctx) {
            assert_eq!(id, latest);
            assert_eq!(result.unwrap(), (240, 320));
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    loader.shutdown().unwrap();
}

#[test]
fn preparation_shutdown_deadline_keeps_the_worker_available_for_join() {
    let mut loader = EditorPreparation::default();
    let (started, started_rx) = mpsc::sync_channel(1);
    let (release, release_rx) = mpsc::sync_channel(1);
    loader
        .request_with(Box::new(move |_| {
            started.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            Ok((1, 1))
        }))
        .unwrap();
    loader.poll(&egui::Context::default());
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(loader.shutdown_before(Instant::now()).is_err());
    assert!(loader.running.is_some());
    release.send(()).unwrap();
    loader.shutdown().unwrap();
    assert!(loader.running.is_none());
}

#[test]
fn changing_the_draft_profile_cancels_queued_work_before_it_opens_the_archive() {
    let (_scratch, mut app) = app();
    let id = app
        .editor_preparation
        .request_with(Box::new(|_| panic!("stale profile request")))
        .unwrap();
    let Screen::Settings(settings) = &mut app.screen else {
        panic!("settings")
    };
    settings.editor_request = Some((id, ProfileChoice::Automatic));
    settings.draft.device_profile = ProfileChoice::Manual {
        profile_id: "changed".into(),
    };
    app.process_control_editor_preparation(&egui::Context::default());
    assert!(!app.control_editor_preparing());
    assert!(app.screen.control_editor_mut().is_none());
    assert!(app.display_error.is_none());
    assert!(app.editor_preparation.running.is_none());
}

#[test]
fn archive_failure_is_reported_asynchronously_and_preserves_the_settings_draft() {
    let (_scratch, mut app) = app();
    let path = app.repository.private_jar_path(&app.entries[0]);
    // Only this test's private copy is damaged; the project-owned fixture is untouched.
    std::fs::write(path, b"invalid ZIP").unwrap();
    let Screen::Settings(settings) = &mut app.screen else {
        panic!("settings")
    };
    settings.draft.control_layout.fire.offset_x = 900;
    let draft = settings.draft.clone();
    app.open_control_editor();
    assert!(app.display_error.is_none());
    finish(&mut app, &egui::Context::default());
    assert!(app.display_error.is_some());
    let Screen::Settings(settings) = &app.screen else {
        panic!("settings")
    };
    assert_eq!(settings.draft, draft);
    assert!(settings.control_editor.is_none());
    assert!(settings.editor_request.is_none());
}

#[test]
fn preparation_shutdown_signals_cancellation_before_joining() {
    let mut loader = EditorPreparation::default();
    let (started, started_rx) = mpsc::sync_channel(1);
    loader
        .request_with(Box::new(move |cancelled| {
            started.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            while !cancelled.load(Ordering::Acquire) {
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(1));
            }
            Ok((1, 1))
        }))
        .unwrap();
    loader.poll(&egui::Context::default());
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    loader.shutdown().unwrap();
    assert!(loader.running.is_none());
    assert!(loader.queued.is_none());
}

#[test]
fn preparation_dialog_supports_escape_and_keyboard_cancel_in_a_narrow_viewport() {
    for key in [egui::Key::Escape, egui::Key::Space] {
        let (_scratch, mut app) = app();
        app.open_control_editor();
        let ctx = egui::Context::default();
        app.safe_content_rect =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(240.0, 240.0));
        for pressed in [false, true] {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(app.safe_content_rect),
                    events: if pressed {
                        vec![egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        }]
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                },
                |ui| app.draw_control_editor_preparation(ui.ctx()),
            )
            .drop_without_applying_deltas();
        }
        assert!(!app.control_editor_preparing(), "{key:?}");
        assert!(matches!(app.screen, Screen::Settings(_)));
        assert!(app.editor_preparation.queued.is_none());
    }
}
