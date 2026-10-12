use super::*;

fn active() -> Lifecycle {
    let lifecycle = Lifecycle::default();
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Suspended));
    lifecycle.window(true, false);
    lifecycle.sleep(false);
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Resumed));
    lifecycle
}

#[test]
fn focus_cannot_resume_games_before_system_lifecycle_is_ready() {
    let lifecycle = Lifecycle::default();
    lifecycle.window(true, false);
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Suspended));
    assert_eq!(lifecycle.poll(), None);
    assert!(lifecycle.signal.suspended());
    lifecycle.sleep(false);
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Resumed));
}

#[test]
fn retained_hidden_window_events_do_not_repeat_a_focus_barrier() {
    let lifecycle = active();
    let mut window = WindowInput::default();
    let mut input = eframe::egui::RawInput {
        focused: true,
        events: vec![
            eframe::egui::Event::WindowFocused(false),
            eframe::egui::Event::WindowFocused(true),
        ],
        ..Default::default()
    };
    window.observe(&lifecycle, &input);
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Suspended));
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Resumed));
    for _ in 0..100 {
        window.observe(&lifecycle, &input);
        assert_eq!(lifecycle.poll(), None);
        assert!(!lifecycle.signal.suspended());
    }
    input.events.push(eframe::egui::Event::WindowFocused(false));
    window.observe(&lifecycle, &input);
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Suspended));
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Resumed));
    window.painted();
    input.events = vec![eframe::egui::Event::WindowFocused(false)];
    window.observe(&lifecycle, &input);
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Suspended));
}

#[test]
fn focus_minimize_and_system_sleep_are_independent_suspend_causes() {
    let lifecycle = active();
    lifecycle.sleep(true);
    lifecycle.window(false, true);
    lifecycle.sleep(false);
    assert!(lifecycle.signal.suspended());
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Suspended));
    lifecycle.window(true, true);
    assert!(lifecycle.signal.suspended());
    lifecycle.window(true, false);
    assert!(lifecycle.signal.suspended());
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Resumed));
    assert!(!lifecycle.signal.suspended());
    assert_eq!(lifecycle.poll(), None);
}

#[test]
fn coalesced_focus_and_sleep_cycles_keep_an_input_release_barrier() {
    let lifecycle = active();
    for _ in 0..10_000 {
        lifecycle.window(false, false);
        lifecycle.window(true, false);
        lifecycle.sleep(true);
        lifecycle.sleep(false);
    }
    assert!(lifecycle.signal.suspended());
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Suspended));
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Resumed));
    assert_eq!(lifecycle.poll(), None);
    assert!(!lifecycle.signal.suspended());
}

#[test]
fn destroy_reaches_worker_without_a_paint_and_cannot_be_resumed() {
    let lifecycle = active();
    let worker_signal = lifecycle.signal.clone();
    lifecycle.destroy();
    lifecycle.sleep(false);
    lifecycle.window(true, false);
    assert!(worker_signal.suspended());
    assert!(worker_signal.destroyed());
    assert_eq!(lifecycle.poll(), Some(PlatformLifecycleEvent::Destroyed));
    assert_eq!(lifecycle.poll(), None);
}
