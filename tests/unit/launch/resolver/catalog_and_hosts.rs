use super::*;
use crate::host_compatibility::is_compatible_host;

#[test]
fn touch_and_orientation_selects_the_s40_touch_compatibility_default() {
    let decision = resolve(
        &BTreeMap::from([
            ("midlet-touch-support".to_owned(), "True".to_owned()),
            (
                "nokia-midlet-app-orientation".to_owned(),
                "portrait".to_owned(),
            ),
            ("nokia-midlet-on-screen-keypad".to_owned(), "no".to_owned()),
        ]),
        &ArchiveEvidence::default(),
    );
    assert_eq!(decision.target_selection().profile_id(), "nokia-s40-touch");
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert_eq!(
        decision.profile_source(),
        ProfileSelectionSource::CompatibilityFallback
    );
    assert_eq!(decision.screen_source(), ScreenModeSelection::SuiteMetadata);
    assert_eq!(decision.confidence(), SelectionConfidence::Fallback);
}

#[test]
fn target_display_size_has_priority_over_original_size() {
    let decision = resolve(
        &BTreeMap::from([
            (
                "nokia-midlet-original-display-size".to_owned(),
                "128,160".to_owned(),
            ),
            (
                "nokia-midlet-target-display-size".to_owned(),
                "240,320".to_owned(),
            ),
        ]),
        &ArchiveEvidence::new(Some("nokia-s40.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
}

#[test]
fn build_configuration_selects_family_when_dimensions_are_shared() {
    let decision = resolve(
        &BTreeMap::from([(
            "build-configuration".to_owned(),
            "Nokia240x320_S40".to_owned(),
        )]),
        &ArchiveEvidence::default(),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert_eq!(
        decision.profile_source(),
        ProfileSelectionSource::CompatibilityFallback
    );
}

#[test]
fn static_vendor_api_narrows_to_native_se_profiles_then_uses_stable_fallback() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(None, ["com/mascotcapsule/micro3d/v3/Graphics3D".to_owned()]),
    );
    assert_eq!(decision.target_selection().profile_id(), "se-featurephone");
    assert_eq!(
        decision.profile_source(),
        ProfileSelectionSource::CompatibilityFallback
    );
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "static-vendor-api")
    );
}

#[test]
fn static_api_requirements_are_conjunctive_profile_constraints() {
    let catalog = profiles();
    let decision = resolve_device_selection(
        catalog,
        &BTreeMap::new(),
        &ArchiveEvidence::new(
            None,
            [
                "com/nokia/mid/ui/DirectUtils".to_owned(),
                "java/nio/ByteBuffer".to_owned(),
            ],
        ),
        SelectionOverrides::default(),
    )
    .unwrap();

    assert_eq!(decision.target_selection().profile_id(), "se-jp8-keypad");
    assert!(decision.candidates().iter().all(|candidate| {
        let profile = catalog
            .iter()
            .find(|profile| profile.profile_id() == candidate.selection().profile_id())
            .unwrap();
        profile.java().supports_vendor_api("nokia-ui") && profile.java().supports_jsr("239")
    }));
}

#[test]
fn explicit_profile_cannot_bypass_a_static_api_requirement() {
    let error = resolve_device_selection(
        profiles(),
        &BTreeMap::new(),
        &ArchiveEvidence::new(None, ["java/nio/ByteBuffer".to_owned()]),
        SelectionOverrides::default().with_profile_id("nokia-featurephone"),
    )
    .unwrap_err();
    assert_eq!(error.code(), "device-selection-no-match");
}

#[test]
fn archive_name_breaks_profile_and_screen_ties_weakly() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("10149_nokia_5228_s60_360x640.jar".to_owned()), []),
    );
    assert_eq!(decision.target_selection().profile_id(), "nokia-s60-touch");
    assert_eq!(decision.selection().canvas_dimensions(), (360, 640));
    assert_eq!(
        decision.profile_source(),
        ProfileSelectionSource::ArchiveName
    );
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
    assert_eq!(decision.confidence(), SelectionConfidence::Medium);
}

