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
    let affine = AffineTrans {
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
    };
    draw_affine_mesh(runtime, light, sphere, projection, affine);
}

fn draw_affine_mesh(
    runtime: &mut Runtime,
    light: bool,
    sphere: bool,
    projection: Projection,
    affine: AffineTrans,
) {
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
            affine,
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
    for transform in mesh_bone_transforms() {
        let mut shared = mesh_runtime(false);
        let mut expanded = mesh_runtime(true);
        for runtime in [&mut shared, &mut expanded] {
            let ObjectKind::Figure(figure) = runtime.kind_mut(1).unwrap() else {
                panic!("expected Figure");
            };
            figure.data.bones[0].transform = transform;
        }
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
}

#[test]
#[ignore = "manual indexed figure geometry throughput measurement"]
fn figure_mesh_throughput() {
    for expanded in [false, true] {
        let mut runtime = mesh_runtime(expanded);
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
                    "expanded={expanded} light={light} sphere={sphere} elapsed={:?} checksum={checksum:016x}",
                    started.elapsed()
                );
            }
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

#[test]
fn identity_bone_preserves_extreme_positions_and_normalizes_raw_normals() {
    let mut figure = mesh_fixture(false);
    figure.vertices = vec![
        Vector3D::new(i32::MIN, i32::MAX, -1),
        Vector3D::new(i32::MAX, i32::MIN, 1),
        Vector3D::default(),
        Vector3D::new(7, -11, 13),
    ];
    figure.normals = vec![
        Vector3D::new(3, 4, 0),
        Vector3D::default(),
        Vector3D::new(i32::MIN, i32::MAX, -1),
        Vector3D::new(1, 1, 1),
    ];
    figure.faces.clear();
    figure.bones[0].vertex_count = 4;
    for normals in [false, true] {
        let (positions, actual_normals) = figure_geometry(&figure, None, normals).unwrap();
        assert_eq!(positions, figure.vertices);
        let expected: Vec<_> = if normals {
            figure
                .normals
                .iter()
                .copied()
                .map(|mut normal| {
                    normal.unit();
                    normal
                })
                .collect()
        } else {
            Vec::new()
        };
        assert_eq!(actual_normals, expected);
    }
}

#[test]
fn mixed_identity_bones_keep_world_transforms_in_bind_and_action_poses() {
    let mut figure = mesh_fixture(false);
    figure.vertices = vec![
        Vector3D::new(i32::MIN, 0, i32::MAX),
        Vector3D::new(7, -11, 13),
        Vector3D::new(i32::MAX, i32::MIN, 0),
        Vector3D::new(-9, 5, 17),
        Vector3D::new(i32::MAX, 3, i32::MIN),
        Vector3D::new(3, 4, 0),
    ];
    figure.normals = vec![Vector3D::new(3, 4, 0); 6];
    figure.faces.clear();
    let translation = |x| AffineTrans::new([4096, 0, 0, x, 0, 4096, 0, 0, 0, 0, 4096, 0]);
    figure.bones = vec![
        Bone {
            vertex_count: 2,
            parent: -1,
            transform: AffineTrans::IDENTITY,
        },
        Bone {
            vertex_count: 2,
            parent: 0,
            transform: translation(17),
        },
        Bone {
            vertex_count: 2,
            parent: 1,
            transform: translation(-17),
        },
    ];
    let action = ActionData {
        frame_count: 1,
        segments: [AffineTrans::IDENTITY, translation(3), translation(-3)]
            .map(crate::ActionSegmentData::Affine)
            .to_vec(),
        pattern_keys: Vec::new(),
    };
    for animated in [false, true] {
        for normals in [false, true] {
            let (positions, actual_normals) =
                figure_geometry(&figure, animated.then_some((&action, 0)), normals).unwrap();
            let middle = translation(if animated { 20 } else { 17 });
            let expected: Vec<_> = figure
                .vertices
                .iter()
                .enumerate()
                .map(|(index, position)| {
                    if (2..4).contains(&index) {
                        middle.transform(*position)
                    } else {
                        *position
                    }
                })
                .collect();
            assert_eq!(positions, expected);
            assert_eq!(
                actual_normals,
                if normals {
                    vec![Vector3D::new(2458, 3277, 0); 6]
                } else {
                    Vec::new()
                }
            );
        }
    }
}

#[test]
fn identity_bone_still_validates_parent_before_empty_vertex_ranges() {
    let mut figure = mesh_fixture(false);
    figure.bones[0].vertex_count = 0;
    figure.bones[0].parent = 0;
    assert_eq!(
        figure_geometry(&figure, None, false).unwrap_err().code(),
        "figure-bone-parent"
    );
}

#[test]
#[ignore = "manual skeletal geometry preparation throughput measurement"]
fn geometry_preparation_throughput() {
    let shear = AffineTrans::new([4096, 1024, 0, 0, 0, 3072, 0, 0, 0, 0, 4096, 0]);
    for expanded in [false, true] {
        for mode in 0..5 {
            let mut figure = mesh_fixture(expanded);
            if mode == 1 {
                figure.bones[0].transform.values[3] = 17;
            }
            if mode == 2 {
                figure.bones[0].transform = shear;
            }
            if mode == 3 {
                let count = figure.bones[0].vertex_count;
                figure.bones[0].vertex_count = count / 2;
                figure.bones.push(Bone {
                    vertex_count: count - count / 2,
                    parent: 0,
                    transform: shear,
                });
            }
            let action = ActionData {
                frame_count: 1,
                segments: vec![crate::ActionSegmentData::Affine(AffineTrans::IDENTITY)],
                pattern_keys: Vec::new(),
            };
            let posture = (mode == 4).then_some((&action, 0));
            for normals in [false, true] {
                let repeats = (262_144 / figure.vertices.len()).max(64);
                let started = std::time::Instant::now();
                for _ in 0..repeats {
                    std::hint::black_box(
                        figure_geometry(std::hint::black_box(&figure), posture, normals).unwrap(),
                    );
                }
                let elapsed = started.elapsed();
                let (positions, normal_values) =
                    figure_geometry(&figure, posture, normals).unwrap();
                let checksum = positions
                    .iter()
                    .chain(&normal_values)
                    .fold(0_u64, |sum, value| {
                        [value.x, value.y, value.z]
                            .into_iter()
                            .fold(sum, |sum, component| {
                                sum.wrapping_mul(31)
                                    .wrapping_add(u64::from(component.cast_unsigned()))
                            })
                    });
                eprintln!(
                    "geometry expanded={expanded} mode={mode} normals={normals} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}

fn mesh_bone_transforms() -> [AffineTrans; 3] {
    [
        AffineTrans::IDENTITY,
        AffineTrans::new([4096, 0, 0, 17, 0, 4096, 0, 0, 0, 0, 4096, 0]),
        AffineTrans::new([4096, 1024, 0, 0, 0, 3072, 0, 0, 0, 0, 4096, 0]),
    ]
}

#[test]
#[ignore = "manual full Figure rendering with varied bone transforms"]
fn figure_bone_transform_throughput() {
    for (bone_mode, transform) in mesh_bone_transforms().into_iter().enumerate() {
        for expanded in [false, true] {
            let mut runtime = mesh_runtime(expanded);
            let ObjectKind::Figure(figure) = runtime.kind_mut(1).unwrap() else {
                panic!("expected Figure");
            };
            figure.data.bones[0].transform = transform;
            for light in [false, true] {
                for sphere in [false, true] {
                    draw_mesh(&mut runtime, light, sphere);
                    let started = std::time::Instant::now();
                    for _ in 0..64 {
                        draw_mesh(std::hint::black_box(&mut runtime), light, sphere);
                    }
                    let elapsed = started.elapsed();
                    let checksum = runtime.target_pixels().iter().fold(0_u64, |sum, pixel| {
                        sum.wrapping_mul(31).wrapping_add(u64::from(*pixel))
                    });
                    eprintln!(
                        "bone-mode={bone_mode} expanded={expanded} light={light} sphere={sphere} elapsed={elapsed:?} checksum={checksum:016x}"
                    );
                }
            }
        }
    }
}

#[test]
fn identity_geometry_normalizes_full_width_vectors_with_and_without_bones() {
    let edges = [i32::MIN, i32::MIN + 1, -4096, -1, 0, 1, 4096, i32::MAX];
    let mut input = Vec::new();
    for x in edges {
        for y in edges {
            for z in edges {
                input.push(Vector3D::new(x, y, z));
            }
        }
    }
    let mut seed = 0x21a9_e5d6_7384_0bcf_u64;
    for _ in 0..1024 {
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed as u32).cast_signed()
        };
        input.push(Vector3D::new(next(), next(), next()));
    }
    let expected: Vec<_> = input
        .iter()
        .copied()
        .map(|normal| AffineTrans::IDENTITY.transform_normal(normal))
        .collect();
    for bones in [false, true] {
        let mut figure = mesh_fixture(false);
        figure.vertices = vec![Vector3D::default(); input.len()];
        figure.normals = input.clone();
        if bones {
            figure.bones[0].vertex_count = u16::try_from(input.len()).unwrap();
        } else {
            figure.bones.clear();
        }
        let (_, actual) = rendering::figure_geometry(&figure, None, true).unwrap();
        assert_eq!(actual, expected, "bones={bones}");
    }
}

#[test]
#[ignore = "manual figure layout transform throughput measurement"]
fn figure_layout_transform_throughput() {
    for (mode, affine) in [
        (0, AffineTrans::IDENTITY),
        (
            1,
            AffineTrans::new([4096, 0, 0, 3, 0, 4096, 0, -5, 0, 0, 4096, 10]),
        ),
        (
            2,
            AffineTrans::new([4096, 1024, 0, 0, 0, 3072, 0, 0, 0, 0, 4096, 0]),
        ),
    ] {
        for expanded in [false, true] {
            let mut runtime = mesh_runtime(expanded);
            for light in [false, true] {
                for sphere in [false, true] {
                    draw_affine_mesh(
                        &mut runtime,
                        light,
                        sphere,
                        Projection::ParallelScale,
                        affine,
                    );
                    let started = std::time::Instant::now();
                    for _ in 0..64 {
                        draw_affine_mesh(
                            std::hint::black_box(&mut runtime),
                            light,
                            sphere,
                            Projection::ParallelScale,
                            affine,
                        );
                    }
                    let elapsed = started.elapsed();
                    let checksum = runtime
                        .target_pixels()
                        .iter()
                        .map(|pixel| u64::from(*pixel))
                        .chain(
                            runtime
                                .renderer
                                .depth()
                                .iter()
                                .map(|depth| u64::from(*depth)),
                        )
                        .fold(0_u64, |sum, value| sum.wrapping_mul(31).wrapping_add(value));
                    eprintln!(
                        "layout-mode={mode} expanded={expanded} light={light} sphere={sphere} elapsed={elapsed:?} checksum={checksum:016x}"
                    );
                }
            }
        }
    }
}

#[test]
fn layout_geometry_keeps_exact_clamping_and_normal_rounding() {
    let edges = [i32::MIN, i32::MIN + 1, -4096, -1, 0, 1, 4096, i32::MAX];
    let mut input = Vec::new();
    for x in edges {
        for y in edges {
            for z in edges {
                input.push(Vector3D::new(x, y, z));
            }
        }
    }
    let mut seed = 0xc5e8_106a_7f39_42bd_u64;
    for _ in 0..1024 {
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed as u32).cast_signed()
        };
        input.push(Vector3D::new(next(), next(), next()));
    }
    let mut transforms = vec![
        AffineTrans::IDENTITY,
        AffineTrans::new([4096, 0, 0, 1, 0, 4096, 0, -2, 0, 0, 4096, 3]),
        AffineTrans::new([4096, 0, 0, 0, 0, 4096, 0, 0, 0, 0, 4096, 100]),
        AffineTrans::new([4096, 0, 0, i32::MIN, 0, 4096, 0, i32::MAX, 0, 0, 4096, -1]),
        AffineTrans::new([4096, 0, 0, i32::MAX, 0, 4096, 0, i32::MIN, 0, 0, 4096, 1]),
        AffineTrans::new([8192, 0, 0, 97, 0, 2048, 0, -103, 0, 0, 4096, 7]),
        AffineTrans::new([4096, 512, -64, 23, 128, 4096, 32, -21, -512, 128, 4096, 12]),
        AffineTrans::new([-4096, 0, 0, 0, 0, 4096, 0, 0, 0, 0, 4096, 0]),
        AffineTrans::ZERO,
        AffineTrans::new([
            i32::MIN,
            i32::MAX,
            -1,
            0,
            i32::MAX,
            i32::MIN,
            1,
            0,
            1,
            -1,
            i32::MAX,
            0,
        ]),
    ];
    for _ in 0..16 {
        transforms.push(AffineTrans::new(std::array::from_fn(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed as u32).cast_signed()
        })));
    }
    for transform in transforms {
        for (positions, normals) in [
            (input.len(), input.len()),
            (0, input.len()),
            (input.len(), 0),
            (17, 29),
        ] {
            let mut vertices = input[..positions].to_vec();
            let mut normals = input[..normals].to_vec();
            let expected_vertices: Vec<_> = vertices
                .iter()
                .copied()
                .map(|position| transform.transform(position))
                .collect();
            let expected_normals: Vec<_> = normals
                .iter()
                .copied()
                .map(|normal| transform.transform_normal(normal))
                .collect();
            rendering::transform_figure_geometry(&mut vertices, &mut normals, transform);
            assert_eq!(vertices, expected_vertices, "{transform:?}");
            assert_eq!(normals, expected_normals, "{transform:?}");
        }
    }
}

