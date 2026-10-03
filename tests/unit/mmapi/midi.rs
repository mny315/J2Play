use super::*;
use std::cell::Cell;

fn midi_file(tracks: &[&[u8]]) -> Vec<u8> {
    let mut bytes = b"MThd\0\0\0\x06".to_vec();
    bytes.extend(u16::from(tracks.len() > 1).to_be_bytes());
    bytes.extend(u16::try_from(tracks.len()).unwrap().to_be_bytes());
    bytes.extend(96_u16.to_be_bytes());
    for track in tracks {
        bytes.extend(b"MTrk");
        bytes.extend(u32::try_from(track.len()).unwrap().to_be_bytes());
        bytes.extend_from_slice(track);
    }
    bytes
}

fn truncated_midi(track: &[u8]) -> Vec<u8> {
    let mut declared = track.to_vec();
    declared.push(0);
    let mut bytes = midi_file(&[&declared]);
    bytes.pop();
    bytes
}

#[test]
fn channel_volume_uses_the_full_midi_range() {
    // MIDI CC 7 spans 0..127, independently of MMAPI's 0..100 volume control.
    // https://midi.org/midi-1-0-control-change-messages
    for channel in [0, 9] {
        let mut previous_energy = 0;
        for volume in [0, 64, 100, 101, 127] {
            let track = [
                0,
                0xb0 | channel,
                7,
                volume,
                0,
                0x90 | channel,
                60,
                127,
                48,
                0x80 | channel,
                60,
                0,
                0,
                0xff,
                0x2f,
                0,
            ];
            let clip = decode_midi(&midi_file(&[&track]), Limits::default(), &|| false).unwrap();
            let energy: u64 = clip
                .samples
                .iter()
                .map(|sample| u64::from(sample.unsigned_abs()))
                .sum();
            if volume == 0 {
                assert_eq!(energy, 0, "channel={channel}");
            } else {
                assert!(
                    energy > previous_energy,
                    "channel={channel}, volume={volume}"
                );
            }
            previous_energy = energy;
        }
    }
}

#[test]
fn final_track_recovery_retains_complete_events_at_each_truncated_field() {
    let prefix = [0, 0x90, 60, 100, 48, 0x80, 60, 0];
    let complete = [prefix.as_slice(), &[0, 0xff, 0x2f, 0]].concat();
    let expected = decode_midi(&midi_file(&[&complete]), Limits::default(), &|| false).unwrap();
    for tail in [
        &[][..],
        &[0x80],
        &[0],
        &[0, 0x90],
        &[0, 0xb0, 7],
        &[0, 0xc0],
        &[0, 0xd0],
        &[0, 0xe0, 0],
        &[0, 0xff],
        &[0, 0xff, 0x51],
        &[0, 0xff, 0x51, 3, 7],
        &[0, 0xf0],
        &[0, 0xf7, 3, 0],
    ] {
        let track = [prefix.as_slice(), tail].concat();
        let bytes = truncated_midi(&track);
        let actual = decode_midi(&bytes, Limits::default(), &|| false).unwrap();
        assert_eq!(actual.duration_micros, expected.duration_micros, "{tail:?}");
        assert_eq!(actual.samples, expected.samples, "{tail:?}");
        // The same missing end is invalid when the container declares no loss.
        assert_eq!(
            decode_midi(&midi_file(&[&track]), Limits::default(), &|| false)
                .unwrap_err()
                .code(),
            "media-malformed",
            "{tail:?}"
        );
    }
}

