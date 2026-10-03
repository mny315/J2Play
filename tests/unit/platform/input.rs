use super::*;

#[test]
fn input_state_uses_selected_device_key_map() {
    let mapping = DeviceInputMap::new([(
        HostAction::Up,
        DeviceKey {
            key_code: -101,
            game_action: Some(GAME_UP),
        },
    )]);
    let mut state = InputState::new(mapping);
    let event = state
        .apply(InputEvent {
            tick: 0,
            kind: KeyKind::Pressed,
            action: HostAction::Up,
        })
        .unwrap();
    assert_eq!(event.key_code, -101);
    assert_eq!(event.lcdui_key_code, -1);
    assert_eq!(event.game_action, Some(GAME_UP));
    assert_eq!(event.state, 1_u32 << GAME_UP);
}

#[test]
fn host_pointer_passthrough_does_not_rewrite_device_capabilities() {
    let keypad = PointerInputPolicy::new(false, false);
    assert!(!keypad.device_events());
    assert!(!keypad.device_motion_events());
    for kind in [
        PointerKind::PointerPressed,
        PointerKind::PointerDragged,
        PointerKind::PointerReleased,
    ] {
        assert!(keypad.allows(kind));
    }

    let strict = keypad.with_host_passthrough(false);
    assert!(!strict.allows(PointerKind::PointerPressed));
    assert!(!strict.allows(PointerKind::PointerDragged));
    assert!(!strict.allows(PointerKind::PointerReleased));

    let touch = PointerInputPolicy::new(true, true).with_host_passthrough(false);
    assert!(touch.allows(PointerKind::PointerPressed));
    assert!(touch.allows(PointerKind::PointerDragged));
    assert!(touch.allows(PointerKind::PointerReleased));
}

#[test]
fn simultaneous_keys_and_focus_cleanup() {
    let mut s = InputState::default();
    for a in [HostAction::Up, HostAction::Right, HostAction::Fire] {
        s.apply(InputEvent {
            tick: 0,
            kind: KeyKind::Pressed,
            action: a,
        })
        .unwrap();
    }
    assert_eq!(
        s.game_state(),
        (1 << GAME_UP) | (1 << GAME_RIGHT) | (1 << GAME_FIRE)
    );
    assert_eq!(s.focus_lost().len(), 3);
    assert_eq!(s.game_state(), 0);
}

#[test]
fn repeat_requires_pressed_key() {
    let mut s = InputState::default();
    assert!(
        s.apply(InputEvent {
            tick: 0,
            kind: KeyKind::Repeated,
            action: HostAction::Up
        })
        .is_none()
    );
}

#[test]
fn duplicate_logical_sources_hold_until_the_last_release() {
    let mut state = InputState::default();
    let event = |kind| InputEvent {
        tick: 0,
        kind,
        action: HostAction::Up,
    };
    assert!(state.apply(event(KeyKind::Pressed)).is_some());
    assert!(state.apply(event(KeyKind::Pressed)).is_none());
    assert!(state.apply(event(KeyKind::Released)).is_none());
    assert_eq!(state.game_state(), 1 << GAME_UP);
    assert!(state.apply(event(KeyKind::Released)).is_some());
    assert_eq!(state.game_state(), 0);
}

#[test]
fn distinct_keys_sharing_one_game_action_hold_until_both_are_released() {
    for (first, second, bit) in [
        (HostAction::Up, HostAction::Num2, GAME_UP),
        (HostAction::Fire, HostAction::Num5, GAME_FIRE),
    ] {
        for (first, second) in [(first, second), (second, first)] {
            let mut state = InputState::default();
            let event = |action, kind| InputEvent {
                tick: 0,
                kind,
                action,
            };
            let first_press = state.apply(event(first, KeyKind::Pressed)).unwrap();
            let second_press = state.apply(event(second, KeyKind::Pressed)).unwrap();
            assert_ne!(first_press.key_code, second_press.key_code);
            assert_eq!(first_press.state, 1 << bit);
            assert_eq!(second_press.state, 1 << bit);
            let first_release = state.apply(event(first, KeyKind::Released)).unwrap();
            assert_eq!(first_release.key_code, first_press.key_code);
            assert_eq!(first_release.state, 1 << bit);
            assert_eq!(state.game_state(), 1 << bit);
            assert_eq!(
                state.apply(event(second, KeyKind::Repeated)).unwrap().state,
                1 << bit
            );
            assert_eq!(
                state.apply(event(second, KeyKind::Released)).unwrap().state,
                0
            );
            assert_eq!(state.game_state(), 0);
        }
    }
}

#[test]
fn focus_cleanup_releases_each_key_without_dropping_a_shared_action_early() {
    let mut state = InputState::default();
    for action in [HostAction::Up, HostAction::Up, HostAction::Num2] {
        state.apply(InputEvent {
            tick: 0,
            kind: KeyKind::Pressed,
            action,
        });
    }
    let released = state.focus_lost();
    assert_eq!(released.len(), 2);
    assert_eq!(released[0].state, 1 << GAME_UP);
    assert_eq!(released[1].state, 0);
    assert_eq!(state.game_state(), 0);
    assert!(state.focus_lost().is_empty());
}

#[test]
fn ime_commits_preserve_java_utf16() {
    assert_eq!(
        committed_text_events(7, "Я😀").collect::<Vec<_>>(),
        [0x042f, 0xd83d, 0xde00].map(|code_unit| TextInputEvent {
            tick: 7,
            kind: TextInputKind::Commit,
            code_unit,
        })
    );
}

#[test]
fn planned_releases_preserve_live_input_and_match_individual_last_releases() {
    let actions = [
        HostAction::Up,
        HostAction::Fire,
        HostAction::Num2,
        HostAction::Num5,
    ];
    let mut state = InputState::default();
    for action in actions {
        for _ in 0..2 {
            state.apply(InputEvent {
                tick: 0,
                kind: KeyKind::Pressed,
                action,
            });
        }
    }
    let planned = state.release_events();
    assert_eq!(planned.len(), actions.len());
    assert_eq!(state.game_state(), (1 << GAME_UP) | (1 << GAME_FIRE));
    for (planned, action) in planned.iter().zip(actions) {
        let event = InputEvent {
            tick: 0,
            kind: KeyKind::Released,
            action,
        };
        assert!(state.apply(event).is_none());
        let released = state.apply(event).unwrap();
        assert_eq!(planned.kind, released.kind);
        assert_eq!(planned.key_code, released.key_code);
        assert_eq!(planned.lcdui_key_code, released.lcdui_key_code);
        assert_eq!(planned.game_action, released.game_action);
        assert_eq!(planned.state, released.state);
    }
    assert_eq!(state.game_state(), 0);
    assert!(state.release_events().is_empty());
}
