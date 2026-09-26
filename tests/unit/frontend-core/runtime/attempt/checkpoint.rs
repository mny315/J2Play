use super::super::tests::{driver, submit};
use super::*;
use crate::{InputEvent, KeyState, PauseReason, PointerEvent, PointerPhase, SessionCommandKind};

#[test]
fn long_text_input_survives_repeated_continue_and_stop_without_reordering() {
    for composing in [false, true] {
        let mut current = active_checkpoint_driver();
        current
            .queue_input(InputEvent::Key {
                action: platform::HostAction::Fire,
                state: KeyState::Pressed,
            })
            .unwrap();
        current
            .queue_input(InputEvent::Pointer(PointerEvent {
                touch_id: 17,
                phase: PointerPhase::Pressed,
                canvas_x: 10,
                canvas_y: 20,
            }))
            .unwrap();
        // These two callbacks have already been handed to the VM before the
        // long text edit arrives; their matching releases belong to restore.
        for _ in 0..2 {
            assert!(matches!(
                current.next_step(vm::DriverTurn::HostPoll).unwrap(),
                vm::DriverStep::Call(_)
            ));
        }
        let text = "a🙂".repeat(800);
        submit(
            &current,
            SessionCommandKind::Input(if composing {
                InputEvent::TextComposition {
                    text,
                    anchor: 0,
                    caret: 2400,
                }
            } else {
                InputEvent::TextCommit { text }
            }),
        );
        current.next_step(vm::DriverTurn::HostPoll).unwrap();
        assert!(current.pending_text.len() > 512);

        let mut expected = None;
        for cycle in 0..3 {
            submit(&current, SessionCommandKind::Stop);
            let Some(vm::DriverStep::SaveCheckpoint { driver_state }) =
                current.checkpoint_step().unwrap()
            else {
                panic!("explicit Stop must capture the input queue");
            };
            let saved: SavedDriver = save_state::decode(&driver_state).unwrap();
            if cycle == 0 {
                assert_eq!(saved.releases.len(), 2 + usize::from(composing));
                let mut order = saved.pending;
                order.extend(saved.pending_text);
                order.extend(saved.releases.into_iter().map(vm::DriverStep::Call));
                expected = Some(save_state::encode(&order).unwrap());
            } else {
                assert!(
                    saved.releases.is_empty(),
                    "host input is recreated on resume"
                );
            }
            let mut restored = active_checkpoint_driver();
            restored.restore_driver_checkpoint(&driver_state).unwrap();
            assert_eq!(
                save_state::encode(&restored.pending).unwrap(),
                *expected.as_ref().unwrap()
            );
            current = restored;
        }
    }
}

fn active_checkpoint_driver() -> AttemptDriver {
    let mut current = driver();
    current.checkpoints.enabled = true;
    let mut ams = midp::Ams::new([midp::HostEvent::Launch]).unwrap();
    ams.next_action().unwrap();
    ams.callback_finished(
        midp::LifecycleCallback::Start,
        midp::LifecycleOutcome::Completed,
    )
    .unwrap();
    *current.ams.borrow_mut() = ams;
    current
}

#[test]
fn checkpoint_clears_preedit_after_the_remaining_composition_and_before_key_release() {
    for delivered in 0..=6 {
        let mut current = active_checkpoint_driver();
        current
            .queue_input(InputEvent::Key {
                action: platform::HostAction::Fire,
                state: KeyState::Pressed,
            })
            .unwrap();
        current.next_step(vm::DriverTurn::HostPoll).unwrap();
        current
            .queue_input(InputEvent::TextComposition {
                text: "я🙂".into(),
                anchor: 1,
                caret: 3,
            })
            .unwrap();
        assert_eq!(current.pending_text.len(), 6);
        for _ in 0..delivered {
            current.next_step(vm::DriverTurn::HostPoll).unwrap();
        }
        let remaining = current.pending_text.clone();
        submit(&current, SessionCommandKind::Stop);
        let Some(vm::DriverStep::SaveCheckpoint { driver_state }) =
            current.checkpoint_step().unwrap()
        else {
            panic!("explicit Stop must capture the composition");
        };
        let mut restored = active_checkpoint_driver();
        restored.restore_driver_checkpoint(&driver_state).unwrap();
        let calls = restored
            .pending
            .iter()
            .map(|step| match step {
                vm::DriverStep::Call(call) => call,
                _ => panic!("input replay must contain calls"),
            })
            .collect::<Vec<_>>();
        let last_text = calls
            .iter()
            .rfind(|call| call.name == "__hostTextInput")
            .unwrap();
        assert_eq!(
            last_text.arguments,
            [vm::Value::Int(0x10000)],
            "delivered={delivered}"
        );
        assert_eq!(calls.last().unwrap().name, "__hostKeyReleased");
        let prefix: VecDeque<_> = restored
            .pending
            .iter()
            .take(remaining.len())
            .cloned()
            .collect();
        assert_eq!(
            save_state::encode(&prefix).unwrap(),
            save_state::encode(&remaining).unwrap()
        );
        assert_eq!(
            save_state::encode(&current.pending_text).unwrap(),
            save_state::encode(&remaining).unwrap()
        );
        assert!(current.composing && current.pending.is_empty());
    }
}

