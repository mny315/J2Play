use super::*;
use crate::{NullAudioSink, PlayerState, Runtime};

#[path = "../../support/nokia_ota.rs"]
mod fixture;
use fixture::{Instruction::*, song};

#[test]
fn basic_temporary_and_unicode_songs_render_the_same_a4_note() {
    let mut expected = Vec::new();
    append_synth_note(&mut expected, 69, 13_230, 100, &|| false).unwrap();
    for (basic, unicode) in [(false, false), (true, false), (true, true)] {
        let bytes = song(
            basic,
            unicode,
            &[(
                0,
                0,
                &[Tempo(12), Scale(0), Style(1), Volume(15), Note(10, 2, 0)],
            )],
        );
        let clip = decode(&bytes, Limits::default(), &|| false).unwrap();
        assert_eq!(clip.duration_micros, 600_000);
        assert_eq!(clip.samples.as_ref(), expected);
        // OTA and MIDP ToneControl cannot share a decoder.
        assert!(crate::decode_tone_sequence(&bytes, Limits::default(), &|| false).is_err());
    }
}

#[test]
fn patterns_repeat_and_references_reexecute_controls_in_sequence() {
    let bytes = song(
        false,
        false,
        &[
            (0, 1, &[Tempo(12), Note(1, 2, 0)]),
            (1, 0, &[Tempo(16), Note(3, 2, 0)]),
            (0, 2, &[]),
        ],
    );
    let clip = decode(&bytes, Limits::default(), &|| false).unwrap();
    assert_eq!(clip.duration_micros, 3_375_000); // 5 * 600 ms + 375 ms
    assert_eq!(clip.samples.len(), 74_418);
}

#[test]
fn duration_modifiers_tempo_changes_rests_and_articulation_are_preserved() {
    let bytes = song(
        false,
        false,
        &[(
            0,
            0,
            &[
                Tempo(12),
                Note(0, 2, 0),
                Note(1, 2, 1),
                Note(1, 2, 2),
                Note(1, 2, 3),
                Tempo(16),
                Note(1, 2, 0),
            ],
        )],
    );
    let clip = decode(&bytes, Limits::default(), &|| false).unwrap();
    assert_eq!(clip.duration_micros, 3_325_000);
    assert!(clip.samples[..13_230].iter().all(|value| *value == 0));
    for (style, sounding_frames) in [(0, 11_577), (1, 13_230), (2, 6_615)] {
        let bytes = song(
            false,
            false,
            &[(0, 0, &[Tempo(12), Style(style), Note(1, 2, 0)])],
        );
        let clip = decode(&bytes, Limits::default(), &|| false).unwrap();
        assert!(
            clip.samples[..sounding_frames]
                .iter()
                .any(|value| *value != 0)
        );
        assert!(
            clip.samples[sounding_frames..]
                .iter()
                .all(|value| *value == 0)
        );
    }
    let bytes = song(false, false, &[(0, 0, &[Volume(0), Note(1, 2, 0)])]);
    assert!(
        decode(&bytes, Limits::default(), &|| false)
            .unwrap()
            .samples
            .iter()
            .all(|v| *v == 0)
    );
}

#[test]
fn malformed_truncated_and_unbounded_songs_fail_explicitly() {
    let bytes = fixture::short_tone();
    for end in 0..bytes.len() {
        assert!(
            decode(&bytes[..end], Limits::default(), &|| false).is_err(),
            "prefix {end}"
        );
    }
    for instructions in [
        vec![Note(13, 2, 0)],
        vec![Note(1, 6, 0)],
        vec![Style(3)],
        vec![],
    ] {
        let bytes = song(false, false, &[(0, 0, &instructions)]);
        assert_eq!(
            decode(&bytes, Limits::default(), &|| false)
                .err()
                .unwrap()
                .code(),
            "media-malformed"
        );
    }
    let bytes = song(false, false, &[(0, 15, &[Note(1, 2, 0)])]);
    assert_eq!(
        decode(&bytes, Limits::default(), &|| false)
            .err()
            .unwrap()
            .code(),
        "media-unsupported-format"
    );
    let mut bytes = fixture::short_tone();
    bytes.push(1);
    assert!(decode(&bytes, Limits::default(), &|| false).is_err());
}

#[test]
fn budgets_and_cancellation_leave_the_encoded_player_retryable() {
    let bytes = fixture::short_tone();
    for limits in [
        Limits {
            max_input_bytes: bytes.len() - 1,
            ..Limits::default()
        },
        Limits {
            max_pcm_frames: 13_229,
            ..Limits::default()
        },
        Limits {
            max_tone_events: 4,
            ..Limits::default()
        },
    ] {
        assert_eq!(
            decode(&bytes, limits, &|| false).err().unwrap().code(),
            "media-limit"
        );
    }
    let count = std::cell::Cell::new(0);
    assert_eq!(
        decode(&bytes, Limits::default(), &|| {
            count.set(count.get() + 1);
            count.get() > 8
        })
        .err()
        .unwrap()
        .code(),
        "execution-cancelled"
    );

    let limits = Limits {
        max_total_pcm_frames: 13_230,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    let first = runtime
        .create_from_bytes("audio/vnd.nokia.ringing-tone", &bytes)
        .unwrap();
    let second = runtime
        .create_from_bytes("audio/vnd.nokia.ringing-tone", &bytes)
        .unwrap();
    runtime.realize(first).unwrap();
    assert_eq!(runtime.realize(second).unwrap_err().code(), "media-limit");
    assert_eq!(runtime.state(second, 0).unwrap(), PlayerState::Unrealized);
    runtime.close(first, 0).unwrap();
    runtime.start(second, 0).unwrap();
    runtime.tick(100_000).unwrap();
    assert!(runtime.sink().peak() > 0);
    let checkpoint = runtime.encode_checkpoint().unwrap();
    let mut restored = Runtime::new(NullAudioSink::default(), limits);
    restored.restore_checkpoint(&checkpoint).unwrap();
    restored.tick(200_000).unwrap();
    assert!(restored.sink().peak() > 0);
}
