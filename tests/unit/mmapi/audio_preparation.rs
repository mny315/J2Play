use super::*;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[test]
fn slight_upsampling_preserves_source_samples_even_when_frame_count_is_unchanged() {
    let clip =
        super::resample_mono(vec![0, 22_050, 0, -22_050], 20_000, Limits::default()).unwrap();
    assert_eq!(clip.samples.as_ref(), [0, 20_000, 4_100, -15_900]);
    assert_eq!(clip.duration_micros, 200);
}

#[test]
fn decoded_pcm_releases_source_capacity_after_resampling() {
    for rate in [44_100, OUTPUT_SAMPLE_RATE, 11_025, 192_000] {
        let mut source = Vec::with_capacity(1024);
        source.extend([0, 100, -200, 300]);
        let clip = super::resample_mono(source, rate, Limits::default()).unwrap();
        let samples: Vec<i16> = clip.samples.into();
        assert_eq!(
            samples.capacity(),
            samples.len(),
            "unaccounted retained PCM capacity at {rate} Hz"
        );
    }
}

#[test]
fn tone_preparation_and_replacement_preserve_pcm_capacity_and_state() {
    let mut runtime = Runtime::new(
        NullAudioSink::default(),
        Limits {
            max_total_pcm_frames: 6_000,
            max_total_input_bytes: 8,
            ..Limits::default()
        },
    );
    let first = runtime.create_tone_player().unwrap();
    let second = runtime.create_tone_player().unwrap();
    let short = [0xfe, 1, 60, 8];
    runtime.set_tone_sequence(first, &short).unwrap();
    assert!(runtime.player(first).unwrap().clip.is_none());
    assert_eq!(runtime.total_pcm_frames, 0);
    runtime.realize(first).unwrap();
    assert_eq!(runtime.total_pcm_frames, 5_512);
    let prior = runtime
        .player(first)
        .unwrap()
        .clip
        .as_ref()
        .unwrap()
        .samples
        .clone();
    runtime.set_tone_sequence(second, &short).unwrap();
    assert_eq!(runtime.realize(second).unwrap_err().code(), "media-limit");
    assert_eq!(
        runtime.player(second).unwrap().state,
        PlayerState::Unrealized
    );
    assert_eq!(
        runtime
            .set_tone_sequence(first, &[0xfe, 1, 60, 16])
            .unwrap_err()
            .code(),
        "media-limit"
    );
    assert_eq!(
        runtime
            .player(first)
            .unwrap()
            .clip
            .as_ref()
            .unwrap()
            .samples,
        prior
    );
    assert_eq!(runtime.total_encoded_bytes, 8);
    assert_eq!(runtime.total_pcm_frames, 5_512);
    runtime
        .set_tone_sequence(first, &[0xfe, 1, 0xff, 8])
        .unwrap();
    assert!(
        runtime
            .player(first)
            .unwrap()
            .clip
            .as_ref()
            .unwrap()
            .samples
            .iter()
            .all(|sample| *sample == 0)
    );
    assert_eq!(runtime.total_pcm_frames, 5_512);
    runtime.close(first, 0).unwrap();
    runtime.realize(second).unwrap();
    assert_eq!(
        runtime
            .player(second)
            .unwrap()
            .clip
            .as_ref()
            .unwrap()
            .samples,
        prior
    );
    assert_eq!(runtime.total_pcm_frames, 5_512);
}

#[test]
fn player_handle_exhaustion_does_not_create_unrepresentable_java_handles() {
    let mut runtime = Runtime::new(NullAudioSink::default(), Limits::default());
    runtime.next_handle = i64::MAX.cast_unsigned();
    let handle = runtime.create_tone_player().unwrap();
    assert_eq!(
        java_handle(handle).unwrap(),
        Some(NativeValue::Long(i64::MAX))
    );
    assert_eq!(
        runtime
            .create_from_bytes("audio/mpeg", b"owned")
            .unwrap_err()
            .code(),
        "player-limit"
    );
    assert_eq!(runtime.players.len(), 1);
    assert_eq!(runtime.total_encoded_bytes, 0);
    runtime.close(handle, 0).unwrap();
    assert_eq!(
        runtime.create_midi_player().unwrap_err().code(),
        "player-limit"
    );
    assert_eq!(runtime.players.len(), 1);
}