#[test]
fn final_track_recovery_never_hides_corruption_cancellation_or_limits() {
    let prefix = [0, 0x90, 60, 100, 48, 0x80, 60, 0];
    for tail in [
        &[0, 0xf1][..],
        &[0, 0x90, 128, 100],
        &[0, 0xff, 0x51, 3, 0, 0, 0],
        &[0, 0xff, 0x2f, 1, 0],
        &[0, 0xff, 0x2f, 0, 0],
        &[0x80, 0],
        &[0x80, 0x80, 0x80, 0x80],
    ] {
        let track = [prefix.as_slice(), tail].concat();
        assert_eq!(
            decode_midi(&truncated_midi(&track), Limits::default(), &|| false)
                .unwrap_err()
                .code(),
            "media-malformed",
            "{tail:?}"
        );
    }
    let bytes = truncated_midi(&prefix);
    assert_eq!(
        decode_midi(&bytes, Limits::default(), &|| true)
            .unwrap_err()
            .code(),
        "execution-cancelled"
    );
    assert_eq!(
        decode_midi(
            &bytes,
            Limits {
                max_midi_events: 1,
                ..Limits::default()
            },
            &|| false
        )
        .unwrap_err()
        .code(),
        "media-limit"
    );
    assert_eq!(
        decode_midi(&truncated_midi(&[0, 0x90]), Limits::default(), &|| false)
            .unwrap_err()
            .code(),
        "media-malformed"
    );
}

#[test]
fn end_of_track_preserves_silence_and_loop_duration() {
    use crate::{PlayerEventKind, PlayerState, Runtime, tests::CaptureSink};

    let bytes = midi_file(&[&[
        0, 0x90, 60, 100, // Note for the first half of the track.
        48, 0x80, 60, 0, 48, 0xff, 0x2f, 0, // Silence remains part of the loop.
    ]]);
    let clip = decode_midi(&bytes, Limits::default(), &|| false).unwrap();
    assert_eq!(clip.duration_micros, 500_000);
    assert_eq!(clip.samples.len(), 11_025);
    assert!(clip.samples[..5_512].iter().any(|sample| *sample != 0));
    assert!(clip.samples[5_512..].iter().all(|sample| *sample == 0));

    let mut runtime = Runtime::new(CaptureSink::default(), Limits::default());
    let handle = runtime.create_from_bytes("audio/midi", &bytes).unwrap();
    runtime.set_loop_count(handle, 2).unwrap();
    runtime.start(handle, 0).unwrap();
    runtime.tick(1_000_000).unwrap();
    assert_eq!(runtime.sink().0, clip.samples.repeat(2));
    assert_eq!(
        runtime.state(handle, 1_000_000).unwrap(),
        PlayerState::Prefetched
    );
    let endings: Vec<_> = runtime.players[&handle]
        .events
        .iter()
        .filter(|event| event.kind == PlayerEventKind::EndOfMedia)
        .map(|event| event.media_time_micros)
        .collect();
    assert_eq!(endings, [500_000, 500_000]);
}

#[test]
fn last_track_boundary_uses_the_shared_tempo_map() {
    let tempo = [
        96, 0xff, 0x51, 3, 0x0f, 0x42, 0x40, // 1 s/quarter after the first quarter.
        96, 0xff, 0x2f, 0,
    ];
    let silent = [48, 0xff, 0x2f, 0];
    for tracks in [
        [tempo.as_slice(), silent.as_slice()],
        [silent.as_slice(), tempo.as_slice()],
    ] {
        let clip = decode_midi(&midi_file(&tracks), Limits::default(), &|| false).unwrap();
        assert_eq!(clip.duration_micros, 1_500_000);
        assert_eq!(clip.samples.len(), 33_075);
        assert!(clip.samples.iter().all(|sample| *sample == 0));
    }
}

