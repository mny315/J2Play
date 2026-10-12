use super::chunk;
use crate::{Limits, Runtime, tests::CaptureSink};

fn score_file(sequence: &[u8], with_pcm: bool) -> Vec<u8> {
    let mut track = vec![2, 0, 0, 0];
    track.extend([0; 16]);
    track.extend(chunk(*b"Mtsq", sequence));
    let mut body = chunk(*b"MTR\0", &track);
    if with_pcm {
        // Two ADPCM samples at the output rate, gated to one millisecond.
        let wave = chunk(*b"Mwa\x01", &[0x20, 0x56, 0x22, 0x10]);
        let mut pcm_track = vec![2, 0, 0, 0];
        pcm_track.extend([0; 16]);
        pcm_track.extend(chunk(*b"Mtsq", &[0, 0x90, 0, 127, 1, 1, 0xff, 0x2f, 0]));
        pcm_track.extend(chunk(*b"Mtsp", &wave));
        body.extend(chunk(*b"MTR\x01", &pcm_track));
    }
    body.extend([0, 0]);
    let mut file = b"MMMD".to_vec();
    file.extend(u32::try_from(body.len()).unwrap().to_be_bytes());
    file.extend(body);
    file
}

fn chord() -> Vec<u8> {
    let mut sequence = Vec::new();
    for channel in (0..16).filter(|channel| *channel != 9) {
        sequence.extend([0, 0x90 | channel, 69, 127, 40]);
    }
    sequence.extend([40, 0xff, 0x2f, 0]);
    sequence
}

fn render_file(bytes: &[u8]) -> (i64, Vec<i16>) {
    let mut runtime = Runtime::new(
        CaptureSink::default(),
        Limits {
            max_pcm_frames: 22_050,
            max_total_pcm_frames: 22_050,
            ..Limits::default()
        },
    );
    let handle = runtime.create_from_bytes("audio/mmf", bytes).unwrap();
    runtime.start(handle, 0).unwrap();
    let duration = runtime.duration(handle).unwrap();
    runtime.tick(500_000).unwrap();
    (duration, std::mem::take(&mut runtime.sink_mut().0))
}

#[test]
fn adding_stream_pcm_preserves_the_synthesized_drum_tail() {
    let sequence = [0, 0x99, 49, 127, 1, 1, 0xff, 0x2f, 0];
    let (duration, expected) = render_file(&score_file(&sequence, false));
    let (mixed_duration, actual) = render_file(&score_file(&sequence, true));
    assert!(duration > 1_000);
    assert!(expected[22..].iter().any(|sample| *sample != 0));
    assert_eq!(mixed_duration, duration);
    assert_eq!(&actual[22..], &expected[22..]);
}

#[test]
fn adding_stream_pcm_does_not_compress_unmixed_synthesis_again() {
    let sequence = chord();
    let (duration, expected) = render_file(&score_file(&sequence, false));
    let (mixed_duration, actual) = render_file(&score_file(&sequence, true));
    assert!(expected[22..].iter().any(|sample| sample.abs() > 28_000));
    assert_eq!(mixed_duration, duration);
    assert!(actual[22..] == expected[22..], "unmixed samples changed");
}

#[test]
#[ignore = "manual SMAF synthesis and stream mixing throughput measurement"]
fn smaf_mixing_throughput() {
    for (name, sequence) in [
        ("chord", chord()),
        ("drum", vec![0, 0x99, 49, 127, 1, 1, 0xff, 0x2f, 0]),
    ] {
        let bytes = score_file(&sequence, true);
        let start = std::time::Instant::now();
        let mut checksum = 0_i64;
        for _ in 0..1_000 {
            let (duration, samples) = render_file(std::hint::black_box(&bytes));
            checksum = checksum.wrapping_add(duration);
            checksum = checksum.wrapping_add(samples.iter().map(|sample| i64::from(*sample)).sum());
        }
        eprintln!("smaf-{name}: {:?}, checksum={checksum}", start.elapsed());
    }
}
