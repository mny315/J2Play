use super::*;

fn mesh_fixture(expanded: bool) -> FigureData {
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut faces = Vec::new();
    for y in 0..17 {
        for x in 0..17 {
            vertices.push(Vector3D::new((x - 8) * 3, (y - 8) * 3, x - y));
            normals.push(Vector3D::new((x - 8) * 128, (y - 8) * 128, 4096));
        }
    }
    for y in 0..16 {
        for x in 0..16 {
            let first = y * 17 + x;
            for indices in [
                [first, first + 1, first + 17],
                [first + 1, first + 18, first + 17],
            ] {
                faces.push(Face {
                    indices,
                    uv: [[0, 0]; 3],
                    attributes: FIGURE_ATTR_DOUBLE_FACE,
                    color: Some(0xff80_4020 | (x * 4) << 16 | (y * 4) << 8),
                    pattern: 0,
                    material: None,
                });
            }
        }
    }
    if expanded {
        let source_vertices = vertices;
        let source_normals = normals;
        vertices = Vec::new();
        normals = Vec::new();
        for face in &mut faces {
            for index in &mut face.indices {
                vertices.push(source_vertices[*index as usize]);
                normals.push(source_normals[*index as usize]);
                *index = u32::try_from(vertices.len() - 1).unwrap();
            }
        }
    }
    let vertex_count = u16::try_from(vertices.len()).unwrap();
    FigureData {
        format_version: 5,
        uv_bits: 8,
        vertices,
        normals,
        faces,
        bones: vec![Bone {
            vertex_count,
            parent: -1,
            transform: AffineTrans::IDENTITY,
        }],
        pattern_count: 1,
        material_count: 0,
    }
}

fn mesh_runtime(expanded: bool) -> Runtime {
    let mut runtime = Runtime::new(8, 1 << 20, 64, 64).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Figure(FigureState {
                data: mesh_fixture(expanded),
                textures: Vec::new(),
                selected_texture: 0,
                pattern: 0,
                posture: None,
            }),
        )
        .unwrap();
    runtime
        .create(2, ObjectKind::Light(LightState::default()))
        .unwrap();
    runtime
        .create(
            3,
            ObjectKind::Texture(TextureData {
                width: 16,
                height: 16,
                pixels: (0..256)
                    .map(|index| 0xff00_0020 | ((index % 16) * 16) << 16 | ((index / 16) * 16) << 8)
                    .collect(),
                for_model: false,
                color_key: 0,
            }),
        )
        .unwrap();
    runtime
}

fn draw_mesh(runtime: &mut Runtime, light: bool, sphere: bool) {
    draw_projected_mesh(runtime, light, sphere, Projection::ParallelScale);
}

fn draw_projected_mesh(runtime: &mut Runtime, light: bool, sphere: bool, projection: Projection) {
    runtime.renderer.clear(Some(0xff01_0203), true);
    runtime
        .render_figure(
            1,
            0,
            0,
            FigureLayoutState {
                projection,
                center: [32, 32],
                scale: [4096, 4096],
                ..FigureLayoutState::default()
            },
            AffineTrans {
                values: [
                    4096,
                    1024,
                    0,
                    0,
                    0,
                    3072,
                    0,
                    0,
                    0,
                    0,
                    4096,
                    if matches!(
                        projection,
                        Projection::PerspectiveFov { .. } | Projection::PerspectiveSize { .. }
                    ) {
                        100
                    } else {
                        0
                    },
                ],
            },
            EffectState {
                light: light.then_some(2),
                sphere_texture: sphere.then_some(3),
                ..EffectState::default()
            },
        )
        .unwrap();
}

#[test]
fn shared_figure_geometry_matches_independent_vertices_with_lighting_and_reflections() {
    let mut shared = mesh_runtime(false);
    let mut expanded = mesh_runtime(true);
    for light in [false, true] {
        for sphere in [false, true] {
            for projection in projection_modes() {
                draw_projected_mesh(&mut shared, light, sphere, projection);
                draw_projected_mesh(&mut expanded, light, sphere, projection);
                assert_eq!(shared.target_pixels(), expanded.target_pixels());
                assert_eq!(shared.renderer.depth(), expanded.renderer.depth());
                assert!(
                    shared
                        .target_pixels()
                        .iter()
                        .any(|pixel| *pixel != 0xff01_0203)
                );
            }
        }
    }
}

#[test]
#[ignore = "manual indexed figure geometry throughput measurement"]
fn figure_mesh_throughput() {
    let mut runtime = mesh_runtime(false);
    for light in [false, true] {
        for sphere in [false, true] {
            let started = std::time::Instant::now();
            for _ in 0..256 {
                draw_mesh(std::hint::black_box(&mut runtime), light, sphere);
            }
            let checksum = runtime.target_pixels().iter().fold(0_u64, |sum, pixel| {
                sum.wrapping_mul(31).wrapping_add(u64::from(*pixel))
            });
            eprintln!(
                "light={light} sphere={sphere} elapsed={:?} checksum={checksum:016x}",
                started.elapsed()
            );
        }
    }
}

#[test]
fn static_mesh_without_bones_keeps_lighting_and_reflections() {
    let mut explicit = mesh_runtime(false);
    let mut implicit = mesh_runtime(false);
    let ObjectKind::Figure(figure) = implicit.kind_mut(1).unwrap() else {
        panic!("expected Figure");
    };
    figure.data.bones.clear();
    for light in [false, true] {
        for sphere in [false, true] {
            draw_mesh(&mut explicit, light, sphere);
            draw_mesh(&mut implicit, light, sphere);
            assert_eq!(explicit.target_pixels(), implicit.target_pixels());
            assert_eq!(explicit.renderer.depth(), implicit.renderer.depth());
        }
    }
}

#[test]
#[ignore = "manual figure projection throughput measurement"]
fn figure_projection_throughput() {
    let mut runtime = mesh_runtime(false);
    for (index, projection) in projection_modes().into_iter().enumerate() {
        let started = std::time::Instant::now();
        for _ in 0..512 {
            draw_projected_mesh(std::hint::black_box(&mut runtime), true, false, projection);
        }
        let checksum = runtime.target_pixels().iter().fold(0_u64, |sum, pixel| {
            sum.wrapping_mul(31).wrapping_add(u64::from(*pixel))
        });
        eprintln!(
            "projection={index} elapsed={:?} checksum={checksum:016x}",
            started.elapsed()
        );
    }
}

fn projection_modes() -> [Projection; 4] {
    [
        Projection::ParallelScale,
        Projection::Parallel {
            width: 64,
            height: 64,
        },
        Projection::PerspectiveFov {
            near: 1,
            far: 200,
            angle: 1024,
        },
        Projection::PerspectiveSize {
            near: 1,
            far: 200,
            width: 16384,
            height: 12288,
        },
    ]
}

#[test]
fn invisible_figure_parts_do_not_prepare_an_unused_projection() {
    let mut runtime = mesh_runtime(false);
    let ObjectKind::Figure(figure) = runtime.kind_mut(1).unwrap() else {
        panic!("expected Figure");
    };
    for face in &mut figure.data.faces {
        face.pattern = 1;
    }
    draw_projected_mesh(
        &mut runtime,
        true,
        false,
        Projection::PerspectiveFov {
            near: 0,
            far: 0,
            angle: 1024,
        },
    );
    assert!(
        runtime
            .target_pixels()
            .iter()
            .all(|pixel| *pixel == 0xff01_0203)
    );
    assert_eq!(runtime.renderer.stats().submitted_triangles, 0);
}
