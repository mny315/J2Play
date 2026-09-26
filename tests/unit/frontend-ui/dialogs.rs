use super::*;

#[test]
fn resume_and_import_headers_span_the_window_after_viewport_changes() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
    let prepared = frontend_core::inspect_import(frontend_core::ImportSource::new(
        None,
        include_bytes!("../../fixtures/jar/import-single-midlet.zip").to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    app.import_flow = Some(ImportFlow::ConfirmProfile(Box::new(prepared)));
    for import in [false, true] {
        let ctx = egui::Context::default();
        apply_material_theme(&ctx, &app.material_theme);
        crate::i18n::install(&ctx, frontend_core::Language::Russian);
        let mut time = 0.0;
        // Reuse the window state across fullscreen and narrow viewport changes.
        for size in [
            Vec2::new(800.0, 600.0),
            Vec2::new(1280.0, 720.0),
            Vec2::new(640.0, 360.0),
            Vec2::new(320.0, 800.0),
            Vec2::new(213.0, 800.0),
        ] {
            let viewport = Rect::from_min_size(Pos2::ZERO, size);
            app.safe_content_rect = viewport;
            for frame in 0..6 {
                time += 1.0;
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(viewport),
                        time: Some(time),
                        ..Default::default()
                    },
                    |ui| {
                        if import {
                            app.draw_import_flow(ui.ctx());
                        } else {
                            app.draw_resume_request(
                                ui.ctx(),
                                1,
                                "Fixture",
                                true,
                                "An automatic save is available. Continue from where you left off?",
                            );
                        }
                    },
                );
                if frame == 5 {
                    let window = ctx
                        .memory(|memory| {
                            memory.area_rect(egui::Id::new(if import {
                                "import-game"
                            } else {
                                "resume-game"
                            }))
                        })
                        .expect("dialog frame");
                    let mut header = None;
                    for shape in &output.shapes {
                        if let egui::Shape::Rect(rect) = &shape.shape
                            && rect.fill == ctx.global_style().visuals.widgets.open.weak_bg_fill
                        {
                            header = Some(rect.rect);
                        }
                    }
                    let header = header.expect("active title bar");
                    assert!(
                        viewport.expand(1.0).contains_rect(window),
                        "import={import}, {size:?}: {window:?}"
                    );
                    assert!(
                        (window.left() - header.left()).abs() <= 1.0
                            && (window.right() - header.right()).abs() <= 1.0,
                        "import={import}, {size:?}: header {header:?} must span {window:?}"
                    );
                }
                output.drop_without_applying_deltas();
            }
        }
    }
}

#[test]
fn expanded_error_details_keep_close_inside_a_short_viewport() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(700.0, 320.0));
    app.safe_content_rect = viewport;
    app.display_error = Some(DisplayError {
        title: "Game stopped".into(),
        user_message: "The emulator reported an error.".into(),
        technical_details: "fixtures/Probe::method()V pc=123\n".repeat(100),
    });
    let ctx = egui::Context::default();
    let mut details = Pos2::ZERO;
    let mut close = Rect::NOTHING;
    let mut expanded = false;
    for frame in 0..6 {
        let events = if frame == 2 {
            vec![
                Event::PointerMoved(details),
                Event::PointerButton {
                    pos: details,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                Event::PointerButton {
                    pos: details,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        } else {
            Vec::new()
        };
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(viewport),
                time: Some(f64::from(frame)),
                events,
                ..Default::default()
            },
            |ui| app.draw_error(ui.ctx()),
        );
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                let rect = Rect::from_min_size(text.pos, text.galley.size());
                match text.galley.job.text.as_str() {
                    "Technical details" => details = rect.center(),
                    "Close" => close = rect,
                    detail if detail.starts_with("fixtures/Probe::method") => expanded = true,
                    _ => {}
                }
            }
        }
        output.drop_without_applying_deltas();
    }
    assert_ne!(details, Pos2::ZERO);
    assert!(expanded, "the test must expand the technical details");
    assert!(
        viewport.contains_rect(close),
        "Close must remain visible: {close:?}"
    );
}