#[test]
fn restored_input_queues_share_one_bounded_budget() {
    for (pending, text, releases, valid) in [
        (8768, 0, 0, true),
        (0, 8768, 0, true),
        (512, 8192, 64, true),
        (8769, 0, 0, false),
        (512, 8193, 64, false),
        (0, 0, 65, false),
    ] {
        let saved = SavedDriver {
            pending: VecDeque::from(vec![vm::DriverStep::Tick; pending]),
            pending_text: VecDeque::from(vec![vm::DriverStep::Tick; text]),
            releases: vec![runtime::idle_call(); releases],
        };
        let mut current = active_checkpoint_driver();
        current.pending.push_back(vm::DriverStep::Idle);
        let original = save_state::encode(&current.pending).unwrap();
        let result = current.restore_driver_checkpoint(&save_state::encode(&saved).unwrap());
        if valid {
            result.unwrap();
            assert_eq!(current.pending.len(), pending + text + releases);
        } else {
            assert_eq!(result.unwrap_err().code(), "checkpoint-driver");
            assert_eq!(save_state::encode(&current.pending).unwrap(), original);
        }
    }
}

#[test]
fn restored_input_rejects_terminal_steps_in_both_queues() {
    for step in [
        vm::DriverStep::Stop,
        vm::DriverStep::SaveCheckpoint {
            driver_state: Vec::new(),
        },
        vm::DriverStep::CallAfterApplicationShutdown(runtime::idle_call()),
    ] {
        for text in [false, true] {
            let mut saved: SavedDriver = SavedDriver {
                pending: VecDeque::new(),
                pending_text: VecDeque::new(),
                releases: Vec::new(),
            };
            if text {
                &mut saved.pending_text
            } else {
                &mut saved.pending
            }
            .push_back(step.clone());
            let mut current = active_checkpoint_driver();
            assert_eq!(
                current
                    .restore_driver_checkpoint(&save_state::encode(&saved).unwrap())
                    .unwrap_err()
                    .code(),
                "checkpoint-driver"
            );
            assert!(current.pending.is_empty() && current.pending_text.is_empty());
        }
    }
}

#[test]
fn restoration_resumes_after_a_full_lifecycle_queue_without_changing_the_saved_format() {
    let mut driver = driver();
    let mut ams = midp::Ams::new([midp::HostEvent::Launch]).unwrap();
    ams.next_action().unwrap();
    ams.callback_finished(
        midp::LifecycleCallback::Start,
        midp::LifecycleOutcome::Completed,
    )
    .unwrap();
    for index in 0..16 {
        ams.host_event(if index % 2 == 0 {
            midp::HostEvent::Resume
        } else {
            midp::HostEvent::Pause
        })
        .unwrap();
    }
    let bytes = save_state::encode(&ams).unwrap();
    *driver.ams.borrow_mut() = save_state::decode(&bytes).unwrap();
    let saved: SavedDriver = SavedDriver {
        pending: VecDeque::new(),
        pending_text: VecDeque::new(),
        releases: Vec::new(),
    };
    driver
        .restore_driver_checkpoint(&save_state::encode(&saved).unwrap())
        .unwrap();
    assert_eq!(save_state::encode(&*driver.ams.borrow()).unwrap(), bytes);
    let mut ams = driver.ams.borrow_mut();
    ams.validate_checkpoint().unwrap();
    // The leading Resume was redundant in Active. Restore supplies the final
    // Resume after the eight saved pauses, using one bounded host flag.
    for _ in 0..8 {
        assert_eq!(ams.next_action(), Some(midp::LifecycleAction::Pause));
        ams.callback_finished(
            midp::LifecycleCallback::Pause,
            midp::LifecycleOutcome::Completed,
        )
        .unwrap();
        assert_eq!(ams.next_action(), Some(midp::LifecycleAction::Resume));
        ams.callback_finished(
            midp::LifecycleCallback::Start,
            midp::LifecycleOutcome::Completed,
        )
        .unwrap();
    }
    assert_eq!(ams.next_action(), None);
    assert_eq!(ams.state(), midp::LifecycleState::Active);
}

#[test]
fn checkpoint_runs_only_once_for_explicit_stop_and_never_for_pause_or_shutdown() {
    for mode in ["running", "pause", "suspend", "shutdown", "stop"] {
        let mut driver = driver();
        driver.checkpoints.enabled = true;
        let mut ams = midp::Ams::new([midp::HostEvent::Launch]).unwrap();
        ams.next_action().unwrap();
        *driver.ams.borrow_mut() = ams;
        match mode {
            "pause" => submit(
                &driver,
                SessionCommandKind::SetPause {
                    reason: PauseReason::User,
                    paused: true,
                },
            ),
            "suspend" => driver.lifecycle.set_suspended(true),
            "shutdown" => driver.urgent.request_shutdown(),
            "stop" => submit(&driver, SessionCommandKind::Stop),
            _ => {}
        }
        assert_eq!(
            matches!(
                driver.checkpoint_step().unwrap(),
                Some(vm::DriverStep::SaveCheckpoint { .. })
            ),
            mode == "stop",
            "{mode}"
        );
        assert!(driver.checkpoint_step().unwrap().is_none());
    }
}

