use super::*;

#[test]
fn checkpoint_validates_keyframe_shape_and_order_without_rejecting_unfinished_sequences() {
    let mut valid = KeyframeSequenceState::new(3, 1, Interpolation::Linear).unwrap();
    valid.validate_checkpoint().unwrap(); // Duration and keyframes may still be unset.
    valid.set_keyframe(0, 20, &[2.0]).unwrap();
    valid.set_keyframe(1, 10, &[1.0]).unwrap();
    valid.set_keyframe(2, 0, &[0.0]).unwrap();
    valid.set_valid_range(2, 1).unwrap();
    let restored: KeyframeSequenceState =
        save_state::decode(&save_state::encode(&valid).unwrap()).unwrap();
    restored.validate_checkpoint().unwrap(); // An unfinished range may be unordered.
    for corruption in 0..7 {
        let mut invalid = valid.clone();
        match corruption {
            0 => invalid.components = usize::MAX,
            1 => {
                invalid.values.pop();
            }
            2 => invalid.valid_range = (0, 3),
            3 => invalid.times[0] = -1,
            4 => invalid.values[0] = f32::NAN,
            5 => invalid.unordered_pairs += 1,
            _ => invalid.interpolation = Interpolation::Squad,
        }
        let limits = crate::ArenaLimits::default();
        let mut runtime = crate::Runtime::new_with_graph_depth(limits, 32);
        runtime
            .create(None, crate::ObjectKind::KeyframeSequence(invalid))
            .unwrap();
        assert!(
            runtime
                .validate_checkpoint(limits, 32, 1024, |_| true)
                .is_err()
        );
    }
}

fn sequence(interpolation: Interpolation) -> KeyframeSequenceState {
    let components = if matches!(interpolation, Interpolation::Slerp | Interpolation::Squad) {
        4
    } else {
        1
    };
    let mut sequence = KeyframeSequenceState::new(2, components, interpolation).unwrap();
    sequence.set_duration(100).unwrap();
    sequence
}

#[test]
fn step_linear_spline_and_loop_have_observable_semantics() {
    let mut step = sequence(Interpolation::Step);
    step.set_keyframe(0, 0, &[2.0]).unwrap();
    step.set_keyframe(1, 100, &[6.0]).unwrap();
    assert_eq!(step.sample(50.0).unwrap(), [2.0]);

    let mut linear = sequence(Interpolation::Linear);
    linear.set_keyframe(0, 0, &[2.0]).unwrap();
    linear.set_keyframe(1, 100, &[6.0]).unwrap();
    assert_eq!(linear.sample(50.0).unwrap(), [4.0]);
    linear.set_repeat_mode(RepeatMode::Loop);
    assert_eq!(linear.sample(150.0).unwrap(), [4.0]);

    let mut spline = sequence(Interpolation::Spline);
    spline.set_keyframe(0, 0, &[0.0]).unwrap();
    spline.set_keyframe(1, 100, &[1.0]).unwrap();
    let middle = spline.sample(50.0).unwrap()[0];
    assert!((middle - 0.5).abs() < 1.0e-6);
}

#[test]
fn opposite_large_keys_keep_interpolation_finite_and_preserve_endpoints() {
    for interpolation in [Interpolation::Linear, Interpolation::Spline] {
        let mut sequence = sequence(interpolation);
        sequence.set_keyframe(0, 0, &[-f32::MAX]).unwrap();
        sequence.set_keyframe(1, 100, &[f32::MAX]).unwrap();
        for time in 0..=100 {
            let value = sequence.sample(time as f32).unwrap()[0];
            assert!(value.is_finite(), "{interpolation:?} at {time}: {value}");
            match time.cmp(&50) {
                std::cmp::Ordering::Less => assert!(value < 0.0),
                std::cmp::Ordering::Greater => assert!(value > 0.0),
                std::cmp::Ordering::Equal => assert_eq!(value, 0.0),
            }
        }
        assert_eq!(sequence.sample(0.0).unwrap(), [-f32::MAX]);
        assert_eq!(sequence.sample(100.0).unwrap(), [f32::MAX]);
    }
}

