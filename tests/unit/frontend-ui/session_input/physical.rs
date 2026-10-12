use super::*;
use crate::{FullscreenHelpState, tests::test_storage::Scratch};
use frontend_core::physical_input::{GamepadButton, PhysicalControl, PhysicalInputEvent};

pub(super) fn gameplay_fixture() -> (Scratch, FrontendApp, egui::Context) {
    let scratch = Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.screen = Screen::Gameplay(Box::new(GameplayScreen {
        entry_id: "fixture".into(),
        title: "Input fixture".into(),
        canvas_dimensions: (240, 320),
        pointer_events: false,
        game_scale: crate::GameScale::AutomaticFit,
        portrait_frame_percent: None,
        control_layout: crate::VirtualControlLayout::default(),
        landscape_control_layout: crate::VirtualControlLayout::default(),
        vibration: crate::VibrationSettings::default(),
        orientation: None,
        fast_forward: false,
        fullscreen: false,
    }));
    let start = app.session.start("fixture").unwrap();
    assert!(app.session.apply_event(&SessionEvent {
        session_id: start.session_id,
        attempt_id: start.attempt_id,
        kind: frontend_core::SessionEventKind::Started,
    }));
    let ctx = egui::Context::default();
    ctx.input_mut(|input| input.focused = true);
    (scratch, app, ctx)
}

#[test]
fn virtual_and_physical_keys_share_first_press_and_last_release() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    let physical = |app: &mut FrontendApp, pressed| {
        app.process_physical_event(
            &ctx,
            PhysicalInputEvent::Button {
                device: 1,
                control: PhysicalControl::Gamepad {
                    button: GamepadButton::South,
                },
                pressed,
            },
            true,
        );
    };
    let touch = |app: &mut FrontendApp, pressed: bool| {
        app.move_virtual_key(1, pressed.then_some(HostAction::Num5));
    };
    let key = |state| InputEvent::Key {
        action: HostAction::Num5,
        state,
    };
    for physical_first in [false, true] {
        for physical_released_first in [false, true] {
            app.observed_input = Some(Vec::new());
            if physical_first {
                physical(&mut app, true);
                touch(&mut app, true);
            } else {
                touch(&mut app, true);
                physical(&mut app, true);
            }
            assert_eq!(
                app.observed_input.as_ref().unwrap(),
                &[key(KeyState::Pressed)]
            );
            if physical_released_first {
                physical(&mut app, false);
            } else {
                touch(&mut app, false);
            }
            assert_eq!(
                app.observed_input.as_ref().unwrap(),
                &[key(KeyState::Pressed)],
                "the other source still holds 5"
            );
            if physical_released_first {
                touch(&mut app, false);
            } else {
                physical(&mut app, false);
            }
            assert_eq!(
                app.observed_input.take().unwrap(),
                [key(KeyState::Pressed), key(KeyState::Released)]
            );
        }
    }
}

