use super::*;

#[test]
fn nok_distribution_abbreviation_is_bounded_to_nokia_contexts() {
    for name in [
        "synthetic_(nok)_240x320.jar",
        "synthetic_240x320nok.jar",
        "synthetic_NOKS60E3X1.jar",
        "synthetic_nokn73.jar",
    ] {
        assert!(
            NormalizedArchiveName::new(name).contains_profile_signal("nokia"),
            "{name}"
        );
    }
    for name in ["synthetic_monokini.jar", "synthetic_pinokio.jar"] {
        assert!(
            !NormalizedArchiveName::new(name).contains_profile_signal("nokia"),
            "{name}"
        );
    }
    let valid_generation = NormalizedArchiveName::new("synthetic_s40v3a_240x320.jar");
    assert!(valid_generation.contains_profile_signal("s40v3"));
    let longer_generation = NormalizedArchiveName::new("synthetic_s40v30_240x320.jar");
    assert!(!longer_generation.contains_profile_signal("s40v3"));
}

#[test]
fn common_nokia_distribution_shorthands_select_a_canvas_without_a_chooser() {
    for (archive_name, target, host, canvas) in [
        (
            "synthetic_240s40_3ed.jar",
            "nokia-featurephone",
            "nokia-s60-keypad",
            (240, 320),
        ),
        (
            "synthetic_208nok_s40_2ed.jar",
            "nokia-s40-v2-keypad",
            "nokia-s60-keypad",
            (208, 208),
        ),
        (
            "synthetic_352s60_3ed.jar",
            "nokia-s60-keypad",
            "nokia-s60-keypad",
            (352, 416),
        ),
        (
            "synthetic_s60_3ed_ml_240.jar",
            "nokia-s60-keypad",
            "nokia-s60-keypad",
            (240, 320),
        ),
        (
            "synthetic_320_240_nok_s40_3ed.jar",
            "nokia-featurephone",
            "nokia-s60-keypad",
            (320, 240),
        ),
        (
            "synthetic-100282_128x160_S40_2ed.jar",
            "nokia-s40-v2-keypad",
            "nokia-s60-keypad",
            (128, 160),
        ),
        (
            "synthetic_NOKS60E3X1_240x320.jar",
            "nokia-s60-keypad",
            "nokia-s60-keypad",
            (240, 320),
        ),
        (
            "synthetic_2403206280s40v3a.jar",
            "nokia-featurephone",
            "nokia-s60-keypad",
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
    }
}

#[test]
fn exact_s40_device_evidence_is_preserved_while_the_runtime_is_promoted() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("synthetic_Nokia6300_240x320_s40v3.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert_eq!(decision.selection().profile_id(), "nokia-s60-keypad");
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "archive-device-model")
    );
}

#[test]
fn compact_dimensions_and_platforms_are_order_and_separator_invariant() {
    for (dimensions, platform, expected_profile, expected_mode, expected_canvas) in [
        (
            "128128",
            "s40v1",
            "nokia-s40-v1-keypad",
            "128x128",
            (128, 128),
        ),
        (
            "128160",
            "s40v2",
            "nokia-s40-v2-keypad",
            "128x160",
            (128, 160),
        ),
        (
            "240320",
            "s40v3a",
            "nokia-featurephone",
            "240x320",
            (240, 320),
        ),
        (
            "320240",
            "s40v3a",
            "nokia-featurephone",
            "240x320",
            (320, 240),
        ),
        (
            "176208",
            "s60v1",
            "nokia-s60-v1-keypad",
            "176x208",
            (176, 208),
        ),
        (
            "176208",
            "s60v2",
            "nokia-s60-v2-keypad",
            "176x208",
            (176, 208),
        ),
        ("240320", "s60v3", "nokia-s60-keypad", "240x320", (240, 320)),
        ("360640", "s60v5", "nokia-s60-touch", "360x640", (360, 640)),
    ] {
        for archive_name in [
            format!("{dimensions}{platform}.1.jar"),
            format!("{platform}_{dimensions}.jar"),
            format!("release{dimensions}-{platform}build.jar"),
            format!("release{platform}.{dimensions}build.jar"),
        ] {
            let decision = resolve(
                &BTreeMap::new(),
                &ArchiveEvidence::new(Some(archive_name.clone()), []),
            );
            assert_eq!(
                decision.target_selection().profile_id(),
                expected_profile,
                "{archive_name}"
            );
            assert_eq!(
                decision.selection().screen_mode_id(),
                Some(expected_mode),
                "{archive_name}"
            );
            assert_eq!(
                decision.selection().canvas_dimensions(),
                expected_canvas,
                "{archive_name}"
            );
            assert_eq!(
                decision.profile_source(),
                ProfileSelectionSource::ArchiveName,
                "{archive_name}"
            );
            assert_eq!(
                decision.screen_source(),
                ScreenModeSelection::ArchiveMetadata,
                "{archive_name}"
            );
            assert_eq!(
                decision.confidence(),
                SelectionConfidence::Medium,
                "{archive_name}"
            );
            assert!(
                decision
                    .reasons()
                    .iter()
                    .any(|reason| reason.code() == "archive-compact-dimensions"),
                "{archive_name}"
            );
            assert_eq!(decision.candidates().len(), 1, "{archive_name}");
        }
    }
}

