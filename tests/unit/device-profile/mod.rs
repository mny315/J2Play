use super::*;

const PROFILE: &str = include_str!("../../../profiles/sony-ericsson/featurephone.json");
const SE_JP8_TOUCH_PROFILE: &str = include_str!("../../../profiles/sony-ericsson/jp8-touch.json");
const NOKIA_PROFILE: &str = include_str!("../../../profiles/nokia/featurephone.json");
const NOKIA_S40_V1_KEYPAD_PROFILE: &str =
    include_str!("../../../profiles/nokia/s40-v1-keypad.json");
const NOKIA_S40_V2_KEYPAD_PROFILE: &str =
    include_str!("../../../profiles/nokia/s40-v2-keypad.json");
const NOKIA_S40_TOUCH_PROFILE: &str = include_str!("../../../profiles/nokia/s40-touch.json");
const NOKIA_ASHA_TOUCH_PROFILE: &str = include_str!("../../../profiles/nokia/asha-touch.json");
const NOKIA_S60_V1_KEYPAD_PROFILE: &str =
    include_str!("../../../profiles/nokia/s60-v1-keypad.json");
const NOKIA_S60_V2_KEYPAD_PROFILE: &str =
    include_str!("../../../profiles/nokia/s60-v2-keypad.json");
const NOKIA_S60_KEYPAD_PROFILE: &str = include_str!("../../../profiles/nokia/s60-keypad.json");
const NOKIA_TOUCH_PROFILE: &str = include_str!("../../../profiles/nokia/s60-touch.json");

#[test]
fn loads_se_featurephone_profile_through_public_api() {
    let profile = DeviceProfile::from_reader(PROFILE.as_bytes()).expect("valid profile");
    assert_eq!(profile.schema_version(), 2);
    assert_eq!(profile.family_id(), "se-featurephone");
    assert_eq!(profile.profile_id(), "se-featurephone");
    assert_eq!(
        profile.composition().persona().lineage(),
        "sony-ericsson-jp-keypad"
    );
    assert_eq!(
        profile.composition().persona().automatic_host(),
        "se-featurephone"
    );
    assert!(profile.composition().host().automatic());
    assert_eq!(
        profile
            .composition()
            .host()
            .capacity_limits()
            .map(HostCapacityLimits::heap_bytes),
        Some(6 * 1024 * 1024)
    );
    assert_eq!(profile.display().physical_width().value(), Some(&240));
    assert_eq!(profile.input().canvas_keys().len(), 23);
    assert_eq!(
        profile.java().jsrs()["184"].confidence(),
        Confidence::Confirmed
    );
    assert_eq!(
        profile.java().system_properties().value().unwrap()["supports.mixing"],
        "true"
    );
    assert_eq!(
        profile.java().system_properties().value().unwrap()["microedition.platform"],
        "SonyEricsson"
    );
    assert_eq!(
        profile
            .java()
            .bluetooth_properties()
            .unwrap()
            .value()
            .unwrap()["bluetooth.api.version"],
        "1.0"
    );
    assert_eq!(profile.limits().heap_bytes().value(), Some(&6_291_456));
    assert!(
        profile
            .java()
            .supports_vendor_api("mascot-capsule-micro3d-v3")
    );
    assert!(profile.java().supports_vendor_api("nokia-ui"));
    assert!(profile.java().supports_vendor_api("zh-system"));
    assert!(!profile.java().supports_jsr("256"));
    assert!(profile.java().supports_vendor_api("sensor-probe"));
    assert_eq!(profile.canvas_dimensions(), Some((240, 320)));
    assert_eq!(profile.non_fullscreen_canvas_dimensions(), Some((240, 266)));
    assert_eq!(profile.display().default_screen_mode(), Some("240x320"));
    assert_eq!(profile.display().screen_modes().len(), 2);
    assert!(profile.display().lcd_ui_font_heights().is_none());
    assert_eq!(profile.device().archive_name_hints().len(), 2);
    assert_eq!(
        profile.runtime().interpreter_instructions_per_second(),
        None
    );
    assert_eq!(profile.runtime().frames_per_second(), 60);
    assert_eq!(
        profile
            .m3g()
            .unwrap()
            .compatibility()
            .max_texture_dimension(),
        Some(512)
    );
    assert_eq!(
        profile.input().pointer().unwrap().events().value(),
        Some(&false)
    );
}