pub(super) fn keyboard_up(app: &mut FrontendApp, ctx: &egui::Context, pressed: bool) {
    app.process_game_key(
        ctx,
        &Event::Key {
            key: Key::ArrowUp,
            physical_key: Some(Key::ArrowUp),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        &mut 0,
    );
}

#[test]
fn text_editing_keys_share_ownership_with_controllers_and_touch() {
    for touch in [false, true] {
        for keyboard_first in [false, true] {
            for keyboard_released_first in [false, true] {
                let (_scratch, mut app, ctx) = gameplay_fixture();
                app.set_platform_text_input(true);
                app.observed_input = Some(Vec::new());
                let other = |app: &mut FrontendApp, pressed: bool| {
                    if touch {
                        app.move_virtual_key(1, pressed.then_some(HostAction::Up));
                    } else {
                        app.process_physical_event(
                            &ctx,
                            PhysicalInputEvent::Button {
                                device: 1,
                                control: PhysicalControl::Gamepad {
                                    button: GamepadButton::DpadUp,
                                },
                                pressed,
                            },
                            true,
                        );
                    }
                };
                if keyboard_first {
                    keyboard_up(&mut app, &ctx, true);
                    other(&mut app, true);
                } else {
                    other(&mut app, true);
                    keyboard_up(&mut app, &ctx, true);
                }
                let press = InputEvent::Key {
                    action: HostAction::Up,
                    state: KeyState::Pressed,
                };
                assert_eq!(
                    app.observed_input.as_ref().unwrap(),
                    std::slice::from_ref(&press)
                );
                if keyboard_released_first {
                    keyboard_up(&mut app, &ctx, false);
                } else {
                    other(&mut app, false);
                }
                assert_eq!(
                    app.observed_input.as_ref().unwrap(),
                    std::slice::from_ref(&press)
                );
                if keyboard_released_first {
                    other(&mut app, false);
                } else {
                    keyboard_up(&mut app, &ctx, false);
                }
                assert_eq!(
                    app.observed_input.take().unwrap(),
                    [
                        press,
                        InputEvent::Key {
                            action: HostAction::Up,
                            state: KeyState::Released,
                        }
                    ]
                );
            }
        }
    }
}

#[test]
fn text_editing_keys_require_neutral_after_an_input_barrier() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    app.set_platform_text_input(true);
    keyboard_up(&mut app, &ctx, true);
    app.release_all_input();
    app.observed_input = Some(Vec::new());
    keyboard_up(&mut app, &ctx, true);
    keyboard_up(&mut app, &ctx, false);
    assert!(app.observed_input.as_ref().unwrap().is_empty());
    keyboard_up(&mut app, &ctx, true);
    assert_eq!(
        app.observed_input.take().unwrap(),
        [InputEvent::Key {
            action: HostAction::Up,
            state: KeyState::Pressed,
        }]
    );
}

#[test]
fn text_editing_key_release_keeps_the_action_chosen_at_press_time() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    app.set_platform_text_input(true);
    app.observed_input = Some(Vec::new());
    for (key, pressed) in [(Key::ArrowDown, true), (Key::Numpad2, false)] {
        app.process_game_key(
            &ctx,
            &Event::Key {
                key,
                physical_key: Some(Key::Numpad2),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            &mut 0,
        );
    }
    assert_eq!(
        app.observed_input.take().unwrap(),
        [KeyState::Pressed, KeyState::Released].map(|state| InputEvent::Key {
            action: HostAction::Down,
            state,
        })
    );
}

#[test]
fn text_editor_taps_do_not_release_a_key_held_by_another_source() {
    struct TextKey(Option<crate::PlatformTextInputEvent>);
    impl crate::PlatformBridge for TextKey {
        fn request_document(&mut self, _: crate::DocumentKind) -> Result<(), EmuError> {
            unreachable!("text fixture does not open documents")
        }

        fn poll_document(&mut self) -> Option<Result<crate::DocumentOutcome, EmuError>> {
            None
        }

        fn poll_text_input(&mut self) -> Option<Result<crate::PlatformTextInputEvent, EmuError>> {
            self.0.take().map(Ok)
        }
    }

    for touch in [false, true] {
        let (_scratch, mut app, ctx) = gameplay_fixture();
        app.set_platform_text_input(true);
        let hold = |app: &mut FrontendApp, pressed: bool| {
            if touch {
                app.move_virtual_key(1, pressed.then_some(HostAction::Up));
            } else {
                keyboard_up(app, &ctx, pressed);
            }
        };
        let tap = |app: &mut FrontendApp| {
            app.platform = Box::new(TextKey(Some(crate::PlatformTextInputEvent::Key(
                HostAction::Up,
            ))));
            app.process_platform_text_input(&ctx);
        };
        hold(&mut app, true);
        app.observed_input = Some(Vec::new());
        tap(&mut app);
        assert!(app.observed_input.as_ref().unwrap().is_empty());
        hold(&mut app, false);
        tap(&mut app);
        assert_eq!(
            app.observed_input.take().unwrap(),
            [KeyState::Released, KeyState::Pressed, KeyState::Released].map(|state| {
                InputEvent::Key {
                    action: HostAction::Up,
                    state,
                }
            })
        );
    }
}

