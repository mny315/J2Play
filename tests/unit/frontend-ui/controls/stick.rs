use super::*;

fn bounds() -> Rect {
    Rect::from_center_size(Pos2::new(100.0, 100.0), Vec2::splat(120.0))
}

#[test]
fn stick_has_a_neutral_center_and_matches_all_eight_keypad_directions() {
    let bounds = bounds();
    assert!(contains(bounds, bounds.center()));
    assert!(!contains(bounds, bounds.left_top()));
    assert_eq!(
        touch_owner(bounds, bounds.center(), None, false).virtual_actions()[0],
        None
    );
    for (_, action, x, y) in DIRECTION_PAD_BUTTONS {
        let position = bounds.center() + Vec2::new(x, y) * 25.0;
        assert_eq!(
            touch_owner(bounds, position, None, false).virtual_actions()[0],
            Some(action)
        );
    }
    let TouchOwner::VirtualStick { action, offset, .. } = touch_owner(
        bounds,
        bounds.center() + Vec2::new(1_000.0, 0.0),
        None,
        false,
    ) else {
        panic!("stick must retain its touch owner")
    };
    assert_eq!(action, Some(HostAction::Right));
    assert_eq!(offset, Vec2::X);
}

#[test]
fn radial_and_angular_hysteresis_suppress_jitter_but_release_in_the_center() {
    let bounds = bounds();
    let at_radius = |fraction| bounds.center() + Vec2::X * 36.0 * fraction;
    assert_eq!(
        touch_owner(bounds, at_radius(0.20), None, false).virtual_actions()[0],
        None
    );
    assert_eq!(
        touch_owner(bounds, at_radius(0.24), None, false).virtual_actions()[0],
        Some(HostAction::Right)
    );
    assert_eq!(
        touch_owner(bounds, at_radius(0.20), Some(HostAction::Right), false).virtual_actions()[0],
        Some(HostAction::Right)
    );
    assert_eq!(
        touch_owner(bounds, at_radius(0.14), Some(HostAction::Right), false).virtual_actions()[0],
        None
    );
    let at_angle = |degrees: f32| bounds.center() + Vec2::angled(degrees.to_radians()) * 30.0;
    assert_eq!(
        touch_owner(bounds, at_angle(25.0), None, false).virtual_actions()[0],
        Some(HostAction::Num9)
    );
    assert_eq!(
        touch_owner(bounds, at_angle(25.0), Some(HostAction::Right), false).virtual_actions()[0],
        Some(HostAction::Right)
    );
    assert_eq!(
        touch_owner(bounds, at_angle(32.0), Some(HostAction::Right), false).virtual_actions()[0],
        Some(HostAction::Num9)
    );
    for position in [Pos2::new(f32::NAN, 0.0), Pos2::new(f32::INFINITY, 0.0)] {
        assert_eq!(
            touch_owner(bounds, position, Some(HostAction::Right), false).virtual_actions()[0],
            None
        );
    }
}
