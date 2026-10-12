use super::*;

#[path = "portal/dbus.rs"]
mod dbus;

#[test]
fn approved_external_links_reject_local_files_credentials_and_unsafe_schemes() {
    for url in [
        "file:///tmp/a",
        "content://x",
        "javascript:alert(1)",
        "https://a:b@example.com",
        "https://example.com/\nsecret",
        "relative",
    ] {
        assert!(request::validate_url(url).is_err(), "{url}");
    }
    for url in [
        "https://example.com/a?q=1",
        "http://example.com",
        "mailto:test@example.com",
        "tel:+123",
    ] {
        request::validate_url(url).unwrap();
    }
}

#[test]
fn each_pending_result_is_delivered_once_and_joined() {
    let ctx = egui::Context::default();
    let mut slot = Some(Pending::start(&ctx, |_| Ok(7_u32)).unwrap());
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut observed = Vec::new();
    while slot.is_some() {
        if let Some(result) = Pending::poll_slot(&mut slot) {
            observed.push(result.unwrap());
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(observed, [7]);
}

#[test]
fn cancelled_request_joins_without_delivering_stale_error() {
    let ctx = egui::Context::default();
    let mut slot = Some(
        Pending::<()>::start(&ctx, |cancel| {
            let _ = cancel.recv_blocking();
            Err(operation::error(
                "old-request",
                "Previous game request failed.",
            ))
        })
        .unwrap(),
    );
    slot.as_mut().unwrap().cancel_result();
    let deadline = Instant::now() + Duration::from_secs(2);
    while slot.is_some() {
        assert!(Pending::poll_slot(&mut slot).is_none());
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
fn missing_portal_opens_the_browser_but_cancellation_does_not() {
    let scratch = crate::test_storage::Scratch::new();
    let ctx = egui::Context::default();
    let lifecycle = Lifecycle::default();
    lifecycle.sleep(false);
    let mut portals = Portals::new(ctx.clone(), lifecycle).unwrap();
    portals.browser_directory.clone_from(&scratch.0);
    // The document worker reports the same error as a missing FileChooser
    // service, without using the host's picker or session-bus configuration.
    portals.document_kind = Some(DocumentKind::Jar);
    portals.document = Some(Pending::start(&ctx, |_| Err(request::unavailable())).unwrap());
    let deadline = Instant::now() + Duration::from_secs(3);
    while portals.document_browser().is_none() {
        assert!(portals.poll_document().is_none());
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(portals.in_window_picker);
    portals.cancel_document_browser();
    assert!(matches!(
        wait_document(&mut portals).unwrap(),
        DocumentOutcome::Cancelled {
            kind: DocumentKind::Jar
        }
    ));
    assert!(portals.browser.is_none());
    portals.document_kind = Some(DocumentKind::Jad);
    portals.document = Some(
        Pending::start(&ctx, |_| {
            Ok(DocumentOutcome::Cancelled {
                kind: DocumentKind::Jad,
            })
        })
        .unwrap(),
    );
    assert!(matches!(
        wait_document(&mut portals).unwrap(),
        DocumentOutcome::Cancelled {
            kind: DocumentKind::Jad
        }
    ));
    assert!(portals.document_browser().is_none());
    // A subsequent JAD request uses the same browser directory immediately.
    portals.pick(DocumentKind::Jad).unwrap();
    assert_eq!(portals.document_browser().unwrap().kind, DocumentKind::Jad);
    portals.shutdown().unwrap();
}

#[test]
#[ignore = "requires a desktop FileChooser portal: select the owned conformance.jar, then cancel JAD"]
fn desktop_picker_imports_fixture_and_cancels_optional_jad() {
    use frontend_core::{GameSettings, ImportSource, LibraryRepository, inspect_import};
    let scratch = crate::test_storage::Scratch::new();
    let lifecycle = Lifecycle::default();
    lifecycle.sleep(false);
    let mut portals = Portals::new(egui::Context::default(), lifecycle).unwrap();
    portals.pick(DocumentKind::Jar).unwrap();
    let DocumentOutcome::Selected(document) = wait_document(&mut portals).unwrap() else {
        panic!("select the project-owned fixture");
    };
    assert_eq!(
        document.bytes,
        include_bytes!("../../fixtures/java-me/conformance.jar")
    );
    let prepared = inspect_import(ImportSource::new(
        document.display_name,
        document.bytes,
        None,
    ))
    .unwrap()
    .select_midlet(5)
    .unwrap();
    let repository = LibraryRepository::open(scratch.0.join("library")).unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    portals.pick(DocumentKind::Jad).unwrap();
    assert!(matches!(
        wait_document(&mut portals).unwrap(),
        DocumentOutcome::Cancelled {
            kind: DocumentKind::Jad
        }
    ));
    assert_eq!(repository.load().unwrap().entries, [entry]);
    portals.shutdown().unwrap();
}

fn wait_document(portals: &mut Portals) -> Result<DocumentOutcome, EmuError> {
    let deadline = Instant::now() + REQUEST_TIMEOUT + Duration::from_secs(20);
    loop {
        if let Some(result) = portals.poll_document() {
            return result;
        }
        assert!(
            Instant::now() < deadline,
            "interactive fixture picker deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
