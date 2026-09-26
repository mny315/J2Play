use super::*;
use crate::archive_name::separator_tokens;

#[test]
fn compact_attached_n90_selects_the_s60v2_double_resolution_mode() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("synthetic_s60v2N90.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-s60-v2-keypad"
    );
    assert_eq!(decision.selection().screen_mode_id(), Some("352x416"));
    assert_eq!(decision.selection().canvas_dimensions(), (352, 416));
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
}

#[test]
fn s60_generation_tokens_keep_176x208_v1_and_v2_distinct() {
    for (archive_name, expected_profile) in [
        ("synthetic_s60v1_176x208.jar", "nokia-s60-v1-keypad"),
        ("synthetic_s60v2_176x208.jar", "nokia-s60-v2-keypad"),
    ] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(decision.target_selection().profile_id(), expected_profile);
        assert_eq!(decision.selection().screen_mode_id(), Some("176x208"));
        assert_eq!(decision.selection().canvas_dimensions(), (176, 208));
    }
}

#[test]
fn unversioned_nokia_176x208_uses_the_midp2_compatibility_default() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("synthetic_nokia_176x208.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-s60-v2-keypad"
    );
    assert_eq!(decision.selection().screen_mode_id(), Some("176x208"));
    assert_eq!(decision.selection().canvas_dimensions(), (176, 208));
}

#[test]
fn s60_v2_and_v3_model_hints_select_non_default_screens() {
    for (archive_name, expected_profile, expected_mode, expected_canvas) in [
        (
            "synthetic_nokia_n90_build.jar",
            "nokia-s60-v2-keypad",
            "352x416",
            (352, 416),
        ),
        (
            "synthetic_nokia_5500_build.jar",
            "nokia-s60-keypad",
            "208x208",
            (208, 208),
        ),
        (
            "synthetic_nokia_n80_build.jar",
            "nokia-s60-keypad",
            "352x416",
            (352, 416),
        ),
    ] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(decision.target_selection().profile_id(), expected_profile);
        assert_eq!(decision.selection().screen_mode_id(), Some(expected_mode));
        assert_eq!(decision.selection().canvas_dimensions(), expected_canvas);
        assert_eq!(
            decision.screen_source(),
            ScreenModeSelection::ArchiveMetadata
        );
    }
}

#[test]
fn exact_landscape_dimensions_in_archive_name_select_transposed_mode() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("game_nokia_series_320x240.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert_eq!(decision.selection().screen_mode_id(), Some("240x320"));
    assert_eq!(decision.selection().canvas_dimensions(), (320, 240));
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
}

#[test]
fn exact_archive_screen_beats_native_api_preference_within_the_static_tier() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(
            Some("generic_128x128_s40_2ed.jar".to_owned()),
            ["com/nokia/mid/sound/Sound".to_owned()],
        ),
    );

    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-s40-v2-keypad"
    );
    assert_eq!(decision.selection().screen_mode_id(), Some("128x128"));
    assert_eq!(decision.selection().canvas_dimensions(), (128, 128));
    assert_eq!(decision.profile_source(), ProfileSelectionSource::StaticApi);
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
    assert_eq!(decision.confidence(), SelectionConfidence::Medium);
}

#[test]
fn exact_archive_screen_beats_a_dimensionless_vendor_namespace_hint() {
    let decision = resolve(
        &BTreeMap::from([("nokia-midlet-category".to_owned(), "Game".to_owned())]),
        &ArchiveEvidence::new(
            Some("generic_128x128_s40_2ed.jar".to_owned()),
            ["com/nokia/mid/sound/Sound".to_owned()],
        ),
    );

    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-s40-v2-keypad"
    );
    assert_eq!(decision.selection().screen_mode_id(), Some("128x128"));
    assert_eq!(decision.selection().canvas_dimensions(), (128, 128));
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
}

#[test]
fn archive_device_model_selects_its_evidence_backed_rotated_screen() {
    for archive_name in [
        "generic_nokia_c3_00_build.jar",
        "generic-NOKIA-C3-00-build.jar",
    ] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(
            decision.target_selection().profile_id(),
            "nokia-featurephone"
        );
        assert_eq!(decision.selection().screen_mode_id(), Some("240x320"));
        assert_eq!(decision.selection().canvas_dimensions(), (320, 240));
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
}

#[test]
fn every_catalogued_model_is_recognized_in_supported_archive_forms() {
    let catalog = profiles();
    validate_catalog(catalog, true).unwrap();
    let aliases = ModelAliasIndex::new(catalog);
    for profile in catalog {
        for hint in profile.device().archive_name_hints() {
            for model in hint.model_names() {
                for archive_name in [
                    format!("catalog_{}.jar", model.replace(' ', "_")),
                    format!("catalog_{}.jar", separator_tokens(model).concat()),
                    format!("catalog{}build.jar", separator_tokens(model).concat()),
                ] {
                    assert!(
                        archive_model_specificity(
                            &NormalizedArchiveName::new(&archive_name),
                            model,
                            profile.device().manufacturer(),
                            &aliases,
                        )
                        .is_some(),
                        "{}: {model} ({archive_name})",
                        profile.profile_id()
                    );
                }
            }
        }
    }
}

