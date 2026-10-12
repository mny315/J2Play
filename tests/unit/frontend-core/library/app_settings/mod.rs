use super::*;
mod diagonals;
mod keyboard;
mod language;

#[test]
fn stick_settings_migrate_and_inherit_without_overwriting_game_overrides() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let mut settings = AppSettings::default();
    settings.control_layout.direction_pad.offset_x = 500;
    repository.save_app_settings(&settings).unwrap();
    let mut old = serde_json::to_value(&settings).unwrap();
    old["schema_version"] = 9.into();
    for name in ["control_layout", "landscape_control_layout"] {
        old[name].as_object_mut().unwrap().remove("stick_enabled");
    }
    fs::write(
        scratch.0.join("frontend-state/app-settings.json"),
        serde_json::to_vec(&old).unwrap(),
    )
    .unwrap();
    assert_eq!(repository.load_app_settings().unwrap(), settings);
    settings.landscape_control_layout.stick_enabled = true;
    repository.save_app_settings(&settings).unwrap();
    let loaded = repository.load_app_settings().unwrap();
    assert_eq!(loaded, settings);
    let mut game = GameSettings::default();
    loaded.apply_control_defaults(&mut game);
    assert_eq!(game.control_layout, settings.control_layout);
    assert_eq!(
        game.landscape_control_layout,
        settings.landscape_control_layout
    );
    game.controls_override = true;
    game.landscape_control_layout.stick_enabled = false;
    loaded.apply_control_defaults(&mut game);
    assert!(!game.landscape_control_layout.stick_enabled);
}

fn legacy_default_settings(schema_version: u32) -> AppSettings {
    use crate::physical_input::{GamepadButton as B, PhysicalAction as A, PhysicalControl as C};
    use platform::HostAction as H;
    let mut settings = AppSettings {
        physical_bindings: keyboard::legacy_bindings(),
        ..AppSettings::default()
    };
    settings.physical_bindings.bindings.retain(|binding| {
        !matches!(
            binding.control,
            C::Gamepad {
                button: B::LeftStick | B::RightStick
            }
        )
    });
    for binding in &mut settings.physical_bindings.bindings {
        binding.action = match binding.control {
            C::Gamepad { button: B::South } => A::Phone(H::Fire),
            C::Gamepad { button: B::East } => A::Phone(H::SoftRight),
            C::Gamepad { button: B::West } => A::Phone(if schema_version <= 6 {
                H::Num1
            } else {
                H::Num0
            }),
            C::Gamepad { button: B::North } => A::Phone(if schema_version <= 6 {
                H::Num3
            } else {
                H::Pound
            }),
            _ => binding.action,
        };
    }
    settings
}