#[test]
fn resolves_family_screen_and_pointer_overrides_without_mutating_preset() {
    let profile = DeviceProfile::from_reader(PROFILE.as_bytes()).expect("valid profile");
    let resolved = profile
        .resolve(
            ProfileOverrides::default()
                .with_canvas_dimensions(176, 220)
                .with_pointer(true, true),
        )
        .unwrap();
    assert_eq!(resolved.persona().profile_id(), "se-featurephone");
    assert_eq!(resolved.canvas_dimensions(), (176, 220));
    assert_eq!(resolved.non_fullscreen_canvas_dimensions(), (176, 176));
    assert_eq!(resolved.screen_mode_id(), Some("176x220"));
    assert!(resolved.pointer_events());
    assert!(resolved.pointer_motion_events());
    assert_eq!(profile.canvas_dimensions(), Some((240, 320)));

    let touch_profile =
        DeviceProfile::from_reader(SE_JP8_TOUCH_PROFILE.as_bytes()).expect("valid profile");
    let landscape = touch_profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(432, 240))
        .unwrap();
    assert_eq!(landscape.canvas_dimensions(), (432, 240));
    assert_eq!(landscape.non_fullscreen_canvas_dimensions(), (378, 240));
    assert_eq!(landscape.screen_mode_id(), Some("240x432"));

    let error = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(176, 208))
        .unwrap_err();
    assert_eq!(error.code(), "profile-canvas-size");
    assert!(error.message().contains("declared screen mode"));
}

#[test]
fn loads_independent_nokia_family_contract() {
    let profile = DeviceProfile::from_reader(NOKIA_PROFILE.as_bytes()).expect("valid profile");
    assert_eq!(profile.family_id(), "nokia-featurephone");
    assert_eq!(profile.profile_id(), "nokia-featurephone");
    assert_eq!(profile.device().manufacturer(), "Nokia");
    assert_eq!(profile.display().screen_modes().len(), 4);
    assert_eq!(profile.canvas_dimensions(), Some((240, 320)));
    assert_eq!(profile.non_fullscreen_canvas_dimensions(), Some((240, 248)));
    assert_eq!(
        profile
            .display()
            .lcd_ui_font_heights()
            .and_then(Evidence::value)
            .copied(),
        Some(LcdUiFontHeights {
            small: 9,
            medium: 12,
            large: 16,
        })
    );
    assert_eq!(profile.device().archive_name_hints().len(), 6);
    assert_eq!(
        profile
            .device()
            .archive_name_hints()
            .iter()
            .flat_map(ArchiveNameHint::model_names)
            .count(),
        126
    );
    let c3_hint = profile
        .device()
        .archive_name_hints()
        .iter()
        .find(|hint| hint.model_names().any(|model| model == "Nokia C3-00"))
        .unwrap();
    let c3_canvas = c3_hint.fullscreen_canvas();
    assert_eq!((c3_canvas.width(), c3_canvas.height()), (320, 240));
    assert_eq!(profile.limits().heap_bytes().value(), Some(&2_097_152));
    assert!(profile.java().supports_jsr("184"));
    assert!(profile.java().supports_vendor_api("nokia-ui"));
    assert!(profile.java().supports_vendor_api("nokia-sound"));
    assert_eq!(
        profile
            .input()
            .canvas_keys()
            .iter()
            .find(|key| key.key_code() == -11)
            .unwrap()
            .name(),
        KeyName::End
    );
    assert!(
        !profile
            .java()
            .supports_vendor_api("mascot-capsule-micro3d-v3")
    );
    assert_eq!(
        profile
            .evidence_sources()
            .iter()
            .find(|source| source.id() == "nokia-midp-ui-article")
            .unwrap()
            .kind(),
        SourceKind::TechnicalPublication
    );

    let compact = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(128, 160))
        .unwrap();
    assert_eq!(compact.non_fullscreen_canvas_dimensions(), (128, 115));
    assert_eq!(compact.screen_mode_id(), Some("128x160"));
    let landscape = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(160, 128))
        .unwrap();
    assert_eq!(landscape.non_fullscreen_canvas_dimensions(), (115, 128));
    assert_eq!(landscape.screen_mode_id(), Some("128x160"));

    let square = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(208, 208))
        .unwrap();
    assert_eq!(square.screen_mode_id(), Some("208x208"));
}

