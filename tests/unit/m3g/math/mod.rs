use super::*;

#[test]
fn bounded_rounding_matches_reference_at_ties_limits_and_non_finite_values() {
    for maximum in [0, 255, 16_777_215, u32::MAX] {
        let check = |value: f64| {
            assert_eq!(
                rounded_u32(value, maximum),
                value.round().clamp(0.0, f64::from(maximum)) as u32,
                "value={value:?}, maximum={maximum}"
            );
        };
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            -0.0,
            0.0,
            f64::MIN_POSITIVE,
            f64::from(maximum),
        ] {
            check(value);
        }
        for integer in (0..=1024).chain((0..32).map(|bit| 1_u32 << bit)) {
            for value in [
                f64::from(integer),
                f64::from(integer) + 0.5,
                f64::from(integer) - 0.5,
            ] {
                check(value.next_down());
                check(value);
                check(value.next_up());
            }
        }
        let mut state = 0x1842_5301_84a5_51e7_u64;
        for _ in 0..50_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            check(f64::from_bits(state));
            check((state as f64 / u64::MAX as f64) * f64::from(maximum));
        }
    }
}

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 1.0e-5
}

#[test]
fn large_finite_axes_and_quaternions_preserve_their_direction() {
    let axis = Vec3::new(f32::MAX, -f32::MAX, f32::MAX)
        .normalized()
        .unwrap();
    assert!(close(axis.dot(axis), 1.0));
    assert!(axis.x > 0.0 && axis.y < 0.0 && axis.z > 0.0);
    let rotation = Quaternion {
        x: f32::MAX,
        y: 0.0,
        z: 0.0,
        w: f32::MAX,
    };
    let normalized = rotation.normalized().unwrap();
    assert!(close(normalized.x, 0.5_f32.sqrt()));
    assert!(close(normalized.w, 0.5_f32.sqrt()));
    assert!(Mat4::from_quaternion(rotation).is_ok());
}

#[test]
fn small_nonzero_axes_and_quaternions_preserve_their_direction() {
    for scale in [f32::from_bits(1), 1.0e-25, 1.0e-8] {
        assert_eq!(
            Vec3::new(scale, 0.0, 0.0).normalized().unwrap(),
            Vec3::new(1.0, 0.0, 0.0)
        );
        let normalized = Quaternion {
            x: scale,
            y: 0.0,
            z: 0.0,
            w: scale,
        }
        .normalized()
        .unwrap();
        assert!(close(normalized.x, 0.5_f32.sqrt()));
        assert!(close(normalized.w, 0.5_f32.sqrt()));
    }
}

#[test]
fn slerp_preserves_rotations_above_half_a_turn() {
    for angle in [90.0, 180.0, 270.0, 350.0, 359.99] {
        for axis in [Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 2.0, 3.0)] {
            let end = Quaternion::from_axis_angle(angle, axis).unwrap();
            for amount in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let sampled =
                    Mat4::from_quaternion(Quaternion::IDENTITY.slerp(end, amount).unwrap())
                        .unwrap();
                let expected = Mat4::rotation(angle * amount, axis).unwrap();
                for (actual, expected) in sampled.as_array().iter().zip(expected.as_array()) {
                    assert!(
                        (actual - expected).abs() < 2.0e-4,
                        "angle={angle}, amount={amount}: {actual} != {expected}"
                    );
                }
            }
        }
    }
}

#[test]
fn slerp_arc_is_preserved_under_a_common_rotation() {
    let rotation = Quaternion::from_axis_angle(73.0, Vec3::new(2.0, -1.0, 4.0)).unwrap();
    let axis = Vec3::new(-3.0, 2.0, 1.0);
    for angle in [270.0, 359.9, 359.99] {
        let end = rotation
            .multiplied(Quaternion::from_axis_angle(angle, axis).unwrap())
            .unwrap();
        for amount in [0.25, 0.5, 0.75] {
            let sampled = Mat4::from_quaternion(rotation.slerp(end, amount).unwrap()).unwrap();
            let expected = Mat4::from_quaternion(
                rotation
                    .multiplied(Quaternion::from_axis_angle(angle * amount, axis).unwrap())
                    .unwrap(),
            )
            .unwrap();
            for (actual, expected) in sampled.as_array().iter().zip(expected.as_array()) {
                assert!(
                    (actual - expected).abs() < 1.0e-3,
                    "angle={angle}, amount={amount}: {actual} != {expected}"
                );
            }
        }
    }
}

