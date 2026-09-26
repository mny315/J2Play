use super::*;
use crate::runtime::MAX_TEXT_COMMIT_BYTES;
use crate::{InputEvent, KeyState, PointerEvent, PointerPhase};
use runtime::idle_call;

mod lifecycle;

#[test]
fn connector_permission_policy_follows_the_selected_persona() {
    for (id, files) in [("nokia-s40-v1-keypad", false), ("se-featurephone", true)] {
        let profile = launch::builtin_device_profile(std::ffi::OsStr::new(id))
            .unwrap()
            .unwrap();
        let permissions = connector_permissions(&profile);
        assert!(permissions.allows(gcf::HTTP_PERMISSION));
        assert!(permissions.allows(gcf::HTTPS_PERMISSION));
        assert_eq!(permissions.allows(gcf::FILE_READ_PERMISSION), files);
        assert_eq!(permissions.allows(gcf::FILE_WRITE_PERMISSION), files);
        assert!(!permissions.allows("unknown.permission"));
    }
}

#[test]
fn rejected_pointer_press_cannot_produce_a_release_during_pause() {
    let mut driver = driver();
    driver.pointer = platform::PointerInputPolicy::new(false, false).with_host_passthrough(false);
    driver
        .queue_input(InputEvent::Pointer(PointerEvent {
            touch_id: 17,
            phase: PointerPhase::Pressed,
            canvas_x: 10,
            canvas_y: 20,
        }))
        .unwrap();
    assert!(driver.pending.is_empty());
    driver.set_pause(PauseReason::User, true).unwrap();
    assert!(driver.pending.is_empty());
    assert!(driver.captured_pointer.is_none());
}

#[test]
fn rejected_pointer_motion_preserves_capture_until_its_matching_release() {
    let mut driver = driver();
    driver.pointer = platform::PointerInputPolicy::new(true, false).with_host_passthrough(false);
    let pointer = PointerEvent {
        touch_id: 17,
        phase: PointerPhase::Pressed,
        canvas_x: 10,
        canvas_y: 20,
    };
    for (phase, touch_id, expected_calls) in [
        (PointerPhase::Pressed, 17, 1),
        (PointerPhase::Dragged, 17, 1),
        (PointerPhase::Released, 18, 1),
        (PointerPhase::Released, 17, 2),
    ] {
        driver
            .queue_input(InputEvent::Pointer(PointerEvent {
                touch_id,
                phase,
                ..pointer
            }))
            .unwrap();
        assert_eq!(driver.pending.len(), expected_calls);
    }
    assert!(driver.captured_pointer.is_none());
    driver.queue_release_all();
    assert_eq!(driver.pending.len(), 2);
}

#[test]
fn heap_recovery_preserves_canvas_and_can_cancel_preparation() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepared = crate::inspect_import(crate::ImportSource::new(
        Some("240320s40v3a.jar".to_owned()),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/java-me/conformance.jar"
        ))
        .to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    let entry = repository
        .commit_import(&prepared, crate::GameSettings::default())
        .unwrap();
    let orientation = Some(launch::CanvasOrientation::Landscape);
    let failed = prepared
        .launch_plan_with_orientation(entry.settings(), orientation)
        .unwrap();
    let dimensions = failed.decision.selection().canvas_dimensions();
    assert!(dimensions.0 > dimensions.1);
    let checks = Cell::new(0);
    let candidates = recovery_candidates(&repository, entry.id(), orientation, || {
        checks.set(checks.get() + 1);
        false
    })
    .unwrap();
    assert!(!candidates.is_empty());
    assert!(
        checks.get() > 4,
        "recovery preparation must poll cancellation"
    );
    for cancel_at in 1..=checks.get() {
        let calls = Cell::new(0);
        assert!(
            recovery_candidates(&repository, entry.id(), orientation, || {
                calls.set(calls.get() + 1);
                calls.get() >= cancel_at
            })
            .is_err()
        );
        assert_eq!(calls.get(), cancel_at);
    }
    for candidate in candidates {
        let settings = crate::GameSettings {
            device_profile: ProfileChoice::Manual {
                profile_id: candidate,
            },
            ..crate::GameSettings::default()
        };
        let plan = prepared
            .launch_plan_with_orientation(&settings, orientation)
            .unwrap();
        assert_eq!(plan.decision.selection().canvas_dimensions(), dimensions);
    }
}

pub(super) fn driver() -> AttemptDriver {
    let audio = AudioMailbox::default();
    audio.activate(SessionId(1), AttemptId(1)).unwrap();
    let vibration = VibrationMailbox::default();
    vibration.activate(SessionId(1), AttemptId(1)).unwrap();
    AttemptDriver::new(
        SessionId(1),
        AttemptId(1),
        CommandQueue::default(),
        Rc::new(RefCell::new(midp::Ams::new([]).unwrap())),
        platform::DeviceInputMap::default(),
        platform::PointerInputPolicy::default(),
        Rc::new(Cell::new((240, 320))),
        audio,
        vibration,
        Rc::new(Cell::new(true)),
        PlatformLifecycleSignal::default(),
    )
}