#[test]
fn oversized_wav_is_rejected_before_sample_decode_work() {
    let mut adpcm = ima_adpcm_wav_constant();
    let block = adpcm[48..].to_vec();
    adpcm.truncate(48);
    adpcm.extend(block.repeat(64));
    let size = u32::try_from(adpcm.len()).unwrap();
    adpcm[4..8].copy_from_slice(&(size - 8).to_le_bytes());
    adpcm[44..48].copy_from_slice(&(size - 48).to_le_bytes());
    for bytes in [wav(&vec![1000; 65_536]), adpcm] {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= 8
        };
        let error = crate::decode_wav(
            &bytes,
            Limits {
                max_pcm_frames: 4,
                ..Limits::default()
            },
            &cancelled,
        )
        .unwrap_err();
        assert_eq!(error.code(), "media-limit");
        assert!(
            checks.get() < 8,
            "duration rejection must not decode samples"
        );
    }
}

#[test]
fn wav_pcm_quota_uses_resampled_frame_count() {
    let mut bytes = wav(&[0, 1, 2, 3, 4, 5, 6, 7]);
    bytes[24..28].copy_from_slice(&44_100_u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&88_200_u32.to_le_bytes());
    let clip = decode_wav(
        &bytes,
        Limits {
            max_pcm_frames: 4,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(clip.samples.as_ref(), [0, 2, 4, 6]);
    let samples: Vec<i16> = clip.samples.into();
    assert_eq!(samples.capacity(), 4);
    assert_eq!(
        decode_wav(
            &bytes,
            Limits {
                max_pcm_frames: 3,
                ..Limits::default()
            }
        )
        .unwrap_err()
        .code(),
        "media-limit"
    );
    let adpcm = ima_adpcm_wav_constant();
    assert_eq!(
        decode_wav(
            &adpcm,
            Limits {
                max_pcm_frames: 24,
                ..Limits::default()
            }
        )
        .unwrap()
        .samples
        .len(),
        24
    );
    assert_eq!(
        decode_wav(
            &adpcm,
            Limits {
                max_pcm_frames: 23,
                ..Limits::default()
            }
        )
        .unwrap_err()
        .code(),
        "media-limit"
    );
}

#[test]
fn cancellation_during_each_audio_codec_preserves_encoded_data_for_retry() {
    let mut amr = amr_nb::FILE_MAGIC.to_vec();
    for _ in 0..8 {
        amr.push(0x3c);
        amr.extend([0; 31]);
    }
    let fixtures = [
        ("audio/wav", wav(&vec![1000; 16_384]), 4),
        (
            "audio/mpeg",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/audio/tone-440.mp3"
            ))
            .to_vec(),
            4,
        ),
        (
            "audio/mp4",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/audio/tone-660.m4a"
            ))
            .to_vec(),
            4,
        ),
        ("audio/amr", amr, 11),
        ("audio/x-tone-seq", vec![0xfe, 1, 69, 64], 5),
    ];
    for (content_type, bytes, cancel_after) in fixtures {
        let mut reference = Runtime::new(NullAudioSink::default(), Limits::default());
        let reference_handle = reference.create_from_bytes(content_type, &bytes).unwrap();
        reference.realize(reference_handle).unwrap();

        let enabled = Arc::new(AtomicBool::new(true));
        let checks = Arc::new(AtomicUsize::new(0));
        let cancel_enabled = Arc::clone(&enabled);
        let cancel_checks = Arc::clone(&checks);
        let mut runtime = Runtime::new(NullAudioSink::default(), Limits::default())
            .with_decode_cancellation(move || {
                cancel_enabled.load(Ordering::Relaxed)
                    && cancel_checks.fetch_add(1, Ordering::Relaxed) >= cancel_after
            });
        let handle = runtime.create_from_bytes(content_type, &bytes).unwrap();
        assert_eq!(
            runtime.realize(handle).unwrap_err().code(),
            "execution-cancelled",
            "{content_type}"
        );
        assert_eq!(checks.load(Ordering::Relaxed), cancel_after + 1);
        assert_eq!(
            runtime.player(handle).unwrap().state,
            PlayerState::Unrealized
        );
        assert!(runtime.player(handle).unwrap().clip.is_none());
        assert_eq!(runtime.total_encoded_bytes, bytes.len());
        assert_eq!(runtime.total_pcm_frames, 0);

        enabled.store(false, Ordering::Relaxed);
        runtime.realize(handle).unwrap();
        let clip = runtime.player(handle).unwrap().clip.as_ref().unwrap();
        let reference = reference
            .player(reference_handle)
            .unwrap()
            .clip
            .as_ref()
            .unwrap();
        assert_eq!(clip.samples, reference.samples, "{content_type}");
        assert_eq!(
            clip.duration_micros, reference.duration_micros,
            "{content_type}"
        );
        assert_eq!(runtime.total_encoded_bytes, 0);
    }
}

