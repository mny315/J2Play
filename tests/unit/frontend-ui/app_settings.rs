use super::*;

#[test]
fn failed_app_settings_write_keeps_the_draft_for_retry() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let path = root.join("library/frontend-state/app-settings.json");
    // A directory at the destination forces an atomic rename error.
    std::fs::create_dir_all(&path).unwrap();
    let mut draft = app.app_settings.clone();
    draft.control_layout.left_soft_key.size_percent = 150;
    draft.landscape_control_layout.right_soft_key.offset_x = -750;
    draft.ui_scale_percent = 125;
    draft.fullscreen = frontend_core::FullscreenMode::LandscapeOnly;
    draft.theme = frontend_core::AppTheme::Oled;
    draft.accent_color = frontend_core::AccentColor::Green;
    draft.language = Some(frontend_core::Language::Arabic);
    app.screen = Screen::AppSettings(draft.clone().into());
    app.save_settings_screen();
    assert!(app.display_error.is_some());
    let Screen::AppSettings(screen) = &app.screen else {
        panic!("failed save must keep the editor")
    };
    assert_eq!(screen.draft, draft);
    assert_eq!(app.app_settings, frontend_core::AppSettings::default());
    std::fs::remove_dir(&path).unwrap();
    app.display_error = None;
    app.save_settings_screen();
    assert_eq!(app.repository.load_app_settings().unwrap(), draft);
    assert_eq!(app.app_settings, draft);
    assert!(matches!(app.screen, Screen::Library));
}

#[test]
fn launch_inherits_every_global_control_and_preserves_explicit_game_overrides() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    for layout in [
        &mut app.app_settings.control_layout,
        &mut app.app_settings.landscape_control_layout,
    ] {
        let controls = [
            &mut layout.direction_pad,
            &mut layout.fire,
            &mut layout.left_soft_key,
            &mut layout.right_soft_key,
        ];
        for (index, transform) in controls
            .into_iter()
            .chain(layout.number_keys.iter_mut())
            .enumerate()
        {
            transform.size_percent = 65 + u16::try_from(index).unwrap() * 5;
            transform.offset_x = i16::try_from(index).unwrap() * 100 - 750;
            transform.offset_y = 500;
            transform.visible = index % 3 != 0;
        }
    }
    app.app_settings.vibration.strength_percent = 37;
    app.app_settings.fullscreen = frontend_core::FullscreenMode::On;
    let physical = frontend_core::physical_input::PhysicalControl::Gamepad {
        button: frontend_core::physical_input::GamepadButton::RightStick,
    };
    app.app_settings
        .physical_bindings
        .assign(
            physical,
            frontend_core::physical_input::PhysicalAction::Phone(HostAction::Num5),
        )
        .unwrap();
    app.repository.save_app_settings(&app.app_settings).unwrap();
    let prepared = frontend_core::inspect_import(frontend_core::ImportSource::new(
        None,
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
    let entry = app
        .repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let id = entry.id().to_owned();
    app.entries.push(entry);
    app.open_game(&id);
    app.sync_gameplay_fullscreen_policy(Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 960.0)));
    let Screen::Gameplay(gameplay) = &app.screen else {
        panic!("fixture must launch")
    };
    assert_eq!(gameplay.control_layout, app.app_settings.control_layout);
    assert_eq!(
        gameplay.landscape_control_layout,
        app.app_settings.landscape_control_layout
    );
    assert_eq!(gameplay.vibration, app.app_settings.vibration);
    assert!(gameplay.fullscreen);
    assert_eq!(app.physical_bindings, app.app_settings.physical_bindings);
    drop(app);
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let manual = GameSettings {
        fullscreen: Some(frontend_core::FullscreenMode::Off),
        controls_override: true,
        physical_bindings: Some(frontend_core::physical_input::PhysicalBindings::default()),
        ..GameSettings::default()
    };
    app.repository
        .save_settings(&mut app.entries[0], manual.clone())
        .unwrap();
    app.open_game(&id);
    app.sync_gameplay_fullscreen_policy(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 453.0)));
    let Screen::Gameplay(gameplay) = &app.screen else {
        panic!("fixture must launch")
    };
    assert_eq!(gameplay.control_layout, manual.control_layout);
    assert_eq!(
        gameplay.landscape_control_layout,
        manual.landscape_control_layout
    );
    assert_eq!(gameplay.vibration, manual.vibration);
    assert!(!gameplay.fullscreen);
    assert_eq!(app.physical_bindings, manual.physical_bindings.unwrap());
    assert_eq!(
        app.physical_bindings.resolve(physical),
        Some(frontend_core::physical_input::PhysicalAction::StopGame)
    );
}

