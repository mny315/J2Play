use crate::*;

fn shorts(values: &[i16], components: usize) -> VertexArrayState {
    let mut array = VertexArrayState::new(
        values.len() / components,
        components,
        VertexComponent::Short,
    )
    .unwrap();
    array.set_shorts(0, array.vertex_count(), values).unwrap();
    array
}

fn bytes(values: &[i8], components: usize) -> VertexArrayState {
    let mut array =
        VertexArrayState::new(values.len() / components, components, VertexComponent::Byte)
            .unwrap();
    array.set_bytes(0, array.vertex_count(), values).unwrap();
    array
}

fn base() -> VertexBufferState {
    let mut base = VertexBufferState::default();
    base.set_positions(Some(shorts(&[4, -4, 8], 3)), 0.25, [1.0, 2.0, 3.0])
        .unwrap();
    base
}

fn morph(
    base: &VertexBufferState,
    targets: &[VertexBufferState],
    weights: &[f32],
) -> Result<Vec<Vertex>, diagnostics::EmuError> {
    Ok(base
        .morphed_vertices(targets, weights)?
        .transformed_vertices(Mat4::IDENTITY))
}

#[test]
fn morph_interpolates_raw_components_and_preserves_fractional_positions() {
    let base = base();
    let mut target = base.clone();
    target
        .set_positions(Some(shorts(&[10, 2, 10], 3)), 800.0, [100.0; 3])
        .unwrap();
    let vertices = morph(&base, &[target], &[0.25]).unwrap();
    assert_eq!(vertices[0].position, Vec4::new(2.375, 1.375, 5.125, 1.0));
}

#[test]
fn morph_can_change_only_the_default_color() {
    let mut base = base();
    base.set_default_color(0xffff_0000);
    let mut target = VertexBufferState::default();
    target.set_default_color(0x8000_00ff);
    let vertices = morph(&base, &[target], &[0.5]).unwrap();
    assert_eq!(vertices[0].position, Vec4::new(2.0, 1.0, 5.0, 1.0));
    assert_eq!(vertices[0].color, 0xc080_0080);
}

#[test]
fn morph_uses_unsigned_colors_and_both_base_texture_scales() {
    let mut base = base();
    base.set_colors(Some(bytes(&[-1, 0, 0, -1], 4))).unwrap();
    base.set_texture_coordinates(0, Some(shorts(&[2, 4], 2)), 0.5, [1.0, 2.0, 0.0])
        .unwrap();
    base.set_texture_coordinates(1, Some(bytes(&[0, 2, -2], 3)), 0.25, [3.0, 4.0, 5.0])
        .unwrap();
    let mut target = base.clone();
    target
        .set_colors(Some(bytes(&[0, 0, -1, -128], 4)))
        .unwrap();
    target
        .set_texture_coordinates(0, Some(shorts(&[-2, 8], 2)), 500.0, [900.0; 3])
        .unwrap();
    target
        .set_texture_coordinates(1, Some(bytes(&[4, 6, 2], 3)), 600.0, [800.0; 3])
        .unwrap();
    let vertices = morph(&base, &[target], &[0.5]).unwrap();
    assert_eq!(vertices[0].color, 0xc080_0080);
    assert_eq!(vertices[0].texture, [[1.0, 5.0, 0.0], [3.5, 5.0, 5.0]]);
}

#[test]
fn morph_validates_target_presence_shape_and_component_type_even_at_zero_weight() {
    let base = base();
    let mut byte_positions = base.clone();
    byte_positions
        .set_positions(Some(bytes(&[4, -4, 8], 3)), 1.0, [0.0; 3])
        .unwrap();
    let mut extra_colors = base.clone();
    extra_colors.set_colors(Some(bytes(&[1, 2, 3], 3))).unwrap();
    let mut extra_vertex = base.clone();
    extra_vertex
        .set_positions(Some(shorts(&[0; 6], 3)), 1.0, [0.0; 3])
        .unwrap();
    for targets in [
        vec![byte_positions],
        vec![extra_colors],
        vec![extra_vertex],
        vec![base.clone(), VertexBufferState::default()],
    ] {
        assert!(morph(&base, &targets, &vec![0.0; targets.len()]).is_err());
    }
}

