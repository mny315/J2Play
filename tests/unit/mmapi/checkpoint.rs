use super::*;
use crate::{Limits, tests::CaptureSink};

#[test]
fn fixed_pcm_storage_preserves_existing_checkpoint_bytes() {
    #[derive(Serialize, Deserialize)]
    struct LegacyClip {
        samples: Vec<i16>,
        duration_micros: i64,
    }

    #[derive(Serialize, Deserialize)]
    struct LegacyOneShot {
        samples: Vec<i16>,
        rendered_frames: usize,
    }

    for samples in [Vec::new(), vec![i16::MIN, -1, 0, 1, i16::MAX]] {
        let previous_clip = LegacyClip {
            samples: samples.clone(),
            duration_micros: 1_234,
        };
        let bytes = save_state::encode(&previous_clip).unwrap();
        let restored: Clip = save_state::decode(&bytes).unwrap();
        assert_eq!(restored.samples.as_ref(), samples);
        let encoded = save_state::encode(&restored).unwrap();
        assert_eq!(encoded, bytes);
        let previous: LegacyClip = save_state::decode(&encoded).unwrap();
        assert_eq!(previous.samples, samples);
        assert_eq!(previous.duration_micros, 1_234);

        let previous_tone = LegacyOneShot {
            samples: samples.clone(),
            rendered_frames: samples.len(),
        };
        let bytes = save_state::encode(&previous_tone).unwrap();
        let restored: OneShot = save_state::decode(&bytes).unwrap();
        assert_eq!(restored.samples.as_ref(), samples);
        let encoded = save_state::encode(&restored).unwrap();
        assert_eq!(encoded, bytes);
        let previous: LegacyOneShot = save_state::decode(&encoded).unwrap();
        assert_eq!(previous.samples, samples);
        assert_eq!(previous.rendered_frames, samples.len());
    }
}

