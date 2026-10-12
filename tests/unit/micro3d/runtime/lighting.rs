use super::*;

#[test]
fn lighting_normalizes_direction_and_preserves_color_alpha() {
    let cases = [
        (
            Vector3D::new(3, 4, 0),
            Vector3D::new(4096, 0, 0),
            0x7f99_9999,
        ),
        (
            Vector3D::new(3, 4, 0),
            Vector3D::new(0, 4096, 0),
            0x7fcc_cccc,
        ),
        (
            Vector3D::new(3, 4, 0),
            Vector3D::new(0, 0, 4096),
            0x7f00_0000,
        ),
        (
            Vector3D::new(1, 1, 1),
            Vector3D::new(4096, 0, 0),
            0x7f93_9393,
        ),
        (Vector3D::default(), Vector3D::new(4096, 0, 0), 0x7f00_0000),
        (Vector3D::new(4096, 0, 0), Vector3D::default(), 0x7f00_0000),
        (
            Vector3D::new(-1, 0, 0),
            Vector3D::new(4096, 0, 0),
            0x7f00_0000,
        ),
    ];
    for (direction, normal, expected) in cases {
        for scale in [1, 4096] {
            let light = LightState {
                direction: Vector3D::new(
                    direction.x * scale,
                    direction.y * scale,
                    direction.z * scale,
                ),
                ambient_intensity: 0,
                ..LightState::default()
            };
            assert_eq!(
                Lighting::new(light, EffectState::default()).shade(0x7fff_ffff, normal),
                expected,
                "direction {direction:?}, scale {scale}"
            );
        }
    }
}

#[test]
fn toon_lighting_preserves_integer_threshold_boundaries() {
    for (threshold, normal_z, expected) in [
        (128, 2056, 0x7f10_1010),
        (128, 2057, 0x7f40_4040),
        (-1, 0, 0x7f40_4040),
        (256, 4095, 0x7f10_1010),
        (256, 4096, 0x7f40_4040),
    ] {
        let effect = EffectState {
            shading: 1,
            toon_threshold: threshold,
            toon_high: 64,
            toon_low: 16,
            ..EffectState::default()
        };
        assert_eq!(
            Lighting::new(
                LightState {
                    ambient_intensity: 0,
                    ..LightState::default()
                },
                effect
            )
            .shade(0x7fff_ffff, Vector3D::new(0, 0, normal_z)),
            expected,
        );
    }
}

#[test]
fn primitive_without_color_data_uses_white_texture_modulation() {
    assert_eq!(primitive_color(0x0300_3010, 0, &[0]), 0xffff_ffff);
    assert_eq!(primitive_color(0x0300_0400, 0, &[0x12_34_56]), 0xff12_3456);
    assert_eq!(
        primitive_color(0x0300_0800, 1, &[0, 0x65_43_21]),
        0xff65_4321
    );
}

#[test]
fn primitive_payload_rejects_missing_selected_color_data() {
    assert_eq!(
        primitive_payload_counts(0x8300_0000_u32.cast_signed(), 1)
            .unwrap_err()
            .code(),
        "primitive-command"
    );
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .load_target(32, 32, &vec![0xff00_0000; 32 * 32])
        .unwrap();
    let error = runtime
        .render_primitives(
            None,
            0,
            0,
            FigureLayoutState::default(),
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(
                0x0300_0800,
                2,
                &[0, 0, 0, 1, 0, 0, 0, 1, 0, 2, 0, 0, 3, 0, 0, 2, 1, 0],
                &[],
                &[],
                &[0x00ff_0000],
            ),
        )
        .unwrap_err();

    assert_eq!(error.code(), "primitive-color");
    assert_eq!(runtime.target_pixels(), vec![0xff00_0000; 32 * 32]);
}

#[test]
fn primitive_per_vertex_normals_drive_lighting() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .create(1, ObjectKind::Light(LightState::default()))
        .unwrap();
    let background = vec![0xff01_0203; 32 * 32];
    runtime.load_target(32, 32, &background).unwrap();
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
            EffectState {
                light: Some(1),
                ..EffectState::default()
            },
            PrimitiveData::new(
                0x0300_0701,
                1,
                &[-10, -10, 0, 10, -10, 0, 0, 10, 0],
                &[0, 0, 4096, 4096, 0, 0, 0, 0, 4096],
                &[],
                &[0x00ff_0000],
            ),
        )
        .unwrap();

    let shaded = runtime
        .target_pixels()
        .iter()
        .copied()
        .filter(|pixel| *pixel != background[0])
        .collect::<BTreeSet<_>>();
    assert!(
        shaded.len() > 8,
        "expected a normal-derived gradient: {shaded:?}"
    );
    assert!(shaded.iter().any(|pixel| (pixel >> 16) & 0xff >= 0xe0));
    assert!(shaded.iter().any(|pixel| (pixel >> 16) & 0xff <= 0x40));
}

