use super::*;

#[test]
fn heap_error_opens_profile_settings_without_changing_the_saved_choice() {
    for manual in [false, true] {
        let scratch = crate::tests::test_storage::Scratch::new();
        let mut app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
        let prepared = frontend_core::inspect_import(ImportSource::new(
            None,
            include_bytes!("../../fixtures/jar/import-single-midlet.zip").to_vec(),
            None,
        ))
        .unwrap()
        .select_midlet(1)
        .unwrap();
        let profile = prepared.automatic_profile_summary();
        let choice = if manual {
            ProfileChoice::Manual {
                profile_id: profile.target_profile_id.clone(),
            }
        } else {
            ProfileChoice::Automatic
        };
        let entry = app
            .repository
            .commit_import(
                &prepared,
                GameSettings {
                    device_profile: choice.clone(),
                    ..GameSettings::default()
                },
            )
            .unwrap();
        let entry_id = entry.id().to_owned();
        app.entries.push(entry);
        app.screen = Screen::Gameplay(Box::new(GameplayScreen {
            entry_id: entry_id.clone(),
            title: "Memory fixture".into(),
            canvas_dimensions: profile.canvas_dimensions,
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
        let command = app.session.start(&entry_id).unwrap();
        let ctx = egui::Context::default();
        crate::i18n::install(&ctx, frontend_core::Language::Russian);
        app.process_runtime_event(
            &ctx,
            SessionEvent {
                session_id: command.session_id,
                attempt_id: command.attempt_id,
                kind: SessionEventKind::ManagedHeapLimit {
                    detail: "Managed heap limit reached".into(),
                    candidate_profile_ids: Vec::new(),
                },
            },
        );
        assert_eq!(app.session.state(), SessionState::Failed);
        app.safe_content_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 360.0));
        click_recovery_settings(&mut app, &ctx);
        let Screen::Settings(settings) = &app.screen else {
            panic!("the memory error must open game settings");
        };
        assert!(settings.focus_profile);
        assert!(matches!(
            &settings.target,
            SettingsTarget::Existing { entry_id: id } if id == &entry_id
        ));
        assert_eq!(settings.draft.device_profile, choice);
        assert_eq!(app.session.state(), SessionState::Idle);
        assert!(app.heap_recovery.is_none());
        app.cancel_settings_screen();
        assert!(matches!(app.screen, Screen::Library));
        assert_eq!(
            app.repository.load().unwrap().entries[0]
                .settings()
                .device_profile,
            choice
        );
    }
}

fn click_recovery_settings(app: &mut FrontendApp, ctx: &egui::Context) {
    let tr = crate::i18n::Translator::from_context(ctx);
    let mut settings = Rect::NOTHING;
    for frame in 0..6 {
        let events = if frame == 5 {
            assert!(app.safe_content_rect.contains_rect(settings));
            [true, false]
                .into_iter()
                .map(|pressed| Event::PointerButton {
                    pos: settings.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                })
                .collect()
        } else {
            Vec::new()
        };
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(app.safe_content_rect),
                time: Some(f64::from(frame)),
                events,
                ..Default::default()
            },
            |ui| app.draw_heap_recovery(ui.ctx()),
        );
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == tr.text("Game settings")
            {
                assert!(!text.galley.elided);
                settings = Rect::from_min_size(text.pos, text.galley.size());
            }
        }
        output.drop_without_applying_deltas();
    }
}

#[test]
fn recovery_save_failure_keeps_the_choices_available_for_retry() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
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
    let profile = prepared.automatic_profile_summary();
    let entry = app
        .repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let entry_id = entry.id().to_owned();
    app.entries.push(entry);
    app.screen = Screen::Gameplay(Box::new(GameplayScreen {
        entry_id: entry_id.clone(),
        title: "Recovery fixture".into(),
        canvas_dimensions: profile.canvas_dimensions,
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
    let command = app.session.start(&entry_id).unwrap();
    app.session.apply_event(&SessionEvent::terminal_error(
        command.session_id,
        command.attempt_id,
        "Fixture OOM",
        "Recovery fixture",
    ));
    app.heap_recovery = Some(HeapRecovery {
        detail: "Recovery fixture".into(),
        candidate_profile_ids: vec![profile.target_profile_id],
    });
    app.safe_content_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 960.0));
    let metadata = root
        .join("library")
        .join("entries")
        .join(format!("{entry_id}.json"));
    let original = metadata.with_extension("original");
    std::fs::rename(&metadata, &original).unwrap();
    std::fs::create_dir(&metadata).unwrap();

    let ctx = egui::Context::default();
    render_recovery(&mut app, &ctx, Vec::new());
    let position = render_recovery(&mut app, &ctx, Vec::new()).unwrap();
    for pressed in [true, false] {
        render_recovery(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
    }
    assert!(
        app.display_error.is_some(),
        "save must fail while its destination is a directory"
    );
    assert!(
        app.heap_recovery.is_some(),
        "failed selection must retain the recovery dialog"
    );
    assert_eq!(app.session.state(), SessionState::Failed);
    assert_eq!(
        app.entries[0].settings().device_profile,
        ProfileChoice::Automatic
    );

    std::fs::remove_dir(&metadata).unwrap();
    std::fs::rename(original, metadata).unwrap();
    app.display_error = None;
    assert!(render_recovery(&mut app, &ctx, Vec::new()).is_some());
    let profile_id = app.heap_recovery.as_ref().unwrap().candidate_profile_ids[0].clone();
    app.retry_with_profile(&profile_id);
    assert!(app.display_error.is_none());
    assert!(app.heap_recovery.is_none());
    assert_eq!(app.session.state(), SessionState::Starting);
    app.runtime.shutdown().unwrap();
}

fn render_recovery(app: &mut FrontendApp, ctx: &egui::Context, events: Vec<Event>) -> Option<Pos2> {
    let tr = crate::i18n::Translator::from_context(ctx);
    let retry_labels: Vec<_> = app
        .heap_recovery
        .iter()
        .flat_map(|recovery| &recovery.candidate_profile_ids)
        .map(|id| {
            let name = profile_display_name(&app.profile_options, id)
                .unwrap_or("Unavailable device profile");
            tr.format(
                "Retry with {profile}",
                &[("profile", &tr.profile_name(name))],
            )
        })
        .collect();
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(app.safe_content_rect),
            events,
            ..Default::default()
        },
        |ui| app.draw_heap_recovery(ui.ctx()),
    );
    let position = output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if retry_labels.iter().any(|label| label == text.galley.text()) => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        _ => None,
    });
    output.drop_without_applying_deltas();
    position
}
