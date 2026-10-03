use super::*;

fn button(device: u32, button: GamepadButton, pressed: bool) -> PhysicalInputEvent {
    PhysicalInputEvent::Button {
        device,
        control: PhysicalControl::Gamepad { button },
        pressed,
    }
}

fn apply(
    state: &mut PhysicalInputState,
    mapping: &PhysicalBindings,
    event: PhysicalInputEvent,
    enabled: bool,
) -> Vec<(PhysicalAction, bool)> {
    state
        .update(event, mapping.dead_zone_percent)
        .into_iter()
        .filter_map(|change| state.resolve(change, mapping, enabled))
        .collect()
}

#[test]
fn repeated_buttons_multiple_devices_and_disconnect_preserve_action_ownership() {
    let mut state = PhysicalInputState::default();
    let mapping = PhysicalBindings::default();
    let key = PhysicalAction::Phone(HostAction::Num5);
    assert_eq!(
        apply(
            &mut state,
            &mapping,
            button(1, GamepadButton::South, true),
            true
        ),
        [(key, true)]
    );
    assert!(
        apply(
            &mut state,
            &mapping,
            button(1, GamepadButton::South, true),
            true
        )
        .is_empty()
    );
    assert!(
        apply(
            &mut state,
            &mapping,
            button(2, GamepadButton::South, true),
            true
        )
        .is_empty()
    );
    assert!(
        apply(
            &mut state,
            &mapping,
            PhysicalInputEvent::Disconnected { device: 1 },
            true
        )
        .is_empty()
    );
    assert_eq!(
        apply(
            &mut state,
            &mapping,
            button(2, GamepadButton::South, false),
            true
        ),
        [(key, false)]
    );
    assert!(
        apply(
            &mut state,
            &mapping,
            button(2, GamepadButton::South, false),
            true
        )
        .is_empty()
    );
}

#[test]
fn dpad_remapping_applies_to_axes_and_buttons_without_duplicate_presses() {
    let mut mapping = PhysicalBindings::default();
    let control = PhysicalControl::Axis {
        axis: GamepadAxis::HatY,
        direction: AxisDirection::Negative,
    };
    let action = PhysicalAction::Phone(HostAction::Num1);
    mapping.assign(control, action).unwrap();
    assert_eq!(mapping.resolve(control), Some(action));
    assert_eq!(
        mapping.resolve(PhysicalControl::Gamepad {
            button: GamepadButton::DpadUp
        }),
        Some(action)
    );
    let mut state = PhysicalInputState::default();
    let hat = |value| PhysicalInputEvent::Axis {
        device: 1,
        axis: GamepadAxis::HatY,
        value,
    };
    assert_eq!(
        apply(
            &mut state,
            &mapping,
            button(1, GamepadButton::DpadUp, true),
            true
        ),
        [(action, true)]
    );
    assert!(apply(&mut state, &mapping, hat(-1.0), true).is_empty());
    assert!(
        apply(
            &mut state,
            &mapping,
            button(1, GamepadButton::DpadUp, false),
            true
        )
        .is_empty()
    );
    assert_eq!(
        apply(&mut state, &mapping, hat(0.0), true),
        [(action, false)]
    );
    mapping
        .assign(
            PhysicalControl::Gamepad {
                button: GamepadButton::DpadUp,
            },
            PhysicalAction::Menu,
        )
        .unwrap();
    assert_eq!(mapping.resolve(control), Some(PhysicalAction::Menu));
    assert_eq!(
        mapping
            .bindings
            .iter()
            .filter(|binding| binding.control.binding_control() == control.binding_control())
            .count(),
        1
    );
    mapping.remove(control);
    assert_eq!(mapping.resolve(control), None);
    assert_eq!(
        mapping.resolve(PhysicalControl::Gamepad {
            button: GamepadButton::DpadUp
        }),
        None
    );
    assert_eq!(
        mapping.resolve(PhysicalControl::Gamepad {
            button: GamepadButton::DpadDown
        }),
        Some(PhysicalAction::Phone(HostAction::Down))
    );
}

#[test]
fn axis_hysteresis_diagonals_and_nonfinite_values_are_bounded() {
    let mut state = PhysicalInputState::default();
    let mapping = PhysicalBindings::default();
    let axis = |axis, value| PhysicalInputEvent::Axis {
        device: 1,
        axis,
        value,
    };
    let right = PhysicalAction::Phone(HostAction::Right);
    let up = PhysicalAction::Phone(HostAction::Up);
    assert!(apply(&mut state, &mapping, axis(GamepadAxis::LeftX, 0.1), true).is_empty());
    assert_eq!(
        apply(&mut state, &mapping, axis(GamepadAxis::LeftX, 0.8), true),
        [(right, true)]
    );
    assert_eq!(
        apply(&mut state, &mapping, axis(GamepadAxis::LeftY, -0.8), true),
        [(up, true)]
    );
    assert!(apply(&mut state, &mapping, axis(GamepadAxis::LeftX, 0.3), true).is_empty());
    assert_eq!(
        apply(&mut state, &mapping, axis(GamepadAxis::LeftX, 0.1), true),
        [(right, false)]
    );
    assert_eq!(
        apply(
            &mut state,
            &mapping,
            axis(GamepadAxis::LeftY, f32::NAN),
            true
        ),
        [(up, false)]
    );
    assert!(
        apply(
            &mut state,
            &mapping,
            axis(GamepadAxis::LeftY, f32::INFINITY),
            true
        )
        .is_empty()
    );
}