#[test]
fn compact_dimensions_require_context_and_a_complete_digit_run() {
    for archive_name in ["240320.jar", "12403201_s40v3a.jar"] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(
            decision.screen_source(),
            ScreenModeSelection::ProfileDefault
        );
        assert!(
            decision
                .reasons()
                .iter()
                .all(|reason| reason.code() != "archive-compact-dimensions")
        );
        assert!(decision.candidates().len() > 1);
    }
}

#[test]
fn compact_s60v2_distribution_tag_and_n70_select_176x208() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("synthetic_176208_s60v2N70.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-s60-v2-keypad"
    );
    assert_eq!(decision.selection().screen_mode_id(), Some("176x208"));
    assert_eq!(decision.selection().canvas_dimensions(), (176, 208));
    assert_eq!(
        decision.profile_source(),
        ProfileSelectionSource::ArchiveName
    );
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "archive-profile-token")
    );
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "archive-device-model")
    );
}

#[test]
fn compact_n5200_overrides_a_misleading_pack_generation_and_selects_128x160() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("synthetic_s40v2a-N5200.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert_eq!(decision.selection().screen_mode_id(), Some("128x160"));
    assert_eq!(decision.selection().canvas_dimensions(), (128, 160));
    assert_eq!(
        decision.profile_source(),
        ProfileSelectionSource::ArchiveName
    );
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "archive-device-model")
    );
}

#[test]
fn unique_bare_numeric_nokia_code_selects_the_catalogued_model() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("synthetic_240x320nok_6282_fixture.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert_eq!(decision.selection().screen_mode_id(), Some("240x320"));
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert_eq!(
        decision.profile_source(),
        ProfileSelectionSource::ArchiveName
    );
    assert!(decision.reasons().iter().any(|reason| {
        reason.code() == "archive-device-model" && reason.detail() == "Nokia 6282"
    }));
    assert_eq!(decision.candidates().len(), 1);
}

#[test]
fn bare_numeric_nokia_code_must_be_unique_and_a_complete_digit_run() {
    for archive_name in ["synthetic_6600.jar", "synthetic_16282.jar"] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert!(
            decision
                .reasons()
                .iter()
                .all(|reason| reason.code() != "archive-device-model"),
            "{archive_name}"
        );
        assert!(decision.candidates().len() > 1, "{archive_name}");
    }
}

#[test]
fn compact_model_alias_requires_a_complete_trailing_number() {
    for archive_name in ["synthetic_nok_e7129.jar", "synthetic_SEK8009.jar"] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert!(
            decision
                .reasons()
                .iter()
                .all(|reason| reason.code() != "archive-device-model"),
            "{archive_name}"
        );
    }
}

#[test]
fn compact_nokia_names_select_catalogued_screens() {
    for (archive_name, expected_canvas, expected_model) in [
        (
            "synthetic_Nokia2320Classic_EN.jar",
            (128, 160),
            "Nokia 2320 classic",
        ),
        (
            "synthetic_Nokia_2320Classic_EN.jar",
            (128, 160),
            "Nokia 2320 classic",
        ),
        (
            "synthetic_Nokia2320_Classic_EN.jar",
            (128, 160),
            "Nokia 2320 classic",
        ),
        ("synthetic_Nokia200_EN.jar", (320, 240), "Nokia Asha 200"),
    ] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(
            decision.target_selection().profile_id(),
            "nokia-featurephone"
        );
        assert_eq!(decision.selection().canvas_dimensions(), expected_canvas);
        assert_eq!(
            decision.profile_source(),
            ProfileSelectionSource::ArchiveName
        );
        assert_eq!(
            decision.screen_source(),
            ScreenModeSelection::ArchiveMetadata
        );
        assert!(decision.reasons().iter().any(|reason| {
            reason.code() == "archive-device-model" && reason.detail() == expected_model
        }));
        assert_eq!(decision.candidates().len(), 1);
        assert_eq!(decision.candidates()[0].selection(), decision.selection());
    }
}
