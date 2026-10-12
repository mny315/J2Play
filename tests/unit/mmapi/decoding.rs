use super::*;

use crate::{midi::parse_midi_track, synthesis::scale_synth_sample};

fn decode_amr_nb(bytes: &[u8], limits: Limits) -> Result<Clip, EmuError> {
    crate::media_decode::decode_amr_nb(bytes, limits, &|| false)
}

fn decode_symphonia(bytes: Arc<[u8]>, extension: &str, limits: Limits) -> Result<Clip, EmuError> {
    crate::media_decode::decode_symphonia(bytes, extension, limits, &|| false)
}

fn decode_midi(bytes: &[u8], limits: Limits) -> Result<Clip, EmuError> {
    crate::decode_midi(bytes, limits, &|| false)
}

fn render_midi_events(
    events: &[MidiEvent],
    division: u16,
    limits: Limits,
) -> Result<Clip, EmuError> {
    crate::render_midi_events(events, division, limits, &|| false)
        .map(crate::midi::PcmMix::into_clip)
}

#[test]
fn mono_resampling_reuses_owned_storage_in_both_directions() {
    let same_rate = vec![1_i16, 2, 3];
    let original_pointer = same_rate.as_ptr();
    let same_rate = resample_mono(same_rate, OUTPUT_SAMPLE_RATE, Limits::default()).unwrap();
    assert_eq!(same_rate.samples.as_ptr(), original_pointer);
    assert_eq!(same_rate.samples.as_ref(), [1, 2, 3]);

    let upsampled = resample_mono(vec![0, 22_050], 11_025, Limits::default()).unwrap();
    assert_eq!(upsampled.samples.as_ref(), [0, 11_025, 22_050, 22_050]);

    let downsampled = resample_mono(vec![0, 100, 200, 300], 44_100, Limits::default()).unwrap();
    assert_eq!(downsampled.samples.as_ref(), [0, 200]);

    let extremes = [i16::MIN, i16::MAX, i16::MIN, i16::MAX];
    let upsampled = resample_mono(extremes.to_vec(), 11_025, Limits::default()).unwrap();
    assert_eq!(
        upsampled.samples.as_ref(),
        [i16::MIN, 0, i16::MAX, 0, i16::MIN, 0, i16::MAX, i16::MAX]
    );
    let downsampled = resample_mono(extremes.to_vec(), 33_075, Limits::default()).unwrap();
    assert_eq!(downsampled.samples.as_ref(), [i16::MIN, 0]);
}

#[test]
fn ima_adpcm_accepts_legacy_nominal_byte_rate() {
    let mut wav = ima_adpcm_wav_constant();
    // Some encoders record the nominal 4-bit payload rate without the
    // four-byte predictor block header.
    wav[28..32].copy_from_slice(&(8_000_u32 / 2).to_le_bytes());
    let clip = decode_wav(&wav, Limits::default()).unwrap();
    assert_eq!(clip.samples.len(), 24);
    assert_eq!(clip.duration_micros, 1_125);
}

#[test]
fn synth_scaling_keeps_normal_q15_amplitude() {
    for (wave, amplitude, envelope, expected) in [
        (32_767, 18_000, 32_767, 17_999),
        (-32_768, 18_000, 32_767, -18_000),
        (32_768, 18_000, 16_384, 9_000),
        (0, 18_000, 32_767, 0),
        (32_767, 0, 32_767, 0),
        (32_767, 18_000, 0, 0),
        (32_767, 40_000, 32_767, i16::MAX),
        (-32_768, 40_000, 32_767, i16::MIN),
    ] {
        assert_eq!(
            scale_synth_sample(wave, amplitude, envelope),
            expected,
            "wave={wave}, amplitude={amplitude}, envelope={envelope}"
        );
    }
}

