use super::super::tests::chunk;
use super::*;
use crate::smaf::WaveTrigger;

#[test]
fn yamaha_adpcm_uses_low_nibble_first() {
    // Use a packed byte at the mixer rate so this also checks nibble order
    // in the wave reader, without resampling changing the two samples.
    let clip = decode_wave(&[0x20, 0x56, 0x22, 0x10], Limits::default(), &|| false).unwrap();
    assert_eq!(clip.samples.as_ref(), [15, 62]);
}

#[test]
fn cancellation_inside_one_wave_does_not_commit_a_partial_bank() {
    let mut wave = vec![0x20, 0x56, 0x22];
    wave.extend([0x12; 16_384]);
    let bank = chunk(*b"Mwa\x01", &wave);
    let mut state = DecodeState::new();
    let mut ids = Vec::new();
    let checks = std::cell::Cell::new(0);
    let error = parse_wave_bank(&bank, 0, &mut state, &mut ids, Limits::default(), &|| {
        checks.set(checks.get() + 1);
        checks.get() == 3
    })
    .unwrap_err();
    assert_eq!(error.code(), "execution-cancelled");
    assert_eq!(checks.get(), 3);
    assert_eq!(state.wave_frames, 0);
    assert!(state.waves.is_empty());
    assert!(ids.is_empty());
    parse_wave_bank(&bank, 0, &mut state, &mut ids, Limits::default(), &|| false).unwrap();
    assert_eq!(state.wave_frames, 32_768);
    assert_eq!(ids, [1]);
}

#[test]
fn wave_bank_memory_is_independent_of_the_synthesis_work_budget() {
    // Each project-owned 22.05 kHz ADPCM wave expands to eight PCM frames.
    let wave = [0x20, 0x56, 0x22, 0x10, 0x32, 0x54, 0x76];
    for work_limit in [0, 1_000_000] {
        let limits = Limits {
            max_pcm_frames: 8,
            max_total_pcm_frames: 8,
            max_midi_voice_frames: work_limit,
            ..Limits::default()
        };
        let mut state = DecodeState::new();
        let mut ids = Vec::new();
        parse_wave_bank(
            &chunk(*b"Mwa\x01", &wave),
            0,
            &mut state,
            &mut ids,
            limits,
            &|| false,
        )
        .unwrap();
        assert_eq!(state.wave_frames, 8);
        assert_eq!(
            parse_wave_bank(
                &chunk(*b"Mwa\x02", &wave),
                0,
                &mut state,
                &mut ids,
                limits,
                &|| false
            )
            .unwrap_err()
            .code(),
            "media-limit"
        );
        assert_eq!(state.waves.len(), 1);
        assert_eq!(state.wave_frames, 8);
        assert_eq!(ids, [1]);
    }
}

#[test]
fn stream_pcm_work_limit_is_not_reported_as_memory_pressure() {
    let mut state = DecodeState::new();
    state.end_micros = 1_000;
    state.waves.insert(
        (0, 1),
        Clip {
            samples: vec![127; 8].into_boxed_slice(),
            duration_micros: 1_000,
        },
    );
    state.triggers.push(WaveTrigger {
        key: (0, 1),
        start_micros: 0,
        gate_micros: 1_000,
        velocity: 127,
    });
    let silence = || PcmMix {
        samples: Vec::new(),
        duration_micros: 1_000,
    };
    let limits = Limits {
        max_midi_voice_frames: 8,
        ..Limits::default()
    };
    assert!(mix_stream_pcm(silence(), &state, limits, &|| false).is_ok());
    state.triggers.push(state.triggers[0]);
    assert_eq!(
        mix_stream_pcm(silence(), &state, limits, &|| false)
            .unwrap_err()
            .code(),
        "media-work-limit"
    );
}

#[test]
fn stream_pcm_mixing_polls_cancellation_within_a_long_wave() {
    let mut state = DecodeState::new();
    state.end_micros = 1_000_000;
    state.waves.insert(
        (0, 1),
        Clip {
            samples: vec![127; 12_000].into_boxed_slice(),
            duration_micros: 1_000_000,
        },
    );
    state.triggers.push(WaveTrigger {
        key: (0, 1),
        start_micros: 0,
        gate_micros: 1_000_000,
        velocity: 127,
    });
    let checks = std::cell::Cell::new(0);
    let error = mix_stream_pcm(
        PcmMix {
            samples: Vec::new(),
            duration_micros: 1_000_000,
        },
        &state,
        Limits::default(),
        &|| {
            checks.set(checks.get() + 1);
            checks.get() == 2
        },
    )
    .unwrap_err();
    assert_eq!(error.code(), "execution-cancelled");
    assert_eq!(checks.get(), 2);
    assert_eq!(state.waves[&(0, 1)].samples.as_ref(), vec![127; 12_000]);
}
