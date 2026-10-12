use super::*;
use std::collections::{BTreeMap, HashSet};

#[test]
fn frontend_text_input_uses_a_dedicated_display_dispatch() {
    let call = text_input_call(platform::TextInputEvent {
        tick: 1,
        kind: platform::TextInputKind::Commit,
        code_unit: 'я' as u16,
    });
    assert_eq!(call.name, "__hostTextInput");
    assert_eq!(call.descriptor, "(I)V");
    assert_eq!(call.arguments, vec![vm::Value::Int('я' as i32)]);

    let backspace = text_input_call(platform::TextInputEvent {
        tick: 2,
        kind: platform::TextInputKind::DeleteBackward,
        code_unit: 0,
    });
    assert_eq!(backspace.arguments, vec![vm::Value::Int(8)]);
}

#[test]
fn frontend_key_dispatch_keeps_raw_and_normalized_codes_separate() {
    let call = key_call(&platform::MidpKeyEvent {
        kind: platform::KeyKind::Pressed,
        key_code: -59,
        lcdui_key_code: -1,
        game_action: Some(platform::GAME_UP),
        state: 1 << platform::GAME_UP,
    });
    assert_eq!(call.name, "__hostKeyPressed");
    assert_eq!(call.descriptor, "(III)V");
    assert_eq!(
        call.arguments,
        vec![
            vm::Value::Int(-59),
            vm::Value::Int((1_u32 << platform::GAME_UP).cast_signed()),
            vm::Value::Int(-1),
        ]
    );
}

#[test]
fn micro3d_bootstrap_is_profile_gated() {
    let source = include_bytes!("../../../profiles/sony-ericsson/featurephone.json");
    let se_featurephone = device_profile::DeviceProfile::from_reader(source.as_slice()).unwrap();
    assert!(profile_supports_m3g(&se_featurephone));
    assert!(profile_supports_micro3d(&se_featurephone));

    let nokia_featurephone = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/featurephone.json").as_slice(),
    )
    .unwrap();
    assert!(profile_supports_m3g(&nokia_featurephone));
    assert!(!profile_supports_micro3d(&nokia_featurephone));
    assert!(nokia_featurephone.java().supports_vendor_api("nokia-ui"));
    assert!(nokia_featurephone.java().supports_vendor_api("nokia-sound"));

    let nokia_s60_v1_keypad = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/s60-v1-keypad.json").as_slice(),
    )
    .unwrap();
    assert!(!profile_supports_m3g(&nokia_s60_v1_keypad));
    assert!(!profile_supports_micro3d(&nokia_s60_v1_keypad));

    let nokia_s60_v2_keypad = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/s60-v2-keypad.json").as_slice(),
    )
    .unwrap();
    assert!(profile_supports_m3g(&nokia_s60_v2_keypad));
    assert!(!profile_supports_micro3d(&nokia_s60_v2_keypad));

    let nokia_s60_keypad = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/s60-keypad.json").as_slice(),
    )
    .unwrap();
    assert!(profile_supports_m3g(&nokia_s60_keypad));
    assert!(!profile_supports_micro3d(&nokia_s60_keypad));
    assert_eq!(
        nokia_s60_keypad.input().pointer().unwrap().events().value(),
        Some(&false)
    );

    let nokia_s60_touch = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/s60-touch.json").as_slice(),
    )
    .unwrap();
    assert!(profile_supports_m3g(&nokia_s60_touch));
    assert!(!profile_supports_micro3d(&nokia_s60_touch));
    assert!(!nokia_s60_touch.java().supports_jsr("256"));
    assert!(!nokia_s60_touch.java().supports_vendor_api("sensor-probe"));
}