#[test]
fn s60_is_the_compatibility_host_for_s40_distributions_with_a_shared_canvas() {
    for archive_name in [
        "synthetic_240x320_s40_3ed.jar",
        "synthetic_240320s40v3.jar",
        "synthetic_240x320_s40_3ed_eng240x320_s60_3ed_eng.jar",
    ] {
        let decision = resolve(
            &BTreeMap::new(),
            &ArchiveEvidence::new(
                Some(archive_name.to_owned()),
                [
                    "javax/microedition/media/Player".to_owned(),
                    "javax/microedition/m3g/Graphics3D".to_owned(),
                ],
            ),
        );
        assert_eq!(
            decision.target_selection().profile_id(),
            "nokia-featurephone",
            "{archive_name}"
        );
        assert_eq!(
            decision.selection().profile_id(),
            "nokia-s60-keypad",
            "{archive_name}"
        );
        assert_eq!(
            decision.selection().screen_mode_id(),
            Some("240x320"),
            "{archive_name}"
        );
        assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
        assert!(decision.reasons().iter().any(|reason| {
            reason.code() == "compatibility-host"
                && reason.detail() == "nokia-featurephone -> nokia-s60-keypad at 240x320"
        }));
        assert!(decision.candidates().iter().all(|candidate| {
            candidate.selection().profile_id() == "nokia-s60-keypad"
                && candidate.selection().canvas_dimensions() == (240, 320)
        }));
        assert!(decision.candidates().iter().any(|candidate| {
            candidate.target_selection().profile_id() == "nokia-featurephone"
        }));
    }

    let s40_only_canvas = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("synthetic_128160s40v2.jar".to_owned()), []),
    );
    assert_eq!(
        s40_only_canvas.target_selection().profile_id(),
        "nokia-s40-v2-keypad"
    );
    assert_eq!(s40_only_canvas.selection().profile_id(), "nokia-s60-keypad");
    assert_eq!(s40_only_canvas.selection().canvas_dimensions(), (128, 160));
}

#[test]
fn explicit_legacy_profile_stays_available_without_host_promotion() {
    let decision = resolve_device_selection(
        profiles(),
        &BTreeMap::new(),
        &ArchiveEvidence::default(),
        SelectionOverrides::default()
            .with_profile_id("nokia-s40-v2-keypad")
            .with_canvas_dimensions(208, 208),
    )
    .unwrap();
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-s40-v2-keypad"
    );
    assert_eq!(decision.selection(), decision.target_selection());
    assert_eq!(decision.confidence(), SelectionConfidence::Explicit);
    let catalog = profiles();
    let resolved = decision
        .resolve_profile(catalog, ProfileOverrides::default())
        .unwrap();
    assert!(!resolved.uses_automatic_host());
}

#[test]
fn resolved_contract_keeps_persona_visibility_and_host_capacity_separate() {
    let catalog = profiles();
    let decision = resolve_device_selection(
        catalog,
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("Nokia_3510i.jar".to_owned()), []),
        SelectionOverrides::default(),
    )
    .unwrap();
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-s40-v1-keypad"
    );
    assert_eq!(decision.selection().profile_id(), "nokia-s60-keypad");

    let resolved = decision
        .resolve_profile(catalog, ProfileOverrides::default())
        .unwrap();
    assert_eq!(resolved.persona().profile_id(), "nokia-s40-v1-keypad");
    assert_eq!(resolved.runtime_host().profile_id(), "nokia-s60-keypad");
    assert!(resolved.uses_automatic_host());
    assert_eq!(resolved.canvas_dimensions(), (96, 65));
    assert_eq!(
        resolved
            .persona()
            .java()
            .configuration()
            .value()
            .map(String::as_str),
        Some("CLDC-1.0")
    );
    assert_eq!(
        resolved
            .runtime_host()
            .java()
            .configuration()
            .value()
            .map(String::as_str),
        Some("CLDC-1.1")
    );
    assert!(!resolved.persona().java().supports_jsr("184"));
    assert!(resolved.runtime_host().java().supports_jsr("184"));
}

#[test]
fn compatibility_host_matrix_is_explicit_and_covers_every_persona_mode() {
    let catalog = profiles();
    let expected = expected_compatibility_hosts();
    assert_eq!(expected.len(), catalog.len());
    let mut resolved_modes = 0;
    for (target_id, host_id) in expected {
        let target = catalog
            .iter()
            .find(|profile| profile.profile_id() == target_id)
            .unwrap_or_else(|| panic!("missing target persona {target_id}"));
        let host = catalog
            .iter()
            .find(|profile| profile.profile_id() == host_id)
            .unwrap_or_else(|| panic!("missing runtime host {host_id}"));
        assert_eq!(
            target.composition().persona().automatic_host(),
            host_id,
            "{target_id}"
        );
        assert!(is_compatible_host(host, target), "{target_id} -> {host_id}");
        assert!(!target.display().screen_modes().is_empty(), "{target_id}");
        for mode in target.display().screen_modes() {
            let dimensions = mode.fullscreen_canvas();
            let declared = (dimensions.width(), dimensions.height());
            let resolved = target
                .resolve_selected_canvas_with_host(
                    host,
                    ProfileOverrides::default(),
                    declared,
                    ScreenModeSelection::ExplicitOverride,
                )
                .unwrap_or_else(|error| {
                    panic!("{target_id} -> {host_id} mode {}: {error}", mode.id())
                });
            assert_eq!(resolved.persona().profile_id(), target_id);
            assert_eq!(resolved.runtime_host().profile_id(), host_id);
            assert_eq!(resolved.canvas_dimensions(), declared);
            assert!(resolved.uses_automatic_host());
            resolved_modes += 1;

            if dimensions.width() != dimensions.height() {
                let rotated = (dimensions.height(), dimensions.width());
                let resolved = target
                    .resolve_selected_canvas_with_host(
                        host,
                        ProfileOverrides::default(),
                        rotated,
                        ScreenModeSelection::ExplicitOverride,
                    )
                    .unwrap_or_else(|error| {
                        panic!(
                            "{target_id} -> {host_id} rotated mode {}: {error}",
                            mode.id()
                        )
                    });
                assert_eq!(resolved.canvas_dimensions(), rotated);
                resolved_modes += 1;
            }
        }
    }
    assert!(resolved_modes > catalog.len());
}