#[test]
fn exactly_opposite_quaternions_keep_endpoints_and_finite_unit_samples() {
    let first = Quaternion::IDENTITY;
    let last = Quaternion { w: -1.0, ..first };
    assert_eq!(first.slerp(last, 0.0).unwrap(), first);
    assert_eq!(first.slerp(last, 1.0).unwrap(), last);
    for amount in [0.25, 0.5, 0.75] {
        let value = first.slerp(last, amount).unwrap();
        assert!(close(
            value.x * value.x + value.y * value.y + value.z * value.z + value.w * value.w,
            1.0
        ));
        assert_eq!(value, first.slerp(last, amount).unwrap());
    }
}

#[test]
fn composition_uses_column_vectors_and_jsr_array_layout() {
    let translate = Mat4::translation(2.0, 3.0, 4.0).unwrap();
    let scale = Mat4::scale(2.0, 3.0, 4.0).unwrap();
    let point = translate
        .multiplied(scale)
        .transform(Vec4::new(1.0, 1.0, 1.0, 1.0));
    assert_eq!(point, Vec4::new(4.0, 6.0, 8.0, 1.0));
    assert_eq!(translate.as_array()[12..15], [2.0, 3.0, 4.0]);
}

#[test]
fn jsr_row_major_arrays_round_trip_without_transposing_semantics() {
    let values = [
        1.0, 0.0, 0.0, 2.0, 0.0, 1.0, 0.0, 3.0, 0.0, 0.0, 1.0, 4.0, 0.0, 0.0, 0.0, 1.0,
    ];
    let matrix = Mat4::from_row_major(values).unwrap();
    assert_eq!(matrix.to_row_major(), values);
    assert_eq!(
        matrix.transform(Vec4::new(1.0, 1.0, 1.0, 1.0)),
        Vec4::new(3.0, 4.0, 5.0, 1.0)
    );
}

#[test]
fn inverse_identity_holds_for_affine_transform() {
    let matrix = Mat4::translation(-2.0, 3.0, 7.0)
        .unwrap()
        .multiplied(Mat4::rotation(37.0, Vec3::new(1.0, 2.0, 3.0)).unwrap())
        .multiplied(Mat4::scale(2.0, 3.0, 4.0).unwrap());
    let product = matrix.multiplied(matrix.inverted().unwrap());
    for (actual, expected) in product.as_array().iter().zip(Mat4::IDENTITY.as_array()) {
        assert!(close(*actual, *expected), "{actual} != {expected}");
    }
}

#[test]
fn inversion_accepts_small_invertible_scales_and_homogeneous_rescaling() {
    for scale in [1.0, 1.0e-8, 1.0e-18, 1.0e18] {
        for matrix in [
            Mat4::from_row_major([
                scale,
                0.0,
                0.0,
                3.0 * scale,
                0.0,
                2.0 * scale,
                0.0,
                -4.0 * scale,
                0.0,
                0.0,
                3.0 * scale,
                5.0 * scale,
                0.0,
                0.0,
                0.0,
                1.0,
            ])
            .unwrap(),
            Mat4::from_array(Mat4::IDENTITY.as_array().map(|value| value * scale)).unwrap(),
        ] {
            let inverse = matrix.inverted().unwrap();
            // Multiplying inverse first keeps cancellation errors relative to
            // local units even for a large translation in world coordinates.
            let identity = *inverse.multiplied(matrix).as_array();
            for (actual, expected) in identity.into_iter().zip(Mat4::IDENTITY.as_array()) {
                assert!(
                    (actual - expected).abs() < 1.0e-5,
                    "{scale}: {actual} != {expected}"
                );
            }
        }
    }
}

