use super::*;

// Metadata-only ZIPs: import inspection never executes the declared class.
const SINGLE: &[u8] = include_bytes!("../../fixtures/jar/import-single-midlet.zip");
const EXTERNAL: &[u8] = include_bytes!("../../fixtures/jar/import-external-midlet.zip");
const JAD: &[u8] = b"MIDlet-Name: Import Fixture\nMIDlet-1: Import Fixture,,fixtures.ImportMidlet\nNokia-MIDlet-Target-Display-Size: 240,320\nNokia-MIDlet-App-Orientation: landscape\n";

struct ImportPlatform;

struct ExternalPlatform(Option<DocumentOutcome>);

impl PlatformBridge for ExternalPlatform {
    fn request_document(&mut self, _: DocumentKind) -> Result<(), EmuError> {
        Ok(())
    }
    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }
    fn poll_external_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        self.0.take().map(Ok)
    }
}

#[test]
fn system_open_waits_for_settings_and_existing_import_then_uses_normal_review() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let document = DocumentOutcome::Selected(PickedDocument {
        kind: DocumentKind::Jar,
        display_name: Some("External.JAR".into()),
        bytes: SINGLE.to_vec(),
    });
    let mut app = FrontendApp::new(&scratch.0, Box::new(ExternalPlatform(Some(document)))).unwrap();
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.process_platform_events();
    assert!(app.import_flow.is_none());
    assert!(matches!(app.screen, Screen::AppSettings(_)));
    app.screen = Screen::Library;
    accept_jar(&mut app, SINGLE);
    app.process_platform_events();
    assert_eq!(prepared(&app).archive_leaf_name(), Some("fixture.jar"));
    app.import_flow = None;
    app.platform_suspended = true;
    app.process_platform_events();
    assert!(app.import_flow.is_none());
    app.platform_suspended = false;
    app.process_platform_events();
    assert_eq!(prepared(&app).archive_leaf_name(), Some("External.JAR"));
    assert!(
        app.entries.is_empty(),
        "system open still needs the user's import choice"
    );
    app.import_flow = None;
    app.process_platform_events();
    assert!(
        app.import_flow.is_none(),
        "the same activation must not be delivered twice"
    );
}

impl PlatformBridge for ImportPlatform {
    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        assert_eq!(kind, DocumentKind::Jad);
        Ok(())
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }
}

fn accept_jar(app: &mut FrontendApp, bytes: &[u8]) {
    app.accept_document(PickedDocument {
        kind: DocumentKind::Jar,
        display_name: Some("fixture.jar".into()),
        bytes: bytes.to_vec(),
    });
}

fn accept_jad(app: &mut FrontendApp) {
    app.picker_pending = false;
    app.accept_document(PickedDocument {
        kind: DocumentKind::Jad,
        display_name: Some("fixture.jad".into()),
        bytes: JAD.to_vec(),
    });
    assert!(app.display_error.is_none());
}

fn prepared(app: &FrontendApp) -> &PreparedImport {
    let Some(ImportFlow::ConfirmProfile(prepared)) =
        app.import_flow.as_ref().map(ImportFlow::review)
    else {
        panic!("import must show its device profile")
    };
    prepared
}

