use super::*;

#[derive(Default)]
struct FailingSink {
    writes: usize,
}

impl AudioSink for FailingSink {
    fn write(&mut self, _samples: &[i16]) -> Result<(), EmuError> {
        self.writes += 1;
        Err(media_error("fixture-sink", "injected audio output failure"))
    }
}

fn started_player<S: AudioSink>(runtime: &mut Runtime<S>) -> u64 {
    let handle = runtime
        .create_from_bytes("audio/wav", &wav(&[1000; 128]))
        .unwrap();
    runtime.start_exclusive(handle, 0).unwrap();
    handle
}

#[test]
fn closing_after_failed_audio_output_releases_resources_and_remains_idempotent() {
    let mut runtime = Runtime::new(
        FailingSink::default(),
        Limits {
            max_players: 1,
            max_total_pcm_frames: 128,
            max_pending_events: 1,
            ..Limits::default()
        },
    );
    let first = started_player(&mut runtime);
    assert_eq!(
        runtime.close(first, 1000).unwrap_err().code(),
        "fixture-sink"
    );
    let closed = runtime.player(first).unwrap();
    assert_eq!(closed.state, PlayerState::Closed);
    assert!(closed.clip.is_none());
    assert!(matches!(closed.encoded, EncodedMedia::Closed));
    assert_eq!(closed.events.len(), 1);
    assert_eq!(closed.events[0].kind, PlayerEventKind::Closed);
    assert_eq!(runtime.total_pcm_frames, 0);
    assert_eq!(runtime.total_encoded_bytes, 0);
    assert_eq!(runtime.exclusive_player, None);

    let second = started_player(&mut runtime);
    runtime.close(first, 1000).unwrap();
    assert_eq!(runtime.sink().writes, 1);
    assert_eq!(runtime.player(second).unwrap().state, PlayerState::Started);
    assert_eq!(runtime.total_pcm_frames, 128);
    assert_eq!(runtime.exclusive_player, Some(second));
    assert_eq!(
        runtime.close(second, 1000).unwrap_err().code(),
        "fixture-sink"
    );
    assert_eq!(runtime.total_pcm_frames, 0);
    assert_eq!(runtime.total_encoded_bytes, 0);
}

#[test]
fn excessive_time_advance_cannot_prevent_close_or_deallocation() {
    for close in [false, true] {
        let mut runtime = Runtime::new(
            NullAudioSink::default(),
            Limits {
                max_tick_frames: 1,
                ..Limits::default()
            },
        );
        let handle = started_player(&mut runtime);
        let error = if close {
            runtime.close(handle, i64::MAX)
        } else {
            runtime.deallocate(handle, i64::MAX)
        }
        .unwrap_err();
        assert_eq!(error.code(), "media-time-limit");
        let player = runtime.player(handle).unwrap();
        assert_eq!(
            player.state,
            if close {
                PlayerState::Closed
            } else {
                PlayerState::Realized
            }
        );
        assert_eq!(runtime.exclusive_player, None);
        assert_eq!(runtime.sink().frames_written(), 0);
        assert_eq!(runtime.total_pcm_frames, if close { 0 } else { 128 });
        if !close {
            runtime.start(handle, i64::MAX).unwrap();
            assert_eq!(runtime.player(handle).unwrap().state, PlayerState::Started);
        }
    }
}

#[test]
fn deallocation_releases_playback_despite_sink_failure_or_full_event_queue() {
    for queue_full in [false, true] {
        let mut runtime = Runtime::new(
            FailingSink::default(),
            Limits {
                max_pending_events: if queue_full { 1 } else { 8 },
                ..Limits::default()
            },
        );
        let handle = started_player(&mut runtime);
        let error = runtime
            .deallocate(handle, if queue_full { 0 } else { 1000 })
            .unwrap_err();
        assert_eq!(
            error.code(),
            if queue_full {
                "event-queue-limit"
            } else {
                "fixture-sink"
            }
        );
        let player = runtime.player(handle).unwrap();
        assert_eq!(player.state, PlayerState::Realized);
        assert_eq!(player.events.back().unwrap().kind, PlayerEventKind::Stopped);
        assert_eq!(runtime.exclusive_player, None);
        assert_eq!(runtime.total_pcm_frames, 128);
        runtime.deallocate(handle, 1000).unwrap();
        while runtime.next_event(handle, 1000).unwrap().is_some() {}
        runtime.start(handle, 1000).unwrap();
        assert_eq!(runtime.player(handle).unwrap().state, PlayerState::Started);
    }
}
