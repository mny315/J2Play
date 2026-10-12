use super::*;
use crate::Vec4;

#[test]
fn triangles_wholly_outside_each_clip_plane_are_discarded_at_all_scales() {
    for scale in [f32::from_bits(4), 1.0e-30, 1.0, 1.0e30, f32::MAX / 8.0] {
        for axis in 0..3 {
            for sign in [-1.0, 1.0] {
                let mut first = [ClipVertex::default(); MAX_CLIPPED_VERTICES];
                let mut second = first;
                for (index, vertex) in first[..3].iter_mut().enumerate() {
                    let mut position =
                        [[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]][index];
                    position[axis] = sign * (1.25 + index as f32 * 0.25);
                    vertex.position = Vec4::new(
                        position[0] * scale,
                        position[1] * scale,
                        position[2] * scale,
                        scale,
                    );
                }
                assert_eq!(
                    clip_triangle(&mut first, &mut second),
                    (0, true),
                    "scale={scale} axis={axis} sign={sign}"
                );
            }
        }
    }
}

#[test]
fn visible_triangles_preserve_vertices_and_attributes_at_all_scales() {
    for w in [0.0, f32::from_bits(1), 1.0e-30, 1.0, 1.0e30, f32::MAX] {
        let mut first = [ClipVertex::default(); MAX_CLIPPED_VERTICES];
        let mut second = first;
        for (index, vertex) in first[..3].iter_mut().enumerate() {
            vertex.position = [
                Vec4::new(-w, -w, -w, w),
                Vec4::new(w, -w, w, w),
                Vec4::new(0.0, w, 0.0, w),
            ][index];
            vertex.color = [255.0, index as f64, 64.0, 32.0];
            vertex.texture = [[0.0, index as f64, 1.0], [2.0, 3.0, 4.0]];
            vertex.eye_z = index as f64;
        }
        let original = first;
        assert_eq!(clip_triangle(&mut first, &mut second), (3, false));
        for (actual, expected) in first[..3].iter().zip(&original[..3]) {
            assert_vertex_bits(*actual, *expected);
        }
    }
}

#[test]
fn finite_clip_vertices_remain_finite_within_the_fixed_buffers() {
    let mut random = 0x1841_2011_u32;
    let mut coordinate = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        // Preserve the sign while excluding NaN/infinity exponent bits.
        f32::from_bits(random & 0xff7f_ffff)
    };
    for sample in 0..10_000 {
        let mut first = [ClipVertex::default(); MAX_CLIPPED_VERTICES];
        let mut second = first;
        for (index, vertex) in first[..3].iter_mut().enumerate() {
            vertex.position = Vec4::new(coordinate(), coordinate(), coordinate(), coordinate());
            vertex.color = [255.0, 128.0 - index as f64, 64.0 + index as f64, 32.0];
            vertex.texture =
                std::array::from_fn(|_| std::array::from_fn(|_| f64::from(coordinate())));
            vertex.eye_z = f64::from(coordinate());
        }
        let original = first;
        let mut line = [first[0], first[1]];
        let (visible, _) = clip_line(&mut line);
        if visible {
            for vertex in line {
                assert_finite(vertex, sample);
            }
        }
        let (count, clipped) = clip_triangle(&mut first, &mut second);
        let mut expected = original;
        assert_eq!(
            (count, clipped),
            reference_clip_triangle(
                &mut expected,
                &mut [ClipVertex::default(); MAX_CLIPPED_VERTICES]
            ),
            "sample={sample}"
        );
        assert!(count <= MAX_CLIPPED_VERTICES);
        for (vertex, expected) in first[..count].iter().zip(&expected) {
            assert_finite(*vertex, sample);
            assert_vertex_bits(*vertex, *expected);
        }
    }
}

fn assert_vertex_bits(actual: ClipVertex, expected: ClipVertex) {
    let position = |value: Vec4| [value.x, value.y, value.z, value.w].map(f32::to_bits);
    assert_eq!(position(actual.position), position(expected.position));
    assert_eq!(
        actual.color.map(f64::to_bits),
        expected.color.map(f64::to_bits)
    );
    assert_eq!(
        actual.texture.map(|unit| unit.map(f64::to_bits)),
        expected.texture.map(|unit| unit.map(f64::to_bits)),
    );
    assert_eq!(actual.eye_z.to_bits(), expected.eye_z.to_bits());
}

