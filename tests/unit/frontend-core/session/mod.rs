use super::*;
use platform::VmDebugStats;
use std::sync::Arc;

fn started_controller() -> (SessionController, SessionCommand) {
    let mut controller = SessionController::default();
    let start = controller.start("entry-1").unwrap();
    assert!(controller.apply_event(&SessionEvent {
        session_id: start.session_id,
        attempt_id: start.attempt_id,
        kind: SessionEventKind::Started,
    }));
    (controller, start)
}

#[test]
fn lifecycle_resume_does_not_clear_user_pause() {
    let (mut controller, _) = started_controller();
    controller.set_paused(PauseReason::User, true).unwrap();
    controller.set_paused(PauseReason::Lifecycle, true).unwrap();
    controller
        .set_paused(PauseReason::Lifecycle, false)
        .unwrap();
    assert_eq!(
        controller.state(),
        SessionState::Paused {
            user: true,
            lifecycle: false
        }
    );
    controller.set_paused(PauseReason::User, false).unwrap();
    assert_eq!(controller.state(), SessionState::Running);
}

#[test]
fn stale_attempt_events_and_frames_are_discarded() {
    let (mut controller, first) = started_controller();
    assert!(controller.apply_event(&SessionEvent::terminal_error(
        first.session_id,
        first.attempt_id,
        "Launch failed",
        "heap limit",
    )));
    let retry = controller.retry().unwrap();
    let stale = SessionEvent {
        session_id: first.session_id,
        attempt_id: first.attempt_id,
        kind: SessionEventKind::Stopped,
    };
    assert!(!controller.apply_event(&stale));
    assert_eq!(controller.state(), SessionState::Starting);

    let mailbox = LatestFrameMailbox::default();
    mailbox
        .activate(retry.session_id, retry.attempt_id)
        .unwrap();
    let old =
        Arc::new(Frame::new(first.session_id, first.attempt_id, 1, 1, Arc::from([0_u32])).unwrap());
    assert!(!mailbox.publish(old).unwrap());
    assert!(mailbox.take_latest().unwrap().is_none());
}

#[test]
fn transient_orientation_survives_a_retry_attempt() {
    let mut controller = SessionController::default();
    let first = controller
        .start_with_orientation("entry-1", Some(launch::CanvasOrientation::Landscape))
        .unwrap();
    assert!(controller.apply_event(&SessionEvent::terminal_error(
        first.session_id,
        first.attempt_id,
        "Launch failed",
        "heap limit",
    )));

    let retry = controller.retry().unwrap();
    assert!(matches!(
        retry.kind,
        SessionCommandKind::Start {
            orientation: Some(launch::CanvasOrientation::Landscape),
            ..
        }
    ));
}

#[test]
fn fast_forward_is_an_explicit_running_session_command() {
    let (mut controller, start) = started_controller();
    assert_eq!(
        controller.set_fast_forward(true).unwrap(),
        SessionCommand {
            session_id: start.session_id,
            attempt_id: start.attempt_id,
            kind: SessionCommandKind::SetFastForward { enabled: true },
        }
    );

    controller.set_paused(PauseReason::User, true).unwrap();
    assert_eq!(
        controller.set_fast_forward(false).unwrap_err().code(),
        "session-fast-forward-state"
    );
}

#[test]
fn latest_frame_coalesces_intermediate_publications() {
    let (controller, start) = started_controller();
    assert_eq!(controller.state(), SessionState::Running);
    let mailbox = LatestFrameMailbox::default();
    mailbox
        .activate(start.session_id, start.attempt_id)
        .unwrap();
    for pixel in [1_u32, 2, 3] {
        let frame = Arc::new(
            Frame::new(start.session_id, start.attempt_id, 1, 1, Arc::from([pixel])).unwrap(),
        );
        assert!(mailbox.publish(frame).unwrap());
    }
    assert_eq!(mailbox.take_latest().unwrap().unwrap().pixels[0], 3);
}

#[test]
fn telemetry_is_opt_in_coalesced_and_generation_scoped() {
    let (controller, start) = started_controller();
    assert_eq!(controller.state(), SessionState::Running);
    let mailbox = LatestTelemetryMailbox::default();
    mailbox
        .activate(start.session_id, start.attempt_id)
        .unwrap();
    let sample = |attempt_id, instructions| RuntimeTelemetry {
        session_id: start.session_id,
        attempt_id,
        stats: VmDebugStats {
            instructions,
            ..VmDebugStats::default()
        },
    };

    assert!(!mailbox.publish(sample(start.attempt_id, 1)).unwrap());
    assert!(mailbox.take_latest().unwrap().is_none());

    mailbox.set_enabled(true).unwrap();
    assert!(!mailbox.publish(sample(AttemptId(99), 2)).unwrap());
    assert!(mailbox.publish(sample(start.attempt_id, 3)).unwrap());
    assert!(mailbox.publish(sample(start.attempt_id, 4)).unwrap());
    assert_eq!(
        mailbox.take_latest().unwrap().unwrap().stats.instructions,
        4
    );

    mailbox.set_enabled(false).unwrap();
    assert!(!mailbox.enabled());
    assert!(!mailbox.publish(sample(start.attempt_id, 5)).unwrap());
    assert!(mailbox.take_latest().unwrap().is_none());
}

#[test]
fn frame_maps_only_the_declared_guest_canvas() {
    let frame = Frame::with_canvas_region(
        SessionId(1),
        AttemptId(1),
        6,
        8,
        platform::LogicalRect {
            x: 1,
            y: 2,
            width: 4,
            height: 5,
        },
        Arc::from(vec![0_u32; 48]),
    )
    .unwrap();
    assert_eq!(frame.canvas_point(1, 2), Some((0, 0)));
    assert_eq!(frame.canvas_point(4, 6), Some((3, 4)));
    assert_eq!(frame.canvas_point(0, 2), None);
    assert_eq!(frame.canvas_point(5, 6), None);
}

#[test]
fn terminal_errors_bound_both_messages_and_preserve_the_attempt_identity() {
    let event = SessionEvent::terminal_error(
        SessionId(7),
        AttemptId(9),
        &"Я".repeat(MAX_USER_ERROR_BYTES),
        &"🙂".repeat(MAX_TECHNICAL_ERROR_BYTES),
    );
    assert_eq!(event.session_id, SessionId(7));
    assert_eq!(event.attempt_id, AttemptId(9));
    let SessionEventKind::TerminalError {
        user_message,
        technical_details,
    } = event.kind
    else {
        panic!("terminal_error must create a terminal event");
    };
    for (text, maximum) in [
        (user_message, MAX_USER_ERROR_BYTES),
        (technical_details, MAX_TECHNICAL_ERROR_BYTES),
    ] {
        assert!(text.len() <= maximum);
        assert!(text.ends_with('…'));
    }
}