#[test]
fn tone_sequence_is_bounded_and_audible_at_mixer_rate() {
    let sequence = [
        0xfe, 1, 0xfd, 30, 0xfc, 64, 60, 8, 0xf8, 50, 0xff, 8, 0xf7, 2, 64, 8,
    ];
    let clip = decode_tone_sequence(&sequence, Limits::default()).unwrap();
    assert_eq!(clip.duration_micros(), 1_000_000);
    assert_eq!(clip.samples.len(), OUTPUT_SAMPLE_RATE as usize);
    let peak = clip
        .samples
        .iter()
        .map(|sample| sample.unsigned_abs())
        .max()
        .unwrap_or(0);
    assert!(peak > 5_000);
    assert!(
        clip.samples
            .iter()
            .all(|sample| sample.unsigned_abs() <= 18_000)
    );
}

#[test]
fn midi_format_zero_decodes_tempo_and_note() {
    let midi = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk\0\0\0\x13\0\xff\x51\x03\x07\xa1\x20\0\x90\x45\x7f\x60\x80\x45\0\0\xff\x2f\0";
    let clip = decode_midi(midi, Limits::default()).unwrap();
    assert_eq!(clip.duration_micros(), 500_000);
    let peak = clip
        .samples
        .iter()
        .map(|sample| sample.unsigned_abs())
        .max()
        .unwrap_or(0);
    assert!(peak > 1_000);
}

#[test]
fn midi_accepts_small_truncation_of_the_final_legacy_track() {
    let mut midi = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk\0\0\0\x1a\0\xff\x51\x03\x07\xa1\x20\0\x90\x45\x7f\x60\x80\x45\0\0\xff\x51\x03\x07\xa1\x20\0\xff\x2f\0".to_vec();
    midi.truncate(midi.len() - 6);
    let clip = decode_midi(&midi, Limits::default()).unwrap();
    assert_eq!(clip.duration_micros(), 500_000);
    let peak = clip
        .samples
        .iter()
        .map(|sample| sample.unsigned_abs())
        .max()
        .unwrap_or(0);
    assert!(peak > 1_000);
}

#[test]
fn midi_accepts_bounded_zero_padding_after_the_final_track() {
    let midi = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk\0\0\0\x13\0\xff\x51\x03\x07\xa1\x20\0\x90\x45\x7f\x60\x80\x45\0\0\xff\x2f\0";
    let mut padded = midi.to_vec();
    padded.resize(padded.len() + 548, 0);

    let clip = decode_midi(&padded, Limits::default()).unwrap();
    assert_eq!(clip.duration_micros(), 500_000);

    padded[midi.len()] = 1;
    assert_eq!(
        decode_midi(&padded, Limits::default()).unwrap_err().code(),
        "media-malformed"
    );

    let mut excessive = midi.to_vec();
    excessive.resize(excessive.len() + MAX_MIDI_ZERO_PADDING_BYTES + 1, 0);
    assert_eq!(
        decode_midi(&excessive, Limits::default())
            .unwrap_err()
            .code(),
        "media-malformed"
    );
}

#[test]
fn midi_accepts_sysex_terminator_inside_smf_payload() {
    let midi = [
        b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 1, 0xe0, b'M', b'T', b'r', b'k', 0, 0, 0,
        12, 0, 0xf0, 5, 0x7e, 0x7f, 0x09, 0x01, 0xf7, 0, 0xff, 0x2f, 0,
    ];
    let clip = decode_midi(&midi, Limits::default()).unwrap();
    assert!(clip.samples.is_empty());
}

#[test]
fn midi_accepts_more_than_sixteen_tracks_within_bounded_limit() {
    let mut midi = vec![b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 1, 0, 17, 1, 0xe0];
    for _ in 0..17 {
        midi.extend_from_slice(&[b'M', b'T', b'r', b'k', 0, 0, 0, 4, 0, 0xff, 0x2f, 0]);
    }
    let clip = decode_midi(&midi, Limits::default()).unwrap();
    assert!(clip.samples.is_empty());
}