#[test]
fn loads_series40_generation_and_touch_families() {
    for (source, profile_id, default_canvas, mode_count, model_count, pointer) in [
        (
            NOKIA_S40_V1_KEYPAD_PROFILE,
            "nokia-s40-v1-keypad",
            (128, 128),
            3,
            48,
            false,
        ),
        (
            NOKIA_S40_V2_KEYPAD_PROFILE,
            "nokia-s40-v2-keypad",
            (128, 160),
            4,
            38,
            false,
        ),
        (
            NOKIA_S40_TOUCH_PROFILE,
            "nokia-s40-touch",
            (240, 320),
            2,
            15,
            true,
        ),
        (
            NOKIA_ASHA_TOUCH_PROFILE,
            "nokia-asha-touch",
            (240, 320),
            1,
            5,
            true,
        ),
    ] {
        let profile = DeviceProfile::from_reader(source.as_bytes()).expect("valid profile");
        assert_eq!(profile.profile_id(), profile_id);
        assert_eq!(profile.canvas_dimensions(), Some(default_canvas));
        assert_eq!(profile.display().screen_modes().len(), mode_count);
        assert_eq!(
            profile
                .device()
                .archive_name_hints()
                .iter()
                .flat_map(ArchiveNameHint::model_names)
                .count(),
            model_count
        );
        assert_eq!(
            profile.input().pointer().unwrap().events().value(),
            Some(&pointer)
        );
    }
}

#[test]
fn loads_nokia_s60_touch_with_asymmetric_rotated_canvas_geometry() {
    let profile =
        DeviceProfile::from_reader(NOKIA_TOUCH_PROFILE.as_bytes()).expect("valid profile");
    assert_eq!(profile.family_id(), "nokia-s60-touch");
    assert_eq!(profile.profile_id(), "nokia-s60-touch");
    assert_eq!(profile.canvas_dimensions(), Some((360, 640)));
    assert_eq!(profile.non_fullscreen_canvas_dimensions(), Some((360, 487)));
    assert_eq!(profile.display().screen_modes().len(), 2);
    assert_eq!(
        profile.display().screen_modes()[0]
            .rotated_non_fullscreen_drawable_area()
            .map(|dimensions| (dimensions.width(), dimensions.height())),
        Some((502, 288))
    );
    assert_eq!(profile.limits().heap_bytes().value(), None);
    assert_eq!(
        profile.m3g().unwrap().properties().depth_bits().value(),
        None
    );
    assert!(!profile.java().supports_jsr("256"));

    let portrait = profile.resolve(ProfileOverrides::default()).unwrap();
    assert_eq!(portrait.canvas_dimensions(), (360, 640));
    assert_eq!(portrait.non_fullscreen_canvas_dimensions(), (360, 487));
    assert!(portrait.pointer_events());
    assert!(portrait.pointer_motion_events());

    let landscape = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(640, 360))
        .unwrap();
    assert_eq!(landscape.canvas_dimensions(), (640, 360));
    assert_eq!(landscape.non_fullscreen_canvas_dimensions(), (502, 288));
    assert_eq!(landscape.screen_mode_id(), Some("360x640"));

    let e6 = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(640, 480))
        .unwrap();
    assert_eq!(e6.non_fullscreen_canvas_dimensions(), (640, 480));
    assert_eq!(e6.screen_mode_id(), Some("640x480"));
}