#[test]
fn pause_capture_and_remap_require_release_before_another_press() {
    let mut state = PhysicalInputState::default();
    let mut mapping = PhysicalBindings::default();
    let down = button(1, GamepadButton::South, true);
    let up = button(1, GamepadButton::South, false);
    assert!(apply(&mut state, &mapping, down, false).is_empty());
    assert!(apply(&mut state, &mapping, down, true).is_empty());
    assert!(apply(&mut state, &mapping, up, true).is_empty());
    assert_eq!(apply(&mut state, &mapping, down, true).len(), 1);
    state.release_actions();
    mapping
        .assign(
            PhysicalControl::Gamepad {
                button: GamepadButton::South,
            },
            PhysicalAction::Phone(HostAction::Fire),
        )
        .unwrap();
    assert!(apply(&mut state, &mapping, down, true).is_empty());
    assert!(apply(&mut state, &mapping, up, true).is_empty());
    assert_eq!(
        apply(&mut state, &mapping, down, true),
        [(PhysicalAction::Phone(HostAction::Fire), true)]
    );
}

#[test]
fn released_action_uses_press_time_mapping_and_trigger_negative_is_invalid() {
    let mut state = PhysicalInputState::default();
    let mut mapping = PhysicalBindings::default();
    let input = PhysicalControl::Axis {
        axis: GamepadAxis::RightTrigger,
        direction: AxisDirection::Positive,
    };
    mapping
        .assign(input, PhysicalAction::Phone(HostAction::Num7))
        .unwrap();
    let trigger = |value| PhysicalInputEvent::Axis {
        device: 4,
        axis: GamepadAxis::RightTrigger,
        value,
    };
    assert_eq!(
        apply(&mut state, &mapping, trigger(1.0), true),
        [(PhysicalAction::Phone(HostAction::Num7), true)]
    );
    mapping.assign(input, PhysicalAction::Menu).unwrap();
    assert_eq!(
        apply(&mut state, &mapping, trigger(0.0), true),
        [(PhysicalAction::Phone(HostAction::Num7), false)]
    );
    assert!(
        mapping
            .assign(
                PhysicalControl::Axis {
                    axis: GamepadAxis::RightTrigger,
                    direction: AxisDirection::Negative
                },
                PhysicalAction::Menu
            )
            .is_err()
    );
}

#[test]
fn mapping_validation_roundtrip_and_failed_assignment_are_atomic() {
    let mut mapping = PhysicalBindings::default();
    mapping.validate().unwrap();
    let saved = mapping.clone();
    assert_eq!(
        serde_json::from_slice::<PhysicalBindings>(&serde_json::to_vec(&mapping).unwrap()).unwrap(),
        mapping
    );
    assert!(
        mapping
            .assign(
                PhysicalControl::AndroidKey { code: 26 },
                PhysicalAction::Menu
            )
            .is_err()
    );
    assert_eq!(mapping, saved);
    mapping.bindings.push(mapping.bindings[0]);
    assert!(mapping.validate().is_err());
    mapping = saved;
    mapping.dead_zone_percent = 0;
    assert!(mapping.validate().is_err());
    mapping.dead_zone_percent = 35;
    mapping.bindings = vec![mapping.bindings[0]; MAX_PHYSICAL_BINDINGS + 1];
    assert!(mapping.validate().is_err());
}

#[test]
fn full_keyboard_and_controller_bindings_fit_saved_settings_with_an_enforced_limit() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = crate::LibraryRepository::open(&scratch.0).unwrap();
    let mut settings = crate::AppSettings::default();
    let action = PhysicalAction::Phone(HostAction::SoftRight);
    for usage in 4..=231 {
        settings
            .physical_bindings
            .assign(PhysicalControl::Keyboard { usage }, action)
            .unwrap();
    }
    for number in 1..=16 {
        if settings.physical_bindings.bindings.len() == MAX_PHYSICAL_BINDINGS {
            break;
        }
        settings
            .physical_bindings
            .assign(
                PhysicalControl::Gamepad {
                    button: GamepadButton::Extra(number),
                },
                action,
            )
            .unwrap();
    }
    assert_eq!(
        settings.physical_bindings.bindings.len(),
        MAX_PHYSICAL_BINDINGS
    );
    repository.save_app_settings(&settings).unwrap();
    assert_eq!(repository.load_app_settings().unwrap(), settings);
    let before = settings.physical_bindings.clone();
    assert!(
        settings
            .physical_bindings
            .assign(PhysicalControl::AndroidKey { code: 24 }, action)
            .is_err()
    );
    assert_eq!(settings.physical_bindings, before);
}

#[test]
fn game_schema_migration_preserves_existing_controls_and_inherits_physical_bindings() {
    let mut game = crate::GameSettings {
        schema_version: 8,
        controls_override: true,
        ..Default::default()
    };
    game.control_layout.fire.size_percent = 150;
    let mut json = serde_json::to_value(&game).unwrap();
    json.as_object_mut().unwrap().remove("physical_bindings");
    let mut migrated: crate::GameSettings = serde_json::from_value(json).unwrap();
    assert!(migrated.migrate_to_current().unwrap());
    assert!(migrated.physical_bindings.is_none());
    assert!(migrated.controls_override);
    assert_eq!(migrated.control_layout.fire.size_percent, 150);
    migrated.validate(&[]).unwrap();
}
