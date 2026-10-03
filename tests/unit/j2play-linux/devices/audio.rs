use super::*;
use crate::test_storage::Scratch;
use frontend_core::{
    GameSettings, ImportSource, LibraryRepository, RuntimeWorker, SessionController,
    SessionEventKind, inspect_import,
};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires a real audio server/device; run separately with PulseAudio and PipeWire"]
fn audio_device_plays_midlet_pcm_and_stops_on_suspend() {
    let scratch = Scratch::new();
    let repository = LibraryRepository::open(scratch.0.join("library")).unwrap();
    let prepared = inspect_import(ImportSource::new(
        None,
        include_bytes!("../../../fixtures/java-me/conformance.jar").to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(6)
    .unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let lifecycle = PlatformLifecycleSignal::default();
    let mut worker = RuntimeWorker::spawn_with_lifecycle(repository, lifecycle.clone()).unwrap();
    let sdl = sdl2::init().unwrap();
    let input = Input::default();
    let mut output = Output::new(
        &sdl,
        worker.audio().clone(),
        lifecycle.clone(),
        input.clone(),
    );
    let mut session = SessionController::default();
    worker.submit(session.start(entry.id()).unwrap()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while output.played.load(Ordering::Relaxed) == 0 {
        output.pump().unwrap();
        for event in worker.poll_events().unwrap() {
            assert!(
                !matches!(event.kind, SessionEventKind::TerminalError { .. }),
                "{event:?}"
            );
            session.apply_event(&event);
        }
        assert!(
            input.poll_error().is_none(),
            "audio adapter reported an error"
        );
        assert!(
            Instant::now() < deadline,
            "no non-silent PCM reached the native callback"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(output.device.is_some());
    eprintln!(
        "audio fixture: driver={}, played={} frames",
        output.subsystem.as_ref().unwrap().current_audio_driver(),
        output.played.load(Ordering::Relaxed)
    );
    lifecycle.set_suspended(true);
    output.pump().unwrap();
    let stopped_at = output.played.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(output.played.load(Ordering::Relaxed), stopped_at);
    assert_eq!(
        output.device.as_ref().unwrap().status(),
        sdl2::audio::AudioStatus::Paused
    );
    worker.shutdown().unwrap();
    output.stop();
    assert!(output.device.is_none());
}