#[test]
fn automatic_host_does_not_leak_optional_apis_into_the_persona() {
    let catalog = launch::builtin_device_profiles().unwrap();
    let decision = launch::resolve_device_selection(
        catalog,
        &BTreeMap::new(),
        &launch::ArchiveEvidence::new(Some("Nokia_3510i.jar".to_owned()), []),
        launch::SelectionOverrides::default(),
    )
    .unwrap();
    let resolved = decision
        .resolve_profile(catalog, device_profile::ProfileOverrides::default())
        .unwrap();
    assert_eq!(resolved.persona().profile_id(), "nokia-s40-v1-keypad");
    assert_eq!(resolved.runtime_host().profile_id(), "nokia-s60-keypad");
    assert!(resolved.uses_automatic_host());
    assert!(!profile_supports_m3g(resolved.persona()));
    assert!(profile_supports_m3g(resolved.runtime_host()));

    let mut limits = vm::Limits::default();
    apply_profile_limits(&mut limits, &resolved).unwrap();
    assert_eq!(limits.max_heap_bytes, 16 * 1024 * 1024);
    assert_eq!((limits.lcd_width, limits.lcd_height), (96, 65));
    let mut program = vm::Program::new();
    install_rust_bootstrap(&mut program, &limits, &resolved, &HashSet::new(), false).unwrap();
    assert!(program.contains_class("javax/microedition/lcdui/Canvas"));
    assert!(!program.contains_class("javax/microedition/m3g/Graphics3D"));
}

#[test]
fn automatic_self_host_capacity_is_distinct_from_the_exact_device_limit() {
    let profile = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/asha-touch.json").as_slice(),
    )
    .unwrap();

    let exact = profile
        .resolve(device_profile::ProfileOverrides::default())
        .unwrap();
    assert!(!exact.uses_automatic_host());
    let mut exact_limits = vm::Limits::default();
    apply_profile_limits(&mut exact_limits, &exact).unwrap();
    assert_eq!(exact_limits.max_heap_bytes, 3 * 1024 * 1024);

    let automatic = profile
        .resolve_selected_canvas_with_host(
            &profile,
            device_profile::ProfileOverrides::default(),
            (240, 320),
            device_profile::ScreenModeSelection::ProfileDefault,
        )
        .unwrap();
    assert!(automatic.uses_automatic_host());
    let mut automatic_limits = vm::Limits::default();
    apply_profile_limits(&mut automatic_limits, &automatic).unwrap();
    assert_eq!(automatic_limits.max_heap_bytes, 16 * 1024 * 1024);
}

#[test]
fn samsung_touch_host_provides_the_full_bounded_m3g_arena() {
    let profile = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/samsung/touch.json").as_slice(),
    )
    .unwrap();
    let resolved = profile
        .resolve_selected_canvas_with_host(
            &profile,
            device_profile::ProfileOverrides::default(),
            (240, 400),
            device_profile::ScreenModeSelection::ProfileDefault,
        )
        .unwrap();

    let mut limits = vm::Limits::default();
    apply_profile_limits(&mut limits, &resolved).unwrap();

    assert!(resolved.uses_automatic_host());
    assert_eq!(limits.m3g_arena.bytes, 8 * 1024 * 1024);
}