#[test]
fn common_java_me_audio_mime_aliases_are_canonicalized() {
    for value in [
        "audio/wav",
        "audio/wave",
        "audio/vnd.wave",
        "audio/x-wav; codecs=1",
    ] {
        assert_eq!(canonical_content_type(value).unwrap(), "audio/x-wav");
    }
    for value in [
        "audio/midi",
        "audio/mid",
        "audio/x-mid",
        "audio/x-midi",
        "audio/sp-midi",
    ] {
        assert_eq!(canonical_content_type(value).unwrap(), "audio/midi");
    }
    for value in [
        "audio/mpeg",
        "audio/mp3",
        "audio/x-mp3",
        "audio/mpeg3",
        "audio/x-mpeg",
    ] {
        assert_eq!(canonical_content_type(value).unwrap(), "audio/mpeg");
    }
    for value in ["audio/mp4", "audio/mp4a-latm", "audio/aac", "audio/x-aac"] {
        assert_eq!(canonical_content_type(value).unwrap(), "audio/mp4a-latm");
    }
    for value in ["audio/amr", "audio/amr-nb", "audio/AMR; rate=8000"] {
        assert_eq!(canonical_content_type(value).unwrap(), "audio/amr");
    }
    for value in [
        "audio/mmf",
        "application/vnd.smaf",
        "application/vnd.yamaha.smaf-audio",
    ] {
        assert_eq!(canonical_content_type(value).unwrap(), "audio/mmf");
    }
    assert_eq!(content_type_from_path("/sound.MP3").unwrap(), "audio/mpeg");
    assert_eq!(
        content_type_from_path("/sound.M4A").unwrap(),
        "audio/mp4a-latm"
    );
    assert_eq!(content_type_from_path("/sound.AMR").unwrap(), "audio/amr");
    assert_eq!(content_type_from_path("/sound.MMF").unwrap(), "audio/mmf");
}

#[test]
fn amr_nb_decodes_and_resamples_to_the_mixer_rate() {
    let mut amr = amr_nb::FILE_MAGIC.to_vec();
    amr.push(0x3c);
    amr.extend_from_slice(&[0; 31]);
    let clip = decode_amr_nb(&amr, Limits::default()).unwrap();
    assert_eq!(
        clip.samples.len(),
        160 * OUTPUT_SAMPLE_RATE as usize / amr_nb::SAMPLE_RATE as usize
    );
    assert_eq!(clip.duration_micros(), 20_000);
}

#[test]
fn mp3_and_mp4_aac_decode_to_bounded_mono_pcm() {
    let mp3 = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/audio/tone-440.mp3"
    ));
    let aac = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/audio/tone-660.m4a"
    ));
    for (bytes, extension) in [(mp3.as_slice(), "mp3"), (aac.as_slice(), "m4a")] {
        let clip = decode_symphonia(Arc::from(bytes), extension, Limits::default()).unwrap();
        assert!(!clip.samples.is_empty());
        assert!(clip.samples.len() <= OUTPUT_SAMPLE_RATE as usize);
        assert!(clip.samples.iter().any(|sample| *sample != 0));
        let mut limits = Limits {
            max_pcm_frames: clip.samples.len() - 1,
            ..Limits::default()
        };
        assert_eq!(
            decode_symphonia(Arc::from(bytes), extension, limits)
                .unwrap_err()
                .code(),
            "media-limit"
        );
        limits.max_pcm_frames += 1;
        assert_eq!(
            decode_symphonia(Arc::from(bytes), extension, limits)
                .unwrap()
                .samples,
            clip.samples
        );
    }
}

#[test]
fn ima_adpcm_wav_decodes_and_resamples() {
    let clip = decode_wav(&ima_adpcm_wav_constant(), Limits::default()).unwrap();
    assert_eq!(clip.samples.len(), 9 * OUTPUT_SAMPLE_RATE as usize / 8_000);
    assert!(clip.samples.iter().all(|sample| *sample == 1_000));
}

#[test]
fn midi_parser_keeps_program_changes_and_percussion() {
    let track = [
        0, 0xc0, 80, // synth lead on channel 0
        0, 0x90, 60, 100, // melodic note
        0, 0x99, 36, 127, // kick drum on channel 10
        0x60, 0x80, 60, 0, 0, 0xff, 0x2f, 0,
    ];
    let mut events = Vec::new();
    parse_midi_track(&track, &mut events, 100, &|| false).unwrap();
    assert!(events.iter().any(|event| matches!(
        event.kind,
        MidiKind::ProgramChange {
            channel: 0,
            program: 80
        }
    )));
    assert!(events.iter().any(|event| matches!(
        event.kind,
        MidiKind::NoteOn {
            channel: 9,
            note: 36,
            velocity: 127
        }
    )));
    let clip = render_midi_events(&events, 96, Limits::default()).unwrap();
    let peak = clip
        .samples
        .iter()
        .map(|sample| sample.unsigned_abs())
        .max()
        .unwrap_or(0);
    assert!(peak > 1_000);
}

