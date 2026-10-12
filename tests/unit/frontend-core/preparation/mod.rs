use super::*;

const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/java-me/conformance.jar"
));

#[test]
fn import_uses_opened_bytes_and_requires_explicit_midlet_choice() {
    let inspected = inspect_import(ImportSource::new(
        Some("/provider/private/test-fixtures.jar".to_owned()),
        FIXTURE.to_vec(),
        None,
    ))
    .unwrap();
    assert_eq!(inspected.midlets().len(), 8);
    let prepared = inspected.select_midlet(2).unwrap();
    assert_eq!(prepared.midlet().index, 2);
    assert_eq!(prepared.archive_leaf_name(), Some("test-fixtures.jar"));
    assert!(prepared.automatic_profile_summary().canvas_dimensions.0 > 0);
    assert_eq!(
        prepared.automatic_profile_summary().reason,
        "Supports the game's Java ME APIs (JSR-135)."
    );
}

#[test]
fn import_keeps_only_cross_platform_leaf_name_evidence() {
    let inspected = inspect_import(ImportSource::new(
        Some(r"C:\private\provider\test-fixtures.jar".to_owned()),
        FIXTURE.to_vec(),
        None,
    ))
    .unwrap();
    let prepared = inspected.select_midlet(1).unwrap();
    assert_eq!(prepared.archive_leaf_name(), Some("test-fixtures.jar"));
}

#[test]
fn import_combines_manifest_midlets_with_optional_jad_overrides() {
    let original = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None)).unwrap();
    let second = &original.midlets()[1];
    for descriptor in [
        "Custom-Property: retained\n".to_owned(),
        format!(
            "Custom-Property: retained\nMIDlet-{}: Descriptor title, , {}\n",
            second.index, second.class_name
        ),
    ] {
        let inspected = inspect_import(ImportSource::new(
            None,
            FIXTURE.to_vec(),
            Some(descriptor.into_bytes()),
        ))
        .unwrap();
        assert_eq!(inspected.midlets().len(), original.midlets().len());
        assert_eq!(inspected.midlets()[0], original.midlets()[0]);
        let selected = inspected.midlets()[1].clone();
        let prepared = inspected.select_midlet(selected.index).unwrap();
        assert_eq!(prepared.midlet(), &selected);
        assert_eq!(prepared.suite.properties["custom-property"], "retained");
        let plan = prepared.launch_plan(&GameSettings::default()).unwrap();
        assert_eq!(plan.midlet, selected);
    }
}

#[test]
fn manual_profile_is_an_explicit_launch_decision() {
    let inspected = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None)).unwrap();
    let prepared = inspected.select_midlet(1).unwrap();
    let profiles = launch::builtin_device_profiles().unwrap();
    let profile_id = profiles.last().unwrap().profile_id().to_owned();
    let settings = GameSettings {
        device_profile: ProfileChoice::Manual {
            profile_id: profile_id.clone(),
        },
        ..GameSettings::default()
    };
    let plan = prepared.launch_plan(&settings).unwrap();
    assert_eq!(plan.profile_summary().reason, "Selected in Game settings.");
    assert_eq!(plan.decision.target_selection().profile_id(), profile_id);
    let selected = profiles
        .iter()
        .find(|profile| profile.profile_id() == profile_id)
        .unwrap();
    assert_eq!(
        plan.automatic_fps_limit,
        selected.runtime().frames_per_second()
    );
    assert_eq!(
        plan.profile_summary().canvas_dimensions,
        selected.canvas_dimensions().unwrap()
    );
    assert_eq!(
        plan.decision.screen_source(),
        device_profile::ScreenModeSelection::ExplicitOverride
    );
}

#[test]
fn manual_profile_canvas_overrides_weak_archive_orientation() {
    let inspected = inspect_import(ImportSource::new(
        Some("synthetic_nokia_5800_EN_L_build.jar".to_owned()),
        FIXTURE.to_vec(),
        None,
    ))
    .unwrap();
    let prepared = inspected.select_midlet(1).unwrap();
    let settings = GameSettings {
        device_profile: ProfileChoice::Manual {
            profile_id: "se-featurephone".to_owned(),
        },
        ..GameSettings::default()
    };

    let portrait = prepared.launch_plan(&settings).unwrap();
    assert_eq!(portrait.profile_summary().canvas_dimensions, (240, 320));
    assert_eq!(
        portrait.decision.screen_source(),
        device_profile::ScreenModeSelection::ExplicitOverride
    );

    let landscape = prepared
        .launch_plan_with_orientation(&settings, Some(launch::CanvasOrientation::Landscape))
        .unwrap();
    assert_eq!(landscape.profile_summary().canvas_dimensions, (320, 240));
}

#[test]
fn transient_orientation_changes_the_guest_canvas_only_for_that_plan() {
    let inspected = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None)).unwrap();
    let prepared = inspected.select_midlet(1).unwrap();
    let settings = GameSettings::default();

    let landscape = prepared
        .launch_plan_with_orientation(&settings, Some(launch::CanvasOrientation::Landscape))
        .unwrap();
    let landscape_dimensions = landscape.profile_summary().canvas_dimensions;
    assert!(landscape_dimensions.0 >= landscape_dimensions.1);

    let automatic = prepared.launch_plan(&settings).unwrap();
    assert_ne!(
        automatic.profile_summary().canvas_dimensions,
        landscape_dimensions
    );
}
