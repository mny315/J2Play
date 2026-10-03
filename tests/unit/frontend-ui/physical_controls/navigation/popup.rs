use super::*;

#[test]
fn controller_profile_menu_scrolls_through_every_option_and_keeps_focus_inside() {
    for show_all in [false, true] {
        check_profile_menu_navigation(show_all);
    }
}

fn check_profile_menu_navigation(show_all: bool) {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    app.focus_ring.navigate();
    open_settings_page(&mut app, false);
    if let Screen::Settings(settings) = &mut app.screen {
        settings.show_all_profiles = show_all;
    }
    let ctx = egui::Context::default();
    let (owner, output) = paint_settings(&mut app, &ctx, None);
    let owner = owner.unwrap().id;
    output.drop_without_applying_deltas();
    let (first, output) = settings_step(&mut app, &ctx, GamepadButton::South);
    assert_ne!(
        first.id, owner,
        "opening the menu must move focus into its selected option"
    );
    let popup_layer = first.layer_id;
    output.drop_without_applying_deltas();
    let tr = crate::i18n::Translator(frontend_core::Language::English);
    let options: Vec<_> = app
        .profile_options
        .iter()
        .filter(|option| option.visible_in_menu(show_all, &crate::ProfileChoice::Automatic))
        .cloned()
        .collect();
    for option in &options {
        let (focused, output) = settings_step(&mut app, &ctx, GamepadButton::DpadDown);
        assert_eq!(focused.layer_id, popup_layer);
        assert!(
            focused.interact_rect.height() >= 40.0,
            "focused option must scroll into view"
        );
        assert_focused_label(
            &focused,
            &output,
            &tr.profile_name(option.menu_name(show_all)),
        );
        output.drop_without_applying_deltas();
    }
    let (last, output) = settings_step(&mut app, &ctx, GamepadButton::DpadDown);
    assert_eq!(
        last.layer_id, popup_layer,
        "the end of the list must not escape to settings"
    );
    output.drop_without_applying_deltas();
    settings_step(&mut app, &ctx, GamepadButton::South)
        .1
        .drop_without_applying_deltas();
    let Screen::Settings(settings) = &app.screen else {
        panic!("settings must remain open");
    };
    assert_eq!(
        settings.draft.device_profile,
        crate::ProfileChoice::Manual {
            profile_id: options.last().unwrap().profile_id.clone()
        }
    );
    let (returned, output) = paint_settings(&mut app, &ctx, None);
    assert_eq!(returned.unwrap().id, owner);
    output.drop_without_applying_deltas();
}

#[test]
fn profile_menu_reopens_at_the_selected_row_and_back_cancels_only_the_menu() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    app.focus_ring.navigate();
    open_settings_page(&mut app, false);
    let tr = crate::i18n::Translator(frontend_core::Language::English);
    let options = app.profile_options.clone();
    let choice = crate::ProfileChoice::Manual {
        profile_id: options.last().unwrap().profile_id.clone(),
    };
    if let Screen::Settings(settings) = &mut app.screen {
        settings.draft.device_profile = choice.clone();
    }
    if let Screen::Settings(settings) = &mut app.screen {
        settings.show_all_profiles = true;
    }
    let ctx = egui::Context::default();
    let (owner, output) = paint_settings(&mut app, &ctx, None);
    let owner = owner.unwrap().id;
    output.drop_without_applying_deltas();
    let (focused, output) = settings_step(&mut app, &ctx, GamepadButton::South);
    assert_focused_label(
        &focused,
        &output,
        &tr.profile_name(&options.last().unwrap().display_name),
    );
    let layer = focused.layer_id;
    output.drop_without_applying_deltas();
    for label in options
        .iter()
        .rev()
        .skip(1)
        .map(|option| option.display_name.as_str())
        .chain(["Automatic"])
    {
        let (focused, output) = settings_step(&mut app, &ctx, GamepadButton::DpadUp);
        assert_eq!(focused.layer_id, layer);
        assert_focused_label(&focused, &output, &tr.profile_name(label));
        output.drop_without_applying_deltas();
    }
    for button in [
        GamepadButton::DpadUp,
        GamepadButton::DpadLeft,
        GamepadButton::DpadRight,
    ] {
        let (focused, output) = settings_step(&mut app, &ctx, button);
        assert_focused_label(&focused, &output, "Automatic");
        output.drop_without_applying_deltas();
    }
    let (focused, output) = settings_step(&mut app, &ctx, GamepadButton::East);
    assert_eq!(focused.id, owner);
    assert!(!egui::ComboBox::is_open(&ctx, owner));
    output.drop_without_applying_deltas();
    let Screen::Settings(settings) = &app.screen else {
        panic!("Back must keep settings open");
    };
    assert_eq!(
        settings.draft.device_profile, choice,
        "navigation alone must not select a profile"
    );
}