#[test]
fn rotation_and_slerp_agree_at_half_a_turn() {
    let identity = Quaternion::IDENTITY;
    let end = Quaternion::from_axis_angle(180.0, Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let halfway = identity.slerp(end, 0.5).unwrap();
    let point = Mat4::from_quaternion(halfway)
        .unwrap()
        .transform(Vec4::new(1.0, 0.0, 0.0, 1.0));
    assert!(close(point.x, 0.0));
    assert!(close(point.y, 1.0));
}

#[test]
fn quaternion_axis_angle_round_trip_preserves_rotation() {
    for (angle, axis) in [
        (137.0, Vec3::new(1.0, 2.0, 3.0)),
        (270.0, Vec3::new(0.0, 0.0, 1.0)),
        (0.0, Vec3::default()),
    ] {
        let quaternion = Quaternion::from_axis_angle(angle, axis).unwrap();
        let (round_trip_angle, round_trip_axis) = quaternion.to_axis_angle().unwrap();
        let original = Mat4::from_quaternion(quaternion).unwrap();
        let round_trip = Mat4::rotation(round_trip_angle, round_trip_axis).unwrap();
        for (actual, expected) in round_trip.as_array().iter().zip(original.as_array()) {
            assert!(close(*actual, *expected), "{actual} != {expected}");
        }
    }
}

#[test]
fn rejects_non_finite_zero_axis_and_singular_matrix() {
    assert_eq!(
        Mat4::translation(f32::NAN, 0.0, 0.0).unwrap_err().code(),
        "non-finite"
    );
    assert_eq!(
        Mat4::rotation(1.0, Vec3::default()).unwrap_err().code(),
        "zero-length"
    );
    assert_eq!(
        Mat4::rotation(0.0, Vec3::default()).unwrap(),
        Mat4::IDENTITY
    );
    assert_eq!(
        Mat4::scale(0.0, 1.0, 1.0)
            .unwrap()
            .inverted()
            .unwrap_err()
            .code(),
        "singular-transform"
    );
}

#[test]
fn quaternion_axis_angle_preserves_small_rotations() {
    for angle in [0.1, 0.01, 1.0e-3, 1.0e-10, 1.0e-30] {
        for axis in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, -2.0, 3.0)] {
            let rotation = Quaternion::from_axis_angle(angle, axis).unwrap();
            for sign in [-1.0, 1.0] {
                let rotation = Quaternion {
                    x: rotation.x * sign,
                    y: rotation.y * sign,
                    z: rotation.z * sign,
                    w: rotation.w * sign,
                };
                let (actual_angle, actual_axis) = rotation.to_axis_angle().unwrap();
                assert!(
                    (actual_angle / angle - 1.0).abs() < 1.0e-5,
                    "angle={angle}, actual={actual_angle}, sign={sign}"
                );
                let expected_axis = axis.normalized().unwrap();
                for (actual, expected) in [actual_axis.x, actual_axis.y, actual_axis.z]
                    .into_iter()
                    .zip([expected_axis.x, expected_axis.y, expected_axis.z])
                {
                    assert!((actual - expected).abs() < 1.0e-6);
                }
                let original = Mat4::from_quaternion(rotation).unwrap();
                let restored = Mat4::rotation(actual_angle, actual_axis).unwrap();
                for (actual, expected) in restored.as_array().iter().zip(original.as_array()) {
                    assert!((actual - expected).abs() <= angle.to_radians() * 1.0e-5);
                }
            }
        }
    }
}

#[test]
fn quaternion_axis_angle_handles_scale_identity_and_invalid_values() {
    let expected_angle = (2.0 * 14.0_f64.sqrt().atan2(4.0)).to_degrees() as f32;
    let expected_axis = Vec3::new(1.0, -2.0, 3.0).normalized().unwrap();
    for scale in [f32::from_bits(1), 1.0e-30, 1.0, 1.0e30] {
        let (angle, axis) = Quaternion {
            x: scale,
            y: -2.0 * scale,
            z: 3.0 * scale,
            w: 4.0 * scale,
        }
        .to_axis_angle()
        .unwrap();
        assert!((angle - expected_angle).abs() < 1.0e-5);
        assert!((axis.x - expected_axis.x).abs() < 1.0e-6);
        assert!((axis.y - expected_axis.y).abs() < 1.0e-6);
        assert!((axis.z - expected_axis.z).abs() < 1.0e-6);
        for sign in [-1.0, 1.0] {
            assert_eq!(
                Quaternion {
                    w: sign * scale,
                    ..Quaternion::IDENTITY
                }
                .to_axis_angle()
                .unwrap(),
                (0.0, Vec3::default())
            );
        }
    }
    for (value, code) in [
        (0.0, "zero-length"),
        (f32::NAN, "non-finite"),
        (f32::INFINITY, "non-finite"),
    ] {
        assert_eq!(
            Quaternion {
                w: value,
                ..Quaternion::IDENTITY
            }
            .to_axis_angle()
            .unwrap_err()
            .code(),
            code
        );
    }
}

#[test]
#[ignore = "manual quaternion axis-angle throughput measurement"]
fn quaternion_axis_angle_throughput() {
    for base_angle in [0.01, 45.0] {
        let rotations: Vec<_> = (0..1024)
            .map(|index| {
                Quaternion::from_axis_angle(
                    base_angle + (index % 90) as f32,
                    Vec3::new(1.0, -2.0, 3.0),
                )
                .unwrap()
            })
            .collect();
        let started = std::time::Instant::now();
        let mut checksum = 0_u64;
        for index in 0..1_000_000 {
            let (angle, axis) = std::hint::black_box(rotations[index % rotations.len()])
                .to_axis_angle()
                .unwrap();
            std::hint::black_box(axis);
            checksum = checksum.wrapping_add(angle.round() as u64);
        }
        eprintln!(
            "axis-angle base={base_angle}: elapsed={:?}, checksum={checksum:016x}",
            started.elapsed()
        );
    }
}