pub(super) fn submit(driver: &AttemptDriver, kind: SessionCommandKind) {
    driver
        .commands
        .push(SessionCommand {
            session_id: driver.session_id,
            attempt_id: driver.attempt_id,
            kind,
        })
        .unwrap();
}

#[test]
fn text_commit_precedes_later_keys_without_expanding_the_whole_queue() {
    let mut driver = driver();
    submit(
        &driver,
        SessionCommandKind::Input(InputEvent::TextCommit {
            text: "a".repeat(MAX_TEXT_COMMIT_BYTES),
        }),
    );
    submit(
        &driver,
        SessionCommandKind::Input(InputEvent::Key {
            action: platform::HostAction::Fire,
            state: KeyState::Pressed,
        }),
    );
    submit(
        &driver,
        SessionCommandKind::Input(InputEvent::TextCommit {
            text: "b".repeat(MAX_TEXT_COMMIT_BYTES),
        }),
    );

    let step = driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    assert!(matches!(step, vm::DriverStep::Call(call)
        if call.name == "__hostTextInput" && call.arguments == [vm::Value::Int(97)]));
    assert_eq!(driver.pending_text.len(), MAX_TEXT_COMMIT_BYTES - 1);
    assert!(matches!(
        driver.commands.pop_normal().unwrap().unwrap().kind,
        SessionCommandKind::Input(InputEvent::Key { .. })
    ));
}

#[test]
fn pausing_cancels_a_partially_delivered_composition_before_guest_pause() {
    let mut driver = driver();
    submit(
        &driver,
        SessionCommandKind::Input(InputEvent::TextComposition {
            text: "я🙂".into(),
            anchor: 1,
            caret: 3,
        }),
    );
    driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    assert!(driver.composing);
    driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    submit(
        &driver,
        SessionCommandKind::SetPause {
            reason: PauseReason::User,
            paused: true,
        },
    );
    let step = driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    assert!(matches!(step, vm::DriverStep::Call(call)
        if call.name == "__hostTextInput" && call.arguments == [vm::Value::Int(0x10000)]));
    assert!(driver.pending_text.is_empty());
    assert!(!driver.composing);
}

#[test]
fn command_pause_and_platform_pause_are_independent() {
    let mut driver = driver();
    submit(
        &driver,
        SessionCommandKind::SetPause {
            reason: PauseReason::Lifecycle,
            paused: true,
        },
    );
    driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    assert!(driver.paused.lifecycle);
    assert_eq!(driver.audio.active_generation().unwrap(), None);

    driver.lifecycle.set_suspended(true);
    driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    submit(
        &driver,
        SessionCommandKind::SetPause {
            reason: PauseReason::Lifecycle,
            paused: false,
        },
    );
    driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    assert_eq!(driver.audio.active_generation().unwrap(), None);

    driver.lifecycle.set_suspended(false);
    driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    assert_eq!(
        driver.audio.active_generation().unwrap(),
        Some((SessionId(1), AttemptId(1)))
    );
}

#[test]
fn worker_acknowledges_suspend_only_after_muting_outputs_and_releasing_input() {
    let mut driver = driver();
    driver
        .audio
        .publish(SessionId(1), AttemptId(1), &[1, 2, 3])
        .unwrap();
    submit(
        &driver,
        SessionCommandKind::Input(InputEvent::Key {
            action: platform::HostAction::Fire,
            state: KeyState::Pressed,
        }),
    );
    driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    driver.lifecycle.set_suspended(true);
    assert!(!driver.lifecycle.suspension_acknowledged());
    let release = driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    assert!(matches!(release, vm::DriverStep::Call(call) if call.name == "__hostKeyReleased"));
    assert!(driver.lifecycle.suspension_acknowledged());
    assert_eq!(driver.audio.stats().unwrap().queued_frames, 0);
    assert_eq!(driver.audio.active_generation().unwrap(), None);
    assert_eq!(
        driver.vibration.take_latest().unwrap().unwrap().request,
        natives::VibrationRequest::Stop
    );
}

