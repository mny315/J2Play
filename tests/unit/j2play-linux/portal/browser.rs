use super::*;
use crate::test_storage::Scratch;

fn start(kind: DocumentKind, path: &Path) -> Browser {
    let lifecycle = Lifecycle::default();
    lifecycle.sleep(false);
    Browser::start(kind, path.to_owned(), egui::Context::default(), lifecycle).unwrap()
}

fn ready(browser: &mut Browser) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while browser.pending.is_some() {
        assert!(browser.poll().is_none());
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

fn outcome(browser: &mut Browser) -> DocumentOutcome {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(result) = browser.poll() {
            return result.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
fn browses_and_imports_owned_jar_without_a_portal_then_rejects_stale_entries() {
    let scratch = Scratch::new();
    let folder = scratch.0.join("SD card");
    std::fs::create_dir(&folder).unwrap();
    let bytes = include_bytes!("../../../fixtures/jar/import-single-midlet.zip");
    std::fs::write(folder.join("Fixture.JAR"), bytes).unwrap();
    let mut browser = start(DocumentKind::Jar, &scratch.0);
    ready(&mut browser);
    let old_id = browser.view.entries[0].id;
    browser.activate(old_id).unwrap();
    ready(&mut browser);
    assert_eq!(browser.current_directory(), folder);
    assert_eq!(
        browser.activate(old_id).unwrap_err().code(),
        "linux-document-selection"
    );
    browser.activate(browser.view.entries[0].id).unwrap();
    let DocumentOutcome::Selected(document) = outcome(&mut browser) else {
        panic!("expected selection")
    };
    assert_eq!(document.display_name.as_deref(), Some("Fixture.JAR"));
    assert_eq!(document.bytes, bytes);
    let prepared = frontend_core::inspect_import(frontend_core::ImportSource::new(
        document.display_name,
        document.bytes,
        None,
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    let repository = frontend_core::LibraryRepository::open(scratch.0.join("library")).unwrap();
    let entry = repository
        .commit_import(&prepared, frontend_core::GameSettings::default())
        .unwrap();
    assert_eq!(repository.load().unwrap().entries, [entry]);
    browser.shutdown().unwrap();
}

#[test]
fn cancel_and_sleep_discard_pending_selection_before_it_can_reach_the_importer() {
    let scratch = Scratch::new();
    std::fs::write(scratch.0.join("fixture.jad"), b"MIDlet-Name: Fixture\n").unwrap();
    for sleep in [false, true] {
        let mut browser = start(DocumentKind::Jad, &scratch.0);
        ready(&mut browser);
        browser.activate(browser.view.entries[0].id).unwrap();
        if sleep {
            browser.lifecycle.sleep(true);
        } else {
            browser.cancel();
        }
        assert!(matches!(
            outcome(&mut browser),
            DocumentOutcome::Cancelled {
                kind: DocumentKind::Jad
            }
        ));
        assert!(browser.view().is_none());
        assert!(browser.pending.is_none());
        browser.shutdown().unwrap();
    }
}

#[test]
fn missing_folder_keeps_navigation_and_cancel_available() {
    let scratch = Scratch::new();
    let mut browser = start(DocumentKind::Jar, &scratch.0.join("missing"));
    ready(&mut browser);
    assert!(browser.view.error.is_some());
    assert!(!browser.view.busy);
    browser.activate(browser.view.parent.unwrap()).unwrap();
    ready(&mut browser);
    assert!(browser.view.error.is_none());
    assert_eq!(browser.current_directory(), scratch.0);
    browser.cancel();
    assert!(matches!(
        outcome(&mut browser),
        DocumentOutcome::Cancelled {
            kind: DocumentKind::Jar
        }
    ));
}

#[test]
fn only_gamescope_sessions_select_the_in_window_picker_directly() {
    use crate::window::gaming_session;
    for desktop in ["gamescope", "Gamescope", "gamescope:Steam"] {
        assert!(gaming_session(
            |name| (name == "XDG_CURRENT_DESKTOP").then(|| desktop.into())
        ));
    }
    for desktop in ["KDE", "GNOME", "niri", "not-gamescope"] {
        assert!(!gaming_session(|_| Some(desktop.into())));
    }
    assert!(!gaming_session(|_| None));
}
