use super::*;
use crate::tests::test_storage::Scratch;
use crate::{DocumentKind, DocumentOutcome, PlatformBridge};
use frontend_core::PlatformLifecycleSignal;
use std::time::Duration;

const JAR: &[u8] = include_bytes!("../../../fixtures/java-me/conformance.jar");

struct SuspendedPlatform(PlatformLifecycleSignal);

impl PlatformBridge for SuspendedPlatform {
    fn lifecycle_signal(&self) -> PlatformLifecycleSignal {
        self.0.clone()
    }

    fn request_document(&mut self, _: DocumentKind) -> Result<(), EmuError> {
        unreachable!("launch fixture does not open documents")
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }
}

fn fixture() -> (Scratch, FrontendApp, PlatformLifecycleSignal) {
    let scratch = Scratch::new();
    let signal = PlatformLifecycleSignal::initially_suspended();
    let mut app =
        FrontendApp::new(&scratch.0, Box::new(SuspendedPlatform(signal.clone()))).unwrap();
    let prepared =
        frontend_core::inspect_import(frontend_core::ImportSource::new(None, JAR.to_vec(), None))
            .unwrap()
            .select_midlet(2)
            .unwrap();
    app.entries.push(
        app.repository
            .commit_import(&prepared, crate::GameSettings::default())
            .unwrap(),
    );
    (scratch, app, signal)
}

fn wait_for(app: &mut FrontendApp, expected: SessionState) {
    let ctx = egui::Context::default();
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.session.state() != expected && Instant::now() < deadline {
        app.process_runtime_events(&ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(app.session.state(), expected, "{:?}", app.display_error);
}

#[test]
fn launch_can_be_cancelled_before_the_worker_reads_the_archive() {
    let (_scratch, mut app, _signal) = fixture();
    let id = app.entries[0].id().to_owned();
    std::fs::remove_file(app.repository.private_jar_path(&app.entries[0])).unwrap();
    app.open_game(&id);
    assert_eq!(app.session.state(), SessionState::Starting);
    assert!(matches!(app.screen, Screen::Gameplay(_)));
    assert!(app.display_error.is_none());
    app.stop_active_session();
    wait_for(&mut app, SessionState::Idle);
    assert!(matches!(app.screen, Screen::Library));
    assert!(app.display_error.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn worker_reports_preparation_failure_after_launch_leaves_the_ui_thread() {
    let (_scratch, mut app, signal) = fixture();
    let id = app.entries[0].id().to_owned();
    std::fs::remove_file(app.repository.private_jar_path(&app.entries[0])).unwrap();
    app.open_game(&id);
    assert_eq!(app.session.state(), SessionState::Starting);
    assert!(app.display_error.is_none());
    signal.set_suspended(false);
    wait_for(&mut app, SessionState::Idle);
    assert!(matches!(app.screen, Screen::Library));
    assert!(
        app.display_error
            .as_ref()
            .unwrap()
            .technical_details
            .contains("library-private-jar")
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn worker_plan_replaces_the_loading_hint_and_refreshes_the_display_cache() {
    let (scratch, mut app, signal) = fixture();
    let entry = &app.entries[0];
    let id = entry.id().to_owned();
    let plan = app
        .repository
        .prepare_launch(entry)
        .unwrap()
        .launch_plan(entry.settings())
        .unwrap();
    let dimensions = plan.decision.selection().canvas_dimensions();
    let metadata = scratch.0.join("library/entries").join(format!("{id}.json"));
    let original = std::fs::read_to_string(&metadata).unwrap();
    let changed = original
        .replace(
            &format!("\"canvas_width\": {}", dimensions.0),
            "\"canvas_width\": 1",
        )
        .replace(
            &format!("\"canvas_height\": {}", dimensions.1),
            "\"canvas_height\": 2",
        );
    assert_ne!(original, changed);
    std::fs::write(&metadata, changed).unwrap();
    app.entries = app.repository.load().unwrap().entries;
    app.open_game(&id);
    let Screen::Gameplay(gameplay) = &app.screen else {
        panic!("loading screen")
    };
    assert_eq!(gameplay.canvas_dimensions, (1, 2));
    assert!(!gameplay.pointer_events);
    signal.set_suspended(false);
    wait_for(&mut app, SessionState::Running);
    let Screen::Gameplay(gameplay) = &app.screen else {
        panic!("running screen")
    };
    assert_eq!(gameplay.canvas_dimensions, dimensions);
    assert_eq!(gameplay.pointer_events, plan.pointer_events);
    let restored = app.repository.load().unwrap();
    assert_eq!(
        restored.entries[0].automatic_canvas_dimensions(&app.catalog_fingerprint),
        Some(dimensions)
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn delayed_preparation_cannot_change_a_stopping_screen() {
    let (_scratch, mut app, _signal) = fixture();
    let entry = &app.entries[0];
    let id = entry.id().to_owned();
    let plan = app
        .repository
        .prepare_launch(entry)
        .unwrap()
        .launch_plan(entry.settings())
        .unwrap();
    app.open_game(&id);
    let Screen::Gameplay(gameplay) = &mut app.screen else {
        panic!("loading screen")
    };
    gameplay.canvas_dimensions = (1, 2);
    app.stop_active_session();
    app.apply_launch_preparation(&plan);
    let Screen::Gameplay(gameplay) = &app.screen else {
        panic!("stopping screen")
    };
    assert_eq!(gameplay.canvas_dimensions, (1, 2));
    assert!(!gameplay.pointer_events);
    app.runtime.shutdown().unwrap();
}
