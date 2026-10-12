use super::*;

#[test]
fn partial_numeric_archive_tokens_do_not_guess_a_screen_mode() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("generic-build-128.jar".to_owned()), []),
    );
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ProfileDefault
    );
}

#[test]
fn optional_touch_support_and_exact_screen_select_s40_touch() {
    let decision = resolve(
        &BTreeMap::from([
            ("midlet-touch-support".to_owned(), "true".to_owned()),
            (
                "nokia-midlet-app-orientation".to_owned(),
                "portrait".to_owned(),
            ),
        ]),
        &ArchiveEvidence::new(Some("sony_240x320.jar".to_owned()), []),
    );
    assert_eq!(decision.target_selection().profile_id(), "nokia-s40-touch");
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
}

#[test]
fn descriptor_profile_hint_beats_conflicting_archive_dimensions() {
    let decision = resolve(
        &BTreeMap::from([("midlet-platform".to_owned(), "Nokia".to_owned())]),
        &ArchiveEvidence::new(Some("sony_240x432.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
}

#[test]
fn static_api_evidence_beats_conflicting_archive_dimensions() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(
            Some("nokia_s60_640x360.jar".to_owned()),
            ["com/mascotcapsule/micro3d/v3/Graphics3D".to_owned()],
        ),
    );
    assert_eq!(decision.target_selection().profile_id(), "se-featurephone");
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
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
fn generic_suite_uses_stable_compatibility_fallback() {
    let catalog = profiles();
    let decision = resolve_device_selection(
        catalog,
        &BTreeMap::new(),
        &ArchiveEvidence::default(),
        SelectionOverrides::default(),
    )
    .unwrap();
    assert_eq!(decision.target_selection().profile_id(), "se-featurephone");
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert_eq!(
        decision.profile_source(),
        ProfileSelectionSource::CompatibilityFallback
    );
    assert_eq!(decision.confidence(), SelectionConfidence::Fallback);
    assert_eq!(decision.candidates().len(), catalog.len());
    assert_eq!(decision.candidates()[0].selection(), decision.selection());
    let target_personas = decision
        .candidates()
        .iter()
        .map(|candidate| candidate.target_selection().profile_id())
        .collect::<HashSet<_>>();
    assert_eq!(target_personas.len(), decision.candidates().len());
    assert!(decision.candidates().iter().all(|candidate| {
        catalog
            .iter()
            .find(|profile| profile.profile_id() == candidate.selection().profile_id())
            .is_some_and(|profile| profile.composition().host().automatic())
    }));
    let resolved = decision
        .resolve_profile(catalog, ProfileOverrides::default())
        .unwrap();
    assert_eq!(resolved.persona().profile_id(), "se-featurephone");
    assert_eq!(resolved.runtime_host().profile_id(), "se-featurephone");
    assert!(resolved.uses_automatic_host());
}

#[test]
fn known_profile_with_unknown_screen_retains_its_declared_modes() {
    let catalog = profiles();
    let decision = resolve_device_selection(
        catalog,
        &BTreeMap::new(),
        &ArchiveEvidence::default(),
        SelectionOverrides::default().with_profile_id("nokia-featurephone"),
    )
    .unwrap();
    assert_eq!(decision.confidence(), SelectionConfidence::Fallback);
    assert!(decision.candidates().len() > 1);
    assert!(
        decision
            .candidates()
            .iter()
            .all(|candidate| candidate.selection().profile_id() == "nokia-featurephone")
    );
    let dimensions = decision
        .candidates()
        .iter()
        .map(|candidate| candidate.selection().canvas_dimensions())
        .collect::<HashSet<_>>();
    assert!(dimensions.contains(&(128, 160)));
    assert!(dimensions.contains(&(320, 240)));
    assert!(dimensions.contains(&(320, 480)));
}

#[test]
fn touch_support_prefers_a_pointer_profile_but_not_a_screen_mode() {
    let decision = resolve(
        &BTreeMap::from([("midlet-touch-support".to_owned(), "yes".to_owned())]),
        &ArchiveEvidence::default(),
    );
    assert_eq!(decision.target_selection().profile_id(), "nokia-s40-touch");
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ProfileDefault
    );
    assert_eq!(decision.confidence(), SelectionConfidence::Fallback);
}

#[test]
fn pointer_override_is_observable_on_an_explicit_non_touch_profile() {
    let catalog = profiles();
    let decision = resolve_device_selection(
        catalog,
        &BTreeMap::from([("midlet-touch-support".to_owned(), "true".to_owned())]),
        &ArchiveEvidence::default(),
        SelectionOverrides::default()
            .with_profile_id("se-featurephone")
            .with_pointer_events(),
    )
    .unwrap();
    assert_eq!(decision.target_selection().profile_id(), "se-featurephone");
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "pointer-override")
    );
}

#[test]
fn touch_support_does_not_override_an_exact_keypad_family_and_screen() {
    let catalog = profiles();
    let decision = resolve_device_selection(
        catalog,
        &BTreeMap::from([
            ("midlet-touch-support".to_owned(), "true".to_owned()),
            ("nokia-midlet-category".to_owned(), "Game".to_owned()),
        ]),
        &ArchiveEvidence::new(Some("generic_128x160_S40_2ed.jar".to_owned()), []),
        SelectionOverrides::default(),
    )
    .unwrap();

    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-s40-v2-keypad"
    );
    assert_eq!(decision.selection().profile_id(), "nokia-s60-keypad");
    assert_eq!(decision.selection().screen_mode_id(), Some("128x160"));
    assert_eq!(decision.selection().canvas_dimensions(), (128, 160));
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ArchiveMetadata
    );
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "suite-touch-support")
    );

    let resolved = decision
        .resolve_profile(catalog, ProfileOverrides::default())
        .unwrap();
    assert!(!resolved.pointer_events());
    assert!(!resolved.pointer_motion_events());
}

