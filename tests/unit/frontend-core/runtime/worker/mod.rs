use super::*;
use crate::test_storage::Scratch;
use crate::{AttemptId, Frame, SessionId, VibrationEffect};
use natives::VibrationRequest;
use std::time::{Duration, Instant};

mod checkpoint;

fn worker() -> (Scratch, RuntimeWorker) {
    let scratch = Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    (scratch, RuntimeWorker::spawn(repository).unwrap())
}

fn command(kind: SessionCommandKind) -> SessionCommand {
    SessionCommand {
        session_id: SessionId(1),
        attempt_id: AttemptId(1),
        kind,
    }
}

#[test]
fn suspension_acknowledgement_belongs_to_one_cycle_and_destroy_is_final() {
    let signal = PlatformLifecycleSignal::default();
    assert!(!signal.suspension_acknowledged());
    signal.set_suspended(true);
    let first = signal.snapshot();
    assert!(!signal.suspension_acknowledged());
    signal.acknowledge_suspension(first);
    assert!(signal.suspension_acknowledged());
    signal.set_suspended(false);
    signal.set_suspended(true);
    signal.acknowledge_suspension(first);
    assert!(!signal.suspension_acknowledged());
    signal.acknowledge_suspension(signal.snapshot());
    assert!(signal.suspension_acknowledged());
    signal.mark_destroyed();
    signal.set_suspended(false);
    assert!(signal.suspended());
    assert!(signal.destroyed());
    assert!(!signal.suspension_acknowledged());
}

#[test]
fn a_suspended_worker_acknowledges_without_starting_a_queued_guest() {
    let scratch = Scratch::new();
    let signal = PlatformLifecycleSignal::initially_suspended();
    let mut worker = RuntimeWorker::spawn_with_lifecycle(
        LibraryRepository::open(&scratch.0).unwrap(),
        signal.clone(),
    )
    .unwrap();
    worker
        .submit(command(SessionCommandKind::Start {
            entry_id: "must-not-load".into(),
            orientation: None,
        }))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    while !signal.suspension_acknowledged() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert!(signal.suspension_acknowledged());
    assert!(worker.poll_events().unwrap().is_empty());
    assert_eq!(worker.audio.active_generation().unwrap(), None);
    worker.shutdown().unwrap();
    assert!(worker.thread.is_none());
}

#[test]
fn an_audio_failure_cannot_block_stop_or_vibration_cancellation() {
    let (_scratch, mut worker) = worker();
    worker
        .vibration
        .activate(SessionId(1), AttemptId(1))
        .unwrap();
    worker
        .vibration
        .publish(VibrationEffect {
            session_id: SessionId(1),
            attempt_id: AttemptId(1),
            request: VibrationRequest::Continuous { level: None },
        })
        .unwrap();
    worker.audio.poison_for_test();

    let error = worker
        .submit(command(SessionCommandKind::Stop))
        .unwrap_err();
    assert_eq!(error.code(), "audio-lock");
    assert!(worker.commands.urgent().stop_requested(AttemptId(1)));
    assert_eq!(
        worker.vibration.take_latest().unwrap().unwrap().request,
        VibrationRequest::Stop
    );
    assert_eq!(worker.shutdown().unwrap_err().code(), "audio-lock");
    assert!(worker.thread.is_none());
}

#[test]
fn a_worker_failure_wakes_the_ui_and_is_reported_once_before_join() {
    let (_scratch, mut worker) = worker();
    let (waker, wake) = mpsc::sync_channel(1);
    worker
        .set_event_waker(move || {
            let _ = waker.try_send(());
        })
        .unwrap();
    worker.audio.poison_for_test();
    worker
        .submit(command(SessionCommandKind::Start {
            entry_id: "fixture".to_owned(),
            orientation: None,
        }))
        .unwrap();
    wake.recv_timeout(Duration::from_secs(2))
        .expect("worker completion must wake the UI");
    let error = worker.poll_events().unwrap_err();
    assert_eq!(error.code(), "vm-worker-stopped");
    assert!(error.message().contains("audio-lock"));
    assert!(worker.poll_events().unwrap().is_empty());
    assert_eq!(
        worker
            .submit(command(SessionCommandKind::Stop))
            .unwrap_err()
            .code(),
        "vm-worker-stopped"
    );
    assert_eq!(worker.shutdown().unwrap_err().code(), "audio-lock");
    assert!(worker.thread.is_none());
}