#[test]
fn primitive_sphere_map_uses_transformed_normals_on_texture_unit_one() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Texture(TextureData {
                width: 2,
                height: 1,
                pixels: vec![0xffff_0000, 0xff00_ff00].into(),
                for_model: false,
                color_key: 0,
            }),
        )
        .unwrap();
    let background = vec![0xff01_0203; 32 * 32];
    let layout = FigureLayoutState {
        scale: [4096, 4096],
        center: [16, 16],
        projection: Projection::ParallelScale,
        ..FigureLayoutState::default()
    };
    let effect = EffectState {
        sphere_texture: Some(1),
        ..EffectState::default()
    };
    let coordinates = [-10, -10, 0, 10, -10, 0, 0, 10, 0];

    for (normal, expected) in [([-4096, 0, 0], 0xffff_0000), ([4096, 0, 0], 0xff00_ff00)] {
        runtime.load_target(32, 32, &background).unwrap();
        runtime
            .render_primitives(
                None,
                0,
                0,
                layout.clone(),
                AffineTrans::IDENTITY,
                effect,
                PrimitiveData::new(0x0300_0602, 1, &coordinates, &normal, &[], &[0x00ff_ffff]),
            )
            .unwrap();
        assert_eq!(runtime.target_pixels()[16 * 32 + 16], expected);
    }
}

#[test]
fn command_environment_enables_toon_shading_without_mutating_effect_state() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    let background = vec![0xff01_0203; 32 * 32];
    runtime.load_target(32, 32, &background).unwrap();
    let effect = EffectState {
        shading: 0,
        toon_threshold: 128,
        toon_high: 64,
        toon_low: 16,
        transparency: true,
        ..EffectState::default()
    };
    runtime
        .render_primitives_with_environment(
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
            effect,
            PrimitiveData::new(
                0x0300_0600,
                1,
                &[-10, -10, 0, 10, -10, 0, 0, 10, 0],
                &[0, 0, 4096],
                &[],
                &[0x00ff_0000],
            ),
            PrimitiveEnvironment {
                attributes: Some(0x05),
                light_override: Some(LightState::default()),
            },
        )
        .unwrap();

    assert_eq!(runtime.target_pixels()[16 * 32 + 16], 0xff40_0000);
    assert_eq!(effect.shading, 0);
    assert!(effect.transparency);
}

#[test]
fn command_environment_switches_semitransparent_processing() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Texture(TextureData {
                width: 1,
                height: 1,
                pixels: vec![0x80ff_0000].into(),
                for_model: true,
                color_key: 0,
            }),
        )
        .unwrap();
    let background = vec![0xff00_00ff; 32 * 32];
    let layout = FigureLayoutState {
        scale: [4096, 4096],
        center: [16, 16],
        projection: Projection::ParallelScale,
        ..FigureLayoutState::default()
    };
    let data = PrimitiveData::new(
        0x0300_3000,
        1,
        &[-10, -10, 0, 10, -10, 0, 0, 10, 0],
        &[],
        &[0, 0, 0, 0, 0, 0],
        &[],
    );

    for (attributes, expected) in [(0x08, 0xff80_007f), (0, 0x80ff_0000)] {
        runtime.load_target(32, 32, &background).unwrap();
        runtime
            .render_primitives_with_environment(
                Some(1),
                0,
                0,
                layout.clone(),
                AffineTrans::IDENTITY,
                EffectState {
                    transparency: false,
                    ..EffectState::default()
                },
                data,
                PrimitiveEnvironment {
                    attributes: Some(attributes),
                    light_override: None,
                },
            )
            .unwrap();
        assert_eq!(runtime.target_pixels()[16 * 32 + 16], expected);
    }
}

#[test]
fn figure_sphere_map_uses_environment_texture() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Texture(TextureData {
                width: 2,
                height: 1,
                pixels: vec![0xffff_0000, 0xff00_ff00].into(),
                for_model: false,
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
                        attributes: FIGURE_ATTR_DOUBLE_FACE,
                        color: Some(0xffff_ffff),
                        pattern: 0,
                        material: None,
                    }],
                    bones: vec![Bone {
                        vertex_count: 3,
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
        .load_target(32, 32, &vec![0xff01_0203; 32 * 32])
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
            EffectState {
                sphere_texture: Some(1),
                ..EffectState::default()
            },
        )
        .unwrap();

    assert_eq!(runtime.target_pixels()[16 * 32 + 16], 0xff00_ff00);
}