#[test]
fn alternate_profile_drives_runtime_dimensions_heap_and_input() {
    let mut synthetic: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../profiles/sony-ericsson/featurephone.json"
    ))
    .unwrap();
    synthetic["profile_id"] = serde_json::json!("synthetic-test-device");
    synthetic["device"]["manufacturer"] = serde_json::json!("Test Vendor");
    synthetic["device"]["model"] = serde_json::json!("Test Device");
    synthetic["display"]["physical_width"]["value"] = serde_json::json!(176);
    synthetic["display"]["physical_height"]["value"] = serde_json::json!(208);
    synthetic["display"]["fullscreen_canvas_width"]["value"] = serde_json::json!(176);
    synthetic["display"]["fullscreen_canvas_height"]["value"] = serde_json::json!(208);
    synthetic["display"]["non_fullscreen_drawable_area"]["value"] =
        serde_json::json!({ "width": 176, "height": 182 });
    synthetic["display"]
        .as_object_mut()
        .unwrap()
        .remove("default_screen_mode");
    synthetic["display"]
        .as_object_mut()
        .unwrap()
        .remove("screen_modes");
    synthetic["device"]
        .as_object_mut()
        .unwrap()
        .remove("archive_name_hints");
    synthetic["input"]["canvas_keys"][0]["key_code"] = serde_json::json!(-101);
    synthetic["limits"]["heap_bytes"]["value"] = serde_json::json!(2_000_000);
    synthetic["m3g"]["properties"]["max_lights"]["value"] = serde_json::json!(4);
    synthetic["m3g"]["properties"]["max_viewport_width"]["value"] = serde_json::json!(512);
    synthetic["m3g"]["properties"]["max_viewport_height"]["value"] = serde_json::json!(384);
    synthetic["m3g"]["properties"]["max_viewport_dimension"]["value"] = serde_json::json!(512);
    synthetic["m3g"]["properties"]["support_mipmapping"]["value"] = serde_json::json!(false);
    synthetic["m3g"]["properties"]["support_perspective_correction"]["value"] =
        serde_json::json!(false);
    synthetic["m3g"]["properties"]["max_texture_dimension"]["value"] = serde_json::json!(128);
    synthetic["m3g"]["properties"]["max_sprite_crop_dimension"]["value"] = serde_json::json!(256);
    synthetic["m3g"]["properties"]["max_transforms_per_vertex"]["value"] = serde_json::json!(2);
    synthetic["m3g"]["properties"]["num_texture_units"]["value"] = serde_json::json!(1);
    let bytes = serde_json::to_vec(&synthetic).unwrap();
    let profile = device_profile::DeviceProfile::from_reader(bytes.as_slice()).unwrap();
    let resolved = profile
        .resolve(device_profile::ProfileOverrides::default())
        .unwrap();

    let mut limits = vm::Limits::default();
    apply_profile_limits(&mut limits, &resolved).unwrap();
    assert_eq!((limits.lcd_width, limits.lcd_height), (176, 208));
    assert_eq!(
        (limits.lcd_normal_width, limits.lcd_normal_height),
        (176, 182)
    );
    assert_eq!(limits.max_heap_bytes, 2_000_000);
    assert!(!limits.m3g_support_mipmapping);
    assert!(!limits.m3g_support_perspective_correction);
    assert_eq!(limits.m3g_max_lights, 4);
    assert_eq!(limits.m3g_max_viewport_width, 512);
    assert_eq!(limits.m3g_max_viewport_height, 384);
    assert_eq!(limits.m3g_max_viewport_dimension, 512);
    assert_eq!(limits.m3g_render.max_viewport_width, 512);
    assert_eq!(limits.m3g_render.max_viewport_height, 384);
    assert_eq!(limits.m3g_num_texture_units, 1);
    assert_eq!(limits.m3g_max_texture_dimension, 128);
    assert_eq!(limits.m3g_max_sprite_crop_dimension, 256);
    assert_eq!(limits.m3g_max_transforms_per_vertex, 2);
    assert_eq!(limits.m3g_compatibility_max_texture_dimension, 512);

    let mut input = platform::InputState::new(profile_input_map(&profile));
    let event = input
        .apply(platform::InputEvent {
            tick: 0,
            kind: platform::KeyKind::Pressed,
            action: platform::HostAction::Up,
        })
        .unwrap();
    assert_eq!(event.key_code, -101);
    assert_eq!(event.lcdui_key_code, -1);
    assert_eq!(event.game_action, Some(platform::GAME_UP));
}

