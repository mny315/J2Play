use super::*;

#[test]
fn action_masks_switch_alternative_parts_and_keep_common_geometry() {
    let mut runtime = Runtime::new(4, 1 << 20, 32, 32).unwrap();
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for (index, (pattern, color)) in [
        (0, 0xffff_0000),
        (1, 0xff00_ff00),
        (2, 0xff00_00ff),
        (31, 0xffff_ffff),
    ]
    .into_iter()
    .enumerate()
    {
        let x = -14 + i32::try_from(index).unwrap() * 8;
        let first = u32::try_from(vertices.len()).unwrap();
        vertices.extend([
            Vector3D::new(x, -4, 0),
            Vector3D::new(x + 6, -4, 0),
            Vector3D::new(x + 3, 4, 0),
        ]);
        faces.push(Face {
            indices: [first, first + 1, first + 2],
            uv: [[0, 0]; 3],
            attributes: 0,
            color: Some(color),
            pattern,
            material: None,
        });
    }
    runtime
        .create(
            1,
            ObjectKind::Figure(FigureState {
                data: FigureData {
                    format_version: 5,
                    uv_bits: 8,
                    vertices,
                    normals: Vec::new(),
                    faces,
                    bones: vec![Bone {
                        vertex_count: 12,
                        parent: -1,
                        transform: AffineTrans::IDENTITY,
                    }],
                    pattern_count: 33,
                    material_count: 0,
                },
                textures: Vec::new(),
                selected_texture: 0,
                pattern: 0,
                posture: None,
            }),
        )
        .unwrap();
    let action = ActionData {
        frame_count: 6,
        segments: vec![crate::ActionSegmentData::Affine(AffineTrans::IDENTITY)],
        pattern_keys: vec![[0, 2, 0], [2, 4, 0], [4, 0, 0x8000], [6, 0, 0]],
    };
    for (frame, visible) in [
        (0, Some(0xff00_ff00)),
        (2, Some(0xff00_00ff)),
        (4, Some(0xffff_ffff)),
        (6, None),
    ] {
        runtime
            .set_figure_pattern(1, action.sample_pattern(frame << 16))
            .unwrap();
        runtime
            .load_target(32, 32, &vec![0xff00_0000; 32 * 32])
            .unwrap();
        runtime
            .render_figure(
                1,
                0,
                0,
                FigureLayoutState {
                    center: [16, 16],
                    scale: [4096, 4096],
                    ..FigureLayoutState::default()
                },
                AffineTrans::IDENTITY,
                EffectState::default(),
            )
            .unwrap();
        assert!(runtime.target_pixels().contains(&0xffff_0000));
        for color in [0xff00_ff00, 0xff00_00ff, 0xffff_ffff] {
            assert_eq!(
                runtime.target_pixels().contains(&color),
                visible == Some(color),
                "frame={frame}"
            );
        }
    }
}

#[test]
fn figure_uses_material_textures_and_only_enabled_pattern_groups() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    for (guest, color) in [(1, 0xffff_0000), (2, 0xff00_ff00)] {
        runtime
            .create(
                guest,
                ObjectKind::Texture(TextureData {
                    width: 1,
                    height: 1,
                    pixels: vec![color].into(),
                    for_model: true,
                    color_key: 0,
                }),
            )
            .unwrap();
    }
    runtime
        .create(
            3,
            ObjectKind::Figure(FigureState {
                data: FigureData {
                    format_version: 5,
                    uv_bits: 8,
                    vertices: vec![
                        Vector3D::new(-12, -8, 0),
                        Vector3D::new(-2, -8, 0),
                        Vector3D::new(-7, 8, 0),
                        Vector3D::new(2, -8, 0),
                        Vector3D::new(12, -8, 0),
                        Vector3D::new(7, 8, 0),
                    ],
                    normals: Vec::new(),
                    faces: vec![
                        Face {
                            indices: [0, 1, 2],
                            uv: [[0, 0]; 3],
                            attributes: 0,
                            color: None,
                            pattern: 0,
                            material: Some(0),
                        },
                        Face {
                            indices: [3, 4, 5],
                            uv: [[0, 0]; 3],
                            attributes: 0,
                            color: None,
                            pattern: 1,
                            material: Some(1),
                        },
                    ],
                    bones: vec![Bone {
                        vertex_count: 6,
                        parent: -1,
                        transform: AffineTrans::IDENTITY,
                    }],
                    pattern_count: 2,
                    material_count: 2,
                },
                textures: vec![1, 2],
                selected_texture: 0,
                pattern: 0,
                posture: None,
            }),
        )
        .unwrap();
    let layout = FigureLayoutState {
        scale: [4096, 4096],
        center: [16, 16],
        ..FigureLayoutState::default()
    };
    let background = vec![0xff00_0000; 32 * 32];

    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_figure(
            3,
            0,
            0,
            layout.clone(),
            AffineTrans::IDENTITY,
            EffectState::default(),
        )
        .unwrap();
    assert!(runtime.target_pixels().contains(&0xffff_0000));
    assert!(!runtime.target_pixels().contains(&0xff00_ff00));

    let ObjectKind::Figure(figure) = runtime.kind_mut(3).unwrap() else {
        panic!("expected Figure");
    };
    figure.pattern = 1;
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_figure(
            3,
            0,
            0,
            layout,
            AffineTrans::IDENTITY,
            EffectState::default(),
        )
        .unwrap();
    assert!(runtime.target_pixels().contains(&0xffff_0000));
    assert!(runtime.target_pixels().contains(&0xff00_ff00));
}

