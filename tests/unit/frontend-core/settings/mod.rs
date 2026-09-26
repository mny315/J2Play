use super::*;
mod diagonals;

#[test]
fn stick_defaults_preserve_old_layouts_and_round_trip_per_orientation() {
    let mut original = GameSettings {
        controls_override: true,
        ..GameSettings::default()
    };
    original.control_layout.direction_pad.offset_x = 700;
    original.landscape_control_layout.direction_pad.size_percent = 125;
    let mut old = serde_json::to_value(&original).unwrap();
    old["schema_version"] = 10.into();
    for name in ["control_layout", "landscape_control_layout"] {
        old[name].as_object_mut().unwrap().remove("stick_enabled");
    }
    let mut restored: GameSettings = serde_json::from_value(old).unwrap();
    assert!(restored.migrate_to_current().unwrap());
    assert_eq!(restored, original);
    original.landscape_control_layout.stick_enabled = true;
    let restored: GameSettings =
        serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    restored.validate(&[]).unwrap();
    assert_eq!(restored, original);
    assert!(!restored.control_layout.stick_enabled);
    assert!(!restored.landscape_control_layout.is_default());
}

#[test]
fn existing_settings_default_to_asking_about_resume_and_reject_unknown_choices() {
    let original = GameSettings {
        fps_limit: FpsLimit::Manual {
            frames_per_second: 30,
        },
        ..GameSettings::default()
    };
    let mut encoded = serde_json::to_value(&original).unwrap();
    encoded.as_object_mut().unwrap().remove("resume_behavior");
    let restored: GameSettings = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored, original);
    encoded["resume_behavior"] = "unknown".into();
    assert!(serde_json::from_value::<GameSettings>(encoded).is_err());
}

#[test]
fn defaults_are_explicit_and_round_trip() {
    let settings = GameSettings::default();
    let encoded = serde_json::to_vec(&settings).unwrap();
    let decoded: GameSettings = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, settings);
    assert!(
        String::from_utf8(encoded)
            .unwrap()
            .contains("automatic-fit")
    );
    assert!(settings.control_layout.is_default());
    assert_eq!(settings.vibration, VibrationSettings::default());
}

#[test]
fn previous_game_settings_inherit_fullscreen_and_explicit_modes_round_trip() {
    let mut original = GameSettings {
        controls_override: true,
        portrait_frame_percent: Some(110),
        ..GameSettings::default()
    };
    original.control_layout.fire.size_percent = 140;
    original.landscape_control_layout.visible = false;
    for version in 8..=9 {
        let mut old = serde_json::to_value(&original).unwrap();
        old["schema_version"] = version.into();
        old.as_object_mut().unwrap().remove("fullscreen");
        let mut restored: GameSettings = serde_json::from_value(old).unwrap();
        assert!(restored.migrate_to_current().unwrap());
        assert_eq!(restored, original);
        assert_eq!(restored.fullscreen, None);
        assert!(!restored.migrate_to_current().unwrap());
    }
    for mode in [
        None,
        Some(FullscreenMode::Off),
        Some(FullscreenMode::On),
        Some(FullscreenMode::LandscapeOnly),
    ] {
        original.fullscreen = mode;
        let restored: GameSettings =
            serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        restored.validate(&[]).unwrap();
        assert_eq!(restored, original);
    }
    let mut invalid = serde_json::to_value(&original).unwrap();
    invalid["fullscreen"] = "unknown".into();
    assert!(serde_json::from_value::<GameSettings>(invalid).is_err());
}

