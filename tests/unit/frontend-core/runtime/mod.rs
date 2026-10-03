use super::*;
use crate::runtime::attempt::retain_input_release_steps;
use ::runtime::{idle_call, key_call, pointer_call};

#[test]
fn repeated_text_input_state_does_not_publish_platform_work() {
    let mut active = false;
    assert!(!update_text_input_state(&mut active, false));
    assert!(update_text_input_state(&mut active, true));
    assert!(!update_text_input_state(&mut active, true));
    assert!(update_text_input_state(&mut active, false));
    assert!(!update_text_input_state(&mut active, false));
}

#[test]
fn pause_preserves_release_callbacks_queued_before_the_transition() {
    let mut pending = VecDeque::from([
        vm::DriverStep::Call(idle_call()),
        vm::DriverStep::Call(key_call(&platform::MidpKeyEvent {
            kind: platform::KeyKind::Released,
            key_code: -1,
            game_action: Some(1),
            lcdui_key_code: -1,
            state: 0,
        })),
        vm::DriverStep::Call(pointer_call(platform::PointerEvent {
            tick: 0,
            kind: platform::PointerKind::PointerReleased,
            x: 12,
            y: 34,
        })),
    ]);

    retain_input_release_steps(&mut pending);

    assert_eq!(pending.len(), 2);
    assert!(matches!(
        &pending[0],
        vm::DriverStep::Call(call) if call.name == "__hostKeyReleased"
    ));
    assert!(matches!(
        &pending[1],
        vm::DriverStep::Call(call) if call.name == "__hostPointerReleased"
    ));
}
