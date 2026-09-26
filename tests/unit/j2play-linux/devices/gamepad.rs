use super::*;

#[test]
fn sticks_and_triggers_preserve_their_independent_normalized_ranges() {
    assert_eq!(
        axis_mapping(Axis::LeftX, i16::MIN),
        (GamepadAxis::LeftX, -1.0)
    );
    assert_eq!(
        axis_mapping(Axis::RightY, i16::MAX),
        (GamepadAxis::RightY, 1.0)
    );
    assert_eq!(
        axis_mapping(Axis::TriggerLeft, i16::MIN),
        (GamepadAxis::LeftTrigger, 0.0)
    );
    assert_eq!(
        axis_mapping(Axis::TriggerRight, i16::MAX),
        (GamepadAxis::RightTrigger, 1.0)
    );
    assert_eq!(button_mapping(Button::A), Some(GamepadButton::South));
    assert_eq!(button_mapping(Button::Guide), None);
}