#[test]
fn controller_toggles_full_profile_list_without_changing_saved_historical_choice() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    app.focus_ring.navigate();
    open_settings_page(&mut app, false);
    let choice = crate::ProfileChoice::Manual {
        profile_id: "se-jp8-keypad".to_owned(),
    };
    if let Screen::Settings(settings) = &mut app.screen {
        settings.draft.device_profile = choice.clone();
    }
    let ctx = egui::Context::default();
    let tr = crate::i18n::Translator(frontend_core::Language::English);
    let name = tr
        .profile_name(crate::profile_display_name(&app.profile_options, "se-jp8-keypad").unwrap());
    paint_settings(&mut app, &ctx, None)
        .1
        .drop_without_applying_deltas();
    for all in [true, false] {
        let (toggle, output) = settings_step(&mut app, &ctx, GamepadButton::DpadDown);
        assert_focused_label(&toggle, &output, "Show all profiles");
        output.drop_without_applying_deltas();
        settings_step(&mut app, &ctx, GamepadButton::South)
            .1
            .drop_without_applying_deltas();
        let Screen::Settings(settings) = &app.screen else {
            panic!("settings must remain open")
        };
        assert_eq!(settings.show_all_profiles, all);
        assert_eq!(settings.draft.device_profile, choice);
        settings_step(&mut app, &ctx, GamepadButton::DpadUp)
            .1
            .drop_without_applying_deltas();
        let (selected, output) = settings_step(&mut app, &ctx, GamepadButton::South);
        assert_focused_label(&selected, &output, &name);
        output.drop_without_applying_deltas();
        settings_step(&mut app, &ctx, GamepadButton::East)
            .1
            .drop_without_applying_deltas();
    }
}

#[test]
fn language_and_profile_menu_open_without_bouncing_on_first_open_and_reopen() {
    use eframe::App;
    for global in [true, false] {
        let scratch = crate::tests::test_storage::Scratch::new();
        let mut app =
            FrontendApp::new(&scratch.0, Box::new(crate::UnavailablePlatformBridge)).unwrap();
        app.startup_splash = None;
        open_settings_page(&mut app, global);
        if let Screen::AppSettings(settings) = &mut app.screen {
            settings.draft.language = Some(frontend_core::Language::Russian);
        }
        if let Screen::Settings(settings) = &mut app.screen {
            settings.show_all_profiles = true;
        }
        let ctx = egui::Context::default();
        crate::apply_material_theme(&ctx, &app.material_theme);
        crate::i18n::install(&ctx, frontend_core::Language::English);
        let mut frame = eframe::Frame::_new_kittest();
        let mut owner = egui::Id::NULL;
        let mut row_top = None;
        for tick in 0..75 {
            if tick == 4 || tick == 42 {
                egui::Popup::open_id(&ctx, owner.with("popup"));
            }
            if tick == 36 {
                assert!(crate::focus_navigation::close_popup(&ctx));
                row_top = None;
            }
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(453.0, 960.0),
                    )),
                    time: Some(100.0 + f64::from(tick) / 60.0),
                    ..Default::default()
                },
                |ui| app.ui(ui, &mut frame),
            );
            let response = ctx
                .memory(egui::Memory::focused)
                .and_then(|id| ctx.read_response(id))
                .unwrap();
            if tick == 3 {
                owner = response.id;
            }
            // Skip the hidden sizing pass; keep the real scroll animation enabled.
            if (5..36).contains(&tick) || tick >= 43 {
                assert_ne!(response.id, owner);
                assert!(egui::ComboBox::is_open(&ctx, owner));
                let text = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text)
                            if response.rect.contains(text.pos)
                                && text.galley.job.text
                                    == if global {
                                        "Русский"
                                    } else {
                                        "Automatic"
                                    } =>
                        {
                            Some(text)
                        }
                        _ => None,
                    })
                    .expect("the selected row must remain visible throughout opening");
                let expected = *row_top.get_or_insert(text.pos.y);
                assert!(
                    (text.pos.y - expected).abs() < 1.0,
                    "opening must not move edge rows away and snap back: global={global}, tick={tick}, y={}, expected={expected}",
                    text.pos.y
                );
            }
            output.drop_without_applying_deltas();
        }
    }
}
