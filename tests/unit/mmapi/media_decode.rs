use super::*;

fn interleaved_fixture(frames: usize, channels: usize) -> Vec<i16> {
    (0..frames * channels)
        .map(|index| match index % 13 {
            0 => i16::MIN,
            1 => i16::MAX,
            2 => -1,
            3 => 0,
            4 => 1,
            _ => u16::try_from(index.wrapping_mul(7_919) & 65_535)
                .unwrap()
                .cast_signed(),
        })
        .collect()
}

#[test]
fn downmix_preserves_signed_averages_and_existing_samples() {
    for channels in 1..=8 {
        for frames in [0, 1, 7, 4_095, 4_096, 4_097] {
            let interleaved = interleaved_fixture(frames, channels);
            let mut expected = vec![i16::MIN, 0, i16::MAX];
            expected.extend(interleaved.chunks_exact(channels).map(|frame| {
                let sum: i32 = frame.iter().map(|sample| i32::from(*sample)).sum();
                i16::try_from(sum / i32::try_from(channels).unwrap()).unwrap()
            }));
            let mut actual = vec![i16::MIN, 0, i16::MAX];
            append_mono_samples(&mut actual, &interleaved, channels);
            assert_eq!(actual, expected, "channels={channels}, frames={frames}");
        }
    }
}

#[test]
fn resampling_matches_linear_interpolation_at_integer_and_fractional_rates() {
    for rate in [
        8_000,
        11_025,
        22_049,
        22_050,
        22_051,
        44_100,
        48_000,
        66_150,
        88_200,
        192_000,
        4_294_965_150,
    ] {
        for frames in [0, 1, 2, 3, 7, 127, 4_095, 4_096, 4_097, 16_386] {
            let source = interleaved_fixture(frames, 1);
            let count = frames * OUTPUT_SAMPLE_RATE as usize / rate as usize;
            let expected: Vec<i16> = (0..count)
                .map(|index| {
                    let position = u64::try_from(index).unwrap() * u64::from(rate);
                    let base = usize::try_from(position / u64::from(OUTPUT_SAMPLE_RATE)).unwrap();
                    let fraction = i64::try_from(position % u64::from(OUTPUT_SAMPLE_RATE)).unwrap();
                    let left = i64::from(source[base.min(frames - 1)]);
                    let right = i64::from(source[(base + 1).min(frames - 1)]);
                    i16::try_from(
                        (left * (i64::from(OUTPUT_SAMPLE_RATE) - fraction) + right * fraction)
                            / i64::from(OUTPUT_SAMPLE_RATE),
                    )
                    .unwrap()
                })
                .collect();
            let clip = resample_mono(source, rate, Limits::default(), &|| false).unwrap();
            assert_eq!(
                clip.samples.as_ref(),
                expected,
                "rate={rate}, frames={frames}"
            );
            assert_eq!(
                clip.duration_micros,
                i64::try_from(u64::try_from(frames).unwrap() * 1_000_000 / u64::from(rate))
                    .unwrap()
            );
        }
    }
}

#[test]
fn integer_downsampling_retains_bounded_cancellation_checks() {
    use std::cell::Cell;

    for factor in [2, 3, 4] {
        for cancel_at in 1..=5 {
            let calls = Cell::new(0);
            let result = resample_mono(
                interleaved_fixture(8_193 * factor, 1),
                OUTPUT_SAMPLE_RATE * u32::try_from(factor).unwrap(),
                Limits::default(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == cancel_at
                },
            );
            if cancel_at <= 4 {
                assert_eq!(result.unwrap_err().code(), "execution-cancelled");
                assert_eq!(calls.get(), cancel_at);
            } else {
                assert_eq!(result.unwrap().samples.len(), 8_193);
                assert_eq!(calls.get(), 4);
            }
        }
    }
}

#[test]
#[ignore = "manual audio resampling throughput measurement"]
fn resampling_throughput() {
    for frames in [16, 4_096, 44_100] {
        for rate in [
            8_000, 11_025, 22_050, 44_100, 48_000, 66_150, 88_200, 192_000,
        ] {
            let source = interleaved_fixture(frames, 1);
            let iterations = match frames {
                16 => 8_192,
                4_096 => 512,
                _ => 64,
            };
            let inputs: Vec<_> = (0..iterations).map(|_| source.clone()).collect();
            let mut clip = None;
            let started = std::time::Instant::now();
            for source in inputs {
                clip = Some(
                    resample_mono(
                        std::hint::black_box(source),
                        std::hint::black_box(rate),
                        Limits::default(),
                        &|| false,
                    )
                    .unwrap(),
                );
                std::hint::black_box(&clip);
            }
            let elapsed = started.elapsed();
            let clip = clip.unwrap();
            let checksum = clip
                .samples
                .iter()
                .fold(0xcbf2_9ce4_8422_2325_u64, |hash, sample| {
                    (hash ^ u64::from(sample.cast_unsigned())).wrapping_mul(0x0100_0000_01b3)
                });
            eprintln!(
                "resample frames={frames} rate={rate} elapsed={elapsed:?} output={} duration={} checksum={checksum:016x}",
                clip.samples.len(),
                clip.duration_micros
            );
        }
    }
}

#[test]
#[ignore = "manual compressed-audio downmix throughput measurement"]
fn downmix_throughput() {
    for frames in [8, 1_152, 8_192] {
        for channels in [1, 2, 3, 6, 8] {
            let interleaved = interleaved_fixture(frames, channels);
            let mut samples = Vec::with_capacity(frames);
            let iterations = if frames == 8 { 262_144 } else { 2_048 };
            let started = std::time::Instant::now();
            for _ in 0..iterations {
                samples.clear();
                append_mono_samples(
                    std::hint::black_box(&mut samples),
                    std::hint::black_box(&interleaved),
                    std::hint::black_box(channels),
                );
                std::hint::black_box(&samples);
            }
            let elapsed = started.elapsed();
            let checksum = samples
                .iter()
                .fold(0xcbf2_9ce4_8422_2325_u64, |hash, sample| {
                    (hash ^ u64::from(sample.cast_unsigned())).wrapping_mul(0x0100_0000_01b3)
                });
            eprintln!(
                "downmix frames={frames} channels={channels} elapsed={elapsed:?} checksum={checksum:016x}"
            );
        }
    }
}

#[test]
#[ignore = "manual compressed-audio decoding throughput measurement"]
fn compressed_audio_throughput() {
    let fixtures: &[(&[u8], &str)] = &[
        (include_bytes!("../../fixtures/audio/tone-440.mp3"), "mp3"),
        (include_bytes!("../../fixtures/audio/tone-660.m4a"), "m4a"),
    ];
    for &(bytes, extension) in fixtures {
        let bytes: Arc<[u8]> = Arc::from(bytes);
        let mut clip = None;
        let started = std::time::Instant::now();
        for _ in 0..512 {
            clip = Some(
                decode_symphonia(
                    Arc::clone(std::hint::black_box(&bytes)),
                    extension,
                    Limits::default(),
                    &|| false,
                )
                .unwrap(),
            );
            std::hint::black_box(&clip);
        }
        let elapsed = started.elapsed();
        let clip = clip.unwrap();
        let checksum = clip
            .samples
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, sample| {
                (hash ^ u64::from(sample.cast_unsigned())).wrapping_mul(0x0100_0000_01b3)
            });
        eprintln!(
            "decode format={extension} elapsed={elapsed:?} frames={} checksum={checksum:016x}",
            clip.samples.len()
        );
    }
}
