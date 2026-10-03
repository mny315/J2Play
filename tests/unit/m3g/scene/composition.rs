use super::*;

fn fixture(index: usize, general: bool) -> TransformableState {
    let value = index as f32 / 32.0;
    TransformableState {
        translation: Vec3::new(value, -value * 2.0, value * 3.0),
        scale: Vec3::new(0.5 + value, -1.0 - value, 2.0 + value),
        orientation: Quaternion::from_axis_angle(value * 45.0, Vec3::new(1.0, 2.0, 3.0)).unwrap(),
        transform: if general {
            Mat4::from_row_major([
                1.0, 0.25, 0.5, -2.0, -0.5, 2.0, 0.0, 1.0, 0.25, 0.5, 1.0, 3.0, 0.125, 0.0, 0.25,
                1.0,
            ])
            .unwrap()
        } else {
            Mat4::IDENTITY
        },
    }
}

fn dense_composition(state: &TransformableState) -> Mat4 {
    Mat4::translation(
        state.translation.x,
        state.translation.y,
        state.translation.z,
    )
    .unwrap()
    .multiplied(Mat4::from_quaternion(state.orientation).unwrap())
    .multiplied(Mat4::scale(state.scale.x, state.scale.y, state.scale.z).unwrap())
    .multiplied(state.transform)
}

#[test]
fn composition_matches_explicit_translation_rotation_scale_products() {
    for general in [false, true] {
        for index in 0..256 {
            let mut state = fixture(index, general);
            for scale in [0.0, 1.0, -1.0, 1.0e-30, 1.0e30] {
                state.scale = Vec3::new(scale, -scale * 0.5, scale * 0.25);
                assert_eq!(
                    state.composite().unwrap(),
                    dense_composition(&state),
                    "general={general} index={index} scale={scale}",
                );
            }
        }
    }
}

#[test]
#[allow(clippy::float_cmp)] // Exact arithmetic comparison, including overflow to infinity.
fn composition_preserves_finite_component_extremes() {
    let mut random = 0x184_2026_u32;
    let mut coordinate = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        f32::from_bits(random & 0xff7f_ffff)
    };
    for index in 0..4096 {
        let mut state = fixture(index % 256, index % 2 == 0);
        state.translation = Vec3::new(coordinate(), coordinate(), coordinate());
        state.scale = Vec3::new(coordinate(), coordinate(), coordinate());
        let expected = dense_composition(&state);
        let actual = state.composite().unwrap();
        for (actual, expected) in actual.as_array().iter().zip(expected.as_array()) {
            assert!(
                actual == expected || actual.is_nan() && expected.is_nan(),
                "state={state:?} expected={expected:?} actual={actual:?}",
            );
        }
    }
}

#[test]
#[ignore = "manual scene transform composition throughput measurement"]
fn scene_composition_throughput() {
    for general in [false, true] {
        let states: Vec<_> = (0..256).map(|index| fixture(index, general)).collect();
        let started = std::time::Instant::now();
        let mut checksum = 0_u64;
        for _ in 0..512 {
            for state in &states {
                let matrix = std::hint::black_box(state).composite().unwrap();
                for value in matrix.as_array() {
                    checksum = checksum
                        .wrapping_mul(31)
                        .wrapping_add(u64::from(value.to_bits()));
                }
            }
        }
        eprintln!(
            "general={general} elapsed={:?} checksum={checksum:016x}",
            started.elapsed(),
        );
    }
}
