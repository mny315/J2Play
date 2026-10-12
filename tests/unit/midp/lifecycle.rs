use super::*;

#[test]
fn pending_lifecycle_callback_cannot_grow_an_unbounded_event_queue() {
    let mut ams = Ams::new([HostEvent::Launch]).unwrap();
    assert_eq!(ams.next_action(), Some(LifecycleAction::Start));
    for index in 0..MAX_LIFECYCLE_EVENTS {
        ams.host_event(if index % 2 == 0 {
            HostEvent::Pause
        } else {
            HostEvent::Resume
        })
        .unwrap();
    }
    let queued = ams.events.clone();
    let pending = ams.pending;
    assert_eq!(
        ams.host_event(HostEvent::Pause).unwrap_err().code(),
        "lifecycle-event-capacity"
    );
    assert_eq!(ams.events, queued);
    assert_eq!(ams.pending, pending);
    ams.validate_checkpoint().unwrap();
    ams.callback_finished(LifecycleCallback::Start, LifecycleOutcome::Completed)
        .unwrap();
    for index in 0..MAX_LIFECYCLE_EVENTS {
        let (action, callback) = if index % 2 == 0 {
            (LifecycleAction::Pause, LifecycleCallback::Pause)
        } else {
            (LifecycleAction::Resume, LifecycleCallback::Start)
        };
        assert_eq!(ams.next_action(), Some(action));
        ams.callback_finished(callback, LifecycleOutcome::Completed)
            .unwrap();
    }
    assert_eq!(ams.next_action(), None);
}

#[test]
fn initial_lifecycle_queue_stops_reading_at_the_capacity_boundary() {
    let ams = Ams::new(std::iter::repeat_n(HostEvent::Pause, MAX_LIFECYCLE_EVENTS)).unwrap();
    assert_eq!(ams.events.len(), MAX_LIFECYCLE_EVENTS);
    let mut visited = 0;
    let unbounded = std::iter::repeat(HostEvent::Pause).inspect(|_| visited += 1);
    assert_eq!(
        Ams::new(unbounded).unwrap_err().code(),
        "lifecycle-event-capacity"
    );
    assert_eq!(visited, MAX_LIFECYCLE_EVENTS + 1);
}

#[test]
fn unconditional_close_bypasses_full_lifecycle_queues() {
    for close in [
        HostEvent::Close,
        HostEvent::Destroy {
            unconditional: true,
        },
    ] {
        let mut ams = Ams::new([HostEvent::Launch]).unwrap();
        assert_eq!(ams.next_action(), Some(LifecycleAction::Start));
        for _ in 0..MAX_LIFECYCLE_EVENTS {
            ams.host_event(HostEvent::Pause).unwrap();
        }
        ams.resume_after_restore();
        ams.host_event(close).unwrap();
        assert!(ams.events.is_empty());
        ams.host_event(HostEvent::Resume).unwrap();
        assert!(ams.events.is_empty());
        assert_eq!(ams.next_action(), None);
        ams.callback_finished(LifecycleCallback::Start, LifecycleOutcome::Completed)
            .unwrap();
        assert_eq!(
            ams.next_action(),
            Some(LifecycleAction::Destroy {
                unconditional: true
            })
        );
        ams.callback_finished(LifecycleCallback::Destroy, LifecycleOutcome::Completed)
            .unwrap();
        ams.request_close();
        assert_eq!(ams.next_action(), None);
    }
}

#[test]
fn new_host_events_follow_a_restores_pending_resume() {
    let mut ams = Ams::new([]).unwrap();
    ams.state = LifecycleState::Paused;
    ams.resume_after_restore();
    ams.host_event(HostEvent::Pause).unwrap();
    assert_eq!(ams.next_action(), Some(LifecycleAction::Resume));
    ams.callback_finished(LifecycleCallback::Start, LifecycleOutcome::Completed)
        .unwrap();
    assert_eq!(ams.next_action(), Some(LifecycleAction::Pause));
    ams.callback_finished(LifecycleCallback::Pause, LifecycleOutcome::Completed)
        .unwrap();
    assert_eq!(ams.next_action(), None);
    assert_eq!(ams.state(), LifecycleState::Paused);
}

