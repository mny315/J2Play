use super::*;

#[test]
fn trig_boundaries_are_exact_and_periodic() {
    assert_eq!(sin(0), 0);
    assert_eq!(sin(1024), ONE);
    assert_eq!(sin(2048), 0);
    assert_eq!(sin(3072), -ONE);
    assert_eq!(sin(4096), 0);
    assert_eq!(cos(0), ONE);
    assert_eq!(cos(2048), -ONE);
}

#[test]
fn public_trig_values_can_be_used_directly_as_matrix_coefficients() {
    let angle = 341;
    let sine = sin(angle);
    let cosine = cos(angle);
    let mut x = ONE;
    let mut z = 0;

    for _ in 0..12 {
        (x, z) = (
            clamp_i64(div_round(
                i64::from(x) * i64::from(cosine) + i64::from(z) * i64::from(sine),
                i64::from(ONE),
            )),
            clamp_i64(div_round(
                i64::from(z) * i64::from(cosine) - i64::from(x) * i64::from(sine),
                i64::from(ONE),
            )),
        );
    }

    assert!(x.unsigned_abs().max(z.unsigned_abs()) > 3000);
}

#[test]
fn integer_sqrt_ignores_sign_and_floors() {
    assert_eq!(sqrt(15), 3);
    assert_eq!(sqrt(-16), 4);
    assert_eq!(sqrt(i32::MIN), 46_340);
}

#[test]
fn vector_and_affine_contracts_are_observable() {
    let mut vector = Vector3D::new(3, 4, 0);
    vector.unit();
    assert_eq!(vector, Vector3D::new(2458, 3277, 0));
    assert_eq!(AffineTrans::IDENTITY.transform(vector), vector);
    assert_eq!(
        AffineTrans::rotation_z(1024).transform(Vector3D::new(1, 0, 0)),
        Vector3D::new(0, 1, 0)
    );
}

#[test]
fn unit_vectors_preserve_direction_across_coordinate_scales() {
    for (direction, expected) in [
        (Vector3D::new(1, 1, 0), Vector3D::new(2896, 2896, 0)),
        (Vector3D::new(-1, 1, 1), Vector3D::new(-2365, 2365, 2365)),
        (Vector3D::new(0, -1, 0), Vector3D::new(0, -ONE, 0)),
        (Vector3D::default(), Vector3D::default()),
    ] {
        for scale in [1, 2, 3, 4096, i32::MAX] {
            let mut vector = Vector3D::new(
                direction.x * scale,
                direction.y * scale,
                direction.z * scale,
            );
            vector.unit();
            assert_eq!(vector, expected, "direction {direction:?}, scale {scale}");
        }
    }
}

#[test]
fn tiny_affine_coefficients_preserve_normal_directions() {
    for scale in [1, 2, 3, ONE, i32::MAX] {
        let transform = AffineTrans::new([scale, 0, 0, 73, 0, scale, 0, -41, 0, 0, scale, 99]);
        assert_eq!(
            transform.transform_normal(Vector3D::new(1, -1, 0)),
            Vector3D::new(2896, -2896, 0),
            "scale {scale}"
        );
    }
}

#[test]
fn rotation_is_independent_of_axis_length() {
    for angle in [0, 341, 1024, 2048, 3072] {
        assert_eq!(
            AffineTrans::rotation(Vector3D::new(1, -1, 1), angle),
            AffineTrans::rotation(Vector3D::new(ONE, -ONE, ONE), angle),
            "angle {angle}"
        );
    }
}

#[test]
fn normalization_matches_geometric_length_for_small_and_extreme_vectors() {
    let coordinates = [-8, -3, -2, -1, 0, 1, 2, 3, 8, i32::MIN, i32::MAX];
    for x in coordinates {
        for y in coordinates {
            for z in coordinates {
                let mut vector = Vector3D::new(x, y, z);
                vector.unit();
                assert_normalized_components(
                    [x, y, z].map(i128::from),
                    [vector.x, vector.y, vector.z],
                );
            }
        }
    }

    let wide_coordinates = [i128::MIN, -(1 << 95), -1, 0, 1, 1 << 95, i128::MAX];
    for x in wide_coordinates {
        for y in wide_coordinates {
            for z in wide_coordinates {
                let vector = normalize_i128([x, y, z]);
                assert_normalized_components([x, y, z], [vector.x, vector.y, vector.z]);
            }
        }
    }
}

fn assert_normalized_components(input: [i128; 3], output: [i32; 3]) {
    let reference = input.map(|value| value as f64);
    let length = reference
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    if length == 0.0 {
        assert_eq!(output, [0; 3]);
        return;
    }
    for (actual, component) in output.into_iter().zip(reference) {
        let expected = component * f64::from(ONE) / length;
        assert!(
            (f64::from(actual) - expected).abs() <= 0.501,
            "input {input:?}, output {output:?}, component {expected}"
        );
    }
}