#[test]
fn app_settings_save_cancel_back_and_recovery() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let settings = frontend_core::AppSettings {
        language: Some(frontend_core::Language::Chinese),
        fps_limit: FpsLimit::Manual {
            frames_per_second: 30,
        },
        ui_scale_percent: 65,
        library_view: frontend_core::LibraryView::Tiles,
        fullscreen: frontend_core::FullscreenMode::LandscapeOnly,
        theme: frontend_core::AppTheme::Oled,
        accent_color: frontend_core::AccentColor::Orange,
        ..Default::default()
    };
    app.screen = Screen::AppSettings(settings.clone().into());
    app.cancel_settings_screen();
    assert!(matches!(app.screen, Screen::Library));
    assert_eq!(
        app.repository.load_app_settings().unwrap(),
        frontend_core::AppSettings::default()
    );
    app.screen = Screen::AppSettings(settings.clone().into());
    app.save_settings_screen();
    assert!(matches!(app.screen, Screen::Library));
    drop(app);
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    assert_eq!(app.app_settings, settings);
    app.startup_splash = None;
    app.screen = Screen::AppSettings(frontend_core::AppSettings::default().into());
    let ctx = egui::Context::default();
    ctx.run_ui(
        egui::RawInput {
            events: vec![Event::Key {
                key: Key::BrowserBack,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ui| app.handle_back(ui.ctx(), true),
    )
    .drop_without_applying_deltas();
    assert!(matches!(app.screen, Screen::Library));
    assert!(!app.exit_confirmation);
    assert_eq!(app.app_settings, settings);
    drop(app);
    std::fs::write(
        root.join("library/frontend-state/app-settings.json"),
        b"invalid",
    )
    .unwrap();
    let app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    assert!(app.display_error.is_some());
    assert_eq!(app.app_settings, frontend_core::AppSettings::default());
}

#[test]
fn library_app_actions_fit_scaled_phone_widths_and_open_settings() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let ctx = egui::Context::default();
    for width in [213.0, 260.0, 302.0, 360.0, 453.0, 640.0] {
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 900.0));
        let mut gear = Rect::NOTHING;
        let mut import = Rect::NOTHING;
        for _ in 0..3 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(viewport),
                    ..Default::default()
                },
                |ui| {
                    apply_material_theme(ui.ctx(), &app.material_theme);
                    app.draw_library_app_bar(ui);
                },
            );
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    let rect = Rect::from_min_size(text.pos, text.galley.size());
                    match text.galley.job.text.as_str() {
                        "⚙" => gear = rect,
                        "+" | "+ Import" | "+ Import game" => import = rect,
                        _ => {}
                    }
                }
            }
            output.drop_without_applying_deltas();
        }
        assert!(
            viewport.contains_rect(gear),
            "settings at {width}: {gear:?}"
        );
        assert!(
            viewport.contains_rect(import),
            "import at {width}: {import:?}"
        );
        assert!(gear.right() < import.left());
        let pos = gear.center();
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
            |ui| app.draw_library_app_bar(ui),
        )
        .drop_without_applying_deltas();
        assert!(matches!(app.screen, Screen::AppSettings(_)));
        app.cancel_settings_screen();
    }
}

#[test]
fn global_editor_transactions_orientations_back_and_persistence() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_control_editor();
    let editor = app.screen.control_editor_mut().unwrap();
    assert!(editor.layout.is_default());
    editor.layout.fire.offset_x = 750;
    editor.select_orientation(true);
    editor.layout.number_keys[0].visible = false;
    editor.vibration.strength_percent = 37;
    editor.reset_layout();
    assert!(editor.layout.is_default());
    assert_eq!(editor.other_layout.fire.offset_x, 750);
    editor.layout.fire.size_percent = 140;
    editor.landscape_requested = true;
    let ctx = egui::Context::default();
    let back = |app: &mut FrontendApp| {
        ctx.run_ui(
            egui::RawInput {
                events: vec![Event::Key {
                    key: Key::BrowserBack,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| app.handle_back(ui.ctx(), true),
        )
        .drop_without_applying_deltas();
    };
    back(&mut app);
    assert!(!app.screen.control_editor_mut().unwrap().landscape_requested);
    assert_eq!(
        app.screen
            .control_editor_mut()
            .unwrap()
            .layout
            .fire
            .size_percent,
        140
    );
    app.screen.commit_control_editor();
    assert_eq!(
        app.repository.load_app_settings().unwrap(),
        frontend_core::AppSettings::default()
    );
    app.open_control_editor();
    let editor = app.screen.control_editor_mut().unwrap();
    assert_eq!(editor.layout.fire.offset_x, 750);
    assert_eq!(editor.other_layout.fire.size_percent, 140);
    editor.layout.fire.offset_x = -900;
    back(&mut app);
    let Screen::AppSettings(settings) = &app.screen else {
        panic!("must return to app settings")
    };
    assert!(settings.control_editor.is_none());
    assert_eq!(settings.draft.control_layout.fire.offset_x, 750);
    app.save_settings_screen();
    drop(app);
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    assert_eq!(app.app_settings.control_layout.fire.offset_x, 750);
    assert_eq!(
        app.app_settings.landscape_control_layout.fire.size_percent,
        140
    );
    assert_eq!(app.app_settings.vibration.strength_percent, 37);
    let saved = app.app_settings.clone();
    app.screen = Screen::AppSettings(saved.clone().into());
    app.open_control_editor();
    app.screen.control_editor_mut().unwrap().reset_layout();
    app.screen.commit_control_editor();
    app.cancel_settings_screen();
    assert_eq!(app.repository.load_app_settings().unwrap(), saved);
}