#[test]
fn loads_nokia_s60_v1_keypad_with_176x208_fullscreen_fallback() {
    let profile =
        DeviceProfile::from_reader(NOKIA_S60_V1_KEYPAD_PROFILE.as_bytes()).expect("valid profile");
    assert_eq!(profile.family_id(), "nokia-s60-v1-keypad");
    assert_eq!(profile.canvas_dimensions(), Some((176, 208)));
    assert_eq!(profile.non_fullscreen_canvas_dimensions(), Some((176, 208)));
    assert_eq!(profile.display().screen_modes().len(), 1);
    assert_eq!(
        profile.display().screen_modes()[0].non_fullscreen_drawable_area(),
        None
    );
    assert_eq!(
        profile.java().configuration().value().map(String::as_str),
        Some("CLDC-1.0")
    );
    assert_eq!(
        profile.java().profile().value().map(String::as_str),
        Some("MIDP-1.0")
    );
    assert!(!profile.java().supports_jsr("184"));
    assert!(profile.m3g().is_none());

    let landscape = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(208, 176))
        .unwrap();
    assert_eq!(landscape.non_fullscreen_canvas_dimensions(), (208, 176));
    assert_eq!(landscape.screen_mode_id(), Some("176x208"));
}

#[test]
fn loads_nokia_s60_v2_keypad_screen_family() {
    let profile =
        DeviceProfile::from_reader(NOKIA_S60_V2_KEYPAD_PROFILE.as_bytes()).expect("valid profile");
    assert_eq!(profile.family_id(), "nokia-s60-v2-keypad");
    assert_eq!(profile.canvas_dimensions(), Some((176, 208)));
    assert_eq!(profile.non_fullscreen_canvas_dimensions(), Some((176, 208)));
    assert_eq!(profile.display().screen_modes().len(), 2);
    assert_eq!(profile.device().archive_name_hints().len(), 2);
    assert!(profile.java().supports_jsr("184"));
    assert_eq!(
        profile.m3g().unwrap().version().value().map(String::as_str),
        Some("1.0")
    );

    let double_resolution = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(352, 416))
        .unwrap();
    assert_eq!(double_resolution.canvas_dimensions(), (352, 416));
    assert_eq!(
        double_resolution.non_fullscreen_canvas_dimensions(),
        (352, 416)
    );
    assert_eq!(double_resolution.screen_mode_id(), Some("352x416"));
}

#[test]
fn loads_nokia_s60_v3_keypad_as_a_scalable_screen_family() {
    let profile =
        DeviceProfile::from_reader(NOKIA_S60_KEYPAD_PROFILE.as_bytes()).expect("valid profile");
    assert_eq!(profile.family_id(), "nokia-s60-keypad");
    assert_eq!(profile.profile_id(), "nokia-s60-keypad");
    assert_eq!(profile.canvas_dimensions(), Some((240, 320)));
    assert_eq!(profile.non_fullscreen_canvas_dimensions(), Some((240, 320)));
    assert_eq!(profile.display().screen_modes().len(), 5);
    assert_eq!(profile.device().archive_name_hints().len(), 6);
    let e71_hint = profile
        .device()
        .archive_name_hints()
        .iter()
        .find(|hint| hint.model_names().any(|model| model == "Nokia E71"))
        .unwrap();
    assert!(e71_hint.model_names().any(|model| model == "Nokia E71"));
    let e71_canvas = e71_hint.fullscreen_canvas();
    assert_eq!((e71_canvas.width(), e71_canvas.height()), (320, 240));
    assert_eq!(
        profile.java().profile().value().map(String::as_str),
        Some("MIDP-2.0")
    );
    assert!(profile.java().supports_jsr("184"));
    assert_eq!(profile.limits().heap_bytes().value(), None);
    assert_eq!(
        profile.input().pointer().unwrap().events().value(),
        Some(&false)
    );

    let landscape = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(320, 240))
        .unwrap();
    assert_eq!(landscape.canvas_dimensions(), (320, 240));
    assert_eq!(landscape.non_fullscreen_canvas_dimensions(), (320, 240));
    assert_eq!(landscape.screen_mode_id(), Some("240x320"));

    let compact = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(176, 208))
        .unwrap();
    assert_eq!(compact.non_fullscreen_canvas_dimensions(), (176, 208));
    assert_eq!(compact.screen_mode_id(), Some("176x208"));
}