#[test]
fn saved_original_bindings_upgrade_but_custom_bindings_are_preserved() {
    use crate::physical_input::{GamepadButton as B, PhysicalAction as A, PhysicalControl as C};
    use platform::HostAction as H;
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    for schema_version in 1..=8 {
        let settings = legacy_default_settings(schema_version);
        repository.save_app_settings(&settings).unwrap();
        // Deliberately choosing an old layout in the current version is retained.
        assert_eq!(repository.load_app_settings().unwrap(), settings);
        let write_legacy = |settings: &AppSettings| {
            let mut old = serde_json::to_value(settings).unwrap();
            old["schema_version"] = schema_version.into();
            fs::write(
                root.join("frontend-state/app-settings.json"),
                serde_json::to_vec(&old).unwrap(),
            )
            .unwrap();
        };
        write_legacy(&settings);
        let loaded = repository.load_app_settings().unwrap();
        assert_eq!(loaded, AppSettings::default());
        for (button, action) in [
            (B::South, A::Phone(H::Num5)),
            (B::North, A::Phone(H::Num0)),
            (B::West, A::Phone(H::Pound)),
            (B::East, A::Phone(H::Star)),
            (B::LeftShoulder, A::Phone(H::SoftLeft)),
            (B::RightShoulder, A::Phone(H::SoftRight)),
            (B::LeftStick, A::DebugOverlay),
            (B::RightStick, A::StopGame),
        ] {
            assert_eq!(
                loaded.physical_bindings.resolve(C::Gamepad { button }),
                Some(action)
            );
        }
        // Bindings outside the requested changes retain their order and values.
        let unchanged = |bindings: &crate::physical_input::PhysicalBindings| {
            bindings
                .bindings
                .iter()
                .copied()
                .filter(|binding| {
                    !matches!(
                        binding.control,
                        C::Keyboard { .. }
                            | C::Gamepad {
                                button: B::South
                                    | B::North
                                    | B::West
                                    | B::East
                                    | B::LeftStick
                                    | B::RightStick
                            }
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            unchanged(&loaded.physical_bindings),
            unchanged(&settings.physical_bindings)
        );

        for variant in 0..4 {
            let mut custom = settings.clone();
            match variant {
                0 => custom
                    .physical_bindings
                    .assign(C::Gamepad { button: B::South }, A::Phone(H::Num5))
                    .unwrap(),
                1 => custom.physical_bindings.dead_zone_percent = 40,
                2 => custom
                    .physical_bindings
                    .remove(C::Gamepad { button: B::West }),
                _ => custom.physical_bindings.bindings.clear(),
            }
            write_legacy(&custom);
            let loaded = repository.load_app_settings().unwrap();
            let expected = if custom.physical_bindings.bindings.is_empty() {
                custom
            } else {
                keyboard::with_updated_keyboard(custom)
            };
            assert_eq!(loaded, expected);
        }
    }
}

#[test]
fn defaults_precedence_and_atomic_persistence() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let mut settings = repository.load_app_settings().unwrap();
    assert_eq!(settings, AppSettings::default());
    assert!(settings.control_layout.visible);
    assert!(settings.landscape_control_layout.visible);
    assert_eq!(
        settings.effective_manual_fps_limit(FpsLimit::Automatic),
        None
    );
    settings.fps_limit = FpsLimit::Manual {
        frames_per_second: 30,
    };
    settings.ui_scale_percent = 125;
    settings.library_view = LibraryView::Tiles;
    settings.fullscreen = FullscreenMode::LandscapeOnly;
    settings.theme = AppTheme::Oled;
    settings.accent_color = AccentColor::Teal;
    settings.control_layout.fire.offset_x = -1250;
    settings.control_layout.number_keys[3].visible = false;
    settings.landscape_control_layout.direction_pad.size_percent = 130;
    settings.landscape_control_layout.visible = false;
    settings.vibration.strength_percent = 37;
    settings
        .physical_bindings
        .assign(
            crate::physical_input::PhysicalControl::Gamepad {
                button: crate::physical_input::GamepadButton::RightStick,
            },
            crate::physical_input::PhysicalAction::Phone(platform::HostAction::Num5),
        )
        .unwrap();
    assert_eq!(
        settings.effective_manual_fps_limit(FpsLimit::Automatic),
        Some(30)
    );
    assert_eq!(
        settings.effective_manual_fps_limit(FpsLimit::Manual {
            frames_per_second: 50
        }),
        Some(50)
    );
    for percent in [50, 65, 75, 100, 125, 150] {
        settings.ui_scale_percent = percent;
        repository.save_app_settings(&settings).unwrap();
        assert_eq!(
            LibraryRepository::open(root)
                .unwrap()
                .load_app_settings()
                .unwrap(),
            settings
        );
    }
    for percent in [0, 49, 151, u16::MAX] {
        let mut invalid = settings.clone();
        invalid.ui_scale_percent = percent;
        assert!(repository.save_app_settings(&invalid).is_err());
        assert_eq!(repository.load_app_settings().unwrap(), settings);
    }
    for variant in 0..3 {
        let mut invalid = settings.clone();
        match variant {
            0 => invalid.control_layout.fire.offset_x = 10001,
            1 => invalid.landscape_control_layout.number_keys[11].size_percent = 201,
            _ => invalid.vibration.strength_percent = 0,
        }
        assert!(repository.save_app_settings(&invalid).is_err());
        assert_eq!(repository.load_app_settings().unwrap(), settings);
    }
    let path = root.join("frontend-state/app-settings.json");
    for bytes in [
        b"{".to_vec(),
        vec![b' '; usize::try_from(MAX_APP_SETTINGS_BYTES).unwrap() + 1],
        b"{\"schema_version\":99,\"fps_limit\":{\"mode\":\"automatic\"},\"ui_scale_percent\":100}"
            .to_vec(),
    ] {
        fs::write(&path, bytes).unwrap();
        assert!(repository.load_app_settings().is_err());
        assert!(repository.load().unwrap().entries.is_empty());
    }
}

#[test]
fn shell_control_defaults_apply_only_without_saved_settings() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root)
        .unwrap()
        .with_default_virtual_controls_visible(false);
    let settings = repository.clone().load_app_settings().unwrap();
    assert!(!settings.control_layout.visible);
    assert!(!settings.landscape_control_layout.visible);
    let path = root.join("frontend-state/app-settings.json");
    assert!(
        !path.exists(),
        "loading defaults must not persist preferences"
    );

    let mut game = GameSettings::default();
    settings.apply_control_defaults(&mut game);
    assert!(!game.control_layout.visible);
    assert!(!game.landscape_control_layout.visible);
    let mut explicit = GameSettings {
        controls_override: true,
        ..GameSettings::default()
    };
    settings.apply_control_defaults(&mut explicit);
    assert!(explicit.control_layout.visible);
    assert!(explicit.landscape_control_layout.visible);

    for (portrait, landscape) in [(true, true), (false, true), (true, false), (false, false)] {
        let mut saved = settings.clone();
        saved.control_layout.visible = portrait;
        saved.landscape_control_layout.visible = landscape;
        repository.save_app_settings(&saved).unwrap();
        let bytes = fs::read(&path).unwrap();
        let reopened = LibraryRepository::open(root)
            .unwrap()
            .with_default_virtual_controls_visible(false);
        assert_eq!(reopened.load_app_settings().unwrap(), saved);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    // Older settings retain the controls they used before this shell default.
    fs::write(
        &path,
        br#"{"schema_version":1,"fps_limit":{"mode":"automatic"},"ui_scale_percent":125}"#,
    )
    .unwrap();
    let migrated = repository.load_app_settings().unwrap();
    assert!(migrated.control_layout.visible);
    assert!(migrated.landscape_control_layout.visible);
    assert_eq!(migrated.ui_scale_percent, 125);
}

#[test]
fn separate_hat_and_button_settings_preserve_the_latest_dpad_assignment() {
    use crate::physical_input::{
        AxisDirection, GamepadAxis, PhysicalAction, PhysicalBinding, PhysicalBindings,
        PhysicalControl,
    };
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    repository
        .save_app_settings(&AppSettings::default())
        .unwrap();
    let mut previous = AppSettings {
        ui_scale_percent: 125,
        ..AppSettings::default()
    };
    previous.control_layout.fire.size_percent = 130;
    previous.physical_bindings.bindings.push(PhysicalBinding {
        control: PhysicalControl::Axis {
            axis: GamepadAxis::HatY,
            direction: AxisDirection::Negative,
        },
        action: PhysicalAction::Phone(platform::HostAction::Num1),
    });
    fs::write(
        root.join("frontend-state/app-settings.json"),
        serde_json::to_vec(&previous).unwrap(),
    )
    .unwrap();
    let settings = repository.load_app_settings().unwrap();
    assert_eq!(settings.ui_scale_percent, 125);
    assert_eq!(settings.control_layout.fire.size_percent, 130);
    assert_eq!(
        settings.physical_bindings.bindings.len(),
        PhysicalBindings::default().bindings.len()
    );
    let dpad = PhysicalControl::Gamepad {
        button: crate::physical_input::GamepadButton::DpadUp,
    };
    assert_eq!(
        settings.physical_bindings.resolve(dpad),
        Some(PhysicalAction::Phone(platform::HostAction::Num1))
    );
    repository.save_app_settings(&settings).unwrap();
    assert_eq!(repository.load_app_settings().unwrap(), settings);
    let mut game = GameSettings {
        physical_bindings: Some(previous.physical_bindings),
        ..GameSettings::default()
    };
    assert!(game.migrate_to_current().unwrap());
    game.validate(&[]).unwrap();
    assert_eq!(
        game.physical_bindings.as_ref().unwrap().resolve(dpad),
        settings.physical_bindings.resolve(dpad)
    );
    assert!(!game.migrate_to_current().unwrap());
}

#[test]
fn original_defaults_inheritance_and_explicit_game_controls() {
    let mut global = AppSettings::default();
    let original = GameSettings::default();
    let mut game = original.clone();
    global.apply_control_defaults(&mut game);
    assert_eq!(game, original);
    global.control_layout.fire.size_percent = 140;
    global.landscape_control_layout.visible = false;
    global.vibration.enabled = false;
    global.apply_control_defaults(&mut game);
    assert_eq!(game.control_layout.fire.size_percent, 140);
    assert!(!game.landscape_control_layout.visible);
    assert!(!game.vibration.enabled);
    assert!(!game.controls_override);
    global.control_layout.fire.size_percent = 160;
    global.apply_control_defaults(&mut game);
    assert_eq!(game.control_layout.fire.size_percent, 160);
    // Even an explicitly saved original layout must override a custom global layout.
    let mut manual = GameSettings {
        controls_override: true,
        ..original
    };
    let saved = manual.clone();
    global.apply_control_defaults(&mut manual);
    assert_eq!(manual, saved);
    manual.controls_override = false;
    global.apply_control_defaults(&mut manual);
    assert_eq!(manual.control_layout, global.control_layout);
    assert_eq!(
        manual.landscape_control_layout,
        global.landscape_control_layout
    );
    assert_eq!(manual.vibration, global.vibration);
}

#[test]
fn version_one_app_settings_keep_fps_and_scale_with_original_controls() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    repository
        .save_app_settings(&AppSettings::default())
        .unwrap();
    fs::write(root.join("frontend-state/app-settings.json"),
            br#"{"schema_version":1,"fps_limit":{"mode":"manual","frames_per_second":40},"ui_scale_percent":125}"#).unwrap();
    let settings = repository.load_app_settings().unwrap();
    assert_eq!(settings.schema_version, 12);
    assert_eq!(settings.library_view, LibraryView::Automatic);
    assert_eq!(settings.ui_scale_percent, 125);
    assert_eq!(
        settings.effective_manual_fps_limit(FpsLimit::Automatic),
        Some(40)
    );
    assert!(settings.control_layout.is_default());
    assert!(settings.landscape_control_layout.is_default());
    assert_eq!(settings.vibration, VibrationSettings::default());
}

#[test]
fn appearance_migration_retains_existing_preferences_and_rejects_unknown_choices() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let mut settings = AppSettings {
        ui_scale_percent: 125,
        library_view: LibraryView::Tiles,
        fullscreen: FullscreenMode::LandscapeOnly,
        ..AppSettings::default()
    };
    settings.control_layout.fire.size_percent = 140;
    repository.save_app_settings(&settings).unwrap();
    let path = root.join("frontend-state/app-settings.json");
    for version in 1..=5 {
        let mut old = serde_json::to_value(&settings).unwrap();
        old["schema_version"] = version.into();
        old.as_object_mut().unwrap().remove("theme");
        old.as_object_mut().unwrap().remove("accent_color");
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(repository.load_app_settings().unwrap(), settings);
    }
    for field in ["theme", "accent_color"] {
        let mut invalid = serde_json::to_value(&settings).unwrap();
        invalid[field] = "unknown".into();
        fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert_eq!(
            repository.load_app_settings().unwrap_err().code(),
            "app-settings-json"
        );
    }
}