#[test]
fn sample_rate_conversion_polls_cancellation_in_both_directions() {
    for rate in [11_025, 44_100] {
        let checks = Cell::new(0);
        let error = crate::media_decode::resample_mono(
            vec![1000; 16_384],
            rate,
            Limits::default(),
            &|| {
                checks.set(checks.get() + 1);
                checks.get() == 3
            },
        )
        .unwrap_err();
        assert_eq!(error.code(), "execution-cancelled");
        assert_eq!(checks.get(), 3);
    }
}

#[test]
fn nested_empty_tone_blocks_take_linear_work() {
    let mut sequence = vec![0xfe, 1, 0xfb, 0, 0xfa, 0];
    for number in 1..=6 {
        sequence.extend([0xfb, number]);
        for _ in 0..64 {
            sequence.extend([0xf9, number - 1]);
        }
        sequence.extend([0xfa, number]);
    }
    sequence.extend([0xf9, 6, 60, 1]);
    let checks = Cell::new(0);
    let clip = crate::tone::decode_tone_sequence(&sequence, Limits::default(), &|| {
        checks.set(checks.get() + 1);
        checks.get() >= 4096
    })
    .expect("empty blocks must not amplify expansion work");
    let direct = decode_tone_sequence(&[0xfe, 1, 60, 1], Limits::default()).unwrap();
    assert_eq!(clip.samples, direct.samples);
    assert_eq!(clip.duration_micros, direct.duration_micros);
}

#[test]
fn cached_tone_blocks_preserve_volume_order_depth_and_event_limits() {
    let sequence = [
        0xfe, 1, 0xfb, 0, 0xf8, 50, 60, 2, 0xfa, 0, 0xfb, 1, 0xf9, 0, 0xf8, 80, 64, 1, 0xf9, 0,
        0xfa, 1, 0xf9, 1,
    ];
    let direct = [0xfe, 1, 0xf8, 50, 60, 2, 0xf8, 80, 64, 1, 0xf8, 50, 60, 2];
    let expanded = decode_tone_sequence(&sequence, Limits::default()).unwrap();
    let reference = decode_tone_sequence(&direct, Limits::default()).unwrap();
    assert_eq!(expanded.samples, reference.samples);
    assert_eq!(expanded.duration_micros, reference.duration_micros);
    let limits = Limits {
        max_tone_events: 5,
        ..Limits::default()
    };
    assert_eq!(
        decode_tone_sequence(&sequence, limits).unwrap_err().code(),
        "media-limit"
    );

    let mut nested = vec![0xfe, 1, 0xfb, 0, 0xfa, 0];
    for number in 1..=16 {
        nested.extend([0xfb, number, 0xf9, number - 1, 0xfa, number]);
    }
    nested.extend([0xf9, 16]);
    assert_eq!(
        decode_tone_sequence(&nested, Limits::default())
            .unwrap_err()
            .code(),
        "tone-sequence"
    );
}

#[test]
fn cancelling_tone_replacement_preserves_the_existing_sound_and_budgets() {
    for single_tone in [false, true] {
        let enabled = Arc::new(AtomicBool::new(false));
        let checks = Arc::new(AtomicUsize::new(0));
        let cancel_enabled = Arc::clone(&enabled);
        let cancel_checks = Arc::clone(&checks);
        let mut runtime = Runtime::new(NullAudioSink::default(), Limits::default())
            .with_decode_cancellation(move || {
                cancel_enabled.load(Ordering::Relaxed)
                    && cancel_checks.fetch_add(1, Ordering::Relaxed) >= 3
            });
        let handle = runtime.create_tone_player().unwrap();
        let sequence = [0xfe, 1, 69, 64];
        let prior = if single_tone {
            runtime.play_tone(60, 100, 50, 0).unwrap();
            runtime.one_shot.as_ref().unwrap().samples.clone()
        } else {
            runtime
                .set_tone_sequence(handle, &[0xfe, 1, 60, 8])
                .unwrap();
            runtime.realize(handle).unwrap();
            runtime
                .player(handle)
                .unwrap()
                .clip
                .as_ref()
                .unwrap()
                .samples
                .clone()
        };
        let encoded = runtime.total_encoded_bytes;
        let pcm = runtime.total_pcm_frames;
        enabled.store(true, Ordering::Relaxed);
        let error = if single_tone {
            runtime.play_tone(69, 1_000, 100, 0)
        } else {
            runtime.set_tone_sequence(handle, &sequence)
        }
        .unwrap_err();
        assert_eq!(error.code(), "execution-cancelled");
        assert_eq!(runtime.total_encoded_bytes, encoded);
        assert_eq!(runtime.total_pcm_frames, pcm);
        let current = if single_tone {
            &runtime.one_shot.as_ref().unwrap().samples
        } else {
            &runtime
                .player(handle)
                .unwrap()
                .clip
                .as_ref()
                .unwrap()
                .samples
        };
        assert_eq!(current, &prior);
        enabled.store(false, Ordering::Relaxed);
        if single_tone {
            runtime.play_tone(69, 1_000, 100, 0).unwrap();
        } else {
            runtime.set_tone_sequence(handle, &sequence).unwrap();
        }
    }
}

