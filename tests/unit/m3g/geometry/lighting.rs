use super::*;

fn lit_mesh() -> VertexBufferState {
    let count = 4096;
    let mut positions = VertexArrayState::new(count, 3, VertexComponent::Short).unwrap();
    let mut normals = VertexArrayState::new(count, 3, VertexComponent::Short).unwrap();
    let mut colors = VertexArrayState::new(count, 3, VertexComponent::Byte).unwrap();
    let mut position_data = Vec::new();
    let mut normal_data = Vec::new();
    let mut color_data = Vec::new();
    for index in 0..count {
        let x = (index % 64) as i16 - 32;
        let y = (index / 64) as i16 - 32;
        position_data.extend([x, y, x / 4 - y / 8]);
        normal_data.extend([x * 256, y * 256, 16384]);
        color_data.extend([x as i8 * 2, y as i8 * 2, 64]);
    }
    positions.set_shorts(0, count, &position_data).unwrap();
    normals.set_shorts(0, count, &normal_data).unwrap();
    colors.set_bytes(0, count, &color_data).unwrap();
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(Some(positions), 1.0, [0.0; 3])
        .unwrap();
    buffer.set_normals(Some(normals)).unwrap();
    buffer.set_colors(Some(colors)).unwrap();
    buffer
}

fn light_sources() -> [LightSource; 4] {
    [
        LightSource::Ambient {
            color: 0x0040_6080,
            intensity: 0.25,
        },
        LightSource::Directional {
            direction: Vec3::new(1.0, 2.0, 4.0),
            color: 0x00e0_c0a0,
            intensity: 0.75,
        },
        LightSource::Omni {
            position: Vec3::new(-16.0, 8.0, 64.0),
            color: 0x00a0_b0c0,
            intensity: 1.5,
            attenuation: [1.0, 0.05, 0.001],
        },
        LightSource::Spot {
            position: Vec3::new(4.0, 8.0, 64.0),
            direction: Vec3::new(-1.0, 0.0, -8.0),
            color: 0x0090_c0e0,
            intensity: 2.0,
            attenuation: [0.5, 0.01, 0.002],
            angle_degrees: 45.0,
            exponent: 8.0,
        },
    ]
}

fn lit_material() -> MaterialState {
    MaterialState {
        specular: 0x0040_3020,
        emissive: 0x0001_0203,
        shininess: 16.0,
        ..MaterialState::default()
    }
}

#[test]
fn batch_lighting_matches_individual_vertices_for_each_light_kind() {
    let mesh = lit_mesh();
    let lights = light_sources();
    let camera = Vec3::new(0.0, 0.0, 128.0);
    for tracking in [false, true] {
        for count in 1..=lights.len() {
            let vertices = mesh
                .transformed_lit_vertices(
                    Mat4::IDENTITY,
                    Mat4::IDENTITY,
                    camera,
                    lit_material(),
                    tracking,
                    &lights[..count],
                    1.0,
                )
                .unwrap();
            assert_eq!(vertices.len(), 4096);
            for (index, vertex) in vertices.iter().enumerate() {
                let original = mesh.vertex(index).unwrap();
                let normal = mesh.normals.as_ref().unwrap().normal(index).unwrap();
                let expected = crate::shade_lit_vertex(
                    lit_material(),
                    original.color,
                    tracking,
                    Vec3::new(normal.x, normal.y, normal.z),
                    Vec3::new(
                        original.position.x,
                        original.position.y,
                        original.position.z,
                    ),
                    Vec3::new(
                        camera.x - original.position.x,
                        camera.y - original.position.y,
                        camera.z - original.position.z,
                    ),
                    &lights[..count],
                )
                .unwrap();
                assert_eq!(vertex.color, expected);
            }
        }
    }
}

#[test]
#[ignore = "manual fixed-function vertex lighting throughput measurement"]
fn vertex_lighting_throughput() {
    measure_vertex_lighting(true);
}

#[test]
#[ignore = "manual diffuse vertex lighting throughput measurement"]
fn diffuse_vertex_lighting_throughput() {
    measure_vertex_lighting(false);
}

fn measure_vertex_lighting(specular: bool) {
    let mesh = lit_mesh();
    let lights = light_sources();
    let mut material = lit_material();
    if !specular {
        material.specular = 0;
    }
    for tracking in [false, true] {
        for count in 1..=lights.len() {
            let started = std::time::Instant::now();
            let mut vertices = Vec::new();
            for _ in 0..128 {
                vertices = std::hint::black_box(&mesh)
                    .transformed_lit_vertices(
                        Mat4::IDENTITY,
                        Mat4::IDENTITY,
                        Vec3::new(0.0, 0.0, 128.0),
                        material,
                        tracking,
                        &lights[..count],
                        1.0,
                    )
                    .unwrap();
                std::hint::black_box(&vertices);
            }
            let checksum = vertices.iter().fold(0_u64, |sum, vertex| {
                sum.wrapping_mul(31).wrapping_add(u64::from(vertex.color))
            });
            eprintln!(
                "specular={specular} tracking={tracking} lights={count} elapsed={:?} checksum={checksum:016x}",
                started.elapsed()
            );
        }
    }
}