#[test]
fn reimport_invalidates_the_icon_when_the_descriptor_changes() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    let first = inspect_import(ImportSource::new(None, SINGLE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    assert!(app.commit_import(&first, GameSettings::default()));
    let id = app.entries[0].id().to_owned();
    let ctx = egui::Context::default();
    app.load_library_assets(&ctx, 0);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !app.icons.contains_key(&id) {
        assert!(Instant::now() < deadline, "icon loading did not finish");
        app.prepare_library_icons(&ctx, 1, false);
        app.load_library_assets(&ctx, 0);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.icons.contains_key(&id));

    let updated = inspect_import(ImportSource::new(
        None,
        SINGLE.to_vec(),
        Some(b"MIDlet-1: Updated Fixture, /new-icon.png, fixtures.ImportMidlet\n".to_vec()),
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    assert!(app.commit_import(&updated, GameSettings::default()));
    assert_eq!(app.entries.len(), 1);
    assert_eq!(app.entries[0].id(), id);
    assert_eq!(app.entries[0].icon_resource(), Some("/new-icon.png"));
    assert!(!app.icons.contains_key(&id));
}

#[test]
fn single_midlet_jar_opens_one_review_and_jad_updates_its_profile() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    accept_jar(&mut app, SINGLE);
    let before = prepared(&app).automatic_profile_summary();
    assert!(!prepared(&app).source().has_jad());
    assert!(app.entries.is_empty());
    app.choose_import_jad();
    accept_jad(&mut app);
    let after = prepared(&app).automatic_profile_summary();
    assert_ne!(before.canvas_dimensions, after.canvas_dimensions);
    assert_eq!(after.canvas_dimensions, (320, 240));
    assert!(prepared(&app).source().has_jad());
    assert!(app.entries.is_empty());
    drop(app);

    // An unfinished review restores with the descriptor and its updated profile.
    let app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    assert_eq!(prepared(&app).automatic_profile_summary(), after);
    assert!(prepared(&app).source().has_jad());
}

#[test]
fn recreated_frontend_accepts_the_pending_picker_result() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    accept_jar(&mut app, SINGLE);
    app.choose_import_jad();
    drop(app);
    let mut app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    assert!(!prepared(&app).source().has_jad());
    accept_jad(&mut app);
    assert!(prepared(&app).source().has_jad());
    assert_eq!(
        prepared(&app).automatic_profile_summary().canvas_dimensions,
        (320, 240)
    );
}

#[test]
fn jar_with_external_midlet_declaration_can_still_add_jad() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    accept_jar(&mut app, EXTERNAL);
    assert!(matches!(app.import_flow, Some(ImportFlow::NeedsJad(_))));
    assert!(app.display_error.is_none());
    drop(app);
    let mut app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    assert!(matches!(
        app.import_flow.as_ref().map(ImportFlow::review),
        Some(ImportFlow::NeedsJad(_))
    ));
    assert!(app.display_error.is_none());
    app.choose_import_jad();
    accept_jad(&mut app);
    assert_eq!(prepared(&app).midlet().class_name, "fixtures.ImportMidlet");
    assert!(prepared(&app).source().has_jad());
}

#[test]
fn review_shows_jad_and_profile_together_and_continue_finishes_import() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(ImportPlatform)).unwrap();
    for width in [213.0, 302.0, 453.0, 800.0] {
        accept_jar(&mut app, SINGLE);
        let ctx = egui::Context::default();
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 800.0));
        app.safe_content_rect = viewport;
        let mut labels = std::collections::HashMap::new();
        for _ in 0..3 {
            labels.clear();
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(viewport),
                    ..Default::default()
                },
                |ui| {
                    apply_material_theme(ui.ctx(), &app.material_theme);
                    app.draw_import_flow(ui.ctx());
                },
            );
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    labels.insert(
                        text.galley.job.text.clone(),
                        Rect::from_min_size(text.pos, text.galley.size()),
                    );
                }
            }
            output.drop_without_applying_deltas();
        }
        for label in [
            "Import game",
            "Detected device profile",
            "Choose JAD",
            "Continue",
            "Choose profile",
            "Cancel",
        ] {
            let rect = labels
                .get(label)
                .unwrap_or_else(|| panic!("missing {label}"));
            assert!(
                viewport.contains_rect(*rect),
                "{label} at width {width}: {rect:?}"
            );
        }
        assert!(!labels.contains_key("Continue without JAD"));
        let pos = labels["Continue"].center();
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(viewport),
                events: vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| app.draw_import_flow(ui.ctx()),
        )
        .drop_without_applying_deltas();
        assert!(app.import_flow.is_none());
        assert_eq!(app.entries.len(), 1);
        assert!(matches!(
            app.entries[0].settings().device_profile,
            ProfileChoice::Automatic
        ));
        assert!(app.repository.restore_import().unwrap().is_none());
    }
}
