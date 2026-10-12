use super::*;

#[test]
fn platform_cancellation_precedes_stop_and_survives_full_request_and_diagnostic_queues() {
    let events = EventMailbox::default();
    let event = |kind| SessionEvent {
        session_id: SessionId(1),
        attempt_id: AttemptId(1),
        kind,
    };
    for _ in 0..HOST_REQUEST_CAPACITY {
        events
            .publish(event(SessionEventKind::HostRequestClosed { request_id: 1 }))
            .unwrap();
    }
    for index in 0..DIAGNOSTIC_CAPACITY * 2 {
        events
            .publish(event(SessionEventKind::Diagnostic {
                code: "fixture".into(),
                detail: index.to_string(),
                repeated: 1,
            }))
            .unwrap();
    }
    for _ in 0..1000 {
        events
            .publish(event(SessionEventKind::PlatformRequestCancelled))
            .unwrap();
    }
    events.publish(event(SessionEventKind::Stopped)).unwrap();
    let output = events.take_all().unwrap();
    assert_eq!(output[0].kind, SessionEventKind::PlatformRequestCancelled);
    assert_eq!(output[1].kind, SessionEventKind::Stopped);
    assert_eq!(
        output.len(),
        2 + HOST_REQUEST_CAPACITY + DIAGNOSTIC_CAPACITY
    );
    assert!(events.take_all().unwrap().is_empty());
}

#[test]
fn diagnostic_repeats_coalesce_without_displacing_terminal_error() {
    let events = EventMailbox::default();
    for _ in 0..100 {
        events
            .publish(SessionEvent {
                session_id: SessionId(1),
                attempt_id: AttemptId(1),
                kind: SessionEventKind::Diagnostic {
                    code: "same".to_owned(),
                    detail: "detail".to_owned(),
                    repeated: 1,
                },
            })
            .unwrap();
    }
    events
        .publish(SessionEvent::terminal_error(
            SessionId(1),
            AttemptId(1),
            "Stopped",
            "failure",
        ))
        .unwrap();
    let output = events.take_all().unwrap();
    assert!(
        output
            .iter()
            .any(|event| matches!(event.kind, SessionEventKind::TerminalError { .. }))
    );
    assert!(output.iter().any(|event| matches!(
        event.kind,
        SessionEventKind::Diagnostic { repeated: 100, .. }
    )));
}

#[test]
fn checkpoint_failure_reaches_the_session_before_stop_despite_diagnostic_overflow() {
    let mut session = crate::SessionController::default();
    let start = session.start("fixture").unwrap();
    let events = EventMailbox::default();
    let event = |kind| SessionEvent {
        session_id: start.session_id,
        attempt_id: start.attempt_id,
        kind,
    };
    events
        .publish(event(SessionEventKind::CheckpointSaveFailed {
            detail: "Fixture write failure".into(),
        }))
        .unwrap();
    for index in 0..DIAGNOSTIC_CAPACITY * 2 {
        events
            .publish(event(SessionEventKind::Diagnostic {
                code: "fixture".into(),
                detail: index.to_string(),
                repeated: 1,
            }))
            .unwrap();
    }
    session.stop().unwrap();
    events.publish(event(SessionEventKind::Stopped)).unwrap();
    let mut failure_seen = false;
    for event in events.take_all().unwrap() {
        if session.apply_event(&event) {
            match event.kind {
                SessionEventKind::CheckpointSaveFailed { .. } => {
                    failure_seen = true;
                }
                SessionEventKind::Stopped => assert!(failure_seen, "Stop hid the save failure"),
                _ => {}
            }
        }
    }
    assert!(failure_seen);
    assert_eq!(session.state(), crate::SessionState::Idle);
}

#[test]
fn launch_plan_survives_diagnostics_and_precedes_started_and_frames() {
    let plan = crate::inspect_import(crate::ImportSource::new(
        None,
        include_bytes!("../../../fixtures/java-me/conformance.jar").to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap()
    .launch_plan(&crate::GameSettings::default())
    .unwrap();
    let events = EventMailbox::default();
    for kind in [
        SessionEventKind::LaunchPrepared(Box::new(plan)),
        SessionEventKind::Started,
        SessionEventKind::FrameAvailable,
    ] {
        events
            .publish(SessionEvent {
                session_id: SessionId(1),
                attempt_id: AttemptId(1),
                kind,
            })
            .unwrap();
    }
    for index in 0..DIAGNOSTIC_CAPACITY * 2 {
        events
            .publish(SessionEvent {
                session_id: SessionId(1),
                attempt_id: AttemptId(1),
                kind: SessionEventKind::Diagnostic {
                    code: "fixture".into(),
                    detail: index.to_string(),
                    repeated: 1,
                },
            })
            .unwrap();
    }
    let output = events.take_all().unwrap();
    assert!(matches!(
        output[0].kind,
        SessionEventKind::LaunchPrepared(_)
    ));
    assert!(matches!(output[1].kind, SessionEventKind::Started));
    assert!(matches!(output[2].kind, SessionEventKind::FrameAvailable));
    assert_eq!(output.len(), DIAGNOSTIC_CAPACITY + 3);
    assert!(events.take_all().unwrap().is_empty());
}

#[test]
fn publishing_runtime_event_wakes_bound_event_loop() {
    let events = EventMailbox::default();
    let wake_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = Arc::clone(&wake_count);
    events
        .set_waker(Arc::new(move || {
            observed.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    events
        .publish(SessionEvent {
            session_id: SessionId(1),
            attempt_id: AttemptId(1),
            kind: SessionEventKind::FrameAvailable,
        })
        .unwrap();
    assert_eq!(wake_count.load(Ordering::Relaxed), 1);
    assert!(events.set_waker(Arc::new(|| {})).is_err());
}