#[test]
fn host_close_supersedes_queued_start_and_guest_resume() {
    for close in [
        HostEvent::Close,
        HostEvent::Destroy {
            unconditional: true,
        },
    ] {
        let mut ams = Ams::new([HostEvent::Launch, HostEvent::Pause, HostEvent::Resume]).unwrap();
        ams.host_event(close).unwrap();
        assert_eq!(
            ams.next_action(),
            Some(LifecycleAction::Destroy {
                unconditional: true
            })
        );

        let mut ams = Ams::new([HostEvent::Launch]).unwrap();
        assert_eq!(ams.next_action(), Some(LifecycleAction::Start));
        ams.notify(MidletNotification::Paused);
        ams.host_event(close).unwrap();
        ams.notify(MidletNotification::ResumeRequested);
        assert_eq!(ams.next_action(), None);
        ams.callback_finished(LifecycleCallback::Start, LifecycleOutcome::Completed)
            .unwrap();
        assert_eq!(
            ams.next_action(),
            Some(LifecycleAction::Destroy {
                unconditional: true
            })
        );
    }
}

#[test]
fn host_close_survives_a_pending_conditional_destroy_refusal() {
    for close in [
        HostEvent::Close,
        HostEvent::Destroy {
            unconditional: true,
        },
    ] {
        for outcome in [
            LifecycleOutcome::Completed,
            LifecycleOutcome::StateChangeRejected,
        ] {
            let mut ams = Ams::new([HostEvent::Launch]).unwrap();
            assert_eq!(ams.next_action(), Some(LifecycleAction::Start));
            ams.callback_finished(LifecycleCallback::Start, LifecycleOutcome::Completed)
                .unwrap();
            ams.host_event(HostEvent::Destroy {
                unconditional: false,
            })
            .unwrap();
            assert_eq!(
                ams.next_action(),
                Some(LifecycleAction::Destroy {
                    unconditional: false
                })
            );
            ams.host_event(close).unwrap();
            assert_eq!(ams.next_action(), None);
            ams.callback_finished(LifecycleCallback::Destroy, outcome)
                .unwrap();
            if outcome == LifecycleOutcome::StateChangeRejected {
                assert_eq!(
                    ams.next_action(),
                    Some(LifecycleAction::Destroy {
                        unconditional: true
                    })
                );
                ams.callback_finished(LifecycleCallback::Destroy, LifecycleOutcome::Completed)
                    .unwrap();
            }
            assert_eq!(ams.state(), LifecycleState::Destroyed);
            assert_eq!(ams.next_action(), None);
        }
    }
}

#[test]
fn lifecycle_maps_resume_to_start_app() {
    let calls = [
        LifecycleAction::Start,
        LifecycleAction::Pause,
        LifecycleAction::Resume,
        LifecycleAction::Destroy {
            unconditional: true,
        },
    ]
    .map(lifecycle_call);
    assert_eq!(
        calls
            .iter()
            .map(|call| call.name.as_str())
            .collect::<Vec<_>>(),
        [
            "__amsStartApp",
            "__amsPauseApp",
            "__amsStartApp",
            "__amsDestroyApp"
        ]
    );
    assert_eq!(calls[3].arguments, [Value::Int(1)]);
}

#[test]
fn ams_applies_notifications_before_selecting_the_next_callback() {
    let mut ams = Ams::new([
        HostEvent::Launch,
        HostEvent::Pause,
        HostEvent::Resume,
        HostEvent::Close,
    ])
    .unwrap();
    assert_eq!(ams.next_action(), Some(LifecycleAction::Start));
    ams.callback_finished(LifecycleCallback::Start, LifecycleOutcome::Completed)
        .unwrap();
    ams.notify(MidletNotification::Paused);
    ams.notify(MidletNotification::ResumeRequested);
    assert_eq!(ams.next_action(), Some(LifecycleAction::Resume));
    ams.callback_finished(LifecycleCallback::Start, LifecycleOutcome::Completed)
        .unwrap();
    assert_eq!(ams.next_action(), Some(LifecycleAction::Pause));
    ams.callback_finished(LifecycleCallback::Pause, LifecycleOutcome::Completed)
        .unwrap();
    assert_eq!(ams.next_action(), Some(LifecycleAction::Resume));
    ams.callback_finished(LifecycleCallback::Start, LifecycleOutcome::Completed)
        .unwrap();
    assert_eq!(
        ams.next_action(),
        Some(LifecycleAction::Destroy {
            unconditional: true
        })
    );
    ams.callback_finished(LifecycleCallback::Destroy, LifecycleOutcome::Completed)
        .unwrap();
    assert_eq!(ams.state(), LifecycleState::Destroyed);
    assert_eq!(ams.next_action(), None);
}