#[test]
fn vector_and_affine_extremes_saturate_without_intermediate_overflow() {
    let minimum = Vector3D::new(i32::MIN, i32::MIN, i32::MIN);
    let maximum = Vector3D::new(i32::MAX, i32::MAX, i32::MAX);
    assert_eq!(minimum.inner(minimum), i32::MAX);
    assert_eq!(minimum.inner(maximum), i32::MIN);
    assert_eq!(minimum.outer(maximum), Vector3D::default());
    assert_eq!(
        minimum.outer(Vector3D::new(i32::MIN, i32::MAX, i32::MAX)),
        Vector3D::new(0, i32::MAX, i32::MIN)
    );

    let positive = AffineTrans::new([i32::MAX; 12]);
    assert_eq!(positive.transform(maximum), maximum);
    let negative = AffineTrans::new([i32::MIN; 12]);
    assert_eq!(negative.transform(maximum), minimum);
}

#[test]
fn surface_normals_use_inverse_transpose_without_translation() {
    let transform = AffineTrans::new([8192, 0, 0, 99, 0, 4096, 0, -73, 0, 0, 2048, 41]);
    assert_eq!(
        transform.transform_normal(Vector3D::new(4096, 4096, 0)),
        Vector3D::new(1832, 3664, 0)
    );
    assert_eq!(
        AffineTrans::rotation_z(1024).transform_normal(Vector3D::new(4096, 0, 0)),
        Vector3D::new(0, 4096, 0)
    );
}

#[test]
fn normal_batch_preserves_mirrored_and_singular_transforms() {
    let transform = AffineTrans::new([-8192, 0, 0, 99, 0, 4096, 0, -73, 0, 0, 2048, 41]);
    let batch = transform.normal_transform();
    for (normal, expected) in [
        (Vector3D::new(4096, 4096, 0), Vector3D::new(-1832, 3664, 0)),
        (Vector3D::new(0, 0, 42), Vector3D::new(0, 0, 4096)),
        (Vector3D::new(-1, 0, 0), Vector3D::new(4096, 0, 0)),
        (Vector3D::default(), Vector3D::default()),
    ] {
        assert_eq!(batch(normal), expected);
    }
    let singular = AffineTrans::new([i32::MAX; 12]).normal_transform();
    assert_eq!(
        singular(Vector3D::new(i32::MIN, i32::MAX, 1)),
        Vector3D::default()
    );
}

#[test]
fn look_at_keeps_the_view_direction_independent_of_position() {
    let position = Vector3D::new(10, 20, 30);
    let look = Vector3D::new(0, 0, -4096);
    let up = Vector3D::new(0, 4096, 0);
    let moved = AffineTrans::look_at(position, look, up);

    assert_eq!(moved.transform(position), Vector3D::new(0, 0, 0));
    assert_eq!(
        moved.transform(Vector3D::new(10, 20, 29)),
        Vector3D::new(0, 0, 1)
    );
    assert_eq!(
        moved.transform(Vector3D::new(11, 20, 30)),
        Vector3D::new(1, 0, 0)
    );

    let translated = AffineTrans::look_at(Vector3D::new(110, 20, 30), look, up);
    assert_eq!(moved.values[..3], translated.values[..3]);
    assert_eq!(moved.values[4..7], translated.values[4..7]);
    assert_eq!(moved.values[8..11], translated.values[8..11]);
}

#[test]
fn look_at_preserves_large_positions_until_fixed_point_conversion() {
    let look = Vector3D::new(0, 0, -ONE);
    let up = Vector3D::new(0, ONE, 0);
    for position in [
        Vector3D::new(1_000_000, -2_000_000, 3_000_000),
        Vector3D::new(i32::MAX, -i32::MAX, i32::MAX),
    ] {
        let view = AffineTrans::look_at(position, look, up);
        assert_eq!(view.transform(position), Vector3D::default());
        assert_eq!(view.values[3], -position.x);
        assert_eq!(view.values[7], position.y);
        assert_eq!(view.values[11], position.z);
    }
}

#[test]
fn look_at_preserves_orientation_when_up_direction_is_scaled() {
    let position = Vector3D::new(10, -20, 30);
    for look in [Vector3D::new(0, 0, -ONE), Vector3D::new(1, 2, -3)] {
        for up in [
            Vector3D::new(2, 1, 0),
            Vector3D::new(1, -3, 2),
            Vector3D::new(3, 1, 4),
        ] {
            let expected = AffineTrans::look_at(position, look, up);
            for scale in [1, ONE, 1_000_000, 100_000_000] {
                let scaled_up = Vector3D::new(up.x * scale, up.y * scale, up.z * scale);
                let actual = AffineTrans::look_at(position, look, scaled_up);
                assert_eq!(actual, expected, "look {look:?}, up {up:?}, scale {scale}");
                assert_eq!(actual.transform(position), Vector3D::default());
            }
        }
    }
}

#[test]
fn look_at_keeps_directions_that_are_nearly_parallel_before_normalization() {
    // Normalizing either input first can round away its small component and
    // collapse the cross product of two directions that are not parallel.
    for (look, up, expected) in [
        (
            Vector3D::new(0, ONE, 0),
            Vector3D::new(1, 10_000, 0),
            [0, 0, -ONE, 0, -ONE, 0, 0, 0, 0, ONE, 0, 0],
        ),
        (
            Vector3D::new(10_000, 1, 0),
            Vector3D::new(10_000, 0, 0),
            [0, 0, -ONE, 0, 0, ONE, 0, 0, ONE, 0, 0, 0],
        ),
    ] {
        assert_eq!(
            AffineTrans::look_at(Vector3D::default(), look, up),
            AffineTrans::new(expected)
        );
    }
}