#[test]
fn every_builtin_profile_preserves_raw_keys_and_normalizes_lcdui_actions() {
    let profiles = launch::builtin_device_profiles().unwrap();
    let actions = [
        platform::HostAction::Up,
        platform::HostAction::Down,
        platform::HostAction::Left,
        platform::HostAction::Right,
        platform::HostAction::Fire,
        platform::HostAction::SoftLeft,
        platform::HostAction::SoftRight,
        platform::HostAction::Clear,
        platform::HostAction::Back,
        platform::HostAction::GameA,
        platform::HostAction::GameB,
        platform::HostAction::Star,
        platform::HostAction::Pound,
        platform::HostAction::Num0,
        platform::HostAction::Num1,
        platform::HostAction::Num2,
        platform::HostAction::Num3,
        platform::HostAction::Num4,
        platform::HostAction::Num5,
        platform::HostAction::Num6,
        platform::HostAction::Num7,
        platform::HostAction::Num8,
        platform::HostAction::Num9,
    ];
    let mut soft_key_pairs = HashSet::new();

    for profile in profiles {
        let mapping = profile_input_map(profile);
        let soft_left = mapping
            .key(platform::HostAction::SoftLeft)
            .unwrap_or_else(|| panic!("{} has no SOFT_LEFT mapping", profile.profile_id()));
        let soft_right = mapping
            .key(platform::HostAction::SoftRight)
            .unwrap_or_else(|| panic!("{} has no SOFT_RIGHT mapping", profile.profile_id()));
        soft_key_pairs.insert((soft_left.key_code, soft_right.key_code));

        for action in actions {
            let Some(device_key) = mapping.key(action) else {
                continue;
            };
            let mut input = platform::InputState::new(mapping.clone());
            let event = input
                .apply(platform::InputEvent {
                    tick: 0,
                    kind: platform::KeyKind::Pressed,
                    action,
                })
                .unwrap();
            assert_eq!(
                event.key_code,
                device_key.key_code,
                "{} changed the raw code for {action:?}",
                profile.profile_id()
            );
            assert_eq!(
                event.lcdui_key_code,
                platform::lcdui_key_code(action).unwrap(),
                "{} failed to normalize {action:?}",
                profile.profile_id()
            );
        }
    }

    assert!(soft_key_pairs.contains(&(-6, -7)));
    assert!(soft_key_pairs.contains(&(-1, -4)));
}

#[test]
fn every_siemens_profile_preserves_both_soft_keys_for_the_full_lifecycle() {
    let profiles = launch::builtin_device_profiles().unwrap();
    let mut audited = 0;

    for profile in profiles
        .iter()
        .filter(|profile| profile.device().manufacturer() == "Siemens")
    {
        audited += 1;
        for (action, raw_key, normalized_key) in [
            (platform::HostAction::SoftLeft, -1, -6),
            (platform::HostAction::SoftRight, -4, -7),
        ] {
            let mut input = platform::InputState::new(profile_input_map(profile));
            for (tick, kind) in [
                (0, platform::KeyKind::Pressed),
                (1, platform::KeyKind::Repeated),
                (2, platform::KeyKind::Released),
            ] {
                let event = input
                    .apply(platform::InputEvent { tick, kind, action })
                    .unwrap_or_else(|| {
                        panic!("{} dropped {action:?} {kind:?}", profile.profile_id())
                    });
                assert_eq!(event.kind, kind, "{} {action:?}", profile.profile_id());
                assert_eq!(
                    event.key_code,
                    raw_key,
                    "{} rewrote raw {action:?}",
                    profile.profile_id()
                );
                assert_eq!(
                    event.lcdui_key_code,
                    normalized_key,
                    "{} failed to normalize {action:?}",
                    profile.profile_id()
                );
                assert_eq!(
                    event.game_action,
                    None,
                    "{} {action:?}",
                    profile.profile_id()
                );
                assert_eq!(event.state, 0, "{} {action:?}", profile.profile_id());
            }

            input
                .apply(platform::InputEvent {
                    tick: 3,
                    kind: platform::KeyKind::Pressed,
                    action,
                })
                .unwrap();
            let focus_releases = input.focus_lost();
            assert_eq!(
                focus_releases.len(),
                1,
                "{} {action:?}",
                profile.profile_id()
            );
            assert_eq!(focus_releases[0].kind, platform::KeyKind::Released);
            assert_eq!(focus_releases[0].key_code, raw_key);
            assert_eq!(focus_releases[0].lcdui_key_code, normalized_key);
            assert_eq!(input.game_state(), 0);
        }
    }

    assert_eq!(audited, 5, "the Siemens profile catalog changed");
}