#[test]
fn figure_resolves_only_an_unambiguous_suite_model_texture() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Texture(TextureData {
                width: 1,
                height: 1,
                pixels: vec![0xffff_0000].into(),
                for_model: true,
                color_key: 0,
            }),
        )
        .unwrap();
    runtime
        .create(
            2,
            ObjectKind::Figure(FigureState {
                data: FigureData {
                    format_version: 5,
                    uv_bits: 8,
                    vertices: vec![
                        Vector3D::new(-10, -10, 0),
                        Vector3D::new(10, -10, 0),
                        Vector3D::new(0, 10, 0),
                    ],
                    normals: Vec::new(),
                    faces: vec![Face {
                        indices: [0, 1, 2],
                        uv: [[0, 0]; 3],
                        attributes: 0,
                        color: None,
                        pattern: 0,
                        material: Some(0),
                    }],
                    bones: vec![Bone {
                        vertex_count: 3,
                        parent: -1,
                        transform: AffineTrans::IDENTITY,
                    }],
                    pattern_count: 1,
                    material_count: 1,
                },
                textures: Vec::new(),
                selected_texture: 0,
                pattern: 0,
                posture: None,
            }),
        )
        .unwrap();
    let layout = FigureLayoutState {
        scale: [4096, 4096],
        center: [16, 16],
        projection: Projection::ParallelScale,
        ..FigureLayoutState::default()
    };
    let background = vec![0xff00_0000; 32 * 32];

    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_figure(
            2,
            0,
            0,
            layout.clone(),
            AffineTrans::IDENTITY,
            EffectState::default(),
        )
        .unwrap();
    assert!(runtime.target_pixels().contains(&0xffff_0000));
    assert!(!runtime.target_pixels().contains(&0xffff_ffff));

    runtime
        .create(
            3,
            ObjectKind::Texture(TextureData {
                width: 1,
                height: 1,
                pixels: vec![0xff00_ff00].into(),
                for_model: true,
                color_key: 0,
            }),
        )
        .unwrap();
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_figure(
            2,
            0,
            0,
            layout.clone(),
            AffineTrans::IDENTITY,
            EffectState::default(),
        )
        .unwrap();
    assert!(runtime.target_pixels().contains(&0xffff_ffff));
    assert!(!runtime.target_pixels().contains(&0xffff_0000));

    runtime.set_figure_textures(2, vec![1]).unwrap();
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_figure(
            2,
            0,
            0,
            layout,
            AffineTrans::IDENTITY,
            EffectState::default(),
        )
        .unwrap();
    assert!(runtime.target_pixels().contains(&0xffff_0000));
    assert!(!runtime.target_pixels().contains(&0xffff_ffff));
}