#[test]
fn suite_metadata_selects_only_one_declared_mode_across_profiles() {
    let nokia = DeviceProfile::from_reader(NOKIA_PROFILE.as_bytes()).unwrap();
    let compact = nokia
        .resolve_with_suite_properties(
            ProfileOverrides::default(),
            ["Nokia128x160_S40", "unrelated build 23"],
        )
        .unwrap();
    assert_eq!(compact.canvas_dimensions(), (128, 160));
    assert_eq!(compact.non_fullscreen_canvas_dimensions(), (128, 115));
    assert_eq!(compact.screen_mode_id(), Some("128x160"));
    assert_eq!(
        compact.screen_mode_selection(),
        ScreenModeSelection::SuiteMetadata
    );

    let se = DeviceProfile::from_reader(PROFILE.as_bytes()).unwrap();
    let compact = se
        .resolve_with_suite_properties(ProfileOverrides::default(), ["target=176 X 220"])
        .unwrap();
    assert_eq!(compact.canvas_dimensions(), (176, 220));
    assert_eq!(compact.non_fullscreen_canvas_dimensions(), (176, 176));
    assert_eq!(
        compact.screen_mode_selection(),
        ScreenModeSelection::SuiteMetadata
    );
}

#[test]
fn explicit_or_ambiguous_screen_metadata_does_not_guess() {
    let profile = DeviceProfile::from_reader(NOKIA_PROFILE.as_bytes()).unwrap();
    let explicit = profile
        .resolve_with_suite_properties(
            ProfileOverrides::default().with_canvas_dimensions(240, 320),
            ["target=128x160"],
        )
        .unwrap();
    assert_eq!(explicit.canvas_dimensions(), (240, 320));
    assert_eq!(
        explicit.screen_mode_selection(),
        ScreenModeSelection::ExplicitOverride
    );

    for values in [
        vec!["supports 128x160 and 240x320"],
        vec!["near miss 128x1600"],
        vec!["no display metadata"],
    ] {
        let fallback = profile
            .resolve_with_suite_properties(ProfileOverrides::default(), values)
            .unwrap();
        assert_eq!(fallback.canvas_dimensions(), (240, 320));
        assert_eq!(
            fallback.screen_mode_selection(),
            ScreenModeSelection::ProfileDefault
        );
    }
}

#[test]
fn schema_v2_profile_without_optional_family_extensions_remains_loadable() {
    let mut value: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    value.as_object_mut().unwrap().remove("family_id");
    value.as_object_mut().unwrap().remove("runtime");
    value["display"]
        .as_object_mut()
        .unwrap()
        .remove("default_screen_mode");
    value["display"]
        .as_object_mut()
        .unwrap()
        .remove("screen_modes");
    value["device"]
        .as_object_mut()
        .unwrap()
        .remove("archive_name_hints");
    value["input"].as_object_mut().unwrap().remove("pointer");
    value["java"]
        .as_object_mut()
        .unwrap()
        .remove("bluetooth_properties");
    value["m3g"]
        .as_object_mut()
        .unwrap()
        .remove("compatibility");
    value["profile_id"] = serde_json::json!("legacy-profile");
    let bytes = serde_json::to_vec(&value).unwrap();
    let profile = DeviceProfile::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(profile.family_id(), "legacy-profile");
    assert_eq!(
        profile.runtime().interpreter_instructions_per_second(),
        None
    );
    assert_eq!(profile.runtime().frames_per_second(), 60);
    assert_eq!(
        profile
            .m3g()
            .unwrap()
            .compatibility()
            .max_texture_dimension(),
        None
    );
    assert!(profile.input().pointer().is_none());
    assert!(profile.java().bluetooth_properties().is_none());
    let base = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(240, 320))
        .unwrap();
    assert_eq!(base.non_fullscreen_canvas_dimensions(), (240, 266));
    let landscape = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(320, 240))
        .unwrap();
    assert_eq!(landscape.non_fullscreen_canvas_dimensions(), (266, 240));
    let resolved = profile
        .resolve(ProfileOverrides::default().with_canvas_dimensions(176, 208))
        .unwrap();
    assert_eq!(resolved.non_fullscreen_canvas_dimensions(), (176, 208));
    assert_eq!(resolved.screen_mode_id(), None);
    assert!(!resolved.pointer_events());
}