#[test]
fn benq_siemens_sg2_soft_keys_preserve_raw_codes_and_report_fire() {
    let profiles = launch::builtin_device_profiles().unwrap();
    let profile = profiles
        .iter()
        .find(|profile| profile.profile_id() == "benq-siemens-featurephone")
        .expect("missing BenQ-Siemens SG2 profile");
    let canvas_input = CanvasInputProfile::from_profile(profile);

    for raw_key in [-1, -4] {
        assert_eq!(canvas_input.game_action(raw_key), Some(platform::GAME_FIRE));
    }
    assert_eq!(
        canvas_input.key_code(platform::GAME_FIRE),
        Some(-26),
        "the centre key remains Canvas.getKeyCode(FIRE)'s preferred code"
    );

    for (action, raw_key, normalized_key) in [
        (platform::HostAction::SoftLeft, -1, -6),
        (platform::HostAction::SoftRight, -4, -7),
    ] {
        let mut input = platform::InputState::new(profile_input_map(profile));
        for (tick, kind, expected_state) in [
            (0, platform::KeyKind::Pressed, 1 << platform::GAME_FIRE),
            (1, platform::KeyKind::Repeated, 1 << platform::GAME_FIRE),
            (2, platform::KeyKind::Released, 0),
        ] {
            let event = input
                .apply(platform::InputEvent { tick, kind, action })
                .unwrap_or_else(|| panic!("SG2 dropped {action:?} {kind:?}"));
            assert_eq!(event.key_code, raw_key);
            assert_eq!(event.lcdui_key_code, normalized_key);
            assert_eq!(event.game_action, Some(platform::GAME_FIRE));
            assert_eq!(event.state, expected_state);
        }

        input
            .apply(platform::InputEvent {
                tick: 3,
                kind: platform::KeyKind::Pressed,
                action,
            })
            .unwrap();
        let focus_releases = input.focus_lost();
        assert_eq!(focus_releases.len(), 1);
        assert_eq!(focus_releases[0].kind, platform::KeyKind::Released);
        assert_eq!(focus_releases[0].key_code, raw_key);
        assert_eq!(focus_releases[0].lcdui_key_code, normalized_key);
        assert_eq!(focus_releases[0].game_action, Some(platform::GAME_FIRE));
        assert_eq!(focus_releases[0].state, 0);
        assert_eq!(input.game_state(), 0);
    }
}

#[test]
fn bundled_nokia_full_canvas_can_be_replaced_without_enabling_nokia_ui() {
    let profile = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/siemens/featurephone.json").as_slice(),
    )
    .unwrap();
    assert!(!profile.java().supports_vendor_api("nokia-ui"));
    let resolved = profile
        .resolve(device_profile::ProfileOverrides::default())
        .unwrap();
    let limits = vm::Limits::default();

    let mut ordinary_program = vm::Program::new();
    install_rust_bootstrap(
        &mut ordinary_program,
        &limits,
        &resolved,
        &HashSet::new(),
        false,
    )
    .unwrap();
    assert!(!ordinary_program.contains_class("com/nokia/mid/ui/FullCanvas"));

    let mut adapter_program = vm::Program::new();
    install_rust_bootstrap(
        &mut adapter_program,
        &limits,
        &resolved,
        &HashSet::new(),
        true,
    )
    .unwrap();
    assert!(adapter_program.contains_class("com/nokia/mid/ui/FullCanvas"));
    assert!(!adapter_program.contains_class("com/nokia/mid/ui/DirectGraphics"));
}

#[test]
fn unknown_required_m3g_property_is_rejected_during_profile_loading() {
    let mut synthetic: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../profiles/sony-ericsson/featurephone.json"
    ))
    .unwrap();
    synthetic["profile_id"] = serde_json::json!("unknown-m3g-property-test");
    synthetic["m3g"]["properties"]["max_lights"] = serde_json::json!({
        "value": null,
        "confidence": "unknown",
        "sources": []
    });
    let bytes = serde_json::to_vec(&synthetic).unwrap();
    let error = device_profile::DeviceProfile::from_reader(bytes.as_slice()).unwrap_err();
    assert_eq!(error.code(), "profile-invalid");
    assert!(error.message().contains("max_lights must be known"));
}

