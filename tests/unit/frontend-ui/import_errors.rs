use super::*;

const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/java-me/conformance.jar"
));

struct ImportPlatform;

impl PlatformBridge for ImportPlatform {
    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        assert_eq!(kind, DocumentKind::Jad);
        Ok(())
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }
}

#[test]
fn invalid_replacement_jar_preserves_the_review_and_its_persisted_draft() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(ImportPlatform)).unwrap();
    app.accept_document(PickedDocument {
        kind: DocumentKind::Jar,
        display_name: Some("retained.jar".into()),
        bytes: FIXTURE.to_vec(),
    });
    let Some(ImportFlow::ChooseMidlet(inspection)) = app.import_flow.take() else {
        panic!("missing JAR draft")
    };
    app.select_midlet(*inspection, 2);
    let path = scratch.0.join("library/pending-import.bin");
    let draft = std::fs::read(&path).unwrap();

    app.accept_document(PickedDocument {
        kind: DocumentKind::Jar,
        display_name: Some("broken.jar".into()),
        bytes: b"not an archive".to_vec(),
    });
    assert!(app.display_error.is_some());
    let Some(ImportFlow::ConfirmProfile(prepared)) = &app.import_flow else {
        panic!("invalid replacement must preserve the profile review")
    };
    assert_eq!(prepared.midlet().index, 2);
    assert_eq!(prepared.archive_leaf_name(), Some("retained.jar"));
    assert_eq!(std::fs::read(&path).unwrap(), draft);
    drop(app);

    let app = FrontendApp::new(&scratch.0, Box::new(ImportPlatform)).unwrap();
    let source = app.import_flow.as_ref().unwrap().source().clone();
    let (name, bytes, _) = source.into_parts();
    assert_eq!(name.as_deref(), Some("retained.jar"));
    assert_eq!(bytes, FIXTURE);
}

#[test]
fn jad_save_failure_retains_the_archive_and_allows_retry() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    let inspection = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None)).unwrap();
    let jad = format!(
        "MIDlet-Name: Fixture\nMIDlet-1: Fixture,,{}\n",
        inspection.midlets()[1].class_name
    )
    .into_bytes();
    app.accept_document(PickedDocument {
        kind: DocumentKind::Jar,
        display_name: Some("fixture.jar".into()),
        bytes: FIXTURE.to_vec(),
    });
    let Some(ImportFlow::ChooseMidlet(inspection)) = app.import_flow.take() else {
        panic!("missing JAR draft")
    };
    app.select_midlet(*inspection, 2);
    app.choose_import_jad();
    let pending = root.join("library/pending-import.bin");
    let original = root.join("library/pending-import.original");
    std::fs::rename(&pending, &original).unwrap();
    std::fs::create_dir(&pending).unwrap();
    let document = || PickedDocument {
        kind: DocumentKind::Jad,
        display_name: None,
        bytes: jad.clone(),
    };
    app.accept_document(document());
    assert!(app.display_error.is_some());
    let Some(ImportFlow::ConfirmProfile(prepared)) = &app.import_flow else {
        panic!("a failed JAD write must keep the archive available")
    };
    assert_eq!(prepared.archive_leaf_name(), Some("fixture.jar"));
    assert_eq!(prepared.midlet().index, 2);
    let (_, archive, descriptor) = prepared.source().clone().into_parts();
    assert_eq!(archive, FIXTURE);
    assert!(descriptor.is_none());
    std::fs::remove_dir(&pending).unwrap();
    std::fs::rename(original, pending).unwrap();
    app.display_error = None;
    app.choose_import_jad();
    app.accept_document(document());
    assert!(app.display_error.is_none());
    assert!(matches!(
        app.import_flow,
        Some(ImportFlow::ConfirmProfile(_))
    ));
    let (_, archive, descriptor) = app
        .repository
        .restore_import()
        .unwrap()
        .unwrap()
        .into_parts();
    assert_eq!(archive, FIXTURE);
    assert_eq!(descriptor.as_deref(), Some(jad.as_slice()));
}

#[test]
fn cancelled_or_failed_jad_picker_preserves_the_profile_review() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    app.accept_document(PickedDocument {
        kind: DocumentKind::Jar,
        display_name: Some("fixture.jar".into()),
        bytes: FIXTURE.to_vec(),
    });
    let Some(ImportFlow::ChooseMidlet(inspection)) = app.import_flow.take() else {
        panic!("multiple MIDlets still need an explicit choice")
    };
    app.select_midlet(*inspection, 2);
    app.choose_import_jad();
    assert!(matches!(app.import_flow, Some(ImportFlow::AwaitingJad(_))));
    app.cancelled_document(DocumentKind::Jad);
    let Some(ImportFlow::ConfirmProfile(prepared)) = &app.import_flow else {
        panic!("cancel must return to the profile review")
    };
    assert_eq!(prepared.midlet().index, 2);
    assert!(app.display_error.is_none());

    app.platform = Box::new(UnavailablePlatformBridge);
    app.choose_import_jad();
    let Some(ImportFlow::ConfirmProfile(prepared)) = &app.import_flow else {
        panic!("picker failure must keep the profile review")
    };
    assert_eq!(prepared.midlet().index, 2);
    assert!(app.display_error.is_some());
    assert!(app.repository.restore_import().unwrap().is_some());
}

#[test]
fn invalid_jad_keeps_the_previous_descriptor_and_midlet() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    app.accept_document(PickedDocument {
        kind: DocumentKind::Jar,
        display_name: None,
        bytes: FIXTURE.to_vec(),
    });
    let jad = include_bytes!("../../fixtures/java-me/lifecycle.jad");
    app.choose_import_jad();
    app.accept_document(PickedDocument {
        kind: DocumentKind::Jad,
        display_name: None,
        bytes: jad.to_vec(),
    });
    assert!(app.display_error.is_none());
    let Some(ImportFlow::ChooseMidlet(inspection)) = app.import_flow.take() else {
        panic!("a partial JAD preserves the manifest's other MIDlets")
    };
    assert_eq!(inspection.midlets().len(), 8);
    app.select_midlet(*inspection, 1);
    app.choose_import_jad();
    app.accept_document(PickedDocument {
        kind: DocumentKind::Jad,
        display_name: None,
        bytes: vec![0xff],
    });
    assert!(app.display_error.is_some());
    let Some(ImportFlow::ConfirmProfile(prepared)) = &app.import_flow else {
        panic!("invalid replacement must preserve the reviewed descriptor")
    };
    assert_eq!(prepared.midlet().index, 1);
    let (_, _, descriptor) = prepared.source().clone().into_parts();
    assert_eq!(descriptor.as_deref(), Some(jad.as_slice()));
    let (_, _, stored) = app
        .repository
        .restore_import()
        .unwrap()
        .unwrap()
        .into_parts();
    assert_eq!(stored, descriptor);
}

#[test]
fn unexpected_jad_result_keeps_the_pending_profile_confirmation() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(2)
        .unwrap();
    let expected_hash = prepared.jar_sha256().to_owned();
    app.import_flow = Some(ImportFlow::ConfirmProfile(Box::new(prepared)));
    app.accept_document(PickedDocument {
        kind: DocumentKind::Jad,
        display_name: None,
        bytes: vec![],
    });
    assert!(app.display_error.is_some());
    let Some(ImportFlow::ConfirmProfile(prepared)) = &app.import_flow else {
        panic!("an unexpected document must preserve the current import")
    };
    assert_eq!(prepared.jar_sha256(), expected_hash);
    assert_eq!(prepared.midlet().index, 2);
}