#[test]
fn rejects_unknown_fields() {
    let broken = PROFILE.replacen(
        "\"status\": \"research\"",
        "\"status\": \"research\", \"extra\": true",
        1,
    );
    assert_eq!(
        DeviceProfile::from_reader(broken.as_bytes())
            .unwrap_err()
            .code(),
        "profile-json"
    );
}

#[test]
fn rejects_incoherent_automatic_host_policy() {
    let mut missing_capacity: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    missing_capacity["composition"]["host"]
        .as_object_mut()
        .unwrap()
        .remove("capacity_limits");
    let bytes = serde_json::to_vec(&missing_capacity).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("capacity_limits"));

    let mut manual_with_policy: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    manual_with_policy["composition"]["host"]["automatic"] = serde_json::json!(false);
    let bytes = serde_json::to_vec(&manual_with_policy).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("manual-only host"));
}

#[test]
fn rejects_invalid_screen_mode_catalogs() {
    let mut missing_default: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    missing_default["display"]["default_screen_mode"] = serde_json::json!("missing");
    let bytes = serde_json::to_vec(&missing_default).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("default_screen_mode"));

    let mut empty_catalog: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    empty_catalog["display"]["screen_modes"] = serde_json::json!([]);
    let bytes = serde_json::to_vec(&empty_catalog).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("must not be empty"));

    let mut null_catalog: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    null_catalog["display"]["default_screen_mode"] = serde_json::Value::Null;
    null_catalog["display"]["screen_modes"] = serde_json::Value::Null;
    let bytes = serde_json::to_vec(&null_catalog).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-json");

    let mut duplicate_rotation: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    let mut duplicate = duplicate_rotation["display"]["screen_modes"][1].clone();
    duplicate["id"] = serde_json::json!("320x240");
    duplicate["fullscreen_canvas"] = serde_json::json!({ "width": 320, "height": 240 });
    duplicate["non_fullscreen_drawable_area"] = serde_json::json!({ "width": 266, "height": 240 });
    duplicate_rotation["display"]["screen_modes"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    let bytes = serde_json::to_vec(&duplicate_rotation).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("unique including rotation"));
}

#[test]
fn rejects_archive_name_hint_outside_declared_screen_modes() {
    let mut profile: serde_json::Value = serde_json::from_str(NOKIA_PROFILE).unwrap();
    profile["device"]["archive_name_hints"][0]["fullscreen_canvas"] =
        serde_json::json!({ "width": 360, "height": 640 });
    let bytes = serde_json::to_vec(&profile).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("archive-name hint Canvas"));
}

#[test]
fn rejects_archive_name_hint_duplicates_after_normalization() {
    let mut profile: serde_json::Value = serde_json::from_str(NOKIA_S60_KEYPAD_PROFILE).unwrap();
    let mut duplicate = profile["device"]["archive_name_hints"][0].clone();
    duplicate.as_object_mut().unwrap().remove("models");
    duplicate["model"] = serde_json::json!(" Nokia 3250 ");
    profile["device"]["archive_name_hints"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    let bytes = serde_json::to_vec(&profile).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("models must be unique"));
}

#[test]
fn rejects_base_drawable_area_larger_than_fullscreen_canvas() {
    let mut profile: serde_json::Value = serde_json::from_str(NOKIA_S60_KEYPAD_PROFILE).unwrap();
    profile["display"]["non_fullscreen_drawable_area"] = serde_json::json!({
        "value": { "width": 321, "height": 240 },
        "confidence": "inferred",
        "sources": ["nokia-e71-data-sheet"]
    });
    let bytes = serde_json::to_vec(&profile).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("base drawable area"));
}