#[test]
fn lifecycle_overflow_reaches_the_driver_but_does_not_block_stop_or_shutdown() {
    for ending in ["overflow", "stop", "shutdown"] {
        let mut driver = driver();
        driver
            .ams
            .borrow_mut()
            .host_event(midp::HostEvent::Launch)
            .unwrap();
        assert_eq!(
            driver.ams.borrow_mut().next_action(),
            Some(midp::LifecycleAction::Start)
        );
        // Keep startApp pending while host pause/resume events accumulate.
        for index in 0..16 {
            driver.set_pause(PauseReason::User, index % 2 == 0).unwrap();
        }
        if ending == "overflow" {
            assert_eq!(
                driver
                    .set_pause(PauseReason::User, true)
                    .unwrap_err()
                    .code(),
                "lifecycle-event-capacity"
            );
            assert_eq!(driver.audio.active_generation().unwrap(), None);
            continue;
        }
        driver
            .ams
            .borrow_mut()
            .callback_finished(
                midp::LifecycleCallback::Start,
                midp::LifecycleOutcome::Completed,
            )
            .unwrap();
        submit(
            &driver,
            SessionCommandKind::SetPause {
                reason: PauseReason::User,
                paused: true,
            },
        );
        if ending == "stop" {
            submit(&driver, SessionCommandKind::Stop);
        } else {
            driver.commands.request_shutdown();
        }
        let step = driver.next_step(vm::DriverTurn::HostPoll).unwrap();
        assert!(
            matches!(step, vm::DriverStep::CallAfterApplicationShutdown(call) if call.name == "__amsDestroyApp")
        );
        assert!(driver.close_queued);
        assert_eq!(driver.audio.active_generation().unwrap(), None);
    }
}

#[test]
fn queued_start_cannot_reenable_effects_while_suspended() {
    let mut driver = driver();
    driver
        .ams
        .borrow_mut()
        .host_event(midp::HostEvent::Launch)
        .unwrap();
    driver.lifecycle.set_suspended(true);
    driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    assert_eq!(driver.audio.active_generation().unwrap(), None);
}

#[test]
fn stop_discards_pending_work_and_keeps_effects_suspended() {
    let mut driver = driver();
    driver.pending.push_back(vm::DriverStep::Call(idle_call()));
    driver.paused.user = true;
    submit(&driver, SessionCommandKind::Stop);
    submit(
        &driver,
        SessionCommandKind::SetPause {
            reason: PauseReason::User,
            paused: false,
        },
    );
    let step = driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    assert!(matches!(
        step,
        vm::DriverStep::CallAfterApplicationShutdown(_)
    ));
    assert_eq!(driver.audio.active_generation().unwrap(), None);
}

#[test]
fn pausing_cancels_continuous_vibration_even_when_audio_cleanup_fails() {
    let mut driver = driver();
    driver
        .vibration
        .publish(crate::VibrationEffect {
            session_id: driver.session_id,
            attempt_id: driver.attempt_id,
            request: natives::VibrationRequest::Continuous { level: None },
        })
        .unwrap();
    driver.vibration.take_latest().unwrap();
    driver.audio.poison_for_test();
    submit(
        &driver,
        SessionCommandKind::SetPause {
            reason: PauseReason::Lifecycle,
            paused: true,
        },
    );

    let Err(error) = driver.next_step(vm::DriverTurn::HostPoll) else {
        panic!("poisoned audio mailbox must report an error");
    };
    assert_eq!(error.code(), "audio-lock");
    assert_eq!(
        driver.vibration.take_latest().unwrap().unwrap().request,
        natives::VibrationRequest::Stop,
    );
}

#[test]
fn pause_preserves_queued_pacing_changes_while_discarding_guest_input() {
    let mut driver = driver();
    for enabled in [true, false] {
        submit(&driver, SessionCommandKind::SetFastForward { enabled });
        submit(
            &driver,
            SessionCommandKind::Input(InputEvent::Key {
                action: platform::HostAction::Fire,
                state: KeyState::Pressed,
            }),
        );
        driver.lifecycle.set_suspended(true);
        assert!(matches!(
            driver.next_step(vm::DriverTurn::HostPoll).unwrap(),
            vm::DriverStep::Idle
        ));
        assert_eq!(driver.realtime_pacing.get(), !enabled);
        assert!(driver.input.release_events().is_empty());
        assert!(driver.pending.is_empty());
        driver.lifecycle.set_suspended(false);
        driver.next_step(vm::DriverTurn::HostPoll).unwrap();
        assert_eq!(driver.realtime_pacing.get(), !enabled);
    }
}

#[test]
fn fast_forward_commands_change_pacing_only_for_the_current_attempt() {
    let mut driver = driver();
    assert!(driver.realtime_pacing.get());
    driver
        .commands
        .push(SessionCommand {
            session_id: driver.session_id,
            attempt_id: AttemptId(driver.attempt_id.get() + 1),
            kind: SessionCommandKind::SetFastForward { enabled: true },
        })
        .unwrap();
    driver.next_step(vm::DriverTurn::HostPoll).unwrap();
    assert!(driver.realtime_pacing.get());
    for enabled in [true, false] {
        submit(&driver, SessionCommandKind::SetFastForward { enabled });
        driver.next_step(vm::DriverTurn::HostPoll).unwrap();
        assert_eq!(driver.realtime_pacing.get(), !enabled);
    }
}