#[test]
fn large_spline_tangents_remain_finite_inside_segments_and_across_loop_seams() {
    for interpolation in [Interpolation::Linear, Interpolation::Spline] {
        let mut sequence = KeyframeSequenceState::new(4, 1, interpolation).unwrap();
        sequence.set_duration(80).unwrap();
        sequence.set_repeat_mode(RepeatMode::Loop);
        for (index, (time, value)) in [
            (10, -f32::MAX),
            (30, -f32::MAX),
            (50, f32::MAX),
            (70, f32::MAX),
        ]
        .into_iter()
        .enumerate()
        {
            sequence.set_keyframe(index, time, &[value]).unwrap();
        }
        for time in [0.0, 40.0, 80.0, -40.0] {
            assert_eq!(sequence.sample(time).unwrap(), [0.0]);
        }
        for (time, expected) in [(10.0, -f32::MAX), (50.0, f32::MAX)] {
            assert_eq!(sequence.sample(time).unwrap(), [expected]);
        }
    }
}

#[test]
fn constant_splines_have_zero_tangents_at_the_selected_endpoints() {
    let mut sequence = sequence(Interpolation::Spline);
    sequence.set_keyframe(0, 0, &[0.0]).unwrap();
    sequence.set_keyframe(1, 80, &[8.0]).unwrap();
    assert_eq!(sequence.sample(20.0).unwrap(), [1.25]);
    assert_eq!(sequence.sample(60.0).unwrap(), [6.75]);
}

#[test]
fn non_uniform_spline_intervals_preserve_linear_motion_between_interior_keys() {
    let mut sequence = KeyframeSequenceState::new(4, 2, Interpolation::Spline).unwrap();
    sequence.set_duration(100).unwrap();
    for (index, time) in [0, 10, 40, 100].into_iter().enumerate() {
        sequence
            .set_keyframe(index, time, &[time as f32, 2.0 * time as f32])
            .unwrap();
    }
    for tick in 40..160 {
        let time = tick as f32 * 0.25;
        let value = sequence.sample(time).unwrap();
        assert!(
            (value[0] - time).abs() < 1.0e-5,
            "time {time}, value {value:?}"
        );
        assert!((value[1] - 2.0 * time).abs() < 2.0e-5);
    }
}

#[test]
fn looping_interpolation_matches_replicating_keys_in_both_directions() {
    for interpolation in [Interpolation::Linear, Interpolation::Spline] {
        let mut looping = KeyframeSequenceState::new(4, 1, interpolation).unwrap();
        let mut replicated = KeyframeSequenceState::new(12, 1, interpolation).unwrap();
        looping.set_duration(80).unwrap();
        looping.set_repeat_mode(RepeatMode::Loop);
        replicated.set_duration(240).unwrap();
        for (index, (time, value)) in [(10, 2.0), (20, -3.0), (45, 6.0), (70, 1.0)]
            .into_iter()
            .enumerate()
        {
            looping.set_keyframe(index, time, &[value]).unwrap();
            for cycle in 0..3 {
                replicated
                    .set_keyframe(index + 4 * cycle, time + 80 * cycle as i32, &[value])
                    .unwrap();
            }
        }
        for tick in 0..320 {
            let time = tick as f32 * 0.25;
            assert_eq!(
                looping.sample(time).unwrap(),
                replicated.sample(time + 80.0).unwrap()
            );
        }
    }
}

#[test]
fn keyframe_segments_include_the_start_and_select_the_last_coincident_key() {
    for interpolation in [Interpolation::Step, Interpolation::Linear] {
        for times in [[10, 20, 20, 40], [10, 10, 20, 40], [10, 20, 40, 40]] {
            let mut sequence = KeyframeSequenceState::new(4, 1, interpolation).unwrap();
            sequence.set_duration(50).unwrap();
            for (index, time) in times.into_iter().enumerate() {
                sequence
                    .set_keyframe(index, time, &[(index + 1) as f32])
                    .unwrap();
            }
            assert_eq!(sequence.sample(9.0).unwrap(), [1.0]);
            for time in times {
                let last_coincident = times.iter().rposition(|value| *value == time).unwrap();
                assert_eq!(
                    sequence.sample(time as f32).unwrap(),
                    [(last_coincident + 1) as f32]
                );
            }
            assert_eq!(sequence.sample(41.0).unwrap(), [4.0]);
        }
    }
}