fn expected_compatibility_hosts() -> [(&'static str, &'static str); 43] {
    [
        ("se-featurephone", "se-featurephone"),
        ("nokia-featurephone", "nokia-s60-keypad"),
        ("nokia-s40-v2-keypad", "nokia-s60-keypad"),
        ("nokia-s40-v1-keypad", "nokia-s60-keypad"),
        ("nokia-s40-touch", "nokia-asha-touch"),
        ("nokia-asha-touch", "nokia-asha-touch"),
        ("nokia-s60-v2-keypad", "nokia-s60-keypad"),
        ("nokia-s60-v1-keypad", "nokia-s60-keypad"),
        ("nokia-s60-keypad", "nokia-s60-keypad"),
        ("nokia-s60-touch", "nokia-s60-touch"),
        ("siemens-featurephone", "siemens-featurephone"),
        ("benq-siemens-featurephone", "benq-siemens-featurephone"),
        ("siemens-sgold-keypad", "siemens-featurephone"),
        ("siemens-midp1-color-keypad", "siemens-featurephone"),
        ("siemens-midp1-basic-keypad", "siemens-featurephone"),
        ("siemens-sxg75-keypad", "siemens-featurephone"),
        ("benq-siemens-ef81-keypad", "benq-siemens-featurephone"),
        ("samsung-featurephone", "samsung-featurephone"),
        ("samsung-touch", "samsung-touch"),
        ("samsung-midp2-keypad", "samsung-featurephone"),
        ("samsung-midp2-cldc10-keypad", "samsung-featurephone"),
        ("samsung-midp1-keypad", "samsung-featurephone"),
        ("samsung-midp20-touch", "samsung-touch"),
        ("samsung-midp21-touch", "samsung-touch"),
        ("samsung-f480-touch", "samsung-touch"),
        ("samsung-midp20-m3g-touch", "samsung-touch"),
        ("samsung-wvga-touch", "samsung-wvga-touch"),
        ("se-jp1-keypad", "se-featurephone"),
        ("se-jp2-keypad", "se-featurephone"),
        ("se-jp3-keypad", "se-featurephone"),
        ("se-jp4-keypad", "se-featurephone"),
        ("se-jp5-keypad", "se-featurephone"),
        ("se-jp6-keypad", "se-featurephone"),
        ("se-jp6-no-bluetooth-keypad", "se-featurephone"),
        ("se-jp6-touch", "se-jp8-touch"),
        ("se-jp7-basic-keypad", "se-featurephone"),
        ("se-jp7-media-keypad", "se-featurephone"),
        ("se-jp7-no-bluetooth-keypad", "se-featurephone"),
        ("se-jp8-keypad", "se-jp8-late-keypad"),
        ("se-jp8-late-keypad", "se-jp8-late-keypad"),
        ("se-jp8-touch", "se-jp8-touch"),
        ("se-entry-keypad", "se-featurephone"),
        ("se-entry-3d-keypad", "se-jp8-late-keypad"),
    ]
}

#[test]
fn builtin_profile_names_select_independent_families() {
    for name in [
        "se-featurephone",
        "se-jp1-keypad",
        "se-jp2-keypad",
        "se-jp3-keypad",
        "se-jp4-keypad",
        "se-jp5-keypad",
        "se-jp6-keypad",
        "se-jp6-no-bluetooth-keypad",
        "se-jp6-touch",
        "se-jp7-basic-keypad",
        "se-jp7-media-keypad",
        "se-jp7-no-bluetooth-keypad",
        "se-jp8-keypad",
        "se-jp8-late-keypad",
        "se-jp8-touch",
        "se-entry-keypad",
        "se-entry-3d-keypad",
        "nokia-featurephone",
        "nokia-s40-v1-keypad",
        "nokia-s40-v2-keypad",
        "nokia-s40-touch",
        "nokia-asha-touch",
        "nokia-s60-v1-keypad",
        "nokia-s60-v2-keypad",
        "nokia-s60-keypad",
        "nokia-s60-touch",
    ] {
        let profile = crate::builtin_device_profile(std::ffi::OsStr::new(name))
            .unwrap()
            .unwrap();
        assert_eq!(profile.profile_id(), name);
    }
    assert!(
        crate::builtin_device_profile(std::ffi::OsStr::new("nokia"))
            .unwrap()
            .is_none()
    );
}