#[test]
fn previous_app_settings_gain_automatic_library_view_without_losing_preferences() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let settings = AppSettings {
        ui_scale_percent: 150,
        ..AppSettings::default()
    };
    repository.save_app_settings(&settings).unwrap();
    for version in 1..=3 {
        let mut old = serde_json::to_value(&settings).unwrap();
        old["schema_version"] = version.into();
        old.as_object_mut().unwrap().remove("library_view");
        fs::write(
            root.join("frontend-state/app-settings.json"),
            serde_json::to_vec(&old).unwrap(),
        )
        .unwrap();
        assert_eq!(repository.load_app_settings().unwrap(), settings);
    }
    for view in [
        LibraryView::List,
        LibraryView::Tiles,
        LibraryView::Automatic,
    ] {
        let selected = AppSettings {
            library_view: view,
            ..settings.clone()
        };
        repository.save_app_settings(&selected).unwrap();
        assert_eq!(repository.load_app_settings().unwrap(), selected);
    }
    let mut invalid = serde_json::to_value(&settings).unwrap();
    invalid["library_view"] = "unknown".into();
    fs::write(
        root.join("frontend-state/app-settings.json"),
        serde_json::to_vec(&invalid).unwrap(),
    )
    .unwrap();
    assert!(repository.load_app_settings().is_err());
}

