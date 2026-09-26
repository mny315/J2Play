use super::physical::{gameplay_fixture, keyboard_up};
use super::*;

fn setup(app: &mut FrontendApp) -> Pos2 {
    let center = Pos2::new(100.0, 100.0);
    app.stick_region = Some((
        egui::Rect::from_center_size(center, egui::Vec2::splat(120.0)),
        false,
    ));
    app.control_regions.push((
        egui::Rect::from_center_size(Pos2::new(250.0, 100.0), egui::Vec2::splat(50.0)),
        TouchOwner::VirtualKey(Some(HostAction::Fire)),
    ));
    app.observed_input = Some(Vec::new());
    center
}

fn key(action: HostAction, state: KeyState) -> InputEvent {
    InputEvent::Key { action, state }
}

#[test]
fn layout_switch_uses_the_same_bounds_and_keeps_stick_out_of_key_regions() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    let rect = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 320.0));
    let mut layout = crate::VirtualControlLayout::default();
    let geometry = crate::controls::gameplay_control_geometry(rect, &layout, None).unwrap();
    for enabled in [false, true] {
        layout.stick_enabled = enabled;
        app.control_regions.clear();
        app.stick_region = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            app.draw_virtual_controls(ui, rect, &layout, None);
        })
        .drop_without_applying_deltas();
        assert_eq!(app.stick_region.is_some(), enabled);
        assert_eq!(app.control_regions.len(), if enabled { 15 } else { 23 });
        if let Some((bounds, _)) = app.stick_region {
            assert!(bounds.center().distance(geometry.direction_pad_center) < 0.001);
            assert!((bounds.width() - geometry.direction_button_size * 3.0).abs() < 0.001);
            assert!(app.action_at(bounds.center()).is_none());
        }
    }
    layout.direction_pad.visible = false;
    app.control_regions.clear();
    app.stick_region = None;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        app.draw_virtual_controls(ui, rect, &layout, None);
    })
    .drop_without_applying_deltas();
    assert!(app.stick_region.is_none());
}

#[test]
fn stick_captures_one_finger_and_dragging_outside_keeps_direction_and_fire_independent() {
    let (_scratch, mut app, _) = gameplay_fixture();
    let center = setup(&mut app);
    app.process_touch(1, TouchPhase::Start, center);
    assert!(app.observed_input.as_ref().unwrap().is_empty());
    app.process_touch(3, TouchPhase::Start, center + egui::vec2(-30.0, 0.0));
    assert!(!app.touch_owners.contains_key(&3));
    app.process_touch(1, TouchPhase::Move, center + egui::vec2(30.0, 0.0));
    app.process_touch(2, TouchPhase::Start, Pos2::new(250.0, 100.0));
    app.process_touch(1, TouchPhase::Move, Pos2::new(250.0, 100.0));
    app.process_touch(1, TouchPhase::Move, center + egui::vec2(-30.0, 0.0));
    app.process_touch(1, TouchPhase::End, center);
    assert_eq!(
        app.touch_owners[&2].virtual_actions()[0],
        Some(HostAction::Fire)
    );
    app.process_touch(2, TouchPhase::Cancel, center);
    assert_eq!(
        app.observed_input.take().unwrap(),
        [
            key(HostAction::Right, KeyState::Pressed),
            key(HostAction::Fire, KeyState::Pressed),
            key(HostAction::Right, KeyState::Released),
            key(HostAction::Left, KeyState::Pressed),
            key(HostAction::Left, KeyState::Released),
            key(HostAction::Fire, KeyState::Released),
        ]
    );
    assert!(app.touch_owners.is_empty());
}

#[test]
fn stick_and_physical_direction_share_first_press_and_last_release() {
    let (_scratch, mut app, ctx) = gameplay_fixture();
    let center = setup(&mut app);
    let up = center + egui::vec2(0.0, -30.0);
    for physical_first in [false, true] {
        for physical_released_first in [false, true] {
            app.observed_input = Some(Vec::new());
            if physical_first {
                keyboard_up(&mut app, &ctx, true);
            }
            app.process_touch(1, TouchPhase::Start, up);
            if !physical_first {
                keyboard_up(&mut app, &ctx, true);
            }
            if physical_released_first {
                keyboard_up(&mut app, &ctx, false);
            } else {
                app.process_touch(1, TouchPhase::Cancel, up);
            }
            assert_eq!(
                app.observed_input.as_ref().unwrap(),
                &[key(HostAction::Up, KeyState::Pressed)]
            );
            if physical_released_first {
                app.process_touch(1, TouchPhase::Cancel, up);
            } else {
                keyboard_up(&mut app, &ctx, false);
            }
            assert_eq!(
                app.observed_input.take().unwrap(),
                [
                    key(HostAction::Up, KeyState::Pressed),
                    key(HostAction::Up, KeyState::Released)
                ]
            );
        }
    }
}

#[test]
fn stick_cancellation_and_geometry_barrier_require_a_new_touch() {
    let (_scratch, mut app, _) = gameplay_fixture();
    let center = setup(&mut app);
    let right = center + egui::vec2(30.0, 0.0);
    app.process_touch(1, TouchPhase::Start, right);
    app.process_touch(1, TouchPhase::Cancel, right);
    app.process_touch(1, TouchPhase::Move, right);
    assert!(app.touch_owners.is_empty());
    assert_eq!(
        app.observed_input.take().unwrap(),
        [
            key(HostAction::Right, KeyState::Pressed),
            key(HostAction::Right, KeyState::Released)
        ]
    );
    app.process_touch(1, TouchPhase::Start, right);
    app.clear_gameplay_input_geometry();
    assert!(app.touch_owners.is_empty());
    assert!(app.stick_region.is_none());
    setup(&mut app);
    app.process_touch(1, TouchPhase::Move, right);
    assert!(app.observed_input.as_ref().unwrap().is_empty());
    app.process_touch(1, TouchPhase::End, right);
    app.process_touch(1, TouchPhase::Start, right);
    assert_eq!(
        app.observed_input.take().unwrap(),
        [key(HostAction::Right, KeyState::Pressed)]
    );
}

#[test]
fn stick_diagonal_and_number_button_do_not_release_each_others_key() {
    let (_scratch, mut app, _) = gameplay_fixture();
    let center = setup(&mut app);
    app.process_touch(1, TouchPhase::Start, center + egui::vec2(30.0, -30.0));
    app.move_virtual_key(2, Some(HostAction::Num3));
    app.process_touch(1, TouchPhase::Move, center);
    assert_eq!(
        app.observed_input.as_ref().unwrap(),
        &[key(HostAction::Num3, KeyState::Pressed)]
    );
    app.move_virtual_key(2, None);
    assert_eq!(
        app.observed_input.take().unwrap(),
        [
            key(HostAction::Num3, KeyState::Pressed),
            key(HostAction::Num3, KeyState::Released)
        ]
    );
}
