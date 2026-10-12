use super::*;
use std::cell::Cell;

fn pcm_fixture(bits: u16, channels: u16, frames: usize) -> (Vec<u8>, WavFormat, Vec<i16>) {
    let format = WavFormat {
        encoding: WavEncoding::Pcm,
        channels,
        sample_rate: super::super::OUTPUT_SAMPLE_RATE,
        block_align: channels * bits / 8,
        bits,
        samples_per_block: 1,
    };
    let mut encoded = Vec::new();
    let mut expected = Vec::new();
    for frame in 0..frames {
        let mut sum = 0_i32;
        for channel in 0..channels {
            let index = frame * usize::from(channels) + usize::from(channel);
            let sample = if bits == 8 {
                let byte = u8::try_from(index.wrapping_mul(37) & 255).unwrap();
                encoded.push(byte);
                (i32::from(byte) - 128) * 256
            } else {
                let sample = u16::try_from(index.wrapping_mul(7_919) & 65_535)
                    .unwrap()
                    .cast_signed();
                encoded.extend_from_slice(&sample.to_le_bytes());
                i32::from(sample)
            };
            sum += sample;
        }
        expected.push(i16::try_from(sum / i32::from(channels)).unwrap());
    }
    (encoded, format, expected)
}

#[test]
fn pcm_formats_preserve_samples_and_cancellation_boundaries() {
    for bits in [8, 16] {
        for channels in [1, 2] {
            for frames in [0, 1, 4_095, 4_096, 4_097, 65_537] {
                let (bytes, format, expected) = pcm_fixture(bits, channels, frames);
                let polls = Cell::new(0);
                let decoded = decode_pcm_wav_samples(&bytes, format, frames, &|| {
                    polls.set(polls.get() + 1);
                    false
                })
                .unwrap();
                assert_eq!(
                    decoded, expected,
                    "bits={bits}, channels={channels}, frames={frames}"
                );
                assert_eq!(polls.get(), frames.div_ceil(4_096));
                if frames > 4_096 {
                    polls.set(0);
                    let error = decode_pcm_wav_samples(&bytes, format, frames, &|| {
                        polls.set(polls.get() + 1);
                        polls.get() == 2
                    })
                    .unwrap_err();
                    assert_eq!(error.code(), "execution-cancelled");
                    assert_eq!(polls.get(), 2);
                }
            }
        }
    }
}

#[test]
#[ignore = "manual PCM decoding throughput measurement"]
fn pcm_wav_throughput() {
    for frames in [8, 4_096, 65_536] {
        for bits in [8, 16] {
            for channels in [1, 2] {
                let (bytes, format, expected) = pcm_fixture(bits, channels, frames);
                let iterations = if frames == 8 { 65_536 } else { 128 };
                let started = std::time::Instant::now();
                for _ in 0..iterations {
                    let samples = decode_pcm_wav_samples(
                        std::hint::black_box(&bytes),
                        std::hint::black_box(format),
                        frames,
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(samples, expected);
                }
                let elapsed = started.elapsed();
                eprintln!(
                    "WAV frames={frames} bits={bits} channels={channels} elapsed={elapsed:?}"
                );
            }
        }
    }
}