fn synthesis_cancellation_fixture() -> Vec<u8> {
    let mut track = vec![0, 0xff, 0x51, 3, 7, 0xa1, 0x20];
    for family in 0..16_u8 {
        track.extend([0, 0xc0, family * 8, 0, 0x90, 60 + family, 100]);
        track.extend([0, 0x99, 49, 100, 96, 0x80, 60 + family, 0]);
    }
    track.extend([0, 0xff, 0x2f, 0]);
    let mut bytes = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk".to_vec();
    bytes.extend(u32::try_from(track.len()).unwrap().to_be_bytes());
    bytes.extend(track);
    bytes
}

#[test]
fn cancelled_synthesis_keeps_encoded_player_and_retry_preserves_pcm() {
    let bytes = synthesis_cancellation_fixture();
    for transition in 0..4 {
        let enabled = Arc::new(AtomicBool::new(true));
        let checks = Arc::new(AtomicUsize::new(0));
        let cancel_enabled = Arc::clone(&enabled);
        let cancel_checks = Arc::clone(&checks);
        let mut runtime = Runtime::new(NullAudioSink::default(), Limits::default())
            .with_decode_cancellation(move || {
                cancel_enabled.load(Ordering::Relaxed)
                    && cancel_checks.fetch_add(1, Ordering::Relaxed) >= 3
            });
        let handle = runtime.create_from_bytes("audio/midi", &bytes).unwrap();
        let invoke = |runtime: &mut Runtime<NullAudioSink>| match transition {
            0 => runtime.realize(handle),
            1 => runtime.prefetch(handle),
            2 => runtime.start(handle, 0),
            _ => runtime.start_exclusive(handle, 0),
        };
        assert_eq!(
            invoke(&mut runtime).unwrap_err().code(),
            "execution-cancelled"
        );
        assert_eq!(checks.load(Ordering::Relaxed), 4);
        assert_eq!(
            runtime.player(handle).unwrap().state,
            PlayerState::Unrealized
        );
        assert!(runtime.player(handle).unwrap().clip.is_none());
        assert_eq!(runtime.total_pcm_frames, 0);
        assert_eq!(runtime.total_encoded_bytes, bytes.len());
        assert_eq!(runtime.exclusive_player, None);

        enabled.store(false, Ordering::Relaxed);
        invoke(&mut runtime).unwrap();
        let clip = runtime.player(handle).unwrap().clip.as_ref().unwrap();
        assert_eq!(clip.samples.len(), 176_400);
        let hash = clip
            .samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            });
        // PCM reference for all sixteen melodic families and drums.
        assert_eq!(hash, 0x1cda_bf62_06a0_be7c);
        assert_eq!(runtime.total_encoded_bytes, 0);
    }
}

#[test]
fn sustained_notes_and_drums_poll_cancellation_inside_one_voice() {
    for drum in [false, true] {
        let checks = std::cell::Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() == 2
        };
        let mut samples = vec![0; 12_000];
        let result = if drum {
            mix_drum(&mut samples, 49, 100, &cancelled)
        } else {
            mix_midi_note(&mut samples, 69, 100, 0, &cancelled)
        };
        assert_eq!(result.unwrap_err().code(), "execution-cancelled");
        assert!(samples[..4096].iter().any(|sample| *sample != 0));
        assert!(samples[4096..].iter().all(|sample| *sample == 0));
    }
}
