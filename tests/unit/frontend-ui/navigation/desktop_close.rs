use super::*;
use eframe::App;
use frontend_core::PlatformLifecycleSignal;
use std::time::{Duration, Instant};

use crate::tests::test_storage::Scratch;

struct SessionPlatform {
    signal: PlatformLifecycleSignal,
    fail_shutdown: bool,
}

impl PlatformBridge for SessionPlatform {
    fn close_exits_app(&self) -> bool {
        true
    }
    fn lifecycle_signal(&self) -> PlatformLifecycleSignal {
        self.signal.clone()
    }
    fn cancel_pending_operations(&mut self) {
        self.signal.mark_destroyed();
    }
    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        UnavailablePlatformBridge.request_document(kind)
    }
    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }
    fn shutdown(&mut self) -> Result<(), EmuError> {
        if std::mem::take(&mut self.fail_shutdown) {
            return Err(EmuError::new(
                Category::Platform,
                "fixture-shutdown",
                "Fixture adapter did not finish.",
            ));
        }
        Ok(())
    }
}

fn fixture(midlet: u32, signal: PlatformLifecycleSignal) -> (Scratch, FrontendApp) {
    let scratch = Scratch::new();
    let mut app = FrontendApp::new(
        &scratch.0,
        Box::new(SessionPlatform {
            signal,
            fail_shutdown: false,
        }),
    )
    .unwrap();
    app.startup_splash = None;
    let prepared = frontend_core::inspect_import(frontend_core::ImportSource::new(
        None,
        include_bytes!("../../../fixtures/java-me/conformance.jar").to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(midlet)
    .unwrap();
    let entry = app
        .repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let id = entry.id().to_owned();
    app.entries.push(entry);
    app.open_game(&id);
    (scratch, app)
}

fn wait_for(app: &mut FrontendApp, predicate: impl Fn(&FrontendApp) -> bool) {
    let ctx = egui::Context::default();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate(app) && Instant::now() < deadline {
        app.process_runtime_events(&ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        predicate(app),
        "fixture did not reach required state: {:?}",
        app.display_error
    );
}

#[test]
fn native_close_joins_a_real_guest_during_loading_running_pause_and_host_request() {
    for state in ["loading", "running", "paused", "host-request"] {
        let signal = PlatformLifecycleSignal::default();
        signal.set_suspended(state == "loading");
        let (_scratch, mut app) =
            fixture(if state == "host-request" { 8 } else { 2 }, signal.clone());
        match state {
            "loading" => assert_eq!(app.session.state(), SessionState::Starting),
            "host-request" => wait_for(&mut app, |app| {
                app.host_request.as_ref().is_some_and(|request| {
                    matches!(request.kind, frontend_core::HostRequestKind::Network { .. })
                })
            }),
            _ => wait_for(&mut app, |app| app.latest_frame.is_some()),
        }
        if state == "paused" {
            let pause = app.session.set_paused(PauseReason::User, true).unwrap();
            app.runtime.submit(pause).unwrap();
        }
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let mut frame = eframe::Frame::_new_kittest();
        let start = Instant::now();
        ctx.run_ui(input, |ui| app.logic(ui.ctx(), &mut frame))
            .drop_without_applying_deltas();
        assert!(app.exit_requested, "{state}: {:?}", app.display_error);
        assert!(signal.destroyed());
        app.runtime.shutdown().unwrap();
        assert!(start.elapsed() < Duration::from_secs(4), "{state}");
        assert!(app.repository.private_jar_path(&app.entries[0]).exists());
    }
}

#[test]
fn real_guest_keeps_manual_pause_and_generation_across_platform_resume() {
    let signal = PlatformLifecycleSignal::default();
    let (_scratch, mut app) = fixture(2, signal.clone());
    wait_for(&mut app, |app| app.latest_frame.is_some());
    let generation = app.session.active_ids().unwrap();
    let pause = app.session.set_paused(PauseReason::User, true).unwrap();
    app.runtime.submit(pause).unwrap();
    signal.set_suspended(true);
    wait_for(&mut app, |_| signal.suspension_acknowledged());
    signal.set_suspended(false);
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(app.runtime.audio().active_generation().unwrap(), None);
    assert_eq!(app.session.active_ids(), Some(generation));
    let resume = app.session.set_paused(PauseReason::User, false).unwrap();
    app.runtime.submit(resume).unwrap();
    wait_for(&mut app, |app| {
        app.runtime.audio().active_generation().unwrap() == Some(generation)
    });
    app.runtime.shutdown().unwrap();
}

#[test]
fn platform_shutdown_failure_keeps_close_pending_and_can_be_retried() {
    let scratch = Scratch::new();
    let mut app = FrontendApp::new(
        &scratch.0,
        Box::new(SessionPlatform {
            signal: PlatformLifecycleSignal::default(),
            fail_shutdown: true,
        }),
    )
    .unwrap();
    let ctx = egui::Context::default();
    app.confirm_exit(&ctx);
    assert!(!app.exit_requested);
    assert!(
        app.display_error
            .as_ref()
            .unwrap()
            .technical_details
            .contains("fixture-shutdown")
    );
    app.confirm_exit(&ctx);
    assert!(app.exit_requested);
}
