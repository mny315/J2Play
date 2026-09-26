use super::*;
use crate::{Limits, decode_tone_sequence};

#[test]
fn nokia_sound_adapts_formats_gain_loops_and_frequency_tones() {
    assert_eq!(
        nokia_sound_content_type(5, b"RIFF....WAVE").unwrap(),
        "audio/x-wav"
    );
    assert_eq!(
        nokia_sound_content_type(1, &[254, 1, 60, 8]).unwrap(),
        "audio/x-tone-seq"
    );
    assert_eq!(nokia_sound_loop_count(0).unwrap(), -1);
    assert_eq!(nokia_sound_loop_count(3).unwrap(), 3);
    assert!(nokia_sound_loop_count(-1).is_err());
    assert_eq!(nokia_sound_gain(i32::MIN), 0);
    assert_eq!(nokia_sound_gain(0), 0);
    assert_eq!(nokia_sound_gain(1), 1);
    assert_eq!(nokia_sound_gain(128), 50);
    assert_eq!(nokia_sound_gain(255), 100);
    assert_eq!(nokia_sound_gain(i32::MAX), 100);

    let tone = nokia_frequency_tone(440, 1_000).unwrap();
    assert_eq!(&tone[..6], &[254, 1, 253, 120, 252, 100]);
    assert_eq!(tone[6], 69);
    let decoded = decode_tone_sequence(&tone, Limits::default(), &|| false).unwrap();
    assert_eq!(decoded.duration_micros(), 1_000_000);
    let silence = nokia_frequency_tone(0, 5).unwrap();
    assert_eq!(silence[6], u8::MAX);
    assert_eq!(
        decode_tone_sequence(&silence, Limits::default(), &|| false)
            .unwrap()
            .samples
            .as_ref(),
        vec![0; OUTPUT_SAMPLE_RATE as usize / 200]
    );
}
