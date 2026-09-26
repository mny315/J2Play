use super::*;

#[test]
fn externally_opened_jar_waits_for_library_without_replacing_a_picker() {
    let state = Arc::new(BridgeState::default());
    let mut bridge = AndroidPlatformBridge::with_state(state.clone());
    state.begin_external_document().unwrap();
    let generation = state.begin_document_copy();
    state.complete_document_copy(
        generation,
        Ok(DocumentOutcome::Selected(frontend_ui::PickedDocument {
            kind: DocumentKind::Jar,
            display_name: Some("fixture.jar".into()),
            bytes: vec![1, 2, 3],
        })),
    );
    assert!(bridge.poll_document().is_none());
    assert!(state.begin_external_document().is_err());
    assert!(bridge.request_document(DocumentKind::Jar).is_err());
    assert!(matches!(
        bridge.poll_external_document(),
        Some(Ok(DocumentOutcome::Selected(_)))
    ));
    assert!(bridge.poll_external_document().is_none());
    assert!(!state.document_busy.load(Ordering::Acquire));
    // A late completion never republishes a consumed result.
    state.complete_document_copy(
        generation,
        Ok(DocumentOutcome::Cancelled {
            kind: DocumentKind::Jar,
        }),
    );
    assert!(bridge.poll_document().is_none());
}

#[test]
fn native_text_wakes_the_idle_frontend_without_holding_the_event_queue() {
    let state = Arc::new(BridgeState::default());
    state.suspend(false);
    let mut bridge = AndroidPlatformBridge::with_state(state.clone());
    let wakes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = wakes.clone();
    let weak = Arc::downgrade(&state);
    bridge.set_physical_input_waker(Arc::new(move || {
        assert!(weak.upgrade().unwrap().text.try_lock().is_ok());
        observed.fetch_add(1, Ordering::Relaxed);
    }));
    state.input(PlatformTextInputEvent::Commit("Поиск".into()));
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    assert_eq!(
        bridge.poll_text_input().unwrap().unwrap(),
        PlatformTextInputEvent::Commit("Поиск".into())
    );
}

#[test]
fn text_lifecycle_discards_stale_edits_and_keeps_composition_cancellation() {
    for suspended in [false, true] {
        let state = Arc::new(BridgeState::default());
        state.suspend(false);
        let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
        state.input(PlatformTextInputEvent::Composition {
            text: "old".into(),
            selection_start: 0,
            selection_end: 3,
        });
        state.input(PlatformTextInputEvent::Commit("old".into()));
        if suspended {
            state.suspend(true);
        } else {
            state.focus(false);
        }
        // Hiding the native editor cancels its composition. A late callback
        // from that editor must not edit the guest or a product field.
        state.input(PlatformTextInputEvent::Cancel);
        state.focus(false);
        state.input(PlatformTextInputEvent::DeleteUtf16 {
            before: 1,
            after: 0,
        });
        state.suspend(false);
        state.focus(true);
        assert_eq!(
            bridge.poll_text_input().unwrap().unwrap(),
            PlatformTextInputEvent::Cancel,
            "suspended={suspended}"
        );
        assert!(bridge.poll_text_input().is_none());
        state.input(PlatformTextInputEvent::Commit("new".into()));
        assert_eq!(
            bridge.poll_text_input().unwrap().unwrap(),
            PlatformTextInputEvent::Commit("new".into())
        );
    }
}

#[test]
fn destroyed_activity_rejects_text_and_physical_input() {
    let state = Arc::new(BridgeState::default());
    state.suspend(false);
    let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    state.input(PlatformTextInputEvent::Commit("old".into()));
    state.destroy();
    // Late resume/focus notifications cannot revive a destroyed Activity.
    state.suspend(false);
    state.focus(true);
    state.input(PlatformTextInputEvent::Commit("late".into()));
    state.physical_event(PhysicalInputEvent::Button {
        device: 1,
        control: PhysicalControl::Keyboard { usage: 4 },
        pressed: true,
    });
    assert!(bridge.poll_physical_input().is_none());
    while let Some(event) = bridge.poll_text_input() {
        assert_eq!(event.unwrap(), PlatformTextInputEvent::Cancel);
    }
}