#[test]
fn se_featurephone_canvas_modes_come_from_the_profile() {
    let profile = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/sony-ericsson/featurephone.json").as_slice(),
    )
    .unwrap();
    assert_eq!(profile_canvas_dimensions(&profile).unwrap(), (240, 320));
    assert_eq!(
        profile_non_fullscreen_canvas_dimensions(&profile).unwrap(),
        (240, 266)
    );
    assert!(profile_lcd_ui_font_heights(&profile).is_none());
    let compact = profile
        .resolve(device_profile::ProfileOverrides::default().with_canvas_dimensions(176, 220))
        .unwrap();
    assert_eq!(compact.canvas_dimensions(), (176, 220));
    assert_eq!(compact.non_fullscreen_canvas_dimensions(), (176, 176));
    assert_eq!(compact.screen_mode_id(), Some("176x220"));
}

#[test]
fn nokia_featurephone_drives_dimensions_heap_m3g_and_input() {
    let profile = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/featurephone.json").as_slice(),
    )
    .unwrap();
    let font_heights = profile_lcd_ui_font_heights(&profile).unwrap();
    assert_eq!(
        (
            font_heights.small(),
            font_heights.medium(),
            font_heights.large(),
        ),
        (9, 12, 16)
    );
    let compact = profile
        .resolve(device_profile::ProfileOverrides::default().with_canvas_dimensions(128, 160))
        .unwrap();
    assert_eq!(compact.canvas_dimensions(), (128, 160));
    assert_eq!(compact.non_fullscreen_canvas_dimensions(), (128, 115));
    assert_eq!(compact.screen_mode_id(), Some("128x160"));

    let landscape = profile
        .resolve(device_profile::ProfileOverrides::default().with_canvas_dimensions(320, 240))
        .unwrap();
    assert_eq!(landscape.non_fullscreen_canvas_dimensions(), (248, 240));
    assert_eq!(landscape.screen_mode_id(), Some("240x320"));
    assert!(
        profile
            .resolve(device_profile::ProfileOverrides::default().with_canvas_dimensions(128, 128))
            .is_err()
    );

    let mut limits = vm::Limits::default();
    let resolved = profile
        .resolve(device_profile::ProfileOverrides::default())
        .unwrap();
    apply_profile_limits(&mut limits, &resolved).unwrap();
    assert_eq!(limits.max_heap_bytes, 2_097_152);
    assert_eq!(limits.m3g_max_transforms_per_vertex, 4);
    assert_eq!(limits.m3g_compatibility_max_texture_dimension, 256);

    let mut input = platform::InputState::new(profile_input_map(&profile));
    let star = input
        .apply(platform::InputEvent {
            tick: 0,
            kind: platform::KeyKind::Pressed,
            action: platform::HostAction::Star,
        })
        .unwrap();
    assert_eq!(star.key_code, 42);
    assert_eq!(star.game_action, Some(platform::GAME_D));
    assert!(
        profile_input_map(&profile)
            .key(platform::HostAction::GameA)
            .is_none()
    );
}

#[test]
fn nokia_s60_touch_drives_rotation_pointer_and_m3g() {
    let profile = device_profile::DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/s60-touch.json").as_slice(),
    )
    .unwrap();
    let landscape = profile
        .resolve(device_profile::ProfileOverrides::default().with_canvas_dimensions(640, 360))
        .unwrap();
    assert_eq!(landscape.canvas_dimensions(), (640, 360));
    assert_eq!(landscape.non_fullscreen_canvas_dimensions(), (502, 288));
    assert!(landscape.pointer_events());
    assert!(landscape.pointer_motion_events());

    let mut limits = vm::Limits::default();
    apply_profile_limits(&mut limits, &landscape).unwrap();
    assert_eq!(limits.m3g_max_texture_dimension, 1024);
    assert_eq!(limits.m3g_max_viewport_dimension, 1024);
    assert_eq!(limits.m3g_num_texture_units, 2);
}

#[test]
fn canvas_composition_rejects_excessive_displays_before_allocation() {
    for dimensions in [(u32::MAX, u32::MAX), (4_096, 4_096)] {
        let mut output = vec![0xff12_3456];
        let error = compose_unscaled_lcdui_frame(dimensions, (1, 1), &[1], &mut output)
            .expect_err("unchecked display dimensions must not allocate a framebuffer");
        assert_eq!(error.code(), "framebuffer-size");
        assert_eq!(output, [0xff12_3456]);
    }
}