#[test]
fn cancelled_heap_recovery_finishes_without_a_diagnostic_or_new_prompt() {
    let scratch = Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    for reason in ["stop", "shutdown", "forced", "unrelated", "none"] {
        let commands = CommandQueue::default();
        match reason {
            "stop" => commands.push(command(SessionCommandKind::Stop)).unwrap(),
            "shutdown" => commands.request_shutdown(),
            "forced" => commands.urgent().force_cancel(),
            "unrelated" => commands
                .push(SessionCommand {
                    attempt_id: AttemptId(2),
                    ..command(SessionCommandKind::Stop)
                })
                .unwrap(),
            _ => {}
        }
        let events = EventMailbox::default();
        // A failed recovery read must remain visible unless this attempt was
        // cancelled. An unrelated attempt's Stop cannot hide that failure.
        let start = command(SessionCommandKind::Start {
            entry_id: "missing-entry".into(),
            orientation: None,
        });
        publish_attempt_result(
            &repository,
            "missing-entry",
            None,
            &start,
            &events,
            commands.urgent(),
            Err(EmuError::new(
                Category::Vm,
                vm::MANAGED_HEAP_LIMIT_CODE,
                "fixture",
            )),
        )
        .unwrap();
        let events = events.take_all().unwrap();
        assert!(
            events
                .iter()
                .all(|event| event.session_id == start.session_id
                    && event.attempt_id == start.attempt_id)
        );
        if matches!(reason, "unrelated" | "none") {
            assert_eq!(events.len(), 2);
            assert!(
                events
                    .iter()
                    .any(|event| matches!(event.kind, SessionEventKind::Diagnostic { .. }))
            );
            assert!(
                events.iter().any(|event| matches!(&event.kind, SessionEventKind::ManagedHeapLimit { candidate_profile_ids, .. } if candidate_profile_ids.is_empty()))
            );
        } else {
            assert_eq!(events.len(), 1);
            assert!(matches!(events[0].kind, SessionEventKind::Stopped));
        }
    }
}

#[test]
fn output_cleanup_releases_frames_and_preserves_stop_despite_an_audio_failure() {
    let audio = AudioMailbox::default();
    let telemetry = LatestTelemetryMailbox::default();
    let frames = LatestFrameMailbox::default();
    let vibration = VibrationMailbox::default();
    frames.activate(SessionId(1), AttemptId(1)).unwrap();
    vibration.activate(SessionId(1), AttemptId(1)).unwrap();
    let frame = Arc::new(Frame::new(SessionId(1), AttemptId(1), 1, 1, Arc::from([0])).unwrap());
    let retained = Arc::downgrade(&frame);
    assert!(frames.publish(frame).unwrap());
    audio.poison_for_test();

    for _ in 0..2 {
        assert_eq!(
            clear_outputs(&audio, &telemetry, &frames, &vibration)
                .unwrap_err()
                .code(),
            "audio-lock"
        );
    }
    assert!(retained.upgrade().is_none());
    assert!(frames.take_latest().unwrap().is_none());
    assert_eq!(
        vibration.take_latest().unwrap().unwrap().request,
        VibrationRequest::Stop
    );
    assert!(
        !frames
            .publish(Arc::new(
                Frame::new(SessionId(1), AttemptId(1), 1, 1, Arc::from([0]),).unwrap()
            ))
            .unwrap()
    );
}

#[test]
fn a_clean_shutdown_rejects_future_commands() {
    let (_scratch, mut worker) = worker();
    let start = Instant::now();
    worker.shutdown().unwrap();
    assert!(start.elapsed() < GRACEFUL_SHUTDOWN_DEADLINE + FORCED_SHUTDOWN_DEADLINE);
    assert!(worker.poll_events().unwrap().is_empty());
    assert_eq!(
        worker
            .submit(command(SessionCommandKind::Stop))
            .unwrap_err()
            .code(),
        "vm-worker-stopped"
    );
}

#[test]
fn shutdown_deadline_includes_thread_exit_after_completion_was_reported() {
    let (_scratch, mut worker) = worker();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = std::sync::Mutex::new(release_rx);
    worker
        .set_event_waker(move || {
            let _ = entered_tx.try_send(());
            // A platform wake callback may still be running after the worker
            // publishes its completion. Keep it alive until shutdown returns.
            let _ = release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5));
        })
        .unwrap();
    worker.audio.poison_for_test();
    worker
        .submit(command(SessionCommandKind::Start {
            entry_id: "fixture".into(),
            orientation: None,
        }))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        worker.poll_events().unwrap_err().code(),
        "vm-worker-stopped"
    );
    let started = Instant::now();
    let result = worker.shutdown();
    let elapsed = started.elapsed();
    let retained = worker.thread.is_some();
    release_tx.send(()).unwrap_or(());
    let retry = worker.shutdown();
    assert_eq!(result.unwrap_err().code(), "vm-shutdown-deadline");
    assert!(
        elapsed < Duration::from_secs(4),
        "shutdown took {elapsed:?}"
    );
    assert!(retained, "a timed-out thread must retain its join handle");
    assert_eq!(retry.unwrap_err().code(), "audio-lock");
    assert!(worker.thread.is_none());
}