#[test]
fn text_lifecycle_keeps_overflow_errors_until_the_frontend_reads_them() {
    let state = Arc::new(BridgeState::default());
    state.suspend(false);
    let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    state.input(PlatformTextInputEvent::Commit("x".repeat(4097)));
    state.focus(false);
    state.suspend(true);
    state.suspend(false);
    state.focus(true);
    // Preparing another editor must not silently discard the prior failure,
    // including when posting the native visibility command fails on this host.
    let _ = bridge.set_text_input_active(true);
    assert_eq!(
        bridge.poll_text_input().unwrap().unwrap_err().code(),
        "ime-event-overflow"
    );
    assert!(bridge.poll_text_input().is_none());
    state.input(PlatformTextInputEvent::Commit("new".into()));
    assert_eq!(
        bridge.poll_text_input().unwrap().unwrap(),
        PlatformTextInputEvent::Commit("new".into())
    );
}

#[test]
fn system_text_scale_tracks_android_configuration_without_changing_defaults() {
    let state = Arc::new(BridgeState::default());
    let bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    assert_eq!(
        bridge.system_text_scale(),
        frontend_ui::PlatformTextScale::default()
    );
    for factor in [0.85, 2.0, 1.0] {
        let mut points = [0.0; frontend_ui::PlatformTextScale::SAMPLE_COUNT];
        for (size, point) in (1_u16..).zip(&mut points) {
            *point = f32::from(size) * factor;
        }
        let scale = frontend_ui::PlatformTextScale::from_points(points).unwrap();
        *lock(&state.text_scale) = scale;
        assert_eq!(bridge.system_text_scale(), scale);
    }
}

#[test]
fn cancelling_external_requests_invalidates_queued_urls_and_allows_later_requests() {
    let state = Arc::new(BridgeState::default());
    let mut bridge = AndroidPlatformBridge::with_state(state.clone());
    let previous = state.external_request_generation.load(Ordering::Acquire);
    bridge.cancel_guest_operations();
    state
        .open_external_url::<()>(previous, || panic!("cancelled URL reached Android"))
        .unwrap();
    let current = state.external_request_generation.load(Ordering::Acquire);
    let mut opened = false;
    state
        .open_external_url::<()>(current, || {
            opened = true;
            Ok(())
        })
        .unwrap();
    assert!(opened);
    assert_eq!(
        state.open_external_url(current, || Err("SDK failure")),
        Err("SDK failure")
    );
    state.destroy();
    state
        .open_external_url::<()>(current, || panic!("destroyed Activity opened a URL"))
        .unwrap();
}

#[test]
fn physical_overflow_is_a_single_release_barrier_and_resets_after_delivery() {
    use frontend_core::physical_input::{GamepadButton, PhysicalInputEvent};
    let state = Arc::new(BridgeState::default());
    state.suspend(false);
    let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    let press = PhysicalInputEvent::Button {
        device: 1,
        control: PhysicalControl::Gamepad {
            button: GamepadButton::South,
        },
        pressed: true,
    };
    for _ in 0..1000 {
        state.physical_event(press);
    }
    assert_eq!(
        bridge.poll_physical_input(),
        Some(PhysicalInputEvent::Reset)
    );
    assert_eq!(bridge.poll_physical_input(), None);
    assert_eq!(
        bridge.pump_runtime_effects().unwrap_err().code(),
        "physical-input-overflow"
    );
    state.physical_event(press);
    assert_eq!(bridge.poll_physical_input(), Some(press));
}

#[test]
fn streaming_axes_only_queue_edges_and_wake_for_actual_changes() {
    use frontend_core::physical_input::{AxisDirection, GamepadAxis};
    use std::sync::atomic::AtomicUsize;

    let state = Arc::new(BridgeState::default());
    state.suspend(false);
    let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    let wake_count = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&wake_count);
    bridge.set_physical_input_waker(Arc::new(move || {
        observed.fetch_add(1, Ordering::Relaxed);
    }));
    let axis = |value| PhysicalInputEvent::Axis {
        device: 1,
        axis: GamepadAxis::LeftX,
        value,
    };
    for _ in 0..10_000 {
        state.physical_event(axis(0.01));
    }
    assert_eq!(bridge.poll_physical_input(), None);
    assert_eq!(wake_count.load(Ordering::Relaxed), 0);
    for _ in 0..10_000 {
        state.physical_event(axis(0.8));
    }
    state.physical_event(axis(0.3)); // Held within the hysteresis band.
    state.physical_event(axis(0.0));
    state.physical_event(axis(0.9));
    state.physical_event(axis(0.0)); // A short press/release cannot coalesce away.
    for pressed in [true, false, true, false] {
        assert_eq!(
            bridge.poll_physical_input(),
            Some(PhysicalInputEvent::Button {
                device: 1,
                control: PhysicalControl::Axis {
                    axis: GamepadAxis::LeftX,
                    direction: AxisDirection::Positive,
                },
                pressed,
            })
        );
    }
    assert_eq!(bridge.poll_physical_input(), None);
    assert_eq!(wake_count.load(Ordering::Relaxed), 4);
    bridge.pump_runtime_effects().unwrap();
}