#[test]
fn portrait_frame_bounds_and_schema_five_preserve_independent_layouts() {
    let mut previous = GameSettings::default();
    previous.control_layout.fire.offset_x = -700;
    previous.landscape_control_layout.fire.offset_x = 900;
    previous.controls_override = true;
    let mut encoded = serde_json::to_value(&previous).unwrap();
    encoded["schema_version"] = serde_json::json!(5);
    encoded
        .as_object_mut()
        .unwrap()
        .remove("portrait_frame_percent");
    let mut settings: GameSettings = serde_json::from_value(encoded).unwrap();
    assert!(settings.migrate_to_current().unwrap());
    assert_eq!(settings, previous);
    let profiles = launch::builtin_device_profiles().unwrap();
    for percent in [0, 24, 116, 255] {
        settings.portrait_frame_percent = Some(percent);
        assert_eq!(
            settings.validate(profiles).unwrap_err().code(),
            "settings-portrait-frame"
        );
    }
    for percent in [25, 60, 100, 115] {
        settings.portrait_frame_percent = Some(percent);
        settings.validate(profiles).unwrap();
        let decoded: GameSettings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(decoded, settings);
    }
    settings.schema_version = 6;
    settings.portrait_frame_percent = Some(100);
    assert!(settings.migrate_to_current().unwrap());
    assert_eq!(settings.portrait_frame_percent, Some(100));
    assert_eq!(
        settings.landscape_control_layout,
        previous.landscape_control_layout
    );
}

#[test]
fn manual_values_are_bounded() {
    let profiles = launch::builtin_device_profiles().unwrap();
    let mut settings = GameSettings {
        game_scale: GameScale::Manual { percent: 24 },
        ..GameSettings::default()
    };
    assert_eq!(
        settings.validate(profiles).unwrap_err().code(),
        "settings-scale"
    );

    settings.game_scale = GameScale::AutomaticFit;
    settings.fps_limit = FpsLimit::Manual {
        frames_per_second: 1_001,
    };
    assert_eq!(
        settings.validate(profiles).unwrap_err().code(),
        "settings-fps"
    );

    settings.fps_limit = FpsLimit::Automatic;
    settings.control_layout.fire.size_percent = MAX_CONTROL_SIZE_PERCENT + 1;
    assert_eq!(
        settings.validate(profiles).unwrap_err().code(),
        "settings-control-size"
    );

    settings.control_layout = VirtualControlLayout::default();
    settings.control_layout.corner_radius_percent = MAX_CONTROL_CORNER_RADIUS_PERCENT + 1;
    assert_eq!(
        settings.validate(profiles).unwrap_err().code(),
        "settings-control-corner-radius"
    );

    settings.control_layout = VirtualControlLayout::default();
    settings.vibration.strength_percent = 0;
    assert_eq!(
        settings.validate(profiles).unwrap_err().code(),
        "settings-vibration-strength"
    );
}

#[test]
fn schema_one_migrates_to_the_exact_previous_defaults() {
    let encoded = br#"{
            "schema_version": 1,
            "device_profile": {"mode": "automatic"},
            "game_scale": {"mode": "automatic-fit"},
            "fps_limit": {"mode": "automatic"}
        }"#;
    let mut settings: GameSettings = serde_json::from_slice(encoded).unwrap();
    assert!(settings.migrate_to_current().unwrap());
    assert_eq!(settings, GameSettings::default());
    assert!(!settings.migrate_to_current().unwrap());
}

#[test]
fn schema_two_layout_migrates_with_visible_original_shape() {
    let encoded = br#"{
            "schema_version": 2,
            "device_profile": {"mode": "automatic"},
            "game_scale": {"mode": "automatic-fit"},
            "fps_limit": {"mode": "automatic"},
            "control_layout": {
                "direction_pad": {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                "fire": {"offset_x": -1250, "offset_y": 750, "size_percent": 135},
                "left_soft_key": {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                "right_soft_key": {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                "number_keys": [
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100},
                    {"offset_x": 0, "offset_y": 0, "size_percent": 100}
                ]
            },
            "vibration": {"enabled": true, "strength_percent": 100}
        }"#;
    let mut settings: GameSettings = serde_json::from_slice(encoded).unwrap();
    assert!(settings.migrate_to_current().unwrap());
    assert!(settings.control_layout.visible);
    assert_eq!(
        settings.control_layout.corner_radius_percent,
        MAX_CONTROL_CORNER_RADIUS_PERCENT
    );
    assert_eq!(settings.control_layout.fire.offset_x, -1_250);
    settings
        .validate(launch::builtin_device_profiles().unwrap())
        .unwrap();
}

#[test]
fn schema_three_controls_migrate_as_individually_visible() {
    let encoded = br#"{
            "schema_version": 3,
            "device_profile": {"mode": "automatic"},
            "game_scale": {"mode": "automatic-fit"},
            "fps_limit": {"mode": "automatic"},
            "control_layout": {
                "visible": true,
                "corner_radius_percent": 25,
                "fire": {"offset_x": 500, "offset_y": -750, "size_percent": 120}
            },
            "vibration": {"enabled": true, "strength_percent": 60}
        }"#;
    let mut settings: GameSettings = serde_json::from_slice(encoded).unwrap();
    assert!(settings.migrate_to_current().unwrap());
    assert!(settings.control_layout.fire.visible);
    assert!(settings.control_layout.direction_pad.visible);
    assert_eq!(settings.control_layout.fire.offset_x, 500);
    assert_eq!(settings.control_layout.corner_radius_percent, 25);
    settings
        .validate(launch::builtin_device_profiles().unwrap())
        .unwrap();
}