#[test]
fn fullscreen_migration_preserves_preferences_and_game_off_overrides_app_on() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let mut settings = AppSettings {
        ui_scale_percent: 125,
        library_view: LibraryView::List,
        ..AppSettings::default()
    };
    settings.control_layout.fire.size_percent = 140;
    repository.save_app_settings(&settings).unwrap();
    let path = root.join("frontend-state/app-settings.json");
    for version in 1..=4 {
        let mut old = serde_json::to_value(&settings).unwrap();
        old["schema_version"] = version.into();
        old.as_object_mut().unwrap().remove("fullscreen");
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(repository.load_app_settings().unwrap(), settings);
    }
    for mode in [
        FullscreenMode::Off,
        FullscreenMode::On,
        FullscreenMode::LandscapeOnly,
    ] {
        settings.fullscreen = mode;
        repository.save_app_settings(&settings).unwrap();
        let restored = LibraryRepository::open(root)
            .unwrap()
            .load_app_settings()
            .unwrap();
        assert_eq!(restored, settings);
        assert_eq!(restored.effective_fullscreen(None), mode);
        for game_mode in [
            FullscreenMode::Off,
            FullscreenMode::On,
            FullscreenMode::LandscapeOnly,
        ] {
            assert_eq!(restored.effective_fullscreen(Some(game_mode)), game_mode);
        }
    }
    let mut invalid = serde_json::to_value(&settings).unwrap();
    invalid["fullscreen"] = "unknown".into();
    fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    assert!(repository.load_app_settings().is_err());
}