#[test]
fn figure_honors_additive_polygon_materials() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    let background = vec![0xff12_3456; 32 * 32];
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Texture(TextureData {
                width: 1,
                height: 1,
                pixels: vec![0xff00_2040].into(),
                for_model: true,
                color_key: 0,
            }),
        )
        .unwrap();
    runtime
        .create(
            2,
            ObjectKind::Figure(FigureState {
                data: FigureData {
                    format_version: 5,
                    uv_bits: 8,
                    vertices: vec![
                        Vector3D::new(-10, -10, 0),
                        Vector3D::new(10, -10, 0),
                        Vector3D::new(0, 10, 0),
                    ],
                    normals: Vec::new(),
                    faces: vec![Face {
                        indices: [0, 1, 2],
                        uv: [[0, 0]; 3],
                        attributes: 0x04,
                        color: None,
                        pattern: 0,
                        material: Some(0),
                    }],
                    bones: vec![Bone {
                        vertex_count: 3,
                        parent: -1,
                        transform: AffineTrans::IDENTITY,
                    }],
                    pattern_count: 1,
                    material_count: 1,
                },
                textures: vec![1],
                selected_texture: 0,
                pattern: 0,
                posture: None,
            }),
        )
        .unwrap();
    runtime
        .render_figure(
            2,
            0,
            0,
            FigureLayoutState {
                scale: [4096, 4096],
                center: [16, 16],
                projection: Projection::ParallelScale,
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState::default(),
        )
        .unwrap();

    assert_eq!(runtime.target_pixels()[16 * 32 + 16], 0xff12_5496);
    assert_eq!(runtime.metrics().rasterized_triangles, 1);

    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_figure(
            2,
            0,
            0,
            FigureLayoutState {
                scale: [4096, 4096],
                center: [16, 16],
                projection: Projection::ParallelScale,
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState {
                transparency: false,
                ..EffectState::default()
            },
        )
        .unwrap();

    assert_eq!(runtime.target_pixels()[16 * 32 + 16], 0xff00_2040);
    assert_eq!(runtime.metrics().rasterized_triangles, 2);
}

#[test]
fn figure_light_shades_both_varying_and_uniform_normals() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Figure(FigureState {
                data: FigureData {
                    format_version: 5,
                    uv_bits: 8,
                    vertices: vec![
                        Vector3D::new(-10, -10, 0),
                        Vector3D::new(10, -10, 0),
                        Vector3D::new(0, 10, 0),
                    ],
                    normals: vec![
                        Vector3D::new(0, 0, 4096),
                        Vector3D::new(4096, 0, 0),
                        Vector3D::new(0, 0, 4096),
                    ],
                    faces: vec![Face {
                        indices: [0, 1, 2],
                        uv: [[0, 0]; 3],
                        attributes: 0,
                        color: Some(0xffc0_8040),
                        pattern: 0,
                        material: None,
                    }],
                    bones: vec![Bone {
                        vertex_count: 3,
                        parent: -1,
                        transform: AffineTrans::IDENTITY,
                    }],
                    pattern_count: 1,
                    material_count: 1,
                },
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
    let background = vec![0xff01_0203; 32 * 32];
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_figure(
            1,
            0,
            0,
            FigureLayoutState {
                scale: [4096, 4096],
                center: [16, 16],
                projection: Projection::ParallelScale,
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState {
                light: Some(2),
                ..EffectState::default()
            },
        )
        .unwrap();

    let shaded = runtime
        .target_pixels()
        .iter()
        .copied()
        .filter(|pixel| *pixel != background[0])
        .collect::<BTreeSet<_>>();
    assert!(shaded.len() > 8, "expected a diffuse gradient: {shaded:?}");
    assert!(shaded.iter().any(|pixel| (pixel >> 16) & 0xff >= 0xb0));
    assert!(shaded.iter().any(|pixel| (pixel >> 16) & 0xff <= 0x40));

    let ObjectKind::Figure(figure) = runtime.kind_mut(1).unwrap() else {
        panic!("figure object changed kind");
    };
    figure.data.normals.fill(Vector3D::new(4096, 0, 0));
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_figure(
            1,
            0,
            0,
            FigureLayoutState {
                scale: [4096, 4096],
                center: [16, 16],
                projection: Projection::ParallelScale,
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState {
                light: Some(2),
                ..EffectState::default()
            },
        )
        .unwrap();
    let flat = runtime
        .target_pixels()
        .iter()
        .copied()
        .filter(|pixel| *pixel != background[0])
        .collect::<BTreeSet<_>>();
    assert_eq!(flat, BTreeSet::from([0xff00_0000]));
}

#[test]
fn figure_light_modulates_model_texture_pixels() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    let texture_color = 0xffc0_8040;
    runtime
        .create(
            1,
            ObjectKind::Texture(TextureData {
                width: 1,
                height: 1,
                pixels: vec![texture_color].into(),
                for_model: true,
                color_key: 0,
            }),
        )
        .unwrap();
    runtime
        .create(
            2,
            ObjectKind::Figure(FigureState {
                data: FigureData {
                    format_version: 5,
                    uv_bits: 8,
                    vertices: vec![
                        Vector3D::new(-10, -10, 0),
                        Vector3D::new(10, -10, 0),
                        Vector3D::new(0, 10, 0),
                    ],
                    normals: vec![Vector3D::new(4096, 0, 0); 3],
                    faces: vec![Face {
                        indices: [0, 1, 2],
                        uv: [[0, 0]; 3],
                        attributes: 0,
                        color: None,
                        pattern: 0,
                        material: Some(0),
                    }],
                    bones: vec![Bone {
                        vertex_count: 3,
                        parent: -1,
                        transform: AffineTrans::IDENTITY,
                    }],
                    pattern_count: 1,
                    material_count: 1,
                },
                textures: vec![1],
                selected_texture: 0,
                pattern: 0,
                posture: None,
            }),
        )
        .unwrap();
    runtime
        .create(3, ObjectKind::Light(LightState::default()))
        .unwrap();
    let background = vec![0xff01_0203; 32 * 32];
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_figure(
            2,
            0,
            0,
            FigureLayoutState {
                scale: [4096, 4096],
                center: [16, 16],
                projection: Projection::ParallelScale,
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState {
                light: Some(3),
                ..EffectState::default()
            },
        )
        .unwrap();

    let visible = runtime
        .target_pixels()
        .iter()
        .copied()
        .filter(|pixel| *pixel != background[0])
        .collect::<BTreeSet<_>>();
    assert_eq!(visible, BTreeSet::from([0xff00_0000]));
}
