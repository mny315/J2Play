use super::*;

#[test]
fn sony_ericsson_model_aliases_select_the_jp7_profile_and_canvas() {
    for archive_name in ["K800.jar", "SEK800.jar", "jp7K800.jar"] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(decision.target_selection().profile_id(), "se-featurephone");
        assert_eq!(decision.selection().screen_mode_id(), Some("240x320"));
        assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
        assert_eq!(
            decision.profile_source(),
            ProfileSelectionSource::ArchiveName
        );
        assert!(
            decision
                .reasons()
                .iter()
                .any(|reason| reason.code() == "archive-device-model")
        );
    }
}

#[test]
fn siemens_model_aliases_select_distinct_personae_and_canvases() {
    for (archive_name, target, host, canvas) in [
        (
            "S65.jar",
            "siemens-featurephone",
            "siemens-featurephone",
            (132, 176),
        ),
        (
            "SIE-S65.jar",
            "siemens-featurephone",
            "siemens-featurephone",
            (132, 176),
        ),
        (
            "C65.jar",
            "siemens-sgold-keypad",
            "siemens-featurephone",
            (130, 130),
        ),
        (
            "SXG75.jar",
            "siemens-sxg75-keypad",
            "siemens-featurephone",
            (240, 320),
        ),
        (
            "EF81.jar",
            "benq-siemens-ef81-keypad",
            "benq-siemens-featurephone",
            (240, 320),
        ),
        (
            "benq_sie_f81_240x320.jar",
            "benq-siemens-ef81-keypad",
            "benq-siemens-featurephone",
            (240, 320),
        ),
    ] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(
            decision.target_selection().profile_id(),
            target,
            "{archive_name}"
        );
        assert_eq!(decision.selection().profile_id(), host, "{archive_name}");
        assert_eq!(
            decision.selection().canvas_dimensions(),
            canvas,
            "{archive_name}"
        );
        assert!(
            decision
                .reasons()
                .iter()
                .any(|reason| { reason.code() == "archive-device-model" }),
            "{archive_name}"
        );
    }
}

#[test]
fn samsung_model_aliases_select_exact_personae_and_explicit_hosts() {
    for (archive_name, target, host, canvas) in [
        (
            "SGHE700.jar",
            "samsung-midp1-keypad",
            "samsung-featurephone",
            (128, 160),
        ),
        (
            "D900.jar",
            "samsung-midp2-keypad",
            "samsung-featurephone",
            (240, 320),
        ),
        (
            "E380.jar",
            "samsung-midp2-keypad",
            "samsung-featurephone",
            (176, 220),
        ),
        (
            "SGHE810.jar",
            "samsung-midp2-cldc10-keypad",
            "samsung-featurephone",
            (128, 160),
        ),
        (
            "SGHS5320.jar",
            "samsung-featurephone",
            "samsung-featurephone",
            (240, 320),
        ),
        (
            "F480.jar",
            "samsung-f480-touch",
            "samsung-touch",
            (240, 320),
        ),
        (
            "GTC3510.jar",
            "samsung-midp21-touch",
            "samsung-touch",
            (240, 320),
        ),
        ("GTS8530.jar", "samsung-touch", "samsung-touch", (240, 400)),
        (
            "GTS8600.jar",
            "samsung-wvga-touch",
            "samsung-wvga-touch",
            (480, 800),
        ),
    ] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(
            decision.target_selection().profile_id(),
            target,
            "{archive_name}"
        );
        assert_eq!(decision.selection().profile_id(), host, "{archive_name}");
        assert_eq!(
            decision.selection().canvas_dimensions(),
            canvas,
            "{archive_name}"
        );
        assert!(
            decision
                .reasons()
                .iter()
                .any(|reason| { reason.code() == "archive-device-model" }),
            "{archive_name}: {:?}",
            decision.reasons()
        );
    }
}

