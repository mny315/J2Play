use super::*;

#[test]
fn aggregate_encoded_and_decoded_media_are_bounded_and_released() {
    let limits = Limits {
        max_input_bytes: 8,
        max_total_input_bytes: 5,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    let first = runtime.create_from_bytes("audio/mpeg", b"abc").unwrap();
    assert_eq!(
        runtime
            .create_from_bytes("audio/mpeg", b"def")
            .unwrap_err()
            .code(),
        "media-limit"
    );
    runtime.close(first, 0).unwrap();
    runtime.create_from_bytes("audio/mpeg", b"def").unwrap();

    let limits = Limits {
        max_pcm_frames: 8,
        max_total_pcm_frames: 5,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    let encoded = wav(&[1_000; 4]);
    let first = runtime.create_from_bytes("audio/x-wav", &encoded).unwrap();
    let second = runtime.create_from_bytes("audio/x-wav", &encoded).unwrap();
    runtime.realize(first).unwrap();
    assert_eq!(runtime.realize(second).unwrap_err().code(), "media-limit");
    assert_eq!(runtime.state(second, 0).unwrap(), PlayerState::Unrealized);
    runtime.close(first, 0).unwrap();
    runtime.realize(second).unwrap();
    assert_eq!(runtime.state(second, 0).unwrap(), PlayerState::Realized);
}

#[test]
fn single_tones_share_and_release_the_aggregate_pcm_budget() {
    let limits = Limits {
        max_pcm_frames: 100,
        max_total_pcm_frames: 22,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);

    runtime.play_tone(69, 1, 50, 0).unwrap();
    assert_eq!(runtime.total_pcm_frames, 22);
    runtime.play_tone(72, 1, 50, 0).unwrap();
    assert_eq!(runtime.total_pcm_frames, 22);
    assert_eq!(
        runtime.play_tone(72, 2, 50, 0).unwrap_err().code(),
        "media-limit"
    );
    runtime.tick(2_000).unwrap();
    assert_eq!(runtime.total_pcm_frames, 0);
}

#[test]
fn unreachable_player_sweep_releases_resources_and_handle_history() {
    let limits = Limits {
        max_players: 2,
        max_started_players: 1,
        max_handle_history: 2,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    let clip = wav(&[1_000; 22]);
    let first = runtime.create_from_bytes("audio/x-wav", &clip).unwrap();
    let second = runtime.create_from_bytes("audio/x-wav", &clip).unwrap();
    runtime.start(first, 0).unwrap();
    assert!(runtime.total_pcm_frames > 0);

    assert_eq!(runtime.retain_handles(&[second]), 1);
    assert_eq!(runtime.state(first, 0).unwrap_err().code(), "player-closed");
    assert_eq!(runtime.total_pcm_frames, 0);
    assert_eq!(runtime.total_encoded_bytes, clip.len());

    runtime.start(second, 0).unwrap();
    let third = runtime.create_from_bytes("audio/mpeg", b"new").unwrap();
    assert_eq!(runtime.state(third, 0).unwrap(), PlayerState::Unrealized);
}

#[test]
fn closed_players_release_live_slots_and_started_players_are_bounded() {
    let limits = Limits {
        max_players: 1,
        max_started_players: 1,
        max_handle_history: 3,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    let first = runtime.create_tone_player().unwrap();
    runtime.close(first, 0).unwrap();
    assert_eq!(runtime.state(first, 0).unwrap(), PlayerState::Closed);
    let second = runtime.create_tone_player().unwrap();
    runtime.start(second, 0).unwrap();
    assert_eq!(
        runtime.create_tone_player().unwrap_err().code(),
        "player-limit"
    );

    let limits = Limits {
        max_players: 2,
        max_started_players: 1,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    let clip = wav(&vec![1_000; OUTPUT_SAMPLE_RATE as usize / 10]);
    let first = runtime.create_from_bytes("audio/x-wav", &clip).unwrap();
    let second = runtime.create_from_bytes("audio/x-wav", &clip).unwrap();
    runtime.start(first, 0).unwrap();
    runtime.start(first, 0).unwrap();
    assert_eq!(runtime.player(first).unwrap().events.len(), 1);
    assert_eq!(
        runtime.start(second, 0).unwrap_err().code(),
        "audio-resource-limit"
    );

    let midi = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk\0\0\0\x13\0\xff\x51\x03\x07\xa1\x20\0\x90\x45\x7f\x60\x80\x45\0\0\xff\x2f\0";
    let mut runtime = Runtime::new(NullAudioSink::default(), Limits::default());
    let first = runtime.create_from_bytes("audio/midi", midi).unwrap();
    let second = runtime.create_from_bytes("audio/midi", midi).unwrap();
    runtime.start(first, 0).unwrap();
    runtime.start(first, 0).unwrap();
    assert_eq!(runtime.player(first).unwrap().events.len(), 1);
    assert_eq!(
        runtime.start(second, 0).unwrap_err().code(),
        "audio-resource-limit"
    );

    let limits = Limits {
        max_pending_events: 1,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    let handle = runtime.create_from_bytes("audio/x-wav", &clip).unwrap();
    runtime.start(handle, 0).unwrap();
    assert_eq!(
        runtime.stop(handle, 1_000).unwrap_err().code(),
        "event-queue-limit"
    );
    assert_eq!(runtime.state(handle, 1_000).unwrap(), PlayerState::Started);
}

#[test]
fn default_player_budget_supports_preloaded_sound_banks_and_remains_bounded() {
    let limits = Limits::default();
    assert_eq!(limits.max_players, 32);
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    for _ in 0..32 {
        runtime.create_tone_player().unwrap();
    }
    assert_eq!(
        runtime.create_tone_player().unwrap_err().code(),
        "player-limit"
    );
}