#[test]
fn stop_received_during_command_poll_saves_before_destroy() {
    let mut driver = driver();
    driver.checkpoints.enabled = true;
    let mut ams = midp::Ams::new([midp::HostEvent::Launch]).unwrap();
    ams.next_action().unwrap();
    ams.callback_finished(
        midp::LifecycleCallback::Start,
        midp::LifecycleOutcome::Completed,
    )
    .unwrap();
    *driver.ams.borrow_mut() = ams;

    // Stop can arrive after a turn's first poll, while its priority lane is
    // being drained. It must still pass through saving before destroyApp.
    assert!(driver.checkpoint_step().unwrap().is_none());
    submit(&driver, SessionCommandKind::Stop);
    driver.drain_priority().unwrap();
    assert!(matches!(
        driver.next_step(vm::DriverTurn::Regular).unwrap(),
        vm::DriverStep::SaveCheckpoint { .. }
    ));
    assert!(!driver.close_queued);
    assert_eq!(driver.ams.borrow().state(), midp::LifecycleState::Active);
    assert!(matches!(
        driver.next_step(vm::DriverTurn::Regular).unwrap(),
        vm::DriverStep::CallAfterApplicationShutdown(_)
    ));
}

#[test]
fn capture_borrows_live_queues_and_preserves_the_restore_order_and_format() {
    for composing in [false, true] {
        let mut current = driver();
        current.checkpoints.enabled = true;
        let mut ams = midp::Ams::new([midp::HostEvent::Launch]).unwrap();
        ams.next_action().unwrap();
        *current.ams.borrow_mut() = ams;
        for action in [platform::HostAction::Up, platform::HostAction::Num2] {
            current
                .queue_input(InputEvent::Key {
                    action,
                    state: KeyState::Pressed,
                })
                .unwrap();
        }
        current
            .queue_input(InputEvent::Pointer(PointerEvent {
                touch_id: 17,
                phase: PointerPhase::Pressed,
                canvas_x: 10,
                canvas_y: 20,
            }))
            .unwrap();
        current
            .queue_input(if composing {
                InputEvent::TextComposition {
                    text: "я🙂".into(),
                    anchor: 1,
                    caret: 3,
                }
            } else {
                InputEvent::TextCommit {
                    text: "я🙂".into()
                }
            })
            .unwrap();
        let pending = current.pending.clone();
        let text = current.pending_text.clone();
        let state = current.input.game_state();
        submit(&current, SessionCommandKind::Stop);
        let Some(vm::DriverStep::SaveCheckpoint { driver_state }) =
            current.checkpoint_step().unwrap()
        else {
            panic!("explicit Stop captures the driver");
        };
        assert_eq!(
            save_state::encode(&current.pending).unwrap(),
            save_state::encode(&pending).unwrap()
        );
        assert_eq!(
            save_state::encode(&current.pending_text).unwrap(),
            save_state::encode(&text).unwrap()
        );
        assert_eq!(current.input.game_state(), state);
        assert_eq!(current.captured_pointer, Some(17));
        assert_eq!(current.composing, composing);

        let saved: SavedDriver = save_state::decode(&driver_state).unwrap();
        assert_eq!(saved.releases.len(), 3 + usize::from(composing));
        if composing {
            assert_eq!(saved.releases[0].name, "__hostTextInput");
            assert_eq!(saved.releases[0].arguments, [vm::Value::Int(0x10000)]);
        }
        let releases = &saved.releases[usize::from(composing)..];
        assert_eq!(releases[0].name, "__hostKeyReleased");
        assert_eq!(
            releases[0].arguments,
            [vm::Value::Int(-1), vm::Value::Int(2), vm::Value::Int(-1)]
        );
        assert_eq!(
            releases[1].arguments,
            [vm::Value::Int(50), vm::Value::Int(0), vm::Value::Int(50)]
        );
        assert_eq!(releases[2].name, "__hostPointerReleased");
        assert_eq!(
            releases[2].arguments,
            [vm::Value::Int(10), vm::Value::Int(20)]
        );
        let mut expected_pending = pending;
        // The prior owned-queue representation has this same three-field layout.
        let legacy = save_state::encode(&(&expected_pending, &text, &saved.releases)).unwrap();
        assert_eq!(driver_state, legacy);
        expected_pending.extend(text);
        expected_pending.extend(saved.releases.into_iter().map(vm::DriverStep::Call));
        let mut restored = driver();
        restored.restore_driver_checkpoint(&legacy).unwrap();
        assert_eq!(
            save_state::encode(&restored.pending).unwrap(),
            save_state::encode(&expected_pending).unwrap()
        );
    }
}
