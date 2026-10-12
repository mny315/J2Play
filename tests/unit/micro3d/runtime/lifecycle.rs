use super::*;

#[test]
fn failed_figure_draw_preserves_objects_and_allows_retry() {
    let mut runtime = Runtime::new(4, 1 << 20, 32, 32).unwrap();
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
        ..FigureLayoutState::default()
    };
    let objects = runtime.objects.clone();
    let metrics = runtime.metrics();
    for (figure, light, projection, expected) in [
        (1, None, Projection::ParallelScale, "type"),
        (2, Some(1), Projection::ParallelScale, "type"),
        (2, Some(99), Projection::ParallelScale, "disposed-object"),
        (
            2,
            None,
            Projection::Parallel {
                width: 0,
                height: 1,
            },
            "projection",
        ),
    ] {
        let error = runtime
            .render_figure(
                figure,
                0,
                0,
                FigureLayoutState {
                    projection,
                    ..layout.clone()
                },
                AffineTrans::IDENTITY,
                EffectState {
                    light,
                    ..EffectState::default()
                },
            )
            .unwrap_err();
        assert_eq!(error.code(), expected);
        assert_eq!(runtime.objects, objects);
        assert_eq!(runtime.metrics(), metrics);
    }
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
    assert_eq!(runtime.objects, objects);
    assert_eq!(runtime.metrics().render_calls, 1);
}

#[test]
fn loading_a_new_target_remains_renderable() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    let background = vec![0xff00_0000; 32 * 32];
    let layout = FigureLayoutState {
        scale: [4096, 4096],
        center: [16, 16],
        projection: Projection::ParallelScale,
        ..FigureLayoutState::default()
    };
    let coordinates = [-10, -10, 0, 10, -10, 0, 0, 10, 0];

    for draw_count in 1..=2 {
        runtime.load_target(32, 32, &background).unwrap();
        runtime
            .render_primitives(
                None,
                0,
                0,
                layout.clone(),
                AffineTrans::IDENTITY,
                EffectState::default(),
                PrimitiveData::new(0x0300_0400, 1, &coordinates, &[], &[], &[0x00ff_0000]),
            )
            .unwrap();
        assert_eq!(runtime.target_pixels()[16 * 32 + 16], 0xffff_0000);
        assert_eq!(runtime.metrics().rasterized_triangles, draw_count);
    }
}

#[test]
fn guest_closure_includes_plain_java_objects_owned_by_native_state() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .create(
            2,
            ObjectKind::Layout(FigureLayoutState {
                affines: vec![1],
                ..FigureLayoutState::default()
            }),
        )
        .unwrap();

    assert_eq!(runtime.guest_closure([2]), [1, 2]);
    assert_eq!(runtime.guest_closure([999, 2, 999, 2]), [1, 2]);
    assert!(runtime.guest_closure([1, 999]).is_empty());

    // Filtering initial roots must not drop plain Java endpoints discovered
    // through actual native ownership edges.
    runtime.dispose(2).unwrap();
    assert_eq!(runtime.guest_closure([2, 999]), [2]);
}

#[test]
fn posture_frame_changes_rendered_pixels() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Figure(FigureState {
                data: FigureData {
                    format_version: 5,
                    uv_bits: 8,
                    vertices: vec![
                        Vector3D::new(-5, -5, 0),
                        Vector3D::new(5, -5, 0),
                        Vector3D::new(0, 5, 0),
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
                posture: Some((2, 0, 0)),
            }),
        )
        .unwrap();
    runtime
        .create_action(2, &translating_action(), LoaderLimits::default())
        .unwrap();
    let layout = FigureLayoutState {
        scale: [4096, 4096],
        center: [16, 16],
        projection: Projection::ParallelScale,
        ..FigureLayoutState::default()
    };

    runtime
        .load_target(32, 32, &vec![0xff00_0000; 32 * 32])
        .unwrap();
    runtime
        .render_figure(
            1,
            0,
            0,
            layout.clone(),
            AffineTrans::IDENTITY,
            EffectState::default(),
        )
        .unwrap();
    let at_start = runtime.target_pixels().to_vec();

    let ObjectKind::Figure(figure) = runtime.kind_mut(1).unwrap() else {
        panic!("figure object changed kind");
    };
    figure.posture = Some((2, 0, 10 << 16));
    runtime
        .load_target(32, 32, &vec![0xff00_0000; 32 * 32])
        .unwrap();
    runtime
        .render_figure(
            1,
            0,
            0,
            layout,
            AffineTrans::IDENTITY,
            EffectState::default(),
        )
        .unwrap();

    assert_ne!(runtime.target_pixels(), at_start);
}
