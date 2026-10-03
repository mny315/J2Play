use super::*;

mod mixing;

fn decode(bytes: &[u8], limits: Limits) -> Result<Clip, EmuError> {
    super::decode(bytes, limits, &|| false)
}

pub(super) fn chunk(id: [u8; 4], body: &[u8]) -> Vec<u8> {
    let mut output = id.to_vec();
    output.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
    output.extend_from_slice(body);
    output
}

fn file(track_id: u8, track: &[u8]) -> Vec<u8> {
    let mut root = chunk([b'M', b'T', b'R', track_id], track);
    root.extend_from_slice(&[0, 0]);
    let mut output = b"MMMD".to_vec();
    output.extend_from_slice(&u32::try_from(root.len()).unwrap().to_be_bytes());
    output.extend(root);
    output
}

#[test]
fn both_score_parsers_poll_cancellation_even_for_ignored_messages() {
    for format in [0, 2] {
        let sequence = if format == 0 {
            [0, 0xff, 0].repeat(4096)
        } else {
            [0, 0xd0, 0].repeat(4096)
        };
        let context = TrackContext {
            format,
            number: 0,
            index: 0,
            duration_tick_micros: 1000,
            gate_tick_micros: 1000,
            channel_status: &[],
            wave_ids: &[],
        };
        let parse = if format == 0 {
            parse_handy_phone
        } else {
            parse_mobile
        };
        let mut state = DecodeState::new();
        let checks = std::cell::Cell::new(0);
        let error = parse(&sequence, &context, &mut state, Limits::default(), &|| {
            checks.set(checks.get() + 1);
            checks.get() == 2
        })
        .unwrap_err();
        assert_eq!(error.code(), "execution-cancelled");
        assert_eq!(checks.get(), 2);
        assert!(state.messages > 0 && state.messages < 4096);
        assert_eq!(state.events.len(), 1);

        let mut retried = DecodeState::new();
        parse(
            &sequence,
            &context,
            &mut retried,
            Limits::default(),
            &|| false,
        )
        .unwrap();
        assert_eq!(retried.messages, 4096);
    }
}

#[test]
fn score_timebase_rejects_reserved_values() {
    assert_eq!(timebase_micros(0x03).unwrap(), 5_000);
    assert_eq!(timebase_micros(0x13).unwrap(), 50_000);
    assert_eq!(
        timebase_micros(0x14).unwrap_err().code(),
        "media-unsupported-format"
    );
    assert_eq!(
        timebase_micros(0x20).unwrap_err().code(),
        "media-unsupported-format"
    );
}

#[test]
fn handy_phone_track_numbers_select_distinct_channel_groups() {
    let channel_status = [0xaa, 0xaa];
    let wave_ids = [];
    let context = |number| TrackContext {
        format: 0,
        number,
        index: 0,
        duration_tick_micros: 4_000,
        gate_tick_micros: 4_000,
        channel_status: &channel_status,
        wave_ids: &wave_ids,
    };

    assert_eq!(context(0).channel(0), 0);
    assert_eq!(context(0).channel(3), 3);
    assert_eq!(context(1).channel(0), 4);
    assert_eq!(context(1).channel(3), 7);
}

#[test]
fn mobile_stream_pcm_decodes_and_obeys_gate_time() {
    let wave = chunk(*b"Mwa\x01", &[0x20, 0x1f, 0x40, 0x10, 0x32, 0x54, 0x76]);
    let waves = chunk(*b"Mtsp", &wave);
    let sequence = chunk(*b"Mtsq", &[0, 0x90, 0, 127, 2, 2, 0xff, 0x2f, 0]);
    let mut track = vec![2, 0, 2, 2];
    track.extend_from_slice(&[0; 16]);
    track.extend(sequence);
    track.extend(waves);

    let clip = decode(&file(5, &track), Limits::default()).unwrap();
    assert_eq!(clip.duration_micros(), 8_000);
    assert_eq!(clip.samples.len(), 8 * OUTPUT_SAMPLE_RATE as usize / 1_000);
    assert!(clip.samples.iter().any(|sample| *sample != 0));
}

#[test]
fn both_score_generations_render_bounded_synthesis() {
    let mobile_sequence = chunk(
        *b"Mtsq",
        &[0, 0xc0, 5, 0, 0x90, 69, 100, 10, 10, 0xff, 0x2f, 0],
    );
    let mut mobile = vec![2, 0, 2, 2];
    mobile.extend_from_slice(&[0; 16]);
    mobile.extend(mobile_sequence);

    let handy_sequence = chunk(*b"Mtsq", &[0, 0x29, 10, 10, 0, 0, 0]);
    let mut handy = vec![0, 0, 2, 2, 0xaa, 0xaa];
    handy.extend(handy_sequence);

    for input in [file(5, &mobile), file(1, &handy)] {
        let clip = decode(&input, Limits::default()).unwrap();
        assert_eq!(clip.duration_micros(), 40_000);
        assert!(clip.samples.iter().any(|sample| *sample != 0));
    }
}

#[test]
fn score_chunk_boundary_can_terminate_both_sequence_formats() {
    let mobile_sequence = chunk(*b"Mtsq", &[0, 0x90, 69, 100, 10]);
    let mut mobile = vec![2, 0, 2, 2];
    mobile.extend_from_slice(&[0; 16]);
    mobile.extend(mobile_sequence);

    let handy_sequence = chunk(*b"Mtsq", &[0, 0x29, 10]);
    let mut handy = vec![0, 0, 2, 2, 0xaa, 0xaa];
    handy.extend(handy_sequence);

    for input in [file(5, &mobile), file(0, &handy)] {
        let clip = decode(&input, Limits::default()).unwrap();
        assert_eq!(clip.duration_micros(), 40_000);
        assert!(clip.samples.iter().any(|sample| *sample != 0));
    }
}

#[test]
fn malformed_and_oversized_smaf_are_rejected() {
    let sequence = chunk(*b"Mtsq", &[0, 0x90, 69, 100, 10, 10, 0xff, 0x2f, 0]);
    let mut track = vec![2, 0, 2, 2];
    track.extend_from_slice(&[0; 16]);
    track.extend(sequence);
    let mut input = file(5, &track);
    input[7] = input[7].wrapping_add(1);
    assert_eq!(
        decode(&input, Limits::default()).unwrap_err().code(),
        "media-malformed"
    );

    let limits = Limits {
        max_pcm_frames: 1,
        ..Limits::default()
    };
    assert_eq!(
        decode(&file(5, &track), limits).unwrap_err().code(),
        "media-limit"
    );

    let handy_sequence = chunk(*b"Mtsq", &[0, 0x20, 1]);
    let mut handy = vec![0, 0, 2, 2, 0xaa, 0xaa];
    handy.extend(handy_sequence);
    assert_eq!(
        decode(&file(0, &handy), Limits::default())
            .unwrap_err()
            .code(),
        "media-malformed"
    );

    let invalid_wave = chunk(*b"Mwa\0", &[0x20, 0x1f, 0x40, 0x10]);
    let waves = chunk(*b"Mtsp", &invalid_wave);
    let mut stream_track = vec![2, 0, 2, 2];
    stream_track.extend_from_slice(&[0; 16]);
    stream_track.extend(chunk(*b"Mtsq", &[0, 0x90, 0, 127, 1]));
    stream_track.extend(waves);
    assert_eq!(
        decode(&file(0, &stream_track), Limits::default())
            .unwrap_err()
            .code(),
        "media-malformed"
    );
}