#[test]
fn fullscreen_transition_blocks_later_touch_in_the_same_native_batch() {
    for text_input in [false, true] {
        let (_scratch, mut app, ctx) = gameplay_fixture();
        app.set_platform_text_input(text_input);
        app.fullscreen_help = FullscreenHelpState::Acknowledged;
        app.gameplay_transition.begin(
            egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 640.0)),
            false,
            false,
            Instant::now(),
        );
        app.control_regions.push((
            egui::Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(50.0)),
            TouchOwner::VirtualKey(Some(HostAction::Fire)),
        ));
        app.observed_input = Some(Vec::new());
        ctx.input_mut(|input| {
            input.raw.events = vec![
                Event::Key {
                    key: Key::F11,
                    physical_key: Some(Key::F11),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                Event::Touch {
                    device_id: egui::TouchDeviceId(1),
                    id: egui::TouchId(7),
                    phase: TouchPhase::Start,
                    pos: Pos2::new(20.0, 20.0),
                    force: None,
                },
            ];
        });
        app.process_game_input(&ctx);
        assert!(app.gameplay_transition.busy());
        assert!(app.observed_input.as_ref().unwrap().is_empty());
        assert!(app.touch_owners.is_empty());
    }
}

#[test]
fn key_released_during_text_input_can_press_again_in_gameplay() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    keyboard_up(&mut app, &ctx, true);
    app.set_platform_text_input(true);
    keyboard_up(&mut app, &ctx, false);
    app.set_platform_text_input(false);
    app.observed_input = Some(Vec::new());
    keyboard_up(&mut app, &ctx, true);
    assert_eq!(
        app.observed_input.take().unwrap(),
        [InputEvent::Key {
            action: HostAction::Up,
            state: KeyState::Pressed,
        }]
    );
}

#[test]
fn release_during_resize_or_a_ui_gesture_rearms_the_next_keyboard_press() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    keyboard_up(&mut app, &ctx, true);
    app.clear_gameplay_input_geometry();
    ctx.run_ui(
        egui::RawInput {
            events: vec![Event::Key {
                key: Key::ArrowUp,
                physical_key: Some(Key::ArrowUp),
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ui| app.process_physical_inputs(ui.ctx(), true, false),
    )
    .drop_without_applying_deltas();
    app.observed_input = Some(Vec::new());
    keyboard_up(&mut app, &ctx, true);
    assert_eq!(
        app.observed_input.take().unwrap(),
        [InputEvent::Key {
            action: HostAction::Up,
            state: KeyState::Pressed,
        }]
    );
}

#[test]
fn key_held_when_text_input_closes_requires_release_before_gameplay() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    app.set_platform_text_input(true);
    keyboard_up(&mut app, &ctx, true);
    app.set_platform_text_input(false);
    app.observed_input = Some(Vec::new());
    keyboard_up(&mut app, &ctx, true);
    keyboard_up(&mut app, &ctx, false);
    assert!(app.observed_input.as_ref().unwrap().is_empty());
    keyboard_up(&mut app, &ctx, true);
    assert_eq!(
        app.observed_input.take().unwrap(),
        [InputEvent::Key {
            action: HostAction::Up,
            state: KeyState::Pressed,
        }]
    );
}

#[test]
fn number_pad_and_modifiers_have_independent_physical_bindings() {
    use crate::physical_controls::key_usage;
    assert_eq!(key_usage(Key::Numpad1), Some(89));
    assert_eq!(key_usage(Key::Num1), Some(30));
    assert_eq!(key_usage(Key::NumpadEnter), Some(88));
    assert_eq!(key_usage(Key::Enter), Some(40));
    assert_eq!(key_usage(Key::ControlLeft), Some(224));
    assert_eq!(key_usage(Key::ControlRight), Some(228));
    assert_eq!(key_usage(Key::ShiftLeft), Some(225));
    assert_eq!(key_usage(Key::ShiftRight), Some(229));
}

#[test]
fn mouse_uses_virtual_key_ownership_without_duplicating_native_touch() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    app.control_regions.push((
        egui::Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(50.0, 50.0)),
        TouchOwner::VirtualKey(Some(HostAction::Fire)),
    ));
    app.observed_input = Some(Vec::new());
    let pointer = |pressed| Event::PointerButton {
        pos: Pos2::new(20.0, 20.0),
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    ctx.input_mut(|input| input.raw.events = vec![pointer(true)]);
    app.process_game_input(&ctx);
    ctx.input_mut(|input| {
        input.raw.events = vec![Event::PointerMoved(Pos2::new(21.0, 21.0)), pointer(false)];
    });
    app.process_game_input(&ctx);
    assert_eq!(
        app.observed_input.take().unwrap(),
        [
            InputEvent::Key {
                action: HostAction::Fire,
                state: KeyState::Pressed
            },
            InputEvent::Key {
                action: HostAction::Fire,
                state: KeyState::Released
            }
        ]
    );
    app.observed_input = Some(Vec::new());
    ctx.input_mut(|input| {
        input.raw.events = vec![
            pointer(true),
            Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(7),
                phase: TouchPhase::Start,
                pos: Pos2::new(20.0, 20.0),
                force: None,
            },
        ];
    });
    app.process_game_input(&ctx);
    assert_eq!(app.touch_owners.len(), 1);
    assert!(app.touch_owners.contains_key(&7));
    assert_eq!(app.observed_input.as_ref().unwrap().len(), 1);
    app.release_all_input();
    assert!(app.touch_owners.is_empty());
}

