use super::*;

#[test]
fn primitive_subtract_blend_darkens_the_existing_target() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .load_target(32, 32, &vec![0xff80_8080; 32 * 32])
        .unwrap();
    runtime
        .render_primitives(
            None,
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
            PrimitiveData::new(
                0x0300_0460,
                1,
                &[-10, -10, 0, 10, -10, 0, 0, 10, 0],
                &[],
                &[],
                &[0x0020_3040],
            ),
        )
        .unwrap();

    assert!(runtime.target_pixels().contains(&0xff60_5040));
}

#[test]
fn primitive_color_key_discards_palette_zero_with_normal_and_additive_blending() {
    for (command, palette_zero) in [(0x0300_3010, 0xffff_ffff_u32), (0x0300_3050, 0xff00_0000)] {
        let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
        let background = vec![0xff12_3456; 32 * 32];
        runtime.load_target(32, 32, &background).unwrap();
        runtime
            .create(
                1,
                ObjectKind::Texture(TextureData {
                    width: 1,
                    height: 1,
                    pixels: vec![palette_zero].into(),
                    for_model: true,
                    color_key: palette_zero & 0x00ff_ffff,
                }),
            )
            .unwrap();
        runtime
            .render_primitives(
                Some(1),
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
                PrimitiveData::new(
                    command,
                    1,
                    &[-10, -10, 0, 10, -10, 0, 0, 10, 0],
                    &[],
                    &[0, 0, 0, 0, 0, 0],
                    &[],
                ),
            )
            .unwrap();

        assert_eq!(runtime.target_pixels(), background, "command={command:#x}");
    }
}

#[test]
fn figure_applies_palette_zero_only_to_color_key_faces() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    let background = vec![0xff12_3456; 32 * 32];
    let palette_zero = 0xffab_cdef;
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Texture(TextureData {
                width: 1,
                height: 1,
                pixels: vec![palette_zero].into(),
                for_model: true,
                color_key: palette_zero & 0x00ff_ffff,
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
                textures: vec![1],
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
    assert!(runtime.target_pixels().contains(&palette_zero));

    runtime.load_target(32, 32, &background).unwrap();
    let ObjectKind::Figure(figure) = runtime.kind_mut(2).unwrap() else {
        panic!("expected Figure");
    };
    figure.data.faces[0].attributes = FIGURE_ATTR_TRANSPARENT;
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
    assert_eq!(runtime.target_pixels(), background);
}

#[test]
fn figure_culls_back_faces_unless_material_is_double_sided() {
    let mut runtime = Runtime::new(16, 1 << 20, 48, 32).unwrap();
    let background = vec![0xff00_0000; 48 * 32];
    runtime.load_target(48, 32, &background).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Figure(FigureState {
                data: FigureData {
                    format_version: 5,
                    uv_bits: 8,
                    vertices: vec![
                        Vector3D::new(-20, -8, 0),
                        Vector3D::new(-10, -8, 0),
                        Vector3D::new(-15, 8, 0),
                        Vector3D::new(-5, -8, 0),
                        Vector3D::new(5, -8, 0),
                        Vector3D::new(0, 8, 0),
                        Vector3D::new(10, -8, 0),
                        Vector3D::new(20, -8, 0),
                        Vector3D::new(15, 8, 0),
                    ],
                    normals: Vec::new(),
                    faces: vec![
                        Face {
                            indices: [0, 1, 2],
                            uv: [[0, 0]; 3],
                            attributes: 0,
                            color: Some(0xffff_0000),
                            pattern: 0,
                            material: None,
                        },
                        Face {
                            indices: [3, 5, 4],
                            uv: [[0, 0]; 3],
                            attributes: 0,
                            color: Some(0xff00_ff00),
                            pattern: 0,
                            material: None,
                        },
                        Face {
                            indices: [6, 8, 7],
                            uv: [[0, 0]; 3],
                            attributes: FIGURE_ATTR_DOUBLE_FACE,
                            color: Some(0xff00_00ff),
                            pattern: 0,
                            material: None,
                        },
                    ],
                    bones: vec![Bone {
                        vertex_count: 9,
                        parent: -1,
                        transform: AffineTrans::IDENTITY,
                    }],
                    pattern_count: 1,
                    material_count: 0,
                },
                textures: Vec::new(),
                selected_texture: 0,
                pattern: 0,
                posture: None,
            }),
        )
        .unwrap();
    runtime
        .render_figure(
            1,
            0,
            0,
            FigureLayoutState {
                scale: [4096, 4096],
                center: [24, 16],
                projection: Projection::ParallelScale,
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState::default(),
        )
        .unwrap();

    assert!(runtime.target_pixels().contains(&0xffff_0000));
    assert!(!runtime.target_pixels().contains(&0xff00_ff00));
    assert!(runtime.target_pixels().contains(&0xff00_00ff));
    assert_eq!(runtime.metrics().rasterized_triangles, 2);
}