// Six full buffer passes provide a reference for skipping inactive clip planes.
fn reference_clip_triangle(
    first: &mut [ClipVertex; MAX_CLIPPED_VERTICES],
    second: &mut [ClipVertex; MAX_CLIPPED_VERTICES],
) -> (usize, bool) {
    let outside = [0, 1, 2].map(|index| clip_outcode(first[index].position));
    if outside[0] | outside[1] | outside[2] == 0 {
        return (3, false);
    }
    if outside[0] & outside[1] & outside[2] != 0 {
        return (0, true);
    }
    let mut count = 3;
    let mut clipped = false;
    for plane in 0..6 {
        let (input, output) = if plane % 2 == 0 {
            (&*first, &mut *second)
        } else {
            (&*second, &mut *first)
        };
        let mut output_count = 0;
        if count != 0 {
            let mut previous = input[count - 1];
            let mut previous_distance = plane_distance(previous.position, plane);
            for current in input.iter().copied().take(count) {
                let current_distance = plane_distance(current.position, plane);
                if (previous_distance >= 0.0) != (current_distance >= 0.0) {
                    let amount = previous_distance / (previous_distance - current_distance);
                    output[output_count] = interpolate(previous, current, amount);
                    output_count += 1;
                    clipped = true;
                }
                if current_distance >= 0.0 {
                    output[output_count] = current;
                    output_count += 1;
                } else {
                    clipped = true;
                }
                previous = current;
                previous_distance = current_distance;
            }
        }
        count = output_count;
    }
    (count, clipped)
}

#[test]
#[ignore = "manual partial triangle clipping throughput measurement"]
fn partial_clipping_throughput() {
    for (name, positions) in [
        (
            "one-plane",
            [[-2.0, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]],
        ),
        (
            "three-planes",
            [[-2.0, -0.5, -2.0], [0.5, -0.5, 0.0], [0.0, 2.0, 0.0]],
        ),
        (
            "six-planes",
            [[-2.0, -2.0, -2.0], [2.0, -2.0, 2.0], [0.0, 2.0, 0.0]],
        ),
    ] {
        let source = positions.map(|[x, y, z]| ClipVertex {
            position: Vec4::new(x, y, z, 1.0),
            color: [255.0, f64::from(x) + 128.0, f64::from(y) + 128.0, 64.0],
            texture: [[f64::from(x), f64::from(y), 1.0]; 2],
            eye_z: f64::from(z),
        });
        let mut first = [ClipVertex::default(); MAX_CLIPPED_VERTICES];
        let mut second = first;
        let mut count = 0;
        let started = std::time::Instant::now();
        for _ in 0..131_072 {
            first[..3].copy_from_slice(std::hint::black_box(&source));
            (count, _) = clip_triangle(&mut first, &mut second);
            std::hint::black_box(&first[..count]);
        }
        let elapsed = started.elapsed();
        let checksum = first[..count].iter().fold(0_u64, |sum, vertex| {
            [
                vertex.position.x,
                vertex.position.y,
                vertex.position.z,
                vertex.position.w,
            ]
            .map(f64::from)
            .iter()
            .chain(&vertex.color)
            .chain(vertex.texture.iter().flatten())
            .chain(std::iter::once(&vertex.eye_z))
            .fold(sum, |sum, value| {
                sum.wrapping_mul(31).wrapping_add(value.to_bits())
            })
        });
        eprintln!("clip={name} elapsed={elapsed:?} count={count} checksum={checksum:016x}");
    }
}

fn assert_finite(vertex: ClipVertex, sample: usize) {
    let position = vertex.position;
    assert!(
        [position.x, position.y, position.z, position.w]
            .iter()
            .all(|value| value.is_finite()),
        "position at sample {sample}"
    );
    assert!(
        vertex
            .color
            .iter()
            .chain(vertex.texture.iter().flatten())
            .chain(std::iter::once(&vertex.eye_z))
            .all(|value| value.is_finite()),
        "attributes at sample {sample}"
    );
}