#[test]
fn canvas_composition_reuses_storage_across_changing_shapes_and_odd_margins() {
    let mut output = vec![u32::MAX; 100];
    for display_width in 1_u32..=8 {
        for display_height in 1_u32..=8 {
            for frame_width in 1..=display_width {
                for frame_height in 1..=display_height {
                    let pixels: Vec<_> = (1..=frame_width * frame_height).collect();
                    let region = compose_unscaled_lcdui_frame(
                        (display_width, display_height),
                        (frame_width, frame_height),
                        &pixels,
                        &mut output,
                    )
                    .unwrap();
                    assert_eq!(region.x, (display_width - frame_width) / 2);
                    assert_eq!(region.y, (display_height - frame_height) / 2);
                    assert_eq!((region.width, region.height), (frame_width, frame_height));
                    assert_eq!(output.len(), (display_width * display_height) as usize);
                    for y in 0..display_height {
                        for x in 0..display_width {
                            let in_canvas = x >= region.x
                                && x < region.x + frame_width
                                && y >= region.y
                                && y < region.y + frame_height;
                            let expected = if in_canvas {
                                pixels[((y - region.y) * frame_width + x - region.x) as usize]
                            } else {
                                LCDUI_SYSTEM_CHROME
                            };
                            assert_eq!(output[(y * display_width + x) as usize], expected);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_canvas_composition_preserves_the_previous_frame() {
    for (display, frame, pixels) in [
        ((0, 4), (1, 1), vec![1]),
        ((4, 4), (0, 1), vec![]),
        ((4, 4), (2, 2), vec![1]),
        ((1, 1), (2, 2), vec![1; 4]),
    ] {
        let mut output = vec![7; 4];
        assert!(compose_unscaled_lcdui_frame(display, frame, &pixels, &mut output).is_err());
        assert_eq!(output, [7; 4]);
    }
}

#[test]
fn worker_failure_message_uses_the_cumulative_count() {
    let retained = [vm::ThreadFailure {
        thread_id: 7,
        exception_class: "java/lang/RuntimeException".to_owned(),
        exception_message: Some("fixture detail".to_owned()),
        stack_trace: vec!["Fixture::run()V pc=4".to_owned()],
        managed_heap_limit: false,
    }];
    let message = thread_failure_message(&retained, 3).unwrap();
    assert!(message.contains("RuntimeException: fixture detail"));
    assert!(message.contains("java-stack: Fixture::run()V pc=4"));
    assert!(message.contains("3 worker failures total"));
    assert!(
        thread_failure_message(&[], 2)
            .unwrap()
            .contains("details were not retained")
    );
}

#[test]
fn managed_heap_worker_failure_is_selected_even_when_it_is_not_first() {
    let other = vm::ThreadFailure {
        thread_id: 1,
        exception_class: "java/lang/RuntimeException".into(),
        exception_message: Some("unrelated".into()),
        stack_trace: Vec::new(),
        managed_heap_limit: false,
    };
    let heap = vm::ThreadFailure {
        thread_id: 2,
        exception_class: "java/lang/OutOfMemoryError".into(),
        exception_message: Some("managed heap limit exceeded".into()),
        stack_trace: vec!["Worker::run()V pc=8".into()],
        managed_heap_limit: true,
    };
    for failures in [[other.clone(), heap.clone()], [heap.clone(), other.clone()]] {
        let error = thread_failure_error(&failures, 2).unwrap();
        assert!(vm::is_managed_heap_limit_error(&error));
        assert!(error.message().contains("thread 2"));
        assert!(error.message().contains("Worker::run()V pc=8"));
        assert!(error.message().contains("2 worker failures total"));
        assert!(!error.message().contains("unrelated"));
    }
    let guest_oom = vm::ThreadFailure {
        managed_heap_limit: false,
        ..heap
    };
    let error = thread_failure_error(&[other, guest_oom], 2).unwrap();
    assert!(!vm::is_managed_heap_limit_error(&error));
    assert!(error.message().contains("unrelated"));
    assert!(thread_failure_error(&[], 0).is_none());
}
