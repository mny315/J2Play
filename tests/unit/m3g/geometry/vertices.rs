use super::*;

fn attributed_mesh(colors: bool, texture_units: usize) -> VertexBufferState {
    let count = 4096;
    let mut positions = VertexArrayState::new(count, 3, VertexComponent::Short).unwrap();
    let data: Vec<_> = (0..count)
        .flat_map(|index| {
            let x = (index % 64) as i16 - 32;
            let y = (index / 64) as i16 - 32;
            [x, y, x / 4 - y / 8]
        })
        .collect();
    positions.set_shorts(0, count, &data).unwrap();
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(Some(positions), 0.25, [1.0, -2.0, 3.0])
        .unwrap();
    if colors {
        let mut colors = VertexArrayState::new(count, 4, VertexComponent::Byte).unwrap();
        let data: Vec<_> = (0..count * 4).map(|index| index as i8).collect();
        colors.set_bytes(0, count, &data).unwrap();
        buffer.set_colors(Some(colors)).unwrap();
    }
    for unit in 0..texture_units {
        let mut coordinates =
            VertexArrayState::new(count, 2 + unit, VertexComponent::Short).unwrap();
        let data: Vec<_> = (0..count * (2 + unit))
            .map(|index| (index % 512) as i16 - 256)
            .collect();
        coordinates.set_shorts(0, count, &data).unwrap();
        buffer
            .set_texture_coordinates(unit, Some(coordinates), 0.01, [-0.5, 0.5, 1.0])
            .unwrap();
    }
    buffer
}

#[test]
fn attribute_decoding_matches_individual_components() {
    for colors in [false, true] {
        for textures in 0..=2 {
            let mesh = attributed_mesh(colors, textures);
            for index in 0..mesh.vertex_count() {
                let vertex = mesh.vertex(index).unwrap();
                let (positions, scale, bias) = mesh.positions().unwrap();
                let [x, y, z] = std::array::from_fn(|slot| {
                    f32::from(positions.component(index, slot).unwrap()) * scale + bias[slot]
                });
                assert_eq!(vertex.position, Vec4::new(x, y, z, 1.0));
                let expected_color = mesh.colors().map_or(mesh.default_color(), |colors| {
                    let component = |slot| u32::from(colors.component(index, slot).unwrap() as u8);
                    component(3) << 24 | component(0) << 16 | component(1) << 8 | component(2)
                });
                assert_eq!(vertex.color, expected_color);
                for unit in 0..2 {
                    let expected = mesh.texture_coordinates(unit).map_or(
                        [0.0, 0.0, 1.0],
                        |(array, scale, bias)| {
                            std::array::from_fn(|slot| {
                                if slot < array.component_count() {
                                    f32::from(array.component(index, slot).unwrap()) * scale
                                        + bias[slot]
                                } else {
                                    0.0
                                }
                            })
                        },
                    );
                    assert_eq!(vertex.texture[unit], expected);
                }
            }
            for index in [mesh.vertex_count(), usize::MAX] {
                assert_eq!(mesh.vertex(index).unwrap_err().code(), "vertex-bounds");
            }
        }
    }
}

#[test]
#[ignore = "manual vertex attribute decoding throughput measurement"]
fn vertex_attribute_throughput() {
    for (colors, textures) in [(false, 0), (true, 0), (true, 1), (true, 2)] {
        let mesh = attributed_mesh(colors, textures);
        let transform = Mat4::translation(3.0, -2.0, 1.0)
            .unwrap()
            .multiplied(Mat4::scale(2.0, 0.5, -1.0).unwrap());
        let started = std::time::Instant::now();
        let mut vertices = Vec::new();
        for _ in 0..256 {
            vertices = std::hint::black_box(&mesh)
                .transformed_vertices(transform)
                .unwrap();
            std::hint::black_box(&vertices);
        }
        let elapsed = started.elapsed();
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
            "colors={colors} textures={textures} elapsed={elapsed:?} checksum={checksum:016x}"
        );
    }
}