#[test]
fn looping_keyframes_interpolate_across_the_end_of_each_cycle() {
    let mut sequence = sequence(Interpolation::Linear);
    sequence.set_keyframe(0, 20, &[2.0]).unwrap();
    sequence.set_keyframe(1, 60, &[6.0]).unwrap();
    sequence.set_duration(80).unwrap();
    sequence.set_repeat_mode(RepeatMode::Loop);
    for (time, expected) in [
        (0, 4.0),
        (10, 3.0),
        (20, 2.0),
        (40, 4.0),
        (60, 6.0),
        (70, 5.0),
    ] {
        for cycle in -3..=3 {
            assert_eq!(
                sequence.sample((time + cycle * 80) as f32).unwrap(),
                [expected]
            );
        }
    }
}

#[test]
fn loop_seams_preserve_step_values_and_coincident_key_order() {
    for interpolation in [Interpolation::Step, Interpolation::Linear] {
        let mut sequence = KeyframeSequenceState::new(4, 1, interpolation).unwrap();
        sequence.set_duration(80).unwrap();
        sequence.set_repeat_mode(RepeatMode::Loop);
        for (index, time) in [0, 0, 80, 80].into_iter().enumerate() {
            sequence
                .set_keyframe(index, time, &[(index + 1) as f32])
                .unwrap();
        }
        for time in [-80.0, 0.0, 80.0, 160.0] {
            assert_eq!(sequence.sample(time).unwrap(), [2.0]);
        }
    }
    let mut sequence = sequence(Interpolation::Step);
    sequence.set_keyframe(0, 20, &[2.0]).unwrap();
    sequence.set_keyframe(1, 60, &[6.0]).unwrap();
    sequence.set_duration(80).unwrap();
    sequence.set_repeat_mode(RepeatMode::Loop);
    for time in [-10.0, 0.0, 10.0, 60.0, 70.0, 80.0] {
        assert_eq!(sequence.sample(time).unwrap(), [6.0]);
    }
    assert_eq!(sequence.sample(20.0).unwrap(), [2.0]);
}

#[test]
fn valid_keyframes_can_wrap_in_storage_and_be_updated_out_of_order() {
    let mut sequence = KeyframeSequenceState::new(5, 1, Interpolation::Linear).unwrap();
    sequence.set_duration(50).unwrap();
    for (index, time, value) in [
        (0, 20, 3.0),
        (1, 40, 4.0),
        (2, 100, 99.0),
        (3, 10, 1.0),
        (4, 20, 2.0),
    ] {
        sequence.set_keyframe(index, time, &[value]).unwrap();
    }
    sequence.set_valid_range(3, 1).unwrap();
    for (time, value) in [
        (0.0, 1.0),
        (15.0, 1.5),
        (20.0, 3.0),
        (30.0, 3.5),
        (50.0, 4.0),
    ] {
        assert_eq!(sequence.sample(time).unwrap(), [value]);
    }
    sequence.set_keyframe(4, 25, &[2.0]).unwrap();
    assert_eq!(
        sequence.sample(20.0).unwrap_err().code(),
        "unordered-keyframe"
    );
    sequence.set_keyframe(0, 30, &[3.0]).unwrap();
    assert_eq!(sequence.sample(27.5).unwrap(), [2.5]);
}