#[test]
fn physical_lifecycle_discards_stale_presses_and_device_removal_is_delivered() {
    use frontend_core::physical_input::{GamepadButton, PhysicalInputEvent};
    let state = Arc::new(BridgeState::default());
    state.suspend(false);
    let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    let control = PhysicalControl::Gamepad {
        button: GamepadButton::South,
    };
    let press = PhysicalInputEvent::Button {
        device: 7,
        control,
        pressed: true,
    };
    state.physical_event(press);
    state.suspend(true);
    state.physical_event(press);
    state.suspend(false);
    assert!(bridge.poll_physical_input().is_none());
    assert_eq!(
        bridge.poll_lifecycle(),
        Some(PlatformLifecycleEvent::Suspended)
    );
    assert_eq!(
        bridge.poll_lifecycle(),
        Some(PlatformLifecycleEvent::Resumed)
    );
    state.physical_event(press);
    state.focus(false);
    state.physical_event(press);
    state.focus(true);
    assert!(bridge.poll_physical_input().is_none());
    assert!(!bridge.has_physical_devices());
    state.physical_device(7, Some("A controller\n".repeat(200)));
    assert!(bridge.has_physical_devices());
    assert_eq!(bridge.physical_devices().len(), 1);
    assert_eq!(bridge.physical_devices()[0].chars().count(), 128);
    assert!(!bridge.physical_devices()[0].contains('\n'));
    let _ = bridge.poll_physical_input();
    state.physical_device(7, None);
    assert_eq!(
        bridge.poll_physical_input(),
        Some(PhysicalInputEvent::Disconnected { device: 7 })
    );
    assert!(bridge.physical_devices().is_empty());
    assert!(!bridge.has_physical_devices());
}

#[test]
fn axis_releases_around_focus_loss_do_not_suppress_the_next_press() {
    use frontend_core::physical_input::GamepadAxis;

    for suspend in [false, true] {
        for release_before_barrier in [false, true] {
            let state = Arc::new(BridgeState::default());
            state.suspend(false);
            let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
            let mut frontend = PhysicalInputState::default();
            let axis = |value| PhysicalInputEvent::Axis {
                device: 1,
                axis: GamepadAxis::LeftX,
                value,
            };
            state.physical_event(axis(1.0));
            let press = bridge.poll_physical_input().unwrap();
            assert!(frontend.update(press, 35)[0].pressed);
            if release_before_barrier {
                state.physical_event(axis(0.0));
            }
            if suspend {
                state.suspend(true);
            } else {
                state.focus(false);
            }
            if !release_before_barrier {
                state.physical_event(axis(0.0));
            }
            state.suspend(false);
            state.focus(true);
            // The UI consumes the native backlog without executing actions at
            // the lifecycle barrier, then allows fresh input after resuming.
            while let Some(event) = bridge.poll_physical_input() {
                frontend.update(event, 35);
            }
            frontend.release_actions();
            state.physical_event(axis(1.0));
            let next = bridge.poll_physical_input().unwrap();
            assert_eq!(
                frontend.update(next, 35).len(),
                1,
                "a neutral stick must rearm across focus/suspend"
            );
        }
    }
}

#[test]
fn native_routing_preserves_volume_and_text_unless_capture_is_requested() {
    let state = Arc::new(BridgeState::default());
    let handles = |control| {
        let pressed = state.route_physical_key(7, control, true);
        assert_eq!(state.route_physical_key(7, control, false), pressed);
        pressed
    };
    let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    let volume = PhysicalControl::AndroidKey { code: 24 };
    let keyboard = PhysicalControl::Keyboard { usage: 4 };
    let mut config = PhysicalInputConfig::default();
    let mut bindings = PhysicalBindings::default();
    assert!(!handles(volume));
    assert!(!handles(keyboard));
    config.gameplay = true;
    bindings
        .assign(volume, frontend_core::physical_input::PhysicalAction::Menu)
        .unwrap();
    bridge.configure_physical_input(config, &bindings);
    assert!(handles(volume));
    assert!(handles(keyboard));
    config.text_input = true;
    bridge.configure_physical_input(config, &bindings);
    assert!(!handles(keyboard));
    config.gameplay = false;
    config.capture = true;
    bridge.configure_physical_input(config, &bindings);
    assert!(handles(keyboard));
    assert!(handles(volume));
}