#[test]
fn reused_nokia_numbers_prefer_the_more_specific_variant_name() {
    for (archive_name, expected_profile, expected_canvas) in [
        (
            "catalog_Nokia_3600_slide.jar",
            "nokia-featurephone",
            (240, 320),
        ),
        (
            "catalog_Nokia_6260_slide.jar",
            "nokia-featurephone",
            (320, 480),
        ),
        (
            "catalog_Nokia_6600_fold.jar",
            "nokia-featurephone",
            (240, 320),
        ),
        (
            "catalog_Nokia_7610_Supernova.jar",
            "nokia-featurephone",
            (240, 320),
        ),
        (
            "catalog_Nokia_6220_classic.jar",
            "nokia-s60-keypad",
            (240, 320),
        ),
        (
            "catalog_Nokia_6650_fold.jar",
            "nokia-s60-keypad",
            (240, 320),
        ),
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
fn archive_nokia_5800_model_selects_s60_touch_and_native_screen() {
    let portrait = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("generic_NOKIA_5800_build.jar".to_owned()), []),
    );
    assert_eq!(portrait.selection().profile_id(), "nokia-s60-touch");
    assert_eq!(portrait.selection().screen_mode_id(), Some("360x640"));
    assert_eq!(portrait.selection().canvas_dimensions(), (360, 640));
    assert_eq!(
        portrait.profile_source(),
        ProfileSelectionSource::ArchiveName
    );
    assert_eq!(
        portrait.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
    assert!(
        portrait
            .reasons()
            .iter()
            .any(|reason| reason.code() == "archive-device-model")
    );

    let landscape = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("generic_NOKIA_5800_EN_L_build.jar".to_owned()), []),
    );
    assert_eq!(landscape.selection().profile_id(), "nokia-s60-touch");
    assert_eq!(landscape.selection().screen_mode_id(), Some("360x640"));
    assert_eq!(landscape.selection().canvas_dimensions(), (640, 360));
    assert_eq!(
        landscape.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
    assert_eq!(landscape.candidates().len(), 1);
    assert!(
        landscape
            .reasons()
            .iter()
            .any(|reason| reason.code() == "archive-orientation")
    );

    let ambiguous_short_token = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("generic_NOKIA_5800_L_build.jar".to_owned()), []),
    );
    assert_eq!(
        ambiguous_short_token.selection().canvas_dimensions(),
        (360, 640)
    );
}

#[test]
fn title_initials_do_not_become_a_short_archive_orientation_suffix() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(
            Some("synthetic_king_of_L.A._se_24Ox32O_build.jar".to_owned()),
            [],
        ),
    );

    assert_eq!(decision.target_selection().profile_id(), "se-featurephone");
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert!(
        decision
            .reasons()
            .iter()
            .all(|reason| reason.code() != "archive-orientation")
    );
}

#[test]
fn archive_e71_model_selects_landscape_keypad_s60_family() {
    let decision = resolve(
        &BTreeMap::from([("nokia-midlet-on-screen-keypad".to_owned(), "no".to_owned())]),
        &ArchiveEvidence::new(Some("generic_nokia_e71_build.jar".to_owned()), []),
    );
    assert_eq!(decision.target_selection().profile_id(), "nokia-s60-keypad");
    assert_eq!(decision.selection().screen_mode_id(), Some("240x320"));
    assert_eq!(decision.selection().canvas_dimensions(), (320, 240));
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
fn full_e71_names_disambiguate_nokia_and_benq_siemens() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("generic_benq_siemens_e71.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "benq-siemens-featurephone"
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
            .any(|reason| reason.code() == "archive-device-model"
                && reason.detail() == "BenQ-Siemens E71")
    );

    let ambiguous = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("generic_e71.jar".to_owned()), []),
    );
    assert!(
        ambiguous
            .reasons()
            .iter()
            .all(|reason| reason.code() != "archive-device-model")
    );
}

#[test]
fn stronger_portrait_evidence_beats_e71_model_screen_default() {
    for (properties, archive_name, expected_source) in [
        (
            BTreeMap::from([(
                "nokia-midlet-app-orientation".to_owned(),
                "portrait".to_owned(),
            )]),
            "generic_nokia_e71_build.jar",
            ScreenModeSelection::SuiteMetadata,
        ),
        (
            BTreeMap::new(),
            "generic_nokia_e71_240x320.jar",
            ScreenModeSelection::ArchiveMetadata,
        ),
    ] {
        let decision = resolve(
            &properties,
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(decision.target_selection().profile_id(), "nokia-s60-keypad");
        assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
        assert_eq!(decision.screen_source(), expected_source);
    }
}

#[test]
fn explicit_archive_screen_hint_beats_device_model_default() {
    for archive_name in [
        "generic_nokia_c3_00_portrait.jar",
        "generic_nokia_c3_00_240x320.jar",
    ] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(Some(archive_name.to_owned()), []),
        );
        assert_eq!(
            decision.target_selection().profile_id(),
            "nokia-featurephone"
        );
        assert_eq!(decision.selection().screen_mode_id(), Some("240x320"));
        assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    }
}

#[test]
fn descriptor_orientation_beats_conflicting_archive_model_screen() {
    let decision = resolve(
        &BTreeMap::from([(
            "nokia-midlet-app-orientation".to_owned(),
            "portrait".to_owned(),
        )]),
        &ArchiveEvidence::new(Some("generic_nokia_c3_00_build.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert_eq!(decision.screen_source(), ScreenModeSelection::SuiteMetadata);
}

#[test]
fn archive_orientation_uses_transposed_default_instead_of_square_mode() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("generic-landscape.jar".to_owned()), []),
    );
    assert_eq!(decision.target_selection().profile_id(), "se-featurephone");
    assert_eq!(decision.selection().screen_mode_id(), Some("240x320"));
    assert_eq!(decision.selection().canvas_dimensions(), (320, 240));
}