fn gameplay_key(app: &mut FrontendApp, ctx: &egui::Context, key: Key, pressed: bool) {
    app.process_game_key(
        ctx,
        &Event::Key {
            // A different logical key simulates a changed host keyboard layout.
            key: Key::B,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        &mut 0,
    );
}

#[test]
fn default_gameplay_keys_use_physical_positions_and_number_pad_labels() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    for (key, action) in [
        (Key::W, HostAction::Up),
        (Key::A, HostAction::Left),
        (Key::S, HostAction::Down),
        (Key::D, HostAction::Right),
        (Key::Space, HostAction::Num5),
        (Key::Z, HostAction::Num5),
        (Key::X, HostAction::Num0),
        (Key::C, HostAction::Star),
        (Key::V, HostAction::Pound),
        (Key::Q, HostAction::SoftLeft),
        (Key::E, HostAction::SoftRight),
        (Key::Enter, HostAction::Fire),
        (Key::Numpad0, HostAction::Num0),
        (Key::Numpad1, HostAction::Num1),
        (Key::Numpad2, HostAction::Num2),
        (Key::Numpad3, HostAction::Num3),
        (Key::Numpad4, HostAction::Num4),
        (Key::Numpad5, HostAction::Num5),
        (Key::Numpad6, HostAction::Num6),
        (Key::Numpad7, HostAction::Num7),
        (Key::Numpad8, HostAction::Num8),
        (Key::Numpad9, HostAction::Num9),
    ] {
        app.observed_input = Some(Vec::new());
        gameplay_key(&mut app, &ctx, key, true);
        gameplay_key(&mut app, &ctx, key, false);
        assert_eq!(
            app.observed_input.take().unwrap(),
            [
                InputEvent::Key {
                    action,
                    state: KeyState::Pressed,
                },
                InputEvent::Key {
                    action,
                    state: KeyState::Released,
                },
            ],
            "{key:?}"
        );
    }
}

#[test]
fn space_and_z_share_number_five_while_enter_keeps_its_own_press() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    app.observed_input = Some(Vec::new());
    for (key, pressed) in [
        (Key::Space, true),
        (Key::Z, true),
        (Key::Enter, true),
        (Key::Space, false),
        (Key::Enter, false),
        (Key::Z, false),
    ] {
        gameplay_key(&mut app, &ctx, key, pressed);
    }
    assert_eq!(
        app.observed_input.take().unwrap(),
        [
            (HostAction::Num5, KeyState::Pressed),
            (HostAction::Fire, KeyState::Pressed),
            (HostAction::Fire, KeyState::Released),
            (HostAction::Num5, KeyState::Released),
        ]
        .map(|(action, state)| InputEvent::Key { action, state })
    );
}
