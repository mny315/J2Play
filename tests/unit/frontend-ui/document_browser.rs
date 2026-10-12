use super::*;

struct BrowserPlatform {
    view: Option<DocumentBrowser>,
    outcome: Option<DocumentOutcome>,
}

impl BrowserPlatform {
    fn new(kind: DocumentKind) -> Self {
        Self {
            view: Some(DocumentBrowser {
                kind,
                folder: "/home/deck/Java games".into(),
                revision: 1,
                parent: Some(1),
                shortcuts: [
                    (2, "Home"),
                    (3, "Downloads"),
                    (4, "Removable drives"),
                    (5, "File system"),
                ]
                .into_iter()
                .map(|(id, name)| DocumentBrowserEntry {
                    id,
                    name: name.into(),
                    directory: true,
                })
                .collect(),
                entries: vec![DocumentBrowserEntry {
                    id: 6,
                    name: "Fixture.jar".into(),
                    directory: false,
                }],
                busy: false,
                truncated: false,
                error: None,
            }),
            outcome: None,
        }
    }
}

impl PlatformBridge for BrowserPlatform {
    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        self.view.as_mut().unwrap().kind = kind;
        Ok(())
    }
    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        self.outcome.take().map(Ok)
    }
    fn document_browser(&self) -> Option<&DocumentBrowser> {
        self.view.as_ref()
    }
    fn cancel_document_browser(&mut self) {
        self.outcome = self
            .view
            .take()
            .map(|view| DocumentOutcome::Cancelled { kind: view.kind });
    }
    fn activate_document_entry(&mut self, id: u64) -> Result<(), EmuError> {
        assert_eq!(id, 6);
        self.view = None;
        self.outcome = Some(DocumentOutcome::Selected(PickedDocument {
            kind: DocumentKind::Jar,
            display_name: Some("Fixture.jar".into()),
            bytes: include_bytes!("../../fixtures/jar/import-single-midlet.zip").to_vec(),
        }));
        Ok(())
    }
}

#[test]
fn back_cancels_only_the_jad_browser_and_preserves_import_review() {
    let scratch = test_storage::Scratch::new();
    let mut app = FrontendApp::new(
        &scratch.0,
        Box::new(BrowserPlatform::new(DocumentKind::Jad)),
    )
    .unwrap();
    app.accept_document(PickedDocument {
        kind: DocumentKind::Jar,
        display_name: Some("Fixture.jar".into()),
        bytes: include_bytes!("../../fixtures/jar/import-single-midlet.zip").to_vec(),
    });
    app.choose_import_jad();
    assert!(app.picker_pending);
    assert!(app.overlay_active());
    app.navigate_back();
    app.process_platform_events();
    assert!(app.platform.document_browser().is_none());
    assert!(matches!(
        app.import_flow,
        Some(ImportFlow::ConfirmProfile(_))
    ));
    assert!(!app.picker_pending);
    assert!(!app.exit_confirmation);
    assert!(app.repository.restore_import().unwrap().is_some());
}

#[test]
fn browser_fits_small_screens_and_selects_into_the_existing_import_review() {
    for size in [
        Vec2::new(320.0, 320.0),
        Vec2::new(800.0, 1280.0),
        Vec2::new(1280.0, 800.0),
    ] {
        let scratch = test_storage::Scratch::new();
        let mut app = FrontendApp::new(
            &scratch.0,
            Box::new(BrowserPlatform::new(DocumentKind::Jar)),
        )
        .unwrap();
        let ctx = egui::Context::default();
        apply_material_theme(&ctx, &app.material_theme);
        let viewport = Rect::from_min_size(Pos2::ZERO, size);
        app.safe_content_rect = viewport;
        let mut cancel = None;
        let mut file = None;
        for _ in 0..3 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(viewport),
                    ..Default::default()
                },
                |ui| app.draw_document_browser(ui.ctx()),
            );
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    let rect = Rect::from_min_size(text.pos, text.galley.size());
                    match text.galley.job.text.as_str() {
                        "Cancel" => cancel = Some(rect),
                        "Fixture.jar" => file = Some(rect),
                        _ => {}
                    }
                }
            }
            output.drop_without_applying_deltas();
        }
        assert!(
            viewport.contains_rect(cancel.expect("visible cancel")),
            "size {size:?}"
        );
        if size.x > 320.0 {
            let pos = file.expect("visible file").center();
            for pressed in [true, false] {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(viewport),
                        events: vec![
                            Event::PointerMoved(pos),
                            Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ui| app.draw_document_browser(ui.ctx()),
                )
                .drop_without_applying_deltas();
            }
        } else {
            app.platform.activate_document_entry(6).unwrap();
        }
        app.process_platform_events();
        assert!(matches!(
            app.import_flow,
            Some(ImportFlow::ConfirmProfile(_))
        ));
        assert!(app.entries.is_empty());
        assert!(app.display_error.is_none());
    }
}

#[test]
fn controller_can_reach_and_activate_a_file_in_the_modal() {
    use frontend_core::physical_input::{GamepadButton, PhysicalControl, PhysicalInputEvent};
    let scratch = test_storage::Scratch::new();
    let mut app = FrontendApp::new(
        &scratch.0,
        Box::new(BrowserPlatform::new(DocumentKind::Jar)),
    )
    .unwrap();
    let ctx = egui::Context::default();
    apply_material_theme(&ctx, &app.material_theme);
    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0));
    app.safe_content_rect = viewport;
    let mut file = None;
    let mut paint = |app: &mut FrontendApp, mut button: Option<GamepadButton>| {
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(viewport),
                ..Default::default()
            },
            |ui| {
                if let Some(button) = button.take() {
                    for pressed in [true, false] {
                        app.process_physical_event(
                            &ctx,
                            PhysicalInputEvent::Button {
                                device: 1,
                                control: PhysicalControl::Gamepad { button },
                                pressed,
                            },
                            false,
                        );
                    }
                }
                ui.add_enabled_ui(!app.overlay_active(), |ui| app.draw_library(ui));
                app.draw_document_browser(ui.ctx());
            },
        );
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.job.text == "Fixture.jar"
            {
                file = Some(Rect::from_min_size(text.pos, text.galley.size()).center());
            }
        }
        output.drop_without_applying_deltas();
        ctx.memory(egui::Memory::focused)
            .and_then(|id| ctx.read_response(id))
            .is_some_and(|response| file.is_some_and(|pos| response.rect.contains(pos)))
    };
    for _ in 0..3 {
        paint(&mut app, None);
    }
    let mut reached = false;
    for _ in 0..6 {
        paint(&mut app, Some(GamepadButton::DpadDown));
        if paint(&mut app, None) {
            reached = true;
            break;
        }
    }
    assert!(
        reached,
        "controller must reach the file without leaving the modal"
    );
    paint(&mut app, Some(GamepadButton::South));
    app.process_platform_events();
    assert!(matches!(
        app.import_flow,
        Some(ImportFlow::ConfirmProfile(_))
    ));
}