#[test]
fn a_consumed_press_keeps_its_release_after_navigation_or_capture_cancel() {
    let state = Arc::new(BridgeState::default());
    let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    let keyboard = PhysicalControl::Keyboard { usage: 4 };
    let capture = PhysicalInputConfig {
        capture: true,
        ..Default::default()
    };
    let bindings = PhysicalBindings::default();
    bridge.configure_physical_input(capture, &bindings);
    assert!(state.route_physical_key(7, keyboard, true));
    bridge.configure_physical_input(PhysicalInputConfig::default(), &bindings);
    assert!(state.route_physical_key(7, keyboard, true));
    assert!(state.route_physical_key(7, keyboard, false));
    assert!(!state.route_physical_key(7, keyboard, true));
}
#[test]
fn fast_pause_resume_retains_input_release_and_final_state() {
    let state = Arc::new(BridgeState::default());
    let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    for _ in 0..1000 {
        state.suspend(true);
        state.suspend(false);
    }
    assert_eq!(
        bridge.poll_lifecycle(),
        Some(PlatformLifecycleEvent::Suspended)
    );
    assert_eq!(
        bridge.poll_lifecycle(),
        Some(PlatformLifecycleEvent::Resumed)
    );
    assert_eq!(bridge.poll_lifecycle(), None);
    state.destroy();
    state.suspend(false);
    assert_eq!(
        bridge.poll_lifecycle(),
        Some(PlatformLifecycleEvent::Destroyed)
    );
    assert_eq!(bridge.poll_lifecycle(), None);
}
#[test]
fn input_overflow_cancels_instead_of_delivering_incomplete_edits() {
    let state = Arc::new(BridgeState::default());
    state.suspend(false);
    let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    for _ in 0..1000 {
        state.input(PlatformTextInputEvent::Commit("x".into()));
    }
    assert_eq!(
        bridge.poll_text_input().unwrap().unwrap_err().code(),
        "ime-event-overflow"
    );
    assert!(bridge.poll_text_input().is_none());
    state.input(PlatformTextInputEvent::Commit("late".into()));
    assert!(bridge.poll_text_input().is_none());
}

#[test]
fn document_timeout_cannot_be_replaced_after_frontend_consumes_it() {
    let state = Arc::new(BridgeState::default());
    let mut bridge = AndroidPlatformBridge::with_state(Arc::clone(&state));
    let old = state.begin_document_copy();
    state.complete_document_copy(old, Err(platform_error("timeout", "timeout")));
    assert_eq!(
        bridge.poll_document().unwrap().unwrap_err().code(),
        "timeout"
    );
    state.complete_document_copy(
        old,
        Ok(DocumentOutcome::Cancelled {
            kind: DocumentKind::Jar,
        }),
    );
    assert!(bridge.poll_document().is_none());
    let next = state.begin_document_copy();
    state.complete_document_copy(old, Err(platform_error("late", "late")));
    assert!(bridge.poll_document().is_none());
    state.complete_document_copy(
        next,
        Ok(DocumentOutcome::Cancelled {
            kind: DocumentKind::Jad,
        }),
    );
    assert!(matches!(
        bridge.poll_document(),
        Some(Ok(DocumentOutcome::Cancelled {
            kind: DocumentKind::Jad
        }))
    ));
}

#[test]
fn diagnostic_flood_is_bounded_and_reports_repetition_and_overflow() {
    let state = BridgeState::default();
    for _ in 0..1000 {
        state.error(platform_error("repeated", "SDK failure"));
    }
    for index in 0..10 {
        state.error(platform_error("distinct", format!("failure {index}")));
    }
    let mut errors = lock(&state.errors);
    assert_eq!(errors.pending.len(), 8);
    assert_eq!(
        errors.pop().unwrap().message(),
        "SDK failure (1000 occurrences)"
    );
    for _ in 0..7 {
        assert_eq!(errors.pop().unwrap().code(), "distinct");
    }
    let overflow = errors.pop().unwrap();
    assert_eq!(overflow.code(), "android-diagnostics-overflow");
    assert!(overflow.message().starts_with("3 additional"));
    assert!(errors.pop().is_none());
}
#[test]
fn urls_never_expose_private_files() {
    for url in [
        "FILE:///private",
        "content://provider/item",
        "1bad:value",
        "no-scheme",
        "https://a\0b",
    ] {
        assert!(validate_url(url).is_err());
    }
    assert!(validate_url("https://example.org/path").is_ok());
    assert!(validate_url("mailto:example@example.org").is_ok());
}
