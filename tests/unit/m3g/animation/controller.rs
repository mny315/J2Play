use super::*;

#[test]
fn controller_changes_speed_without_position_discontinuity() {
    let mut controller = AnimationControllerState::default();
    controller.set_position(5.0, 10).unwrap();
    assert_eq!(controller.position(20), 15.0);
    controller.set_speed(2.0, 20).unwrap();
    assert_eq!(controller.position(20), 15.0);
    assert_eq!(controller.position(25), 25.0);
    controller.set_active_interval(10, 30).unwrap();
    assert_eq!(controller.effective_weight(9), 0.0);
    assert_eq!(controller.effective_weight(10), 1.0);
    assert_eq!(controller.effective_weight(29), 1.0);
    assert_eq!(controller.effective_weight(30), 0.0);
}

#[test]
fn controller_preserves_elapsed_time_across_the_full_world_time_range() {
    let mut controller = AnimationControllerState::default();
    for (reference, world_time, expected) in [
        (i32::MIN, i32::MAX, 4_294_967_296.0),
        (i32::MAX, i32::MIN, -4_294_967_296.0),
        (i32::MAX - 1, i32::MAX, 1.0),
        (i32::MIN + 1, i32::MIN, -1.0),
    ] {
        controller.set_speed(1.0, reference).unwrap();
        controller.set_position(0.0, reference).unwrap();
        assert_eq!(controller.position(world_time), expected);
        controller.set_speed(-0.5, world_time).unwrap();
        assert_eq!(controller.position(world_time), expected);
    }
}

#[test]
fn equal_active_interval_endpoints_mean_always_active() {
    let mut controller = AnimationControllerState::default();
    controller.set_active_interval(17, 17).unwrap();
    for world_time in [i32::MIN, 16, 17, 18, i32::MAX] {
        assert_eq!(controller.effective_weight(world_time), 1.0);
    }
}

#[test]
fn controller_rounds_after_combining_reference_position_and_elapsed_time() {
    let mut controller = AnimationControllerState::default();
    for (reference, world_time, position, expected) in [
        (i32::MIN, i32::MAX, -4_294_967_296.0, -1.0),
        (i32::MAX, i32::MIN, 4_294_967_296.0, 1.0),
    ] {
        controller.set_speed(1.0, reference).unwrap();
        controller.set_position(position, reference).unwrap();
        assert_eq!(controller.position(world_time), expected);
        controller.set_speed(-0.5, world_time).unwrap();
        assert_eq!(controller.position(world_time), expected);
    }
}