#[test]
fn notify_destroyed_cancels_all_queued_callbacks() {
    let mut ams = Ams::new([HostEvent::Launch, HostEvent::Pause, HostEvent::Close]).unwrap();
    assert_eq!(ams.next_action(), Some(LifecycleAction::Start));
    ams.notify(MidletNotification::Destroyed);
    assert_eq!(ams.state(), LifecycleState::Destroyed);
    assert_eq!(ams.next_action(), None);
}

#[test]
fn suites_have_independent_lifecycle_state() {
    let mut first = Ams::new([HostEvent::Launch]).unwrap();
    let second = Ams::new([HostEvent::Launch]).unwrap();
    assert_eq!(first.next_action(), Some(LifecycleAction::Start));
    first.notify(MidletNotification::Destroyed);
    assert_eq!(first.state(), LifecycleState::Destroyed);
    assert_eq!(second.state(), LifecycleState::New);
}

#[test]
fn rejected_start_remains_paused_and_can_be_retried() {
    let mut ams = Ams::new([HostEvent::Launch, HostEvent::Resume]).unwrap();
    assert_eq!(ams.next_action(), Some(LifecycleAction::Start));
    ams.callback_finished(
        LifecycleCallback::Start,
        LifecycleOutcome::StateChangeRejected,
    )
    .unwrap();
    assert_eq!(ams.state(), LifecycleState::Paused);
    assert_eq!(ams.next_action(), Some(LifecycleAction::Resume));
}

#[test]
fn lifecycle_runtime_failure_forces_cleanup() {
    for (action, callback) in [
        (LifecycleAction::Start, LifecycleCallback::Start),
        (LifecycleAction::Pause, LifecycleCallback::Pause),
    ] {
        let mut ams = Ams::new([]).unwrap();
        let _ = ams.begin(action);
        ams.callback_finished(callback, LifecycleOutcome::RuntimeFailure)
            .unwrap();
        assert_eq!(
            ams.next_action(),
            Some(LifecycleAction::Destroy {
                unconditional: true
            })
        );
    }
}

#[test]
fn only_conditional_destroy_can_be_rejected() {
    let mut conditional = Ams::new([HostEvent::Launch]).unwrap();
    assert_eq!(conditional.next_action(), Some(LifecycleAction::Start));
    conditional
        .callback_finished(LifecycleCallback::Start, LifecycleOutcome::Completed)
        .unwrap();
    conditional
        .host_event(HostEvent::Destroy {
            unconditional: false,
        })
        .unwrap();
    assert_eq!(
        conditional.next_action(),
        Some(LifecycleAction::Destroy {
            unconditional: false
        })
    );
    conditional
        .callback_finished(
            LifecycleCallback::Destroy,
            LifecycleOutcome::StateChangeRejected,
        )
        .unwrap();
    assert_eq!(conditional.state(), LifecycleState::Active);

    let mut unconditional = Ams::new([HostEvent::Launch]).unwrap();
    assert_eq!(unconditional.next_action(), Some(LifecycleAction::Start));
    unconditional
        .callback_finished(LifecycleCallback::Start, LifecycleOutcome::Completed)
        .unwrap();
    unconditional
        .host_event(HostEvent::Destroy {
            unconditional: true,
        })
        .unwrap();
    assert_eq!(
        unconditional.next_action(),
        Some(LifecycleAction::Destroy {
            unconditional: true
        })
    );
    unconditional
        .callback_finished(
            LifecycleCallback::Destroy,
            LifecycleOutcome::StateChangeRejected,
        )
        .unwrap();
    assert_eq!(unconditional.state(), LifecycleState::Destroyed);
}
