use super::physical::gameplay_fixture;
use super::*;
use crate::{VirtualControlGeometry, VirtualControlLayout};

fn key(action: HostAction, state: KeyState) -> InputEvent {
    InputEvent::Key { action, state }
}

fn draw(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    stick: bool,
    enabled: bool,
) -> VirtualControlGeometry {
    let rect = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 320.0));
    let layout = VirtualControlLayout {
        stick_enabled: stick,
        two_key_diagonals: enabled,
        ..VirtualControlLayout::default()
    };
    app.control_regions.clear();
    app.stick_region = None;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        app.draw_virtual_controls(ui, rect, &layout, None);
    })
    .drop_without_applying_deltas();
    crate::controls::gameplay_control_geometry(rect, &layout, None).unwrap()
}

#[test]
fn both_touch_controls_hold_numeric_pairs_only_when_enabled_and_keep_number_keys_literal() {
    use HostAction::{Num1, Num2, Num3, Num4, Num6, Num7, Num8, Num9};
    for stick in [false, true] {
        for enabled in [false, true] {
            let (_scratch, mut app, ctx) = gameplay_fixture();
            let geometry = draw(&mut app, &ctx, stick, enabled);
            for (x, y, digit, vertical, horizontal, index) in [
                (-1.0, -1.0, Num1, Num2, Num4, 0),
                (1.0, -1.0, Num3, Num2, Num6, 2),
                (-1.0, 1.0, Num7, Num8, Num4, 6),
                (1.0, 1.0, Num9, Num8, Num6, 8),
            ] {
                let position = geometry.direction_pad_center
                    + egui::vec2(x, y) * geometry.direction_button_size * 0.8;
                app.observed_input = Some(Vec::new());
                app.process_touch(1, TouchPhase::Start, position);
                // A stationary finger must not generate extra presses.
                app.process_touch(1, TouchPhase::Move, position);
                app.process_touch(1, TouchPhase::End, position);
                let keys = if enabled {
                    vec![vertical, horizontal]
                } else {
                    vec![digit]
                };
                let expected: Vec<_> = [KeyState::Pressed, KeyState::Released]
                    .into_iter()
                    .flat_map(|state| keys.iter().map(move |action| key(*action, state)))
                    .collect();
                assert_eq!(
                    app.observed_input.take().unwrap(),
                    expected,
                    "stick={stick}, enabled={enabled}, {digit:?}"
                );
                app.observed_input = Some(Vec::new());
                let number = geometry.number_keys[index].0;
                app.process_touch(2, TouchPhase::Start, number);
                app.process_touch(2, TouchPhase::Cancel, number);
                assert_eq!(
                    app.observed_input.take().unwrap(),
                    [
                        key(digit, KeyState::Pressed),
                        key(digit, KeyState::Released)
                    ]
                );
                assert!(app.touch_owners.is_empty());
            }
        }
    }
}

#[test]
fn diagonal_drag_preserves_shared_component_and_other_finger_until_last_release() {
    use HostAction::{Fire, Num2, Num6, Num8};
    for stick in [false, true] {
        let (_scratch, mut app, ctx) = gameplay_fixture();
        let geometry = draw(&mut app, &ctx, stick, true);
        let position = |y| {
            geometry.direction_pad_center
                + egui::vec2(1.0, y) * geometry.direction_button_size * 0.8
        };
        app.observed_input = Some(Vec::new());
        app.process_touch(1, TouchPhase::Start, position(-1.0));
        app.process_touch(2, TouchPhase::Start, geometry.number_keys[5].0);
        app.process_touch(3, TouchPhase::Start, geometry.fire_center);
        app.process_touch(1, TouchPhase::Move, position(1.0));
        app.process_touch(1, TouchPhase::Cancel, position(1.0));
        app.process_touch(2, TouchPhase::End, geometry.number_keys[5].0);
        app.process_touch(3, TouchPhase::End, geometry.fire_center);
        assert_eq!(
            app.observed_input.take().unwrap(),
            [
                key(Num2, KeyState::Pressed),
                key(Num6, KeyState::Pressed),
                key(Fire, KeyState::Pressed),
                key(Num2, KeyState::Released),
                key(Num8, KeyState::Pressed),
                key(Num8, KeyState::Released),
                key(Num6, KeyState::Released),
                key(Fire, KeyState::Released),
            ]
        );
        assert!(app.touch_owners.is_empty());
    }
}

#[test]
fn diagonal_components_share_physical_keys_without_changing_physical_bindings() {
    use HostAction::{Num2, Num6};
    for physical_first in [false, true] {
        for physical_released_first in [false, true] {
            let (_scratch, mut app, ctx) = gameplay_fixture();
            let geometry = draw(&mut app, &ctx, false, true);
            let position = geometry.direction_pad_center
                + egui::vec2(1.0, -1.0) * geometry.direction_button_size;
            let physical = |app: &mut FrontendApp, pressed| {
                app.process_game_key(
                    &ctx,
                    &Event::Key {
                        key: Key::Num6,
                        physical_key: Some(Key::Num6),
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                    &mut 0,
                );
            };
            app.observed_input = Some(Vec::new());
            if physical_first {
                physical(&mut app, true);
            }
            app.process_touch(1, TouchPhase::Start, position);
            if !physical_first {
                physical(&mut app, true);
            }
            if physical_released_first {
                physical(&mut app, false);
            }
            app.process_touch(1, TouchPhase::End, position);
            if !physical_released_first {
                physical(&mut app, false);
            }
            let events = app.observed_input.take().unwrap();
            for action in [Num2, Num6] {
                assert_eq!(events.iter().filter(|event| matches!(event, InputEvent::Key { action: a, .. } if *a == action)).cloned().collect::<Vec<_>>(),
                    [key(action, KeyState::Pressed), key(action, KeyState::Released)]);
            }
            assert_eq!(events.len(), 4);
        }
    }
}

#[test]
fn geometry_barrier_discards_both_components_and_old_finger_cannot_restart_them() {
    for stick in [false, true] {
        let (_scratch, mut app, ctx) = gameplay_fixture();
        let geometry = draw(&mut app, &ctx, stick, true);
        let position =
            geometry.direction_pad_center + egui::vec2(0.8, -0.8) * geometry.direction_button_size;
        app.process_touch(1, TouchPhase::Start, position);
        app.clear_gameplay_input_geometry();
        assert!(app.touch_owners.is_empty());
        draw(&mut app, &ctx, stick, true);
        app.observed_input = Some(Vec::new());
        app.process_touch(1, TouchPhase::Move, position);
        app.process_touch(1, TouchPhase::End, position);
        assert!(app.observed_input.as_ref().unwrap().is_empty());
        app.process_touch(1, TouchPhase::Start, position);
        assert_eq!(
            app.observed_input.take().unwrap(),
            [
                key(HostAction::Num2, KeyState::Pressed),
                key(HostAction::Num6, KeyState::Pressed)
            ]
        );
    }
}