#[test]
fn samsung_wvga_dimensions_select_the_dedicated_touch_persona() {
    for (properties, archive_name, expected_source) in [
        (
            BTreeMap::from([
                (
                    "Samsung-Target-Display-Size".to_owned(),
                    "480x800".to_owned(),
                ),
                ("MIDlet-Touch-Support".to_owned(), "true".to_owned()),
            ]),
            None,
            ScreenModeSelection::SuiteMetadata,
        ),
        (
            BTreeMap::new(),
            Some("Samsung_touch_480x800.jar".to_owned()),
            ScreenModeSelection::ArchiveMetadata,
        ),
    ] {
        let decision = resolve(&properties, &ArchiveEvidence::new(archive_name, []));
        assert_eq!(
            decision.target_selection().profile_id(),
            "samsung-wvga-touch"
        );
        assert_eq!(decision.selection().profile_id(), "samsung-wvga-touch");
        assert_eq!(decision.selection().canvas_dimensions(), (480, 800));
        assert_eq!(decision.screen_source(), expected_source);
    }
}

#[test]
fn samsung_descriptor_and_static_namespaces_select_the_samsung_family() {
    let descriptor = resolve(
        &BTreeMap::from([
            (
                "Samsung-Target-Display-Size".to_owned(),
                "240x400".to_owned(),
            ),
            ("MIDlet-Touch-Support".to_owned(), "true".to_owned()),
        ]),
        &ArchiveEvidence::default(),
    );
    assert_eq!(descriptor.target_selection().profile_id(), "samsung-touch");
    assert_eq!(descriptor.selection().profile_id(), "samsung-touch");
    assert_eq!(descriptor.selection().canvas_dimensions(), (240, 400));
    assert!(
        descriptor.reasons().iter().any(|reason| {
            reason.code() == "suite-family" && reason.detail().contains("Samsung")
        })
    );

    for class in ["com/samsung/util/AudioClip", "com/samsung/util/LCDLight"] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(None, [class.to_owned()]),
        );
        assert_eq!(
            decision.target_selection().profile_id(),
            "samsung-featurephone",
            "{class}"
        );
        assert_eq!(decision.selection().profile_id(), "samsung-featurephone");
        assert!(decision.reasons().iter().any(|reason| {
            reason.code() == "static-manufacturer-api" && reason.detail() == "samsung"
        }));
    }

    let s8500_audio_clip = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(
            Some("GTS8500_AudioClip.jar".to_owned()),
            ["com/samsung/util/AudioClip".to_owned()],
        ),
    );
    assert_eq!(
        s8500_audio_clip.target_selection().profile_id(),
        "samsung-wvga-touch"
    );
    assert_eq!(
        s8500_audio_clip.selection().profile_id(),
        "samsung-wvga-touch"
    );
    assert_eq!(s8500_audio_clip.selection().canvas_dimensions(), (480, 800));

    let s8500_sensor_probe = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(
            Some("S8500Wave.jar".to_owned()),
            ["javax/microedition/sensor/SensorManager".to_owned()],
        ),
    );
    assert_eq!(
        s8500_sensor_probe.target_selection().profile_id(),
        "samsung-wvga-touch"
    );
    assert_eq!(
        s8500_sensor_probe.selection().profile_id(),
        "samsung-wvga-touch"
    );
    assert_eq!(
        s8500_sensor_probe.selection().canvas_dimensions(),
        (480, 800)
    );
}

#[test]
fn standalone_sgh_distribution_tag_uses_the_midp2_keypad_default() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("synthetic_sgh_128x160.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "samsung-midp2-keypad"
    );
    assert_eq!(decision.selection().profile_id(), "samsung-featurephone");
    assert_eq!(decision.selection().canvas_dimensions(), (128, 160));
    assert!(decision.reasons().iter().any(|reason| {
        reason.code() == "archive-profile-token" && reason.detail().starts_with("1 ")
    }));
}

#[test]
fn ef81_model_hint_survives_static_m3g_and_mmapi_requirements() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(
            Some("synthetic_benq_sie_ef81_240_320.jar".to_owned()),
            [
                "javax/microedition/m3g/Graphics3D".to_owned(),
                "javax/microedition/media/Manager".to_owned(),
            ],
        ),
    );

    assert_eq!(
        decision.target_selection().profile_id(),
        "benq-siemens-ef81-keypad"
    );
    assert_eq!(
        decision.selection().profile_id(),
        "benq-siemens-featurephone"
    );
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "archive-device-model")
    );
}