#[test]
fn rejects_unknown_evidence_with_value() {
    let mut broken: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    broken["limits"]["jar_bytes"]["value"] = serde_json::json!(1);
    let bytes = serde_json::to_vec(&broken).unwrap();
    assert_eq!(
        DeviceProfile::from_reader(bytes.as_slice())
            .unwrap_err()
            .code(),
        "profile-invalid"
    );
}

#[test]
fn rejects_unknown_required_display_and_pointer_facts() {
    let unknown = serde_json::json!({
        "value": null,
        "confidence": "unknown",
        "sources": []
    });

    let mut display: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    display["display"]["physical_width"] = unknown.clone();
    let bytes = serde_json::to_vec(&display).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("physical_width must be known"));

    let mut pointer: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    pointer["input"]["pointer"]["events"] = unknown;
    let bytes = serde_json::to_vec(&pointer).unwrap();
    let error = DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("pointer events must be known"));
}

#[test]
fn rejects_missing_source_reference() {
    let mut broken: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    broken["display"]["physical_width"]["sources"][0] = serde_json::json!("absent-source");
    let bytes = serde_json::to_vec(&broken).unwrap();
    assert_eq!(
        DeviceProfile::from_reader(bytes.as_slice())
            .unwrap_err()
            .code(),
        "profile-invalid"
    );
}

#[test]
fn rejects_schema_pattern_and_minimum_violations() {
    let mut bad_version: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    bad_version["profile_version"] = serde_json::json!("version-one");
    let bytes = serde_json::to_vec(&bad_version).unwrap();
    assert_eq!(
        DeviceProfile::from_reader(bytes.as_slice())
            .unwrap_err()
            .code(),
        "profile-invalid"
    );
    let mut zero_width: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    zero_width["display"]["physical_width"]["value"] = serde_json::json!(0);
    let bytes = serde_json::to_vec(&zero_width).unwrap();
    assert_eq!(
        DeviceProfile::from_reader(bytes.as_slice())
            .unwrap_err()
            .code(),
        "profile-invalid"
    );
}

#[test]
fn rejects_invalid_jsr_entry() {
    for invalid_id in [true, false] {
        let mut profile: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        let jsrs = profile["java"]["jsrs"].as_object_mut().unwrap();
        if invalid_id {
            let entry = jsrs.remove("75").unwrap();
            jsrs.insert("bluetooth".into(), entry);
        } else {
            jsrs["75"]["value"] = serde_json::json!(false);
        }
        let bytes = serde_json::to_vec(&profile).unwrap();
        assert_eq!(
            DeviceProfile::from_reader(bytes.as_slice())
                .unwrap_err()
                .code(),
            "profile-invalid"
        );
    }
}

#[test]
fn non_m3g_profile_does_not_need_another_profiles_capability_values() {
    let mut value: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    value["profile_id"] = serde_json::json!("nokia-test-device");
    value["java"]["jsrs"].as_object_mut().unwrap().remove("184");
    value["m3g"] = serde_json::Value::Null;
    let bytes = serde_json::to_vec(&value).unwrap();
    let profile = DeviceProfile::from_reader(bytes.as_slice()).unwrap();
    assert!(!profile.java().supports_jsr("184"));
    assert!(profile.m3g().is_none());
}

#[test]
fn rejects_bluetooth_properties_without_jsr82() {
    let mut value: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    value["profile_id"] = serde_json::json!("generic-without-bluetooth");
    value["java"]["jsrs"].as_object_mut().unwrap().remove("82");
    let bytes = serde_json::to_vec(&value).unwrap();
    assert_eq!(
        DeviceProfile::from_reader(bytes.as_slice())
            .unwrap_err()
            .code(),
        "profile-invalid"
    );
}

