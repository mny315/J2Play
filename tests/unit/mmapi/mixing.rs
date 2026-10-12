use super::*;
use crate::EncodedMedia;

#[test]
fn mixer_matches_each_sample_at_loop_boundaries_clipping_and_saturated_cursors() {
    for frames in [0_usize, 1, 2, 5, 17, 256] {
        let clip = Clip {
            samples: (0..frames)
                .map(|index| [i16::MIN, -101, 0, 23, i16::MAX][index % 5])
                .collect(),
            duration_micros: 0,
        };
        for position in [
            0,
            1,
            frames.saturating_sub(1),
            frames,
            frames + 1,
            frames * 2 + 3,
            usize::MAX - 3,
            usize::MAX,
        ] {
            for loop_count in [-1, 1, 2, 4] {
                for volume in [0, 1, 37, 100] {
                    for muted in [false, true] {
                        for output_frames in [0, 1, 9, 257] {
                            let mut player = Player::new("audio/x-wav", EncodedMedia::Decoded);
                            player.rendered_frames = position;
                            player.loop_count = loop_count;
                            player.volume = volume;
                            player.muted = muted;
                            let mut mixed = (0..output_frames)
                                .map(|index| match index % 3 {
                                    0 => 17,
                                    1 => i32::MAX - 1,
                                    _ => i32::MIN + 1,
                                })
                                .collect::<Vec<_>>();
                            let mut expected = mixed.clone();
                            for (offset, output) in expected.iter_mut().enumerate() {
                                let cursor = position.saturating_add(offset);
                                let plays = if loop_count == -1 {
                                    usize::MAX
                                } else {
                                    usize::try_from(loop_count).unwrap()
                                };
                                if frames != 0 && !muted && cursor / frames < plays {
                                    let sample = i32::from(clip.samples[cursor % frames]);
                                    *output = output.saturating_add(sample * volume / 100);
                                }
                            }
                            if loop_count == 1 && volume == 100 && !muted {
                                let mut one_shot_output = mixed.clone();
                                let one_shot = OneShot {
                                    samples: clip.samples.clone(),
                                    rendered_frames: position,
                                };
                                mix_one_shot(&one_shot, &mut one_shot_output);
                                assert_eq!(one_shot_output, expected);
                            }
                            mix_player(&player, &clip, &mut mixed);
                            assert_eq!(
                                mixed, expected,
                                "frames={frames}, position={position}, loops={loop_count}, volume={volume}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "manual PCM mixer throughput measurement"]
fn pcm_mixer_throughput() {
    for frames in [1, 2, 5, 15, 16, 256, 22_050] {
        let clip = Clip {
            samples: (0..frames)
                .map(|index| [i16::MIN, -101, 0, 23, i16::MAX][index % 5])
                .collect(),
            duration_micros: 0,
        };
        for loops in [-1, 2] {
            for volume in [37, 100] {
                let mut player = Player::new("audio/x-wav", EncodedMedia::Decoded);
                player.rendered_frames = frames - 1;
                player.loop_count = loops;
                player.volume = volume;
                let mut mixed = [0_i32; 512];
                let started = std::time::Instant::now();
                for _ in 0..8_192 {
                    mixed.fill(0);
                    mix_player(
                        std::hint::black_box(&player),
                        std::hint::black_box(&clip),
                        std::hint::black_box(&mut mixed),
                    );
                }
                let elapsed = started.elapsed();
                let checksum = mixed.iter().fold(0_u64, |hash, sample| {
                    hash.wrapping_mul(31)
                        .wrapping_add(u64::from(sample.cast_unsigned()))
                });
                eprintln!(
                    "pcm frames={frames} loops={loops} volume={volume} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}
