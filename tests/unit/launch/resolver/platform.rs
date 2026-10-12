use super::*;

fn platform_properties(value: &str) -> BTreeMap<String, String> {
    BTreeMap::from([("Nokia-Platform".to_owned(), value.to_owned())])
}

#[test]
fn descriptor_platform_selects_a_declared_persona_before_archive_hints() {
    for value in [
        "Nokia6233",
        "Nokia623*",
        "Nokia62??",
        "UnknownDevice@ Nokia6233",
    ] {
        let decision = resolve(
            &platform_properties(value),
            &ArchiveEvidence::new(Some("synthetic_Nokia6300_128x160.jar".to_owned()), []),
        );
        assert_eq!(
            decision.target_selection().profile_id(),
            "nokia-6233-keypad"
        );
        assert_eq!(decision.selection().profile_id(), "nokia-s60-keypad");
        assert_eq!(decision.target_selection().canvas_dimensions(), (240, 320));
        let profile = decision
            .resolve_profile(profiles(), ProfileOverrides::default())
            .unwrap();
        assert_eq!(
            profile
                .persona()
                .java()
                .system_properties()
                .value()
                .unwrap()["microedition.platform"],
            "Nokia6233"
        );
        assert_eq!(profile.non_fullscreen_canvas_dimensions(), (240, 250));
        assert!(
            decision
                .reasons()
                .iter()
                .any(|reason| reason.detail() == "Nokia-Platform matches Nokia6233")
        );
    }
}

#[test]
fn broad_platform_declarations_preserve_the_compatibility_default() {
    let decision = resolve(&platform_properties("Nokia*"), &ArchiveEvidence::default());
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
}

#[test]
fn explicit_persona_is_not_replaced_by_descriptor_identity() {
    let decision = resolve_device_selection(
        profiles(),
        &platform_properties("Nokia6233"),
        &ArchiveEvidence::default(),
        SelectionOverrides::default().with_profile_id("nokia-featurephone"),
    )
    .unwrap();
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
}

#[test]
fn unknown_platform_does_not_fabricate_a_guest_identity() {
    let decision = resolve(
        &platform_properties("NokiaUncataloguedDevice*"),
        &ArchiveEvidence::default(),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-featurephone"
    );
    assert!(
        !decision
            .reasons()
            .iter()
            .any(|reason| reason.detail().starts_with("Nokia-Platform matches"))
    );
}

#[test]
fn descriptor_platform_keeps_static_api_requirements() {
    let result = resolve_device_selection(
        profiles(),
        &platform_properties("Nokia6233"),
        &ArchiveEvidence::new(None, ["com/mascotcapsule/micro3d/v3/Graphics3D".to_owned()]),
        SelectionOverrides::default(),
    );
    assert_eq!(result.unwrap_err().code(), "device-selection-no-match");
}

#[test]
fn exact_archive_model_selects_its_own_device_contract() {
    let decision = resolve(
        &BTreeMap::new(),
        &ArchiveEvidence::new(Some("synthetic_Nokia6233.jar".to_owned()), []),
    );
    assert_eq!(
        decision.target_selection().profile_id(),
        "nokia-6233-keypad"
    );
}