#[test]
fn track_end_enforces_duration_and_payload_limits() {
    let bytes = midi_file(&[&[96, 0xff, 0x2f, 0]]);
    let limits = Limits {
        max_pcm_frames: 11_024,
        ..Limits::default()
    };
    assert_eq!(
        decode_midi(&bytes, limits, &|| false).unwrap_err().code(),
        "media-limit"
    );
    let clip = decode_midi(
        &bytes,
        Limits {
            max_pcm_frames: 11_025,
            ..limits
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(clip.samples.len(), 11_025);

    for track in [&[0, 0xff, 0x2f, 1, 0][..], &[0, 0xff, 0x2f, 0, 0][..]] {
        assert_eq!(
            decode_midi(&midi_file(&[track]), Limits::default(), &|| false)
                .unwrap_err()
                .code(),
            "media-malformed"
        );
    }
}

#[test]
fn timing_is_independent_of_redundant_events_between_notes() {
    let mut track = vec![0, 0x90, 60, 100, 96, 0x80, 60, 0, 0, 0xff, 0x2f, 0];
    let expected = decode_midi(&midi_file(&[&track]), Limits::default(), &|| false).unwrap();
    track.truncate(4);
    for _ in 0..95 {
        track.extend([1, 0xc0, 0]);
    }
    track.extend([1, 0x80, 60, 0, 0, 0xff, 0x2f, 0]);
    let actual = decode_midi(&midi_file(&[&track]), Limits::default(), &|| false).unwrap();
    assert_eq!(actual.duration_micros, expected.duration_micros);
    assert_eq!(actual.samples, expected.samples);
}

#[test]
fn fractional_ticks_are_carried_across_tempo_changes() {
    let track = [
        1, 0xff, 0x51, 3, 7, 0xa1, 0x21, // 500001 us/quarter
        1, 0xff, 0x51, 3, 7, 0xa1, 0x23, // 500003 us/quarter
        1, 0xc0, 0, 0, 0xff, 0x2f, 0,
    ];
    let clip = decode_midi(&midi_file(&[&track]), Limits::default(), &|| false).unwrap();
    assert_eq!(clip.duration_micros, (500_000 + 500_001 + 500_003) / 96);
}

#[test]
fn ignored_messages_can_be_cancelled_before_synthesis() {
    let mut track = [0, 0xd0, 0].repeat(4096);
    track.extend([0, 0xff, 0x2f, 0]);
    let mut bytes = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk".to_vec();
    bytes.extend(u32::try_from(track.len()).unwrap().to_be_bytes());
    bytes.extend(track);
    let checks = Cell::new(0);
    let error = decode_midi(&bytes, Limits::default(), &|| {
        checks.set(checks.get() + 1);
        checks.get() == 2
    })
    .unwrap_err();
    assert_eq!(error.code(), "execution-cancelled");
    assert_eq!(checks.get(), 2);
    let clip = decode_midi(&bytes, Limits::default(), &|| false).unwrap();
    assert!(clip.samples.is_empty());
}

#[test]
fn later_tracks_preserve_event_order_without_exceeding_the_shared_limit() {
    let track = [0, 0xc0, 80, 0, 0x90, 60, 100, 0, 0xff, 0x2f, 0];
    let mut events = Vec::new();
    parse_midi_track(&track, &mut events, 4, &|| false).unwrap();
    assert_eq!(
        parse_midi_track(&track, &mut events, 4, &|| false)
            .unwrap_err()
            .into_error()
            .code(),
        "media-limit"
    );
    assert_eq!(events.len(), 4);
    assert_eq!(
        events.iter().map(|event| event.order).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert!(matches!(events[1].kind, MidiKind::NoteOn { note: 60, .. }));
    assert!(matches!(
        events[3].kind,
        MidiKind::ProgramChange { program: 80, .. }
    ));
}

#[test]
fn event_timeline_can_be_cancelled_before_allocating_pcm() {
    let events: Vec<_> = (0..4096)
        .map(|order| MidiEvent {
            tick: 0,
            order,
            kind: MidiKind::Tempo(500_000),
        })
        .collect();
    let checks = Cell::new(0);
    let error = render_midi_events(&events, 96, Limits::default(), &|| {
        checks.set(checks.get() + 1);
        checks.get() == 2
    })
    .unwrap_err();
    assert_eq!(error.code(), "execution-cancelled");
    assert_eq!(checks.get(), 2);
}