#[test]
fn encoded_media_streams_blocks_with_the_existing_checkpoint_tags_and_payloads() {
    #[derive(Serialize)]
    enum LegacyMedia<'a> {
        Bytes(&'a [u8]),
        ToneDevice(Option<&'a [u8]>),
        MidiDevice,
        Decoded,
        Closed,
    }

    #[derive(Default)]
    struct Writer(usize);

    impl std::io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += 1;
            assert!(self.0 <= 32, "media checkpoint writes individual bytes");
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    for media in [
        EncodedMedia::Bytes(Vec::new().into()),
        EncodedMedia::Bytes((0..=255).collect::<Vec<_>>().into()),
        EncodedMedia::Bytes(vec![17; 1024 * 1024].into()),
        EncodedMedia::ToneDevice(None),
        EncodedMedia::ToneDevice(Some(Vec::new())),
        EncodedMedia::ToneDevice(Some(vec![31; 1024 * 1024])),
        EncodedMedia::MidiDevice,
        EncodedMedia::Decoded,
        EncodedMedia::Closed,
    ] {
        let legacy = match &media {
            EncodedMedia::Bytes(bytes) => LegacyMedia::Bytes(bytes),
            EncodedMedia::ToneDevice(bytes) => LegacyMedia::ToneDevice(bytes.as_deref()),
            EncodedMedia::MidiDevice => LegacyMedia::MidiDevice,
            EncodedMedia::Decoded => LegacyMedia::Decoded,
            EncodedMedia::Closed => LegacyMedia::Closed,
        };
        let previous = save_state::encode(&legacy).unwrap();
        assert_eq!(save_state::encode(&media).unwrap(), previous);
        let restored: EncodedMedia = save_state::decode(&previous).unwrap();
        assert_eq!(save_state::encode(&restored).unwrap(), previous);
        assert_eq!(
            save_state::write(
                &media,
                Writer::default(),
                save_state::MAX_COMPONENT_BYTES,
                &|| false,
            )
            .unwrap(),
            previous.len()
        );
    }
}

#[test]
fn resumes_pcm_from_the_same_position_without_replaying_previous_audio() {
    let mut original = Runtime::new(CaptureSink::default(), Limits::default());
    let handle = original.create_tone_player().unwrap();
    original
        .set_tone_sequence(handle, &[254, 1, 60, 64])
        .unwrap();
    original.start(handle, 0).unwrap();
    original.tick(100_000).unwrap();
    let bytes = original.encode_checkpoint().unwrap();
    original.sink_mut().0.clear();
    original.tick(200_000).unwrap();
    let mut restored = Runtime::new(CaptureSink::default(), Limits::default());
    restored.restore_checkpoint(&bytes).unwrap();
    assert!(restored.sink().0.is_empty());
    restored.tick(200_000).unwrap();
    assert!(!original.sink().0.is_empty());
    assert_eq!(restored.sink().0, original.sink().0);
    assert_eq!(
        restored.player(handle).unwrap().position_micros,
        original.player(handle).unwrap().position_micros
    );
}

#[test]
fn restored_single_tone_shares_and_releases_the_player_pcm_budget() {
    let limits = Limits {
        max_pcm_frames: 6_000,
        max_total_pcm_frames: 5_534,
        ..Limits::default()
    };
    let mut original = Runtime::new(CaptureSink::default(), limits);
    let first = original.create_tone_player().unwrap();
    let second = original.create_tone_player().unwrap();
    for handle in [first, second] {
        original
            .set_tone_sequence(handle, &[254, 1, 60, 8])
            .unwrap();
    }
    original.realize(first).unwrap();
    original.play_tone(69, 1, 50, 0).unwrap();
    assert_eq!(original.total_pcm_frames, 5_534);
    let bytes = original.encode_checkpoint().unwrap();

    let mut restored = Runtime::new(CaptureSink::default(), limits);
    restored.restore_checkpoint(&bytes).unwrap();
    assert_eq!(restored.total_pcm_frames, original.total_pcm_frames);
    assert_eq!(restored.realize(second).unwrap_err().code(), "media-limit");
    restored.tick(2_000).unwrap();
    assert_eq!(restored.total_pcm_frames, 5_512);
    restored.close(first, 2_000).unwrap();
    restored.realize(second).unwrap();
    assert_eq!(restored.total_pcm_frames, 5_512);

    let mut limited = Runtime::new(
        CaptureSink::default(),
        Limits {
            max_total_pcm_frames: limits.max_total_pcm_frames - 1,
            ..limits
        },
    );
    limited.play_tone(72, 1, 50, 0).unwrap();
    let before = limited.encode_checkpoint().unwrap();
    assert_eq!(
        limited.restore_checkpoint(&bytes).unwrap_err().code(),
        "checkpoint-media"
    );
    assert_eq!(limited.encode_checkpoint().unwrap(), before);
    assert_eq!(limited.total_pcm_frames, 22);
}

#[test]
fn restores_the_final_player_handle_without_reusing_it() {
    let mut original = Runtime::new(CaptureSink::default(), Limits::default());
    original.next_handle = i64::MAX.cast_unsigned();
    let handle = original.create_tone_player().unwrap();
    let mut restored = Runtime::new(CaptureSink::default(), Limits::default());
    restored
        .restore_checkpoint(&original.encode_checkpoint().unwrap())
        .unwrap();
    assert_eq!(
        restored.player(handle).unwrap().state,
        PlayerState::Unrealized
    );
    assert_eq!(
        restored.create_tone_player().unwrap_err().code(),
        "player-limit"
    );
    assert_eq!(restored.players.len(), 1);
}

#[test]
fn rejects_invalid_playback_state_and_mixer_capacity_before_installing_players() {
    let limits = Limits::default();
    let mut original = Runtime::new(CaptureSink::default(), limits);
    let handle = original.create_midi_player().unwrap();
    original.start(handle, 0).unwrap();
    let valid = original.encode_checkpoint().unwrap();
    for limits in [
        Limits {
            max_started_players: 0,
            ..limits
        },
        Limits {
            max_started_midi_players: 0,
            ..limits
        },
    ] {
        let mut restored = Runtime::new(CaptureSink::default(), limits);
        assert_eq!(
            restored.restore_checkpoint(&valid).unwrap_err().code(),
            "checkpoint-media"
        );
        assert!(restored.players.is_empty());
    }
    for (loops, remainder) in [(0, 0), (-2, 0), (1, 1_000_000)] {
        original.player_mut(handle).unwrap().loop_count = loops;
        original.render_frame_remainder = remainder;
        let mut restored = Runtime::new(CaptureSink::default(), limits);
        assert_eq!(
            restored
                .restore_checkpoint(&original.encode_checkpoint().unwrap())
                .unwrap_err()
                .code(),
            "checkpoint-media"
        );
        assert!(restored.players.is_empty());
    }
}

#[test]
fn restores_storage_through_each_player_lifecycle_transition() {
    for kind in 0..4 {
        let mut original = Runtime::new(CaptureSink::default(), Limits::default());
        let handle = match kind {
            0 | 1 => original.create_tone_player().unwrap(),
            2 => original.create_midi_player().unwrap(),
            _ => original
                .create_from_bytes("audio/x-tone-seq", &[254, 1, 60, 8])
                .unwrap(),
        };
        if kind == 1 {
            original
                .set_tone_sequence(handle, &[254, 1, 60, 8])
                .unwrap();
        }
        for transition in 0..7 {
            match transition {
                0 => {}
                1 => original.realize(handle).unwrap(),
                2 => original.prefetch(handle).unwrap(),
                3 => original.start(handle, 0).unwrap(),
                4 => original.stop(handle, 0).unwrap(),
                5 => original.deallocate(handle, 0).unwrap(),
                _ => original.close(handle, 0).unwrap(),
            }
            let bytes = original.encode_checkpoint().unwrap();
            let mut restored = Runtime::new(CaptureSink::default(), Limits::default());
            restored.restore_checkpoint(&bytes).unwrap();
            assert_eq!(restored.encode_checkpoint().unwrap(), bytes);
            assert_eq!(restored.total_encoded_bytes, original.total_encoded_bytes);
            assert_eq!(restored.total_pcm_frames, original.total_pcm_frames);
            assert!(restored.sink().0.is_empty());
        }
    }
}

#[test]
fn rejects_inconsistent_player_storage_without_replacing_live_media() {
    let mut original = Runtime::new(CaptureSink::default(), Limits::default());
    let handle = original.create_tone_player().unwrap();
    original
        .set_tone_sequence(handle, &[254, 1, 60, 8])
        .unwrap();
    original.start(handle, 0).unwrap();
    let valid = original.player(handle).unwrap().clone();
    for case in 0..9 {
        let mut broken = valid.clone();
        match case {
            0 => broken.clip = None,
            1 => broken.state = PlayerState::Unrealized,
            2 => broken.encoded = EncodedMedia::Bytes([254, 1, 60, 8].into()),
            3 => broken.encoded = EncodedMedia::Closed,
            4 => broken.encoded = EncodedMedia::MidiDevice,
            5 => broken.content_type = "audio/x-wav",
            6 => broken.state = PlayerState::Closed,
            7 => {
                broken.state = PlayerState::Unrealized;
                broken.clip = None;
                broken.encoded = EncodedMedia::Decoded;
            }
            _ => {
                broken.state = PlayerState::Closed;
                broken.clip = None;
                broken.encoded = EncodedMedia::Bytes([1, 2, 3].into());
            }
        }
        *original.player_mut(handle).unwrap() = broken;
        let bytes = original.encode_checkpoint().unwrap();
        let mut restored = Runtime::new(CaptureSink::default(), Limits::default());
        restored.play_tone(72, 1, 50, 0).unwrap();
        let before = restored.encode_checkpoint().unwrap();
        let error = restored.restore_checkpoint(&bytes).unwrap_err();
        assert_eq!(error.code(), "checkpoint-media", "case {case}");
        assert_eq!(restored.encode_checkpoint().unwrap(), before);
        assert_eq!(restored.total_pcm_frames, 22);
        assert_eq!(restored.total_encoded_bytes, 0);
        assert!(restored.sink().0.is_empty());
    }
}
