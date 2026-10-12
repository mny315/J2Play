use super::*;

#[test]
#[allow(clippy::too_many_lines)]
fn stop_checkpoint_survives_worker_restart_and_resumes_the_saved_frame() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = crate::inspect_import(crate::ImportSource::new(
        None,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/java-me/conformance.jar"
        ))
        .to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(2)
    .unwrap();
    let entry = repository
        .commit_import(&prepared, crate::GameSettings::default())
        .unwrap();
    let (waker, wake) = mpsc::sync_channel(1);
    let mut worker = RuntimeWorker::spawn(repository.clone()).unwrap();
    worker
        .set_event_waker(move || {
            let _ = waker.try_send(());
        })
        .unwrap();
    worker
        .submit(command(SessionCommandKind::Start {
            entry_id: entry.id().into(),
            orientation: None,
        }))
        .unwrap();
    await_event(&mut worker, &wake, |event| {
        matches!(event, SessionEventKind::FrameAvailable)
    });
    let initial = worker.frames().take_latest().unwrap().unwrap();
    assert!(
        !root
            .join("runtime/resume")
            .join(entry.id())
            .join("automatic.save")
            .exists()
    );
    worker
        .submit(command(SessionCommandKind::Input(crate::InputEvent::Key {
            action: platform::HostAction::Fire,
            state: crate::KeyState::Pressed,
        })))
        .unwrap();
    await_event(&mut worker, &wake, |event| {
        matches!(event, SessionEventKind::FrameAvailable)
    });
    let saved = worker.frames().take_latest().unwrap().unwrap();
    assert_ne!(
        saved.pixels, initial.pixels,
        "the key must change the saved scene"
    );
    worker.submit(command(SessionCommandKind::Stop)).unwrap();
    await_event(&mut worker, &wake, |event| {
        matches!(event, SessionEventKind::Stopped)
    });
    let path = root
        .join("runtime/resume")
        .join(entry.id())
        .join("automatic.save");
    assert!(path.exists(), "Stop must publish a checkpoint");
    worker.shutdown().unwrap();
    drop(worker);

    let (waker, wake) = mpsc::sync_channel(1);
    let mut worker = RuntimeWorker::spawn(repository).unwrap();
    worker
        .set_event_waker(move || {
            let _ = waker.try_send(());
        })
        .unwrap();
    worker
        .submit(command(SessionCommandKind::Start {
            entry_id: entry.id().into(),
            orientation: None,
        }))
        .unwrap();
    let event = await_event(&mut worker, &wake, |event| {
        matches!(event, SessionEventKind::HostRequest(_))
    });
    let SessionEventKind::HostRequest(request) = event else {
        unreachable!()
    };
    assert!(matches!(
        request.kind,
        crate::HostRequestKind::ResumeGame {
            can_resume: true,
            ..
        }
    ));
    worker
        .submit(command(SessionCommandKind::ResolveHostRequest {
            request_id: request.request_id,
            decision: crate::HostDecision::ResumeGame {
                continue_game: true,
                remember: false,
            },
        }))
        .unwrap();
    await_event(&mut worker, &wake, |event| {
        matches!(event, SessionEventKind::FrameAvailable)
    });
    let resumed = worker.frames().take_latest().unwrap().unwrap();
    assert_eq!((resumed.width, resumed.height), (saved.width, saved.height));
    assert_eq!(resumed.pixels, saved.pixels);
    worker.submit(command(SessionCommandKind::Stop)).unwrap();
    await_event(&mut worker, &wake, |event| {
        matches!(event, SessionEventKind::Stopped)
    });
    worker.shutdown().unwrap();
    drop(worker);
}

fn await_event(
    worker: &mut RuntimeWorker,
    wake: &mpsc::Receiver<()>,
    matches: impl Fn(&SessionEventKind) -> bool,
) -> SessionEventKind {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        for event in worker.poll_events().unwrap() {
            match &event.kind {
                SessionEventKind::TerminalError {
                    technical_details, ..
                } => panic!("{technical_details}"),
                SessionEventKind::CheckpointSaveFailed { detail } => panic!("{detail}"),
                _ => {}
            }
            if matches(&event.kind) {
                return event.kind;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for checkpoint fixture"
        );
        let _ = wake.recv_timeout(Duration::from_millis(25));
    }
}