#[test]
fn malformed_and_oversized_media_are_controlled() {
    let limits = Limits {
        max_input_bytes: 4,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    let compressed = runtime.create_from_bytes("audio/mpeg", b"x").unwrap();
    assert_eq!(
        runtime.realize(compressed).unwrap_err().code(),
        "media-malformed"
    );
    assert_eq!(
        runtime
            .create_from_bytes("audio/x-wav", b"oversized")
            .unwrap_err()
            .code(),
        "media-limit"
    );
    let mut runtime = Runtime::new(NullAudioSink::default(), Limits::default());
    let handle = runtime.create_from_bytes("audio/x-wav", b"bad").unwrap();
    assert_eq!(
        runtime.realize(handle).unwrap_err().code(),
        "media-malformed"
    );
    assert_eq!(runtime.state(handle, 0).unwrap(), PlayerState::Unrealized);

    let invalid_format_zero =
        b"MThd\0\0\0\x06\0\0\0\x02\0\x60MTrk\0\0\0\x04\0\xff\x2f\0MTrk\0\0\0\x04\0\xff\x2f\0";
    assert_eq!(
        decode_midi(invalid_format_zero, Limits::default())
            .unwrap_err()
            .code(),
        "media-unsupported-format"
    );

    let missing_end = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk\0\0\0\x04\0\x90\x45\x7f";
    assert_eq!(
        decode_midi(missing_end, Limits::default())
            .unwrap_err()
            .code(),
        "media-malformed"
    );
}

#[test]
fn midi_synthesis_work_is_bounded_before_rendering() {
    let events = [
        MidiEvent {
            tick: 0,
            order: 0,
            kind: MidiKind::NoteOn {
                channel: 0,
                note: 69,
                velocity: 100,
            },
        },
        MidiEvent {
            tick: 96,
            order: 1,
            kind: MidiKind::NoteOff {
                channel: 0,
                note: 69,
            },
        },
    ];
    let limits = Limits {
        max_midi_voice_frames: OUTPUT_SAMPLE_RATE as usize / 2 - 1,
        ..Limits::default()
    };
    assert_eq!(
        render_midi_events(&events, 96, limits).unwrap_err().code(),
        "media-work-limit"
    );
}

#[test]
fn multi_minute_polyphonic_midi_fits_default_work_budget() {
    // A two-minute chord with 17 simultaneous voices.
    let mut events = Vec::new();
    for note in 48..65 {
        events.push(MidiEvent {
            tick: 0,
            order: events.len(),
            kind: MidiKind::NoteOn {
                channel: 0,
                note,
                velocity: 80,
            },
        });
    }
    for note in 48..65 {
        events.push(MidiEvent {
            tick: 240,
            order: events.len(),
            kind: MidiKind::NoteOff { channel: 0, note },
        });
    }
    let clip = render_midi_events(&events, 1, Limits::default()).unwrap();
    assert_eq!(clip.duration_micros(), 120_000_000);
    assert_eq!(clip.samples.len(), OUTPUT_SAMPLE_RATE as usize * 120);
    assert!(
        clip.samples
            .iter()
            .any(|sample| sample.unsigned_abs() > 1_000)
    );
}

#[test]
fn short_arbitrary_codec_inputs_never_panic() {
    for length in 0_u8..128 {
        let bytes = (0..length)
            .map(|index| index.wrapping_mul(73).wrapping_add(19))
            .collect::<Vec<_>>();
        let _ = decode_wav(&bytes, Limits::default());
        let _ = decode_midi(&bytes, Limits::default());
        let _ = decode_tone_sequence(&bytes, Limits::default());
    }
    let duplicate_tempo = [0xfe, 1, 0xfd, 30, 0xfd, 31, 60, 1];
    assert_eq!(
        decode_tone_sequence(&duplicate_tempo, Limits::default())
            .unwrap_err()
            .code(),
        "tone-sequence"
    );
}