#[test]
fn rejects_m3g_compatibility_dimension_outside_device_and_budget_bounds() {
    let mut below_device: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    below_device["profile_id"] = serde_json::json!("synthetic-m3g-profile");
    below_device["m3g"]["compatibility"]["max_texture_dimension"] = serde_json::json!(128);
    let bytes = serde_json::to_vec(&below_device).unwrap();
    assert_eq!(
        DeviceProfile::from_reader(bytes.as_slice())
            .unwrap_err()
            .code(),
        "profile-invalid"
    );

    let mut over_budget: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    over_budget["profile_id"] = serde_json::json!("synthetic-m3g-profile");
    over_budget["m3g"]["compatibility"]["max_texture_dimension"] = serde_json::json!(2048);
    let bytes = serde_json::to_vec(&over_budget).unwrap();
    assert_eq!(
        DeviceProfile::from_reader(bytes.as_slice())
            .unwrap_err()
            .code(),
        "profile-invalid"
    );
}

#[test]
fn rejects_unknown_runtime_m3g_texture_dimension_without_panicking() {
    let mut value: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    value["profile_id"] = serde_json::json!("synthetic-m3g-profile");
    value["m3g"]["properties"]["max_texture_dimension"] = serde_json::json!({
        "value": null,
        "confidence": "unknown",
        "sources": []
    });
    let bytes = serde_json::to_vec(&value).unwrap();
    assert_eq!(
        DeviceProfile::from_reader(bytes.as_slice())
            .unwrap_err()
            .code(),
        "profile-invalid"
    );
}

#[test]
fn rejects_duplicate_source_reference() {
    let mut broken: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    broken["display"]["physical_width"]["sources"] =
        serde_json::json!(["se-reference-whitepaper-r2a", "se-reference-whitepaper-r2a"]);
    let bytes = serde_json::to_vec(&broken).unwrap();
    assert_eq!(
        DeviceProfile::from_reader(bytes.as_slice())
            .unwrap_err()
            .code(),
        "profile-invalid"
    );
}

#[test]
fn class_capability_mapping_covers_shared_optional_namespaces() {
    use JavaCapability::{Jsr, VendorApi};

    for (class, capability) in [
        ("com/nokia/mid/ui/DirectUtils", VendorApi("nokia-ui")),
        (
            "com/siemens/mp/color_game/Layer",
            VendorApi("siemens-color-game"),
        ),
        ("com/siemens/mp/game/Light", VendorApi("siemens-game")),
        ("com/siemens/mp/io/File", VendorApi("siemens-extension")),
        ("com/samsung/util/AudioClip", VendorApi("samsung-audioclip")),
        ("java/nio/ByteBuffer", Jsr("239")),
        ("javax/microedition/khronos/opengles/GL", Jsr("239")),
        ("javax/microedition/io/file/FileConnection", Jsr("75")),
        ("javax/microedition/media/Player", Jsr("135")),
        (
            "javax/microedition/sensor/SensorManager",
            VendorApi("sensor-probe"),
        ),
        ("javax/microedition/sensor/SensorConnection", Jsr("256")),
    ] {
        assert_eq!(
            java_capability_for_class(class),
            Some(capability),
            "{class}"
        );
    }
    assert_eq!(
        java_capability_for_class("javax/microedition/lcdui/Canvas"),
        None
    );
}

#[test]
fn dimension_parser_accepts_deployed_metadata_separators() {
    assert_eq!(
        dimension_pairs(
            "target=128 X 160; other 240×320, 320_240, E71_240x320, \
             s40v3_176x208, build_2026_240x320, run_100282_128_160, final 360,640"
        ),
        vec![
            (128, 160),
            (240, 320),
            (320, 240),
            (240, 320),
            (176, 208),
            (240, 320),
            (128, 160),
            (360, 640)
        ]
    );
}

#[test]
fn rejects_oversized_document() {
    let oversized = vec![b' '; DeviceProfile::MAX_JSON_BYTES + 1];
    assert_eq!(
        DeviceProfile::from_reader(oversized.as_slice())
            .unwrap_err()
            .code(),
        "profile-too-large"
    );
}