#[test]
fn circular_storage_preserves_samples_in_every_interpolation_mode() {
    for interpolation in [
        Interpolation::Step,
        Interpolation::Linear,
        Interpolation::Spline,
        Interpolation::Slerp,
        Interpolation::Squad,
    ] {
        let components = if matches!(interpolation, Interpolation::Slerp | Interpolation::Squad) {
            4
        } else {
            1
        };
        let mut original = KeyframeSequenceState::new(6, components, interpolation).unwrap();
        original.set_duration(100).unwrap();
        for (index, time) in [0, 5, 5, 20, 60, 90].into_iter().enumerate() {
            let value = if components == 4 {
                let angle = index as f32 * 0.2;
                vec![0.0, 0.0, angle.sin(), angle.cos()]
            } else {
                vec![(index * index) as f32]
            };
            original.set_keyframe(index, time, &value).unwrap();
        }
        for shift in 1..6 {
            let mut rotated = KeyframeSequenceState::new(6, components, interpolation).unwrap();
            rotated.set_duration(100).unwrap();
            for index in 0..6 {
                let (time, value) = original.keyframe(index).unwrap();
                rotated
                    .set_keyframe((index + shift) % 6, time, value)
                    .unwrap();
            }
            for (first, last) in [(0, 5), (1, 4), (3, 3)] {
                original.set_valid_range(first, last).unwrap();
                rotated
                    .set_valid_range((first + shift) % 6, (last + shift) % 6)
                    .unwrap();
                for repeat in [RepeatMode::Constant, RepeatMode::Loop] {
                    original.set_repeat_mode(repeat);
                    rotated.set_repeat_mode(repeat);
                    for tick in -40..80 {
                        let time = tick as f32 * 2.5;
                        assert_eq!(
                            original.sample(time).unwrap(),
                            rotated.sample(time).unwrap()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn edits_and_range_changes_validate_only_the_selected_keyframe_order() {
    let mut random = 0x184_u64;
    for count in 1..=9 {
        let mut sequence = KeyframeSequenceState::new(count, 1, Interpolation::Step).unwrap();
        sequence.set_duration(100).unwrap();
        let mut times = vec![0; count];
        for index in 0..count {
            sequence.set_keyframe(index, 0, &[index as f32]).unwrap();
        }
        for _ in 0..1000 {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let index = (random >> 32) as usize % count;
            let time = (random % 120) as i32;
            sequence.set_keyframe(index, time, &[index as f32]).unwrap();
            times[index] = time;
            let first = (random >> 8) as usize % count;
            let last = (random >> 16) as usize % count;
            sequence.set_valid_range(first, last).unwrap();
            let selected = (first..count)
                .chain(0..first)
                .take(if first <= last {
                    last - first + 1
                } else {
                    count - first + last + 1
                })
                .collect::<Vec<_>>();
            for repeat in [RepeatMode::Constant, RepeatMode::Loop] {
                sequence.set_repeat_mode(repeat);
                let sample = sequence.sample(15.0);
                if times[last] > 100 {
                    assert_eq!(sample.unwrap_err().code(), "invalid-duration");
                } else if selected
                    .windows(2)
                    .any(|pair| times[pair[0]] > times[pair[1]])
                {
                    assert_eq!(sample.unwrap_err().code(), "unordered-keyframe");
                } else {
                    let default = if repeat == RepeatMode::Loop {
                        last
                    } else {
                        first
                    };
                    let expected = selected
                        .iter()
                        .rev()
                        .find(|index| times[**index] <= 15)
                        .copied()
                        .unwrap_or(default);
                    assert_eq!(sample.unwrap(), [expected as f32]);
                }
            }
        }
    }
}

#[test]
fn large_integer_keyframe_times_remain_distinct_during_interpolation() {
    let mut sequence = sequence(Interpolation::Linear);
    sequence.set_keyframe(0, 16_777_217, &[2.0]).unwrap();
    sequence.set_keyframe(1, 16_777_219, &[6.0]).unwrap();
    sequence.set_duration(i32::MAX).unwrap();
    assert_eq!(sequence.sample(16_777_216.0).unwrap(), [2.0]);
    assert_eq!(sequence.sample(16_777_218.0).unwrap(), [4.0]);
    assert_eq!(sequence.sample(16_777_220.0).unwrap(), [6.0]);
}

#[test]
fn quaternion_modes_preserve_unit_length_and_endpoints() {
    for interpolation in [Interpolation::Slerp, Interpolation::Squad] {
        let mut sequence = sequence(interpolation);
        sequence.set_keyframe(0, 0, &[0.0, 0.0, 0.0, 1.0]).unwrap();
        sequence
            .set_keyframe(1, 100, &[0.0, 0.0, 1.0, 0.0])
            .unwrap();
        let value = sequence.sample(50.0).unwrap();
        let length = value
            .iter()
            .map(|component| component * component)
            .sum::<f32>();
        assert!((length - 1.0).abs() < 1.0e-5);
        assert_eq!(sequence.sample(0.0).unwrap(), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(sequence.sample(100.0).unwrap(), [0.0, 0.0, 1.0, 0.0]);
    }
}

#[test]
fn spherical_cubic_rotation_matches_scalar_spline_angles_on_one_axis() {
    for repeat in [RepeatMode::Constant, RepeatMode::Loop] {
        for angles in [
            [0.0, 0.2, 0.8, 1.1],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.3, 0.3, 0.7],
        ] {
            let mut rotation = KeyframeSequenceState::new(4, 4, Interpolation::Squad).unwrap();
            let mut angle_curve = KeyframeSequenceState::new(4, 1, Interpolation::Spline).unwrap();
            for state in [&mut rotation, &mut angle_curve] {
                state.set_duration(100).unwrap();
                state.set_repeat_mode(repeat);
            }
            for (index, (time, angle)) in [10, 20, 50, 80].into_iter().zip(angles).enumerate() {
                angle_curve.set_keyframe(index, time, &[angle]).unwrap();
                rotation
                    .set_keyframe(index, time, &[0.0, 0.0, angle.sin(), angle.cos()])
                    .unwrap();
            }
            for tick in -40..480 {
                let time = tick as f32 * 0.25;
                let angle = angle_curve.sample(time).unwrap()[0];
                let actual = rotation.sample(time).unwrap();
                let expected = [0.0, 0.0, angle.sin(), angle.cos()];
                assert!(
                    actual
                        .iter()
                        .zip(expected)
                        .all(|(a, b)| (*a - b).abs() < 2.0e-4),
                    "time {time}, actual {actual:?}, expected {expected:?}"
                );
            }
        }
    }
}

#[test]
fn quaternion_keyframes_are_normalized_when_stored_including_constant_endpoints() {
    for interpolation in [Interpolation::Slerp, Interpolation::Squad] {
        let mut sequence = sequence(interpolation);
        for scale in [5.0, -5.0, 1.0e-20, 1.0e20] {
            sequence
                .set_keyframe(0, 10, &[0.0, 0.0, 3.0 * scale, 4.0 * scale])
                .unwrap();
            let expected = [0.0, 0.0, 0.6 * scale.signum(), 0.8 * scale.signum()];
            let (time, value) = sequence.keyframe(0).unwrap();
            assert_eq!(time, 10);
            assert!(
                value
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| (*a - b).abs() < 1.0e-6)
            );
            let value = value.to_vec();
            sequence.set_valid_range(0, 0).unwrap();
            assert_eq!(sequence.sample(0.0).unwrap(), value);
            assert_eq!(sequence.sample(20.0).unwrap(), value);
        }
    }
}

#[test]
fn spherical_cubic_motion_is_preserved_under_common_rotations() {
    use crate::{Quaternion, Vec3};
    let common = Quaternion::from_axis_angle(73.0, Vec3::new(2.0, -1.0, 4.0)).unwrap();
    for right_multiply in [false, true] {
        let transform = |value: Quaternion| {
            if right_multiply {
                value.multiplied(common)
            } else {
                common.multiplied(value)
            }
            .unwrap()
        };
        let mut original = KeyframeSequenceState::new(4, 4, Interpolation::Squad).unwrap();
        let mut transformed = original.clone();
        for (index, (time, angle, axis)) in [
            (10, 0.0, Vec3::new(1.0, 0.0, 0.0)),
            (20, 83.0, Vec3::new(1.0, -2.0, 3.0)),
            (50, 215.0, Vec3::new(-3.0, 4.0, 2.0)),
            (80, 127.0, Vec3::new(2.0, 1.0, 3.0)),
        ]
        .into_iter()
        .enumerate()
        {
            let value = Quaternion::from_axis_angle(angle, axis).unwrap();
            let rotated = transform(value);
            original
                .set_keyframe(index, time, &[value.x, value.y, value.z, value.w])
                .unwrap();
            transformed
                .set_keyframe(index, time, &[rotated.x, rotated.y, rotated.z, rotated.w])
                .unwrap();
        }
        original.set_duration(100).unwrap();
        transformed.set_duration(100).unwrap();
        for repeat in [RepeatMode::Constant, RepeatMode::Loop] {
            original.set_repeat_mode(repeat);
            transformed.set_repeat_mode(repeat);
            for tick in -40..480 {
                let time = tick as f32 * 0.25;
                let value = original.sample(time).unwrap();
                let expected = transform(Quaternion {
                    x: value[0],
                    y: value[1],
                    z: value[2],
                    w: value[3],
                });
                let expected = [expected.x, expected.y, expected.z, expected.w];
                let actual = transformed.sample(time).unwrap();
                assert!(
                    actual
                        .iter()
                        .zip(expected)
                        .all(|(a, b)| (*a - b).abs() < 3.0e-4),
                    "time {time}, actual {actual:?}, expected {expected:?}"
                );
            }
        }
    }
}

#[test]
fn spherical_cubic_samples_follow_mutations_after_a_segment_has_been_sampled() {
    let mut sequence = KeyframeSequenceState::new(4, 4, Interpolation::Squad).unwrap();
    sequence.set_duration(100).unwrap();
    for index in 0..4 {
        let angle = index as f32 * 0.2;
        sequence
            .set_keyframe(
                index,
                10 + 20 * index as i32,
                &[0.0, 0.0, angle.sin(), angle.cos()],
            )
            .unwrap();
    }
    for change in 0..6 {
        let before = sequence.clone();
        sequence.sample(20.0).unwrap();
        sequence.sample(85.0).unwrap();
        assert_eq!(sequence, before);
        match change {
            0 => sequence.set_keyframe(0, 5, &[0.0, 0.0, -0.6, 0.8]).unwrap(),
            1 => sequence.set_repeat_mode(RepeatMode::Loop),
            2 => sequence.set_duration(140).unwrap(),
            3 => sequence.set_valid_range(1, 3).unwrap(),
            4 => sequence.set_valid_range(0, 3).unwrap(),
            _ => sequence.set_keyframe(1, 35, &[0.0, 0.0, 0.8, 0.6]).unwrap(),
        }
        let mut fresh = KeyframeSequenceState::new(4, 4, Interpolation::Squad).unwrap();
        for index in 0..4 {
            let (time, value) = sequence.keyframe(index).unwrap();
            fresh.set_keyframe(index, time, value).unwrap();
        }
        fresh.set_duration(sequence.duration()).unwrap();
        fresh.set_repeat_mode(sequence.repeat_mode());
        let (first, last) = sequence.valid_range();
        fresh.set_valid_range(first, last).unwrap();
        for time in [85.0, 20.0, 40.0, 85.0, 20.0] {
            let actual = sequence.sample(time).unwrap();
            let expected = fresh.sample(time).unwrap();
            assert!(
                actual
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| (*a - b).abs() < 1.0e-5)
            );
        }
    }
}

#[test]
fn rejects_invalid_shapes_ranges_times_and_non_finite_values() {
    assert_eq!(
        KeyframeSequenceState::new(2, 3, Interpolation::Slerp)
            .unwrap_err()
            .code(),
        "invalid-keyframe-shape"
    );
    let mut sequence = sequence(Interpolation::Linear);
    assert_eq!(
        sequence.set_keyframe(0, 0, &[f32::NAN]).unwrap_err().code(),
        "non-finite-keyframe"
    );
    assert_eq!(
        sequence.set_valid_range(2, 0).unwrap_err().code(),
        "invalid-valid-range"
    );
}

#[test]
fn sampling_partially_populated_keyframes_returns_an_error_until_times_are_ordered() {
    let mut sequence = sequence(Interpolation::Linear);
    sequence.set_keyframe(0, 20, &[2.0]).unwrap();
    for repeat in [RepeatMode::Constant, RepeatMode::Loop] {
        sequence.set_repeat_mode(repeat);
        assert_eq!(
            sequence.sample(60.0).unwrap_err().code(),
            "unordered-keyframe"
        );
    }

    sequence.set_keyframe(1, 100, &[6.0]).unwrap();
    assert_eq!(sequence.sample(60.0).unwrap(), [4.0]);
}

#[test]
fn sampling_rejects_an_unset_interior_time_but_allows_an_ordered_subrange() {
    let mut sequence = KeyframeSequenceState::new(3, 1, Interpolation::Linear).unwrap();
    sequence.set_duration(100).unwrap();
    sequence.set_keyframe(0, 10, &[2.0]).unwrap();
    sequence.set_keyframe(2, 30, &[6.0]).unwrap();
    for repeat in [RepeatMode::Constant, RepeatMode::Loop] {
        sequence.set_repeat_mode(repeat);
        assert_eq!(
            sequence.sample(20.0).unwrap_err().code(),
            "unordered-keyframe"
        );
        sequence.set_valid_range(0, 0).unwrap();
        assert_eq!(sequence.sample(20.0).unwrap(), [2.0]);
        sequence.set_valid_range(0, 2).unwrap();
    }
    sequence.set_keyframe(1, 20, &[4.0]).unwrap();
    assert_eq!(sequence.sample(15.0).unwrap(), [3.0]);
    assert_eq!(sequence.sample(25.0).unwrap(), [5.0]);
}

#[test]
fn sequence_duration_must_cover_only_the_selected_keyframe_range() {
    let mut sequence = sequence(Interpolation::Linear);
    sequence.set_keyframe(0, 0, &[2.0]).unwrap();
    sequence.set_keyframe(1, 100, &[6.0]).unwrap();
    sequence.set_duration(50).unwrap();
    for repeat in [RepeatMode::Constant, RepeatMode::Loop] {
        sequence.set_repeat_mode(repeat);
        assert_eq!(
            sequence.sample(25.0).unwrap_err().code(),
            "invalid-duration"
        );
        sequence.set_valid_range(0, 0).unwrap();
        assert_eq!(sequence.sample(25.0).unwrap(), [2.0]);
        sequence.set_valid_range(0, 1).unwrap();
    }
    sequence.set_duration(100).unwrap();
    assert_eq!(sequence.sample(25.0).unwrap(), [3.0]);
}

#[test]
fn keyframe_updates_preserve_time_and_components_when_validation_fails() {
    let mut sequence = KeyframeSequenceState::new(2, 3, Interpolation::Linear).unwrap();
    sequence.set_keyframe(0, 10, &[1.0, 2.0, 3.0]).unwrap();
    sequence.set_keyframe(1, 20, &[4.0, 5.0, 6.0]).unwrap();
    let expected = sequence.clone();
    for (index, time, values) in [
        (0, -1, &[7.0, 8.0, 9.0][..]),
        (2, 15, &[7.0, 8.0, 9.0]),
        (0, 15, &[7.0, 8.0]),
        (0, 15, &[7.0, f32::NAN, 9.0]),
    ] {
        assert!(sequence.set_keyframe(index, time, values).is_err());
        assert_eq!(sequence, expected);
    }
    sequence
        .set_keyframe(0, 15, &[7.0, 8.0, 9.0, f32::NAN])
        .unwrap();
    assert_eq!(sequence.keyframe(0).unwrap(), (15, &[7.0, 8.0, 9.0][..]));
}

#[test]
fn keyframe_components_stay_separate_across_shapes_and_valid_ranges() {
    for components in 1..=16 {
        let mut sequence =
            KeyframeSequenceState::new(7, components, Interpolation::Linear).unwrap();
        sequence.set_duration(70).unwrap();
        for index in (0..7).rev() {
            let (time, value) = sequence.keyframe(index).unwrap();
            assert_eq!(time, 0);
            assert_eq!(value, vec![0.0; components]);
            let value = (0..components)
                .map(|component| (10 * index + component) as f32)
                .collect::<Vec<_>>();
            sequence
                .set_keyframe(index, 10 * index as i32, &value)
                .unwrap();
        }
        for index in 0..7 {
            let (time, value) = sequence.keyframe(index).unwrap();
            assert_eq!(time, 10 * index as i32);
            assert_eq!(
                value,
                (0..components)
                    .map(|component| (10 * index + component) as f32)
                    .collect::<Vec<_>>(),
            );
        }
        sequence.set_valid_range(2, 5).unwrap();
        let expected = (0..components)
            .map(|component| 35.0 + component as f32)
            .collect::<Vec<_>>();
        assert_eq!(sequence.sample(35.0).unwrap(), expected);
        sequence.set_repeat_mode(RepeatMode::Loop);
        assert_eq!(sequence.sample(105.0).unwrap(), expected);
    }
}