#[test]
fn identical_morph_targets_remain_exact_with_large_or_negative_weights() {
    let base = base();
    let expected = base.transformed_vertices(Mat4::IDENTITY).unwrap();
    for weights in [[f32::MAX, f32::MAX], [-1000.0, 500.0], [0.0, 0.0]] {
        let actual = morph(&base, &[base.clone(), base.clone()], &weights).unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn morph_interpolates_normals_without_rounding_and_retains_other_base_arrays() {
    let mut base = base();
    base.set_normals(Some(bytes(&[-128, 0, 127], 3))).unwrap();
    base.set_colors(Some(bytes(&[-1, 0, 0], 3))).unwrap();
    let mut target = VertexBufferState::default();
    target.set_normals(Some(bytes(&[127, 0, -128], 3))).unwrap();
    let prepared = base.morphed_vertices(&[target], &[0.25]).unwrap();
    assert!(prepared.has_normals());
    assert_eq!(
        prepared.transformed_vertices(Mat4::IDENTITY)[0].color,
        0xffff_0000
    );
    let lit = prepared
        .transformed_lit_vertices(
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Vec3::new(0.0, 0.0, 10.0),
            MaterialState {
                diffuse: u32::MAX,
                ..Default::default()
            },
            false,
            &[LightSource::Directional {
                direction: Vec3::new(0.0, 0.0, 1.0),
                color: 0x00ff_ffff,
                intensity: 1.0,
            }],
            0.5,
        )
        .unwrap();
    // Raw interpolation gives (-64.25, 0, 63.25), decoded to
    // (-0.5, 1/255, 0.5), before normalization and directional lighting.
    assert_eq!(lit[0].color, 0x80b4_b4b4);
}

#[test]
fn opposing_morph_differences_preserve_the_base_and_negative_weights_extrapolate() {
    let base = base();
    let mut target = base.clone();
    target
        .set_positions(Some(shorts(&[10, 2, 10], 3)), 1.0, [0.0; 3])
        .unwrap();
    let cancelled = morph(&base, &[target.clone(), target.clone()], &[1.0e18, -1.0e18]).unwrap();
    assert_eq!(
        cancelled,
        base.transformed_vertices(Mat4::IDENTITY).unwrap()
    );
    let extrapolated = morph(&base, &[target], &[-0.5]).unwrap();
    assert_eq!(extrapolated[0].position, Vec4::new(1.25, 0.25, 4.75, 1.0));
}

#[test]
fn morph_bounds_the_work_from_repeated_shared_targets_but_skips_zero_weights() {
    let mut base = VertexBufferState::default();
    base.set_positions(
        Some(VertexArrayState::new(65_535, 3, VertexComponent::Short).unwrap()),
        1.0,
        [0.0; 3],
    )
    .unwrap();
    let targets = vec![base.clone(); 100];
    assert_eq!(
        base.morphed_vertices(&targets, &[0.5; 100])
            .unwrap_err()
            .code(),
        "resource-limit"
    );
    let prepared = base.morphed_vertices(&targets, &[0.0; 100]).unwrap();
    assert_eq!(prepared.transformed_vertices(Mat4::IDENTITY).len(), 65_535);
}

fn animated_mesh(offset: i16, attributes: bool) -> VertexBufferState {
    let count = 2048;
    let data: Vec<_> = (0..count)
        .flat_map(|index| {
            let x = (index % 64) as i16 - 32;
            let y = (index / 64) as i16 - 16;
            [x + offset, y - offset, x / 4 - y / 8 + offset]
        })
        .collect();
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(Some(shorts(&data, 3)), 0.25, [1.0, -2.0, 3.0])
        .unwrap();
    if attributes {
        buffer.set_normals(Some(shorts(&data, 3))).unwrap();
        let colors: Vec<_> = (0..count * 4)
            .map(|index| (index as i8).wrapping_add(offset as i8))
            .collect();
        buffer.set_colors(Some(bytes(&colors, 4))).unwrap();
        for unit in 0..2 {
            let texture: Vec<_> = (0..count * (2 + unit))
                .map(|index| (index % 512) as i16 - 256 + offset)
                .collect();
            buffer
                .set_texture_coordinates(
                    unit,
                    Some(shorts(&texture, 2 + unit)),
                    0.01,
                    [-0.5, 0.5, 1.0],
                )
                .unwrap();
        }
    }
    buffer
}

fn reference_attribute(
    base: &VertexArrayState,
    targets: &[&VertexArrayState],
    weights: &[f32],
    vertex: usize,
    unsigned: bool,
) -> Vec<f32> {
    (0..base.component_count())
        .map(|slot| {
            let read = |array: &VertexArrayState| {
                let value = array.component(vertex, slot).unwrap();
                if unsigned {
                    f64::from(value as u8)
                } else {
                    f64::from(value)
                }
            };
            let base = read(base);
            let mut difference = 0.0;
            for (target, weight) in targets.iter().zip(weights) {
                difference += f64::from(*weight) * (read(target) - base);
            }
            (base + difference) as f32
        })
        .collect()
}

#[test]
fn morph_batches_preserve_component_weight_order_for_every_attribute() {
    let base = animated_mesh(0, true);
    let targets = [-17, 7, 41].map(|offset| animated_mesh(offset, true));
    for weights in [[0.0; 3], [0.2, -0.75, 0.9], [1.0e18, -1.0e18, 0.0]] {
        let prepared = base.morphed_vertices(&targets, &weights).unwrap();
        let vertices = prepared.transformed_vertices(Mat4::IDENTITY);
        for (index, vertex) in vertices.iter().enumerate() {
            let (positions, scale, bias) = base.positions().unwrap();
            let components = reference_attribute(
                positions,
                &targets
                    .each_ref()
                    .map(|target| target.positions().unwrap().0),
                &weights,
                index,
                false,
            );
            assert_eq!(
                vertex.position,
                Vec4::new(
                    components[0] * scale + bias[0],
                    components[1] * scale + bias[1],
                    components[2] * scale + bias[2],
                    1.0
                )
            );
            let colors = reference_attribute(
                base.colors().unwrap(),
                &targets.each_ref().map(|target| target.colors().unwrap()),
                &weights,
                index,
                true,
            );
            let packed = [16, 8, 0, 24]
                .into_iter()
                .zip(colors)
                .fold(0, |color, (shift, value)| {
                    color | (value.clamp(0.0, 255.0).round() as u32) << shift
                });
            assert_eq!(vertex.color, packed);
            for unit in 0..2 {
                let (texture, scale, bias) = base.texture_coordinates(unit).unwrap();
                let components = reference_attribute(
                    texture,
                    &targets
                        .each_ref()
                        .map(|target| target.texture_coordinates(unit).unwrap().0),
                    &weights,
                    index,
                    false,
                );
                let expected = std::array::from_fn(|slot| {
                    components
                        .get(slot)
                        .map_or(0.0, |value| *value * scale + bias[slot])
                });
                assert_eq!(vertex.texture[unit], expected);
            }
            let normal = reference_attribute(
                base.normals().unwrap(),
                &targets.each_ref().map(|target| target.normals().unwrap()),
                &weights,
                index,
                false,
            );
            let normal = normal
                .into_iter()
                .map(|value| (value * 2.0 + 1.0) / 65_535.0)
                .collect::<Vec<_>>();
            let expected = Vec3::new(normal[0], normal[1], normal[2])
                .normalized()
                .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
            assert_eq!(
                prepared
                    .interpolated_normal([index; 3], [1.0, 0.0, 0.0])
                    .unwrap(),
                expected
            );
        }
    }
}

#[test]
fn morph_rejects_non_finite_component_results_before_transforming() {
    let base = animated_mesh(0, true);
    let target = animated_mesh(17, true);
    assert_eq!(
        base.morphed_vertices(&[target], &[f32::MAX])
            .unwrap_err()
            .code(),
        "invalid-object-graph"
    );
}

#[test]
#[ignore = "manual morph attribute throughput measurement"]
fn morph_attribute_throughput() {
    for attributes in [false, true] {
        for target_count in [1, 4, 16] {
            let base = animated_mesh(0, attributes);
            let targets: Vec<_> = (1..=target_count)
                .map(|offset| animated_mesh(offset, attributes))
                .collect();
            let weights: Vec<_> = (0..target_count)
                .map(|index| if index % 2 == 0 { 0.75 } else { -0.25 })
                .collect();
            let started = std::time::Instant::now();
            for _ in 0..128 {
                std::hint::black_box(
                    std::hint::black_box(&base)
                        .morphed_vertices(&targets, &weights)
                        .unwrap(),
                );
            }
            let elapsed = started.elapsed();
            let vertices = base
                .morphed_vertices(&targets, &weights)
                .unwrap()
                .transformed_vertices(Mat4::IDENTITY);
            let checksum = vertices.iter().fold(0_u64, |mut sum, vertex| {
                sum = sum.wrapping_mul(31).wrapping_add(u64::from(vertex.color));
                for component in [
                    vertex.position.x,
                    vertex.position.y,
                    vertex.position.z,
                    vertex.position.w,
                ]
                .into_iter()
                .chain(vertex.texture.into_iter().flatten())
                {
                    sum = sum
                        .wrapping_mul(31)
                        .wrapping_add(u64::from(component.to_bits()));
                }
                sum
            });
            eprintln!(
                "attributes={attributes} targets={target_count} elapsed={elapsed:?} checksum={checksum:016x}"
            );
        }
    }
}
