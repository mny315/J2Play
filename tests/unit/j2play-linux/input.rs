use super::*;
use frontend_core::physical_input::{AxisDirection, GamepadAxis, GamepadButton, PhysicalControl};

fn button(pressed: bool) -> PhysicalInputEvent {
    PhysicalInputEvent::Button {
        device: 1,
        control: PhysicalControl::Gamepad {
            button: GamepadButton::South,
        },
        pressed,
    }
}

#[test]
fn suspended_press_requires_release_before_rearming() {
    let input = Input::default();
    let lifecycle = PlatformLifecycleSignal::default();
    lifecycle.set_suspended(true);
    input.event(button(true), &lifecycle);
    assert!(input.poll().is_none());
    lifecycle.set_suspended(false);
    input.event(button(true), &lifecycle);
    assert!(input.poll().is_none());
    input.event(button(false), &lifecycle);
    assert_eq!(input.poll(), Some(button(false)));
    input.event(button(true), &lifecycle);
    assert_eq!(input.poll(), Some(button(true)));
}

#[test]
fn transport_overflow_releases_input_and_coalesces_diagnostics() {
    let input = Input::default();
    let lifecycle = PlatformLifecycleSignal::default();
    for _ in 0..200 {
        input.event(button(true), &lifecycle);
        input.event(button(false), &lifecycle);
    }
    assert_eq!(input.poll(), Some(PhysicalInputEvent::Reset));
    assert!(input.poll().is_none());
    assert_eq!(
        input.poll_error().unwrap().code(),
        "physical-input-overflow"
    );
    assert!(input.lock().events.len() < 256);
}

#[test]
fn overflow_during_axis_reversal_requires_neutral_before_rearming() {
    let input = Input::default();
    let lifecycle = PlatformLifecycleSignal::default();
    let axis = |value| PhysicalInputEvent::Axis {
        device: 1,
        axis: GamepadAxis::LeftX,
        value,
    };
    input.event(axis(-1.0), &lifecycle);
    input.poll();
    for _ in 0..128 {
        input.event(button(true), &lifecycle);
        input.event(button(false), &lifecycle);
    }
    assert_eq!(input.lock().events.len(), 256);

    // Reversal emits a release followed by a press. Overflow on that release
    // must discard the press too, while remembering the stick is still held.
    input.event(axis(1.0), &lifecycle);
    assert_eq!(input.poll(), Some(PhysicalInputEvent::Reset));
    assert!(input.poll().is_none());
    input.event(axis(1.0), &lifecycle);
    assert!(input.poll().is_none());
    input.event(axis(0.0), &lifecycle);
    let edge = |pressed| PhysicalInputEvent::Button {
        device: 1,
        control: PhysicalControl::Axis {
            axis: GamepadAxis::LeftX,
            direction: AxisDirection::Positive,
        },
        pressed,
    };
    assert_eq!(input.poll(), Some(edge(false)));
    input.event(axis(1.0), &lifecycle);
    assert_eq!(input.poll(), Some(edge(true)));
}

#[test]
fn removal_emits_one_release_barrier_and_forgets_held_controls() {
    let input = Input::default();
    let lifecycle = PlatformLifecycleSignal::default();
    assert!(!input.has_devices());
    input.device_changed(1, Some("Controller".into()));
    assert!(input.has_devices());
    assert_eq!(
        input.poll(),
        Some(PhysicalInputEvent::Disconnected { device: 1 })
    );
    input.event(button(true), &lifecycle);
    assert_eq!(input.poll(), Some(button(true)));

    input.device_changed(1, None);
    assert!(!input.has_devices());
    assert!(input.devices().is_empty());
    assert_eq!(
        input.poll(),
        Some(PhysicalInputEvent::Disconnected { device: 1 })
    );
    assert!(input.poll().is_none());
    input.device_changed(1, Some("Reconnected controller".into()));
    input.poll();
    input.event(button(true), &lifecycle);
    assert_eq!(input.poll(), Some(button(true)));
}

#[test]
fn remapping_requires_release_before_rearming_held_controls() {
    let input = Input::default();
    let lifecycle = PlatformLifecycleSignal::default();
    input.event(button(true), &lifecycle);
    input.poll();
    input.device_changed(1, Some("Remapped controller".into()));
    assert_eq!(
        input.poll(),
        Some(PhysicalInputEvent::Disconnected { device: 1 })
    );
    input.event(button(true), &lifecycle);
    assert!(input.poll().is_none());
    input.event(button(false), &lifecycle);
    assert_eq!(input.poll(), Some(button(false)));
    input.event(button(true), &lifecycle);
    assert_eq!(input.poll(), Some(button(true)));
}

#[test]
fn wake_callbacks_can_access_the_queue_and_run_once_per_update() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let input = Input::default();
    let lifecycle = PlatformLifecycleSignal::default();
    let state = Arc::downgrade(&input.0);
    let wakes = Arc::new(AtomicUsize::new(0));
    let counted = wakes.clone();
    input.waker(Arc::new(move || {
        assert!(state.upgrade().unwrap().try_lock().is_ok());
        counted.fetch_add(1, Ordering::Relaxed);
    }));
    input.event(button(true), &lifecycle);
    input.event(button(true), &lifecycle);
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    input.device_changed(1, None);
    input.error(error("fixture", "Fixture diagnostic."));
    input.overflow();
    assert_eq!(wakes.load(Ordering::Relaxed), 4);
    for _ in 0..1000 {
        input.event(button(true), &lifecycle);
        input.event(button(false), &lifecycle);
        input.overflow();
    }
    assert_eq!(wakes.load(Ordering::Relaxed), 4);
    assert_eq!(input.poll(), Some(PhysicalInputEvent::Reset));
    assert!(input.poll().is_none());
    input.event(button(true), &lifecycle);
    assert_eq!(wakes.load(Ordering::Relaxed), 5);
}