#[test]
fn pointer_override_does_not_hide_native_touch_profile_evidence() {
    let catalog = profiles();
    let decision = resolve_device_selection(
        catalog,
        &BTreeMap::from([("midlet-touch-support".to_owned(), "true".to_owned())]),
        &ArchiveEvidence::default(),
        SelectionOverrides::default().with_pointer_events(),
    )
    .unwrap();
    assert_eq!(decision.target_selection().profile_id(), "nokia-s40-touch");
}

#[test]
fn ambiguous_descriptor_dimensions_do_not_get_overridden_by_archive_name() {
    let decision = resolve(
        &BTreeMap::from([(
            "build-configuration".to_owned(),
            "Nokia128x160_or_240x320".to_owned(),
        )]),
        &ArchiveEvidence::new(Some("nokia_128x160.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ProfileDefault
    );
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "suite-dimensions-ambiguous")
    );
}

#[test]
fn conflicting_descriptor_orientations_do_not_defer_to_archive_name() {
    let decision = resolve(
        &BTreeMap::from([
            ("midlet-orientation".to_owned(), "portrait".to_owned()),
            (
                "nokia-midlet-app-orientation".to_owned(),
                "landscape".to_owned(),
            ),
        ]),
        &ArchiveEvidence::new(Some("nokia-landscape.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ProfileDefault
    );
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "suite-orientation-ambiguous")
    );
}

#[test]
fn explicit_profile_and_screen_override_every_weak_hint() {
    let catalog = profiles();
    let decision = resolve_device_selection(
        catalog,
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("nokia_s60_360x640.jar".to_owned()), []),
        SelectionOverrides::default()
            .with_profile_id("se-featurephone")
            .with_canvas_dimensions(176, 220),
    )
    .unwrap();
    assert_eq!(decision.target_selection().profile_id(), "se-featurephone");
    assert_eq!(decision.selection().canvas_dimensions(), (176, 220));
    assert_eq!(decision.confidence(), SelectionConfidence::Explicit);
}

#[test]
fn explicit_orientation_transposes_the_selected_default_mode() {
    let decision = resolve_device_selection(
        profiles(),
        &BTreeMap::new(),
        &ArchiveEvidence::default(),
        SelectionOverrides::default()
            .with_profile_id("se-featurephone")
            .with_orientation(CanvasOrientation::Landscape),
    )
    .unwrap();
    assert_eq!(decision.selection().screen_mode_id(), Some("240x320"));
    assert_eq!(decision.selection().canvas_dimensions(), (320, 240));
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ExplicitOverride
    );
    assert_eq!(decision.confidence(), SelectionConfidence::Explicit);
}

#[test]
fn explicit_orientation_transposes_suite_selected_mode() {
    let decision = resolve_device_selection(
        profiles(),
        &BTreeMap::from([(
            "build-configuration".to_owned(),
            "Nokia128x160_S40".to_owned(),
        )]),
        &ArchiveEvidence::default(),
        SelectionOverrides::default()
            .with_profile_id("nokia-featurephone")
            .with_orientation(CanvasOrientation::Landscape),
    )
    .unwrap();
    assert_eq!(decision.selection().screen_mode_id(), Some("128x160"));
    assert_eq!(decision.selection().canvas_dimensions(), (160, 128));
    assert_eq!(
        decision.screen_source(),
        ScreenModeSelection::ExplicitOverride
    );
}

#[test]
fn descriptor_orientation_transposes_suite_selected_mode() {
    let decision = resolve_device_selection(
        profiles(),
        &BTreeMap::from([
            (
                "nokia-midlet-target-display-size".to_owned(),
                "240,320".to_owned(),
            ),
            (
                "nokia-midlet-app-orientation".to_owned(),
                "landscape".to_owned(),
            ),
        ]),
        &ArchiveEvidence::default(),
        SelectionOverrides::default(),
    )
    .unwrap();
    assert_eq!(decision.selection().screen_mode_id(), Some("240x320"));
    assert_eq!(decision.selection().canvas_dimensions(), (320, 240));
    assert_eq!(decision.screen_source(), ScreenModeSelection::SuiteMetadata);
    assert!(
        decision
            .reasons()
            .iter()
            .any(|reason| reason.code() == "suite-orientation")
    );
}

#[test]
fn square_screen_is_neutral_when_both_size_and_orientation_are_explicit() {
    let decision = resolve_device_selection(
        profiles(),
        &BTreeMap::new(),
        &ArchiveEvidence::default(),
        SelectionOverrides::default()
            .with_profile_id("se-jp3-keypad")
            .with_canvas_dimensions(128, 128)
            .with_orientation(CanvasOrientation::Landscape),
    )
    .unwrap();
    assert_eq!(decision.selection().canvas_dimensions(), (128, 128));
    assert_eq!(decision.confidence(), SelectionConfidence::Explicit);
}

#[test]
fn contradictory_override_is_a_controlled_error() {
    let error = resolve_device_selection(
        profiles(),
        &BTreeMap::new(),
        &ArchiveEvidence::default(),
        SelectionOverrides::default()
            .with_profile_id("nokia-s60-touch")
            .with_canvas_dimensions(128, 160),
    )
    .unwrap_err();
    assert_eq!(error.code(), "device-selection-no-match");
}