#[test]
fn translation_bones_keep_full_width_positions_and_normal_rounding() {
    let edges = [i32::MIN, i32::MIN + 1, -4096, -1, 0, 1, 4096, i32::MAX];
    let mut input = Vec::new();
    for x in edges {
        for y in edges {
            for z in edges {
                input.push(Vector3D::new(x, y, z));
            }
        }
    }
    let mut seed = 0xf291_8ac5_43d0_76be_u64;
    for _ in 0..1024 {
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed as u32).cast_signed()
        };
        input.push(Vector3D::new(next(), next(), next()));
    }
    for transform in [
        AffineTrans::new([4096, 0, 0, 3, 0, 4096, 0, -5, 0, 0, 4096, 7]),
        AffineTrans::new([4096, 0, 0, i32::MIN, 0, 4096, 0, i32::MAX, 0, 0, 4096, -1]),
        AffineTrans::new([4096, 0, 0, i32::MAX, 0, 4096, 0, i32::MIN, 0, 0, 4096, 1]),
    ] {
        let mut figure = mesh_fixture(false);
        figure.vertices = input.clone();
        figure.normals = input.clone();
        figure.bones[0].vertex_count = u16::try_from(input.len()).unwrap();
        figure.bones[0].transform = transform;
        let expected_vertices: Vec<_> = input
            .iter()
            .copied()
            .map(|vertex| transform.transform(vertex))
            .collect();
        let expected_normals: Vec<_> = input
            .iter()
            .copied()
            .map(|normal| transform.transform_normal(normal))
            .collect();
        for with_normals in [false, true] {
            let (vertices, normals) =
                rendering::figure_geometry(&figure, None, with_normals).unwrap();
            assert_eq!(vertices, expected_vertices, "{transform:?}");
            if with_normals {
                assert_eq!(normals, expected_normals, "{transform:?}");
            } else {
                assert!(normals.is_empty());
            }
        }
    }
}
