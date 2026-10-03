use super::*;
use crate::{KeyState, PauseReason, PointerEvent};
use platform::HostAction;

mod audio;
mod events;
mod vibration;

fn command(attempt: u64, kind: SessionCommandKind) -> SessionCommand {
    SessionCommand {
        session_id: SessionId(1),
        attempt_id: AttemptId(attempt),
        kind,
    }
}

#[test]
fn priority_lane_precedes_normal_input() {
    let queue = CommandQueue::default();
    queue
        .push(command(
            1,
            SessionCommandKind::Input(InputEvent::Key {
                action: HostAction::Fire,
                state: KeyState::Pressed,
            }),
        ))
        .unwrap();
    queue
        .push(command(
            1,
            SessionCommandKind::SetPause {
                reason: PauseReason::Lifecycle,
                paused: true,
            },
        ))
        .unwrap();
    assert!(matches!(
        queue.wait_next(Duration::ZERO).unwrap().unwrap().kind,
        SessionCommandKind::SetPause { .. }
    ));
}

#[test]
fn lifecycle_pause_cannot_overtake_a_queued_start() {
    let queue = CommandQueue::default();
    queue
        .push(command(
            1,
            SessionCommandKind::Start {
                entry_id: "entry-1".to_owned(),
                orientation: None,
            },
        ))
        .unwrap();
    queue
        .push(command(
            1,
            SessionCommandKind::SetPause {
                reason: PauseReason::Lifecycle,
                paused: true,
            },
        ))
        .unwrap();
    assert!(matches!(
        queue.wait_next(Duration::ZERO).unwrap().unwrap().kind,
        SessionCommandKind::Start { .. }
    ));
    assert!(matches!(
        queue.wait_next(Duration::ZERO).unwrap().unwrap().kind,
        SessionCommandKind::SetPause { .. }
    ));
}

#[test]
fn input_release_barrier_discards_queued_input_but_not_other_commands() {
    let queue = CommandQueue::default();
    queue
        .push(command(
            1,
            SessionCommandKind::Input(InputEvent::Key {
                action: HostAction::Fire,
                state: KeyState::Pressed,
            }),
        ))
        .unwrap();
    queue
        .push(command(
            1,
            SessionCommandKind::SetFastForward { enabled: true },
        ))
        .unwrap();
    queue
        .push(command(1, SessionCommandKind::ReleaseAllInput))
        .unwrap();

    assert!(matches!(
        queue.wait_next(Duration::ZERO).unwrap().unwrap().kind,
        SessionCommandKind::ReleaseAllInput
    ));
    assert!(matches!(
        queue.wait_next(Duration::ZERO).unwrap().unwrap().kind,
        SessionCommandKind::SetFastForward { enabled: true }
    ));
    assert!(queue.wait_next(Duration::ZERO).unwrap().is_none());
}

#[test]
fn rapid_pause_transitions_keep_the_release_barrier_and_final_state() {
    let queue = CommandQueue::default();
    for paused in [true, false, true] {
        queue
            .push(command(
                1,
                SessionCommandKind::SetPause {
                    reason: PauseReason::User,
                    paused,
                },
            ))
            .unwrap();
    }
    for paused in [true, false, true] {
        assert_eq!(
            queue.pop_priority().unwrap().unwrap().kind,
            SessionCommandKind::SetPause {
                reason: PauseReason::User,
                paused,
            }
        );
    }
    assert!(queue.pop_priority().unwrap().is_none());
}

#[test]
fn repeated_identical_pause_commands_remain_bounded() {
    let queue = CommandQueue::default();
    for _ in 0..PRIORITY_COMMAND_CAPACITY * 2 {
        queue
            .push(command(
                1,
                SessionCommandKind::SetPause {
                    reason: PauseReason::User,
                    paused: true,
                },
            ))
            .unwrap();
    }
    assert!(queue.pop_priority().unwrap().is_some());
    assert!(queue.pop_priority().unwrap().is_none());
}

#[test]
fn stale_stop_cannot_erase_a_newer_attempts_cancellation_deadline() {
    let urgent = UrgentState::default();
    let now = Instant::now();
    urgent.note_stop_at(AttemptId(2), now);
    urgent.note_stop_at(AttemptId(1), now + Duration::from_secs(1));
    assert!(urgent.stop_requested(AttemptId(2)));
    assert!(urgent.stop_cancellation_due_at(AttemptId(2), now + ATTEMPT_STOP_CANCEL_DEADLINE));
}

#[test]
fn pointer_coalescing_cannot_cross_a_release_and_new_press() {
    let queue = CommandQueue::default();
    let pointer = |phase, x| {
        command(
            1,
            SessionCommandKind::Input(InputEvent::Pointer(PointerEvent {
                touch_id: 7,
                phase,
                canvas_x: x,
                canvas_y: 0,
            })),
        )
    };
    for _ in 0..NORMAL_COMMAND_CAPACITY - 2 {
        queue.push(pointer(PointerPhase::Dragged, 10)).unwrap();
    }
    queue.push(pointer(PointerPhase::Released, 10)).unwrap();
    queue.push(pointer(PointerPhase::Pressed, 20)).unwrap();
    assert!(queue.push(pointer(PointerPhase::Dragged, 30)).is_err());
    for _ in 0..NORMAL_COMMAND_CAPACITY - 2 {
        assert_eq!(
            queue.pop_normal().unwrap().unwrap(),
            pointer(PointerPhase::Dragged, 10)
        );
    }
    assert_eq!(
        queue.pop_normal().unwrap().unwrap(),
        pointer(PointerPhase::Released, 10)
    );
    assert_eq!(
        queue.pop_normal().unwrap().unwrap(),
        pointer(PointerPhase::Pressed, 20)
    );
    assert!(queue.pop_normal().unwrap().is_none());
}

#[test]
fn repeated_stop_does_not_extend_forced_cancellation_deadline() {
    let urgent = UrgentState::default();
    let attempt_id = AttemptId(7);
    let now = Instant::now();
    urgent.note_stop_at(
        attempt_id,
        now.checked_sub(ATTEMPT_STOP_CANCEL_DEADLINE).unwrap(),
    );
    urgent.note_stop_at(attempt_id, now);
    assert!(urgent.stop_cancellation_due_at(attempt_id, now));
    assert!(!urgent.stop_cancellation_due_at(AttemptId(8), now));
}