#[test]
fn schema_four_preserves_both_orientations_then_allows_independent_settings() {
    let mut previous = GameSettings::default();
    previous.control_layout.fire.offset_x = -700;
    previous.control_layout.number_keys[3].size_percent = 125;
    previous.control_layout.visible = false;
    let mut encoded = serde_json::to_value(&previous).unwrap();
    encoded["schema_version"] = serde_json::json!(4);
    encoded
        .as_object_mut()
        .unwrap()
        .remove("landscape_control_layout");
    let mut settings: GameSettings = serde_json::from_value(encoded).unwrap();
    assert!(settings.migrate_to_current().unwrap());
    assert_eq!(settings.control_layout, previous.control_layout);
    assert_eq!(settings.landscape_control_layout, previous.control_layout);
    settings.landscape_control_layout.fire.offset_x = 950;
    let decoded: GameSettings =
        serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
    assert_eq!(decoded.control_layout.fire.offset_x, -700);
    assert_eq!(decoded.landscape_control_layout.fire.offset_x, 950);
    let profiles = launch::builtin_device_profiles().unwrap();
    settings.landscape_control_layout.number_keys[0].size_percent = MAX_CONTROL_SIZE_PERCENT + 1;
    assert_eq!(
        settings.validate(profiles).unwrap_err().code(),
        "settings-control-size"
    );
}

#[test]
fn vibration_policy_preserves_defaults_and_scales_or_stops_requests() {
    use natives::VibrationRequest;

    let timed_default = VibrationRequest::Timed {
        duration_millis: 250,
        level: None,
    };
    assert_eq!(
        VibrationSettings::default().apply_to(timed_default),
        timed_default
    );

    let half = VibrationSettings {
        enabled: true,
        strength_percent: 50,
    };
    for (requested, expected) in [(1, 1), (3, 2), (80, 40), (100, 50)] {
        assert_eq!(
            half.apply_to(VibrationRequest::Continuous {
                level: Some(requested)
            }),
            VibrationRequest::Continuous {
                level: Some(expected)
            }
        );
    }
    assert_eq!(
        half.apply_to(timed_default),
        VibrationRequest::Timed {
            duration_millis: 250,
            level: Some(50),
        }
    );

    let disabled = VibrationSettings {
        enabled: false,
        strength_percent: 50,
    };
    assert_eq!(disabled.apply_to(timed_default), VibrationRequest::Stop);
}
#[test]
fn legacy_game_controls_preserve_customizations_and_inherit_when_unchanged() {
    for version in 1..=7 {
        for customization in 0..4 {
            let mut previous = GameSettings {
                schema_version: version,
                ..GameSettings::default()
            };
            match customization {
                1 => previous.control_layout.fire.offset_x = 750,
                2 => previous.control_layout.visible = false,
                3 => previous.vibration.strength_percent = 37,
                _ => {}
            }
            let mut encoded = serde_json::to_value(&previous).unwrap();
            encoded.as_object_mut().unwrap().remove("controls_override");
            let mut restored: GameSettings = serde_json::from_value(encoded).unwrap();
            assert!(restored.migrate_to_current().unwrap());
            assert_eq!(restored.controls_override, customization != 0);
            assert_eq!(restored.control_layout, previous.control_layout);
            assert_eq!(restored.vibration, previous.vibration);
            assert!(!restored.migrate_to_current().unwrap());
        }
    }
}