#[test]
fn standalone_sie_distribution_tag_selects_the_siemens_qvga_persona() {
    for archive_name in ["synthetic_sie_240x320.jar", "synthetic_(SIE)_240x320.jar"] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(
                Some(archive_name.to_owned()),
                [
                    "javax/microedition/media/Manager".to_owned(),
                    "javax/wireless/messaging/Message".to_owned(),
                ],
            ),
        );
        assert_eq!(
            decision.target_selection().profile_id(),
            "siemens-sxg75-keypad",
            "{archive_name}"
        );
        assert_eq!(
            decision.selection().profile_id(),
            "siemens-featurephone",
            "{archive_name}"
        );
        assert_eq!(
            decision.selection().screen_mode_id(),
            Some("240x320"),
            "{archive_name}"
        );
        assert_eq!(
            decision.selection().canvas_dimensions(),
            (240, 320),
            "{archive_name}"
        );
        assert!(decision.reasons().iter().any(|reason| {
            reason.code() == "archive-profile-token" && reason.detail().starts_with("1 ")
        }));
    }

    for archive_name in [
        "synthetic_sienna_240x320.jar",
        "synthetic_dossier_240x320.jar",
    ] {
        assert!(
            !NormalizedArchiveName::new(archive_name).contains_profile_signal("sie"),
            "{archive_name}"
        );
    }
}

#[test]
fn siemens_descriptor_and_static_api_hints_select_the_new_families() {
    let siemens = resolve(
        &BTreeMap::from([("Siemens-Device".to_owned(), "S65".to_owned())]),
        &ArchiveEvidence::new(None, ["com/siemens/mp/color_game/Layer".to_owned()]),
    );
    assert_eq!(
        siemens.target_selection().profile_id(),
        "siemens-featurephone"
    );
    assert!(siemens.reasons().iter().any(|reason| {
        reason.code() == "static-vendor-api" && reason.detail().contains("siemens-color-game")
    }));

    let benq_siemens = resolve(
        &BTreeMap::from([("BenQ-Siemens-Device".to_owned(), "E71".to_owned())]),
        &ArchiveEvidence::default(),
    );
    assert_eq!(
        benq_siemens.target_selection().profile_id(),
        "benq-siemens-featurephone"
    );
}

#[test]
fn siemens_game_mmapi_and_separated_dimensions_select_sgold_canvas() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(
            Some("generic_sie_130_130.jar".to_owned()),
            [
                "com/siemens/mp/game/Light".to_owned(),
                "javax/microedition/media/Manager".to_owned(),
            ],
        ),
    );

    assert_eq!(
        decision.target_selection().profile_id(),
        "siemens-sgold-keypad"
    );
    assert_eq!(decision.selection().profile_id(), "siemens-featurephone");
    assert_eq!(decision.selection().screen_mode_id(), Some("130x130"));
    assert_eq!(decision.selection().canvas_dimensions(), (130, 130));
    assert_eq!(decision.profile_source(), ProfileSelectionSource::StaticApi);
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
}

#[test]
fn sony_ericsson_optional_api_exceptions_select_separate_profiles() {
    for (archive_name, expected_profile, expected_canvas) in [
        ("K310.jar", "se-jp6-no-bluetooth-keypad", (128, 160)),
        ("W350.jar", "se-jp7-media-keypad", (176, 220)),
        ("W380.jar", "se-jp7-no-bluetooth-keypad", (176, 220)),
        ("Z310.jar", "se-jp7-basic-keypad", (128, 160)),
    ] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(decision.target_selection().profile_id(), expected_profile);
        assert_eq!(decision.selection().canvas_dimensions(), expected_canvas);
    }
}

#[test]
fn sony_ericsson_marketing_name_selects_the_jp8_touch_profile() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("SE_Aino_U10i.jar".to_owned()), []),
    );
    assert_eq!(decision.target_selection().profile_id(), "se-jp8-touch");
    assert_eq!(decision.selection().screen_mode_id(), Some("240x432"));
    assert_eq!(decision.selection().canvas_dimensions(), (240, 432));
    let profile = builtin_device_profile(OsStr::new("se-jp8-touch"))
        .unwrap()
        .unwrap();
    assert_eq!(
        profile.input().pointer().unwrap().events().value(),
        Some(&true)
    );
}
