use super::{DefaultNativeContext, Limits, Program, add_track};

fn constant_sequence(values: &[f32]) -> m3g::KeyframeSequenceState {
    let mut sequence =
        m3g::KeyframeSequenceState::new(1, values.len(), m3g::Interpolation::Step).unwrap();
    sequence.set_duration(100).unwrap();
    sequence.set_keyframe(0, 0, values).unwrap();
    sequence
}

#[test]
fn camera_field_of_view_animation_clamps_by_projection_and_ignores_generic_matrices() {
    for mode in 0..3 {
        for value in [-512.0, -0.0, 0.0, f32::from_bits(1), 45.0, 180.0, 512.0] {
            let program = Program::new();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let initial = match mode {
                0 => m3g::CameraProjection::Generic(m3g::Mat4::IDENTITY),
                1 => m3g::CameraProjection::Parallel {
                    height: 2.0,
                    aspect_ratio: 1.0,
                    near: 1.0,
                    far: 10.0,
                },
                _ => m3g::CameraProjection::Perspective {
                    field_of_view: 60.0,
                    aspect_ratio: 1.0,
                    near: 1.0,
                    far: 10.0,
                },
            };
            let camera = machine
                .m3g
                .runtime
                .create(
                    None,
                    m3g::ObjectKind::Camera {
                        node: m3g::NodeState::default(),
                        projection: initial,
                    },
                )
                .unwrap();
            add_track(
                &mut machine.m3g.runtime,
                camera,
                constant_sequence(&[value]),
                264,
            );
            machine.m3g_animate(camera, 0).unwrap();
            let m3g::ObjectKind::Camera { projection, .. } =
                machine.m3g.runtime.kind(camera).unwrap()
            else {
                panic!("camera changed type");
            };
            projection.validate().unwrap();
            if let Some(actual) = projection.parameters() {
                let expected = if mode == 1 {
                    value.max(f32::from_bits(1))
                } else {
                    value.clamp(f32::from_bits(1), 180.0_f32.next_down())
                };
                assert_eq!(actual, [expected, 1.0, 1.0, 10.0]);
            } else {
                assert_eq!(*projection, initial);
            }
        }
    }
}

#[test]
fn scale_animation_accepts_uniform_and_per_axis_values() {
    for values in [&[-2.0][..], &[-2.0, 0.0, 3.0]] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let node = machine
            .m3g
            .runtime
            .create(None, m3g::ObjectKind::Node(m3g::NodeState::default()))
            .unwrap();
        add_track(
            &mut machine.m3g.runtime,
            node,
            constant_sequence(values),
            270,
        );
        machine.m3g_animate(node, 0).unwrap();
        let (_, scale, _) = machine.m3g.runtime.transform_components(node).unwrap();
        assert_eq!(
            scale,
            if values.len() == 1 {
                m3g::Vec3::new(-2.0, -2.0, -2.0)
            } else {
                m3g::Vec3::new(-2.0, 0.0, 3.0)
            }
        );
    }
}

#[test]
fn morph_animation_zero_fills_missing_weights_and_ignores_excess_components() {
    for values in [&[-2.0][..], &[-2.0, 0.0, 3.0], &[-2.0, 0.0, 3.0, 8.0]] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let vertices = machine
            .m3g
            .runtime
            .create(
                None,
                m3g::ObjectKind::VertexBuffer {
                    state: m3g::VertexBufferState::default(),
                    arrays: [None; 5],
                },
            )
            .unwrap();
        let mesh = machine
            .m3g
            .runtime
            .create(
                None,
                m3g::ObjectKind::MorphingMesh {
                    mesh: m3g::MeshState {
                        node: m3g::NodeState::default(),
                        vertices,
                        submeshes: Vec::new(),
                        appearances: Vec::new(),
                    },
                    targets: vec![vertices; 3],
                    weights: vec![9.0; 3],
                },
            )
            .unwrap();
        add_track(
            &mut machine.m3g.runtime,
            mesh,
            constant_sequence(values),
            266,
        );
        machine.m3g_animate(mesh, 0).unwrap();
        let m3g::ObjectKind::MorphingMesh { weights, .. } = machine.m3g.runtime.kind(mesh).unwrap()
        else {
            panic!("mesh changed type");
        };
        assert_eq!(
            weights,
            &[-2.0, 0.0, if values.len() > 2 { 3.0 } else { 0.0 }]
        );
    }
}

#[test]
fn crop_animation_rounds_coordinates_and_limits_sizes_by_target() {
    for sprite in [false, true] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                m3g_max_sprite_crop_dimension: 7,
                ..Limits::default()
            },
            false,
            &mut context,
        );
        let initial_crop = [0, 0, 5, 6];
        let kind = if sprite {
            let image = machine
                .m3g
                .runtime
                .create(
                    None,
                    m3g::ObjectKind::Image2D(
                        m3g::Image2DState::from_argb(m3g::ImageFormat::Rgba, 1, 1, &[u32::MAX])
                            .unwrap(),
                    ),
                )
                .unwrap();
            m3g::ObjectKind::Sprite3D(m3g::SpriteState {
                node: m3g::NodeState::default(),
                scaled: false,
                image,
                appearance: None,
                crop: initial_crop,
            })
        } else {
            m3g::ObjectKind::Background(m3g::BackgroundState {
                crop: initial_crop,
                ..m3g::BackgroundState::default()
            })
        };
        let target = machine.m3g.runtime.create(None, kind).unwrap();
        for (values, expected) in [
            (&[-1.6, 2.6][..], [-2, 3, 5, 6]),
            (
                &[-1.6, 2.6, -20.0, 20.0],
                if sprite {
                    [-2, 3, -7, 7]
                } else {
                    [-2, 3, 0, 20]
                },
            ),
            (
                &[4.6, -5.6],
                if sprite {
                    [5, -6, -7, 7]
                } else {
                    [5, -6, 0, 20]
                },
            ),
        ] {
            for track in machine
                .m3g
                .runtime
                .animation_tracks(target)
                .unwrap()
                .to_vec()
            {
                machine
                    .m3g
                    .runtime
                    .remove_animation_track(target, track)
                    .unwrap();
            }
            add_track(
                &mut machine.m3g.runtime,
                target,
                constant_sequence(values),
                259,
            );
            machine.m3g_animate(target, 0).unwrap();
            let actual = match machine.m3g.runtime.kind(target).unwrap() {
                m3g::ObjectKind::Sprite3D(state) => state.crop,
                m3g::ObjectKind::Background(state) => state.crop,
                _ => panic!("crop target changed type"),
            };
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn rgb_animation_preserves_separate_alpha_and_combines_with_alpha_tracks() {
    for target_kind in 0..3 {
        for animate_alpha in [false, true] {
            let program = Program::new();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let color = 0x6b12_3456;
            let kind = match target_kind {
                0 => m3g::ObjectKind::Background(m3g::BackgroundState {
                    color,
                    ..m3g::BackgroundState::default()
                }),
                1 => {
                    let mut state = m3g::MaterialObjectState::default();
                    state.material.diffuse = color;
                    m3g::ObjectKind::Material(state)
                }
                _ => {
                    let mut state = m3g::VertexBufferState::default();
                    state.set_default_color(color);
                    m3g::ObjectKind::VertexBuffer {
                        state,
                        arrays: [None; 5],
                    }
                }
            };
            let target = machine.m3g.runtime.create(None, kind).unwrap();
            add_track(
                &mut machine.m3g.runtime,
                target,
                constant_sequence(&[1.0, 0.5, -1.0]),
                if target_kind == 1 { 261 } else { 258 },
            );
            if animate_alpha {
                add_track(
                    &mut machine.m3g.runtime,
                    target,
                    constant_sequence(&[0.25]),
                    256,
                );
            }
            machine.m3g_animate(target, 0).unwrap();
            let actual = match machine.m3g.runtime.kind(target).unwrap() {
                m3g::ObjectKind::Background(state) => state.color,
                m3g::ObjectKind::Material(state) => state.material.diffuse,
                m3g::ObjectKind::VertexBuffer { state, .. } => state.default_color(),
                _ => panic!("color target changed type"),
            };
            assert_eq!(
                actual,
                if animate_alpha {
                    0x40ff_8000
                } else {
                    0x6bff_8000
                }
            );
        }
    }
}

#[test]
fn rgb_animation_updates_texture_blend_color() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let image = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::Image2D(
                m3g::Image2DState::from_argb(m3g::ImageFormat::Rgba, 1, 1, &[u32::MAX]).unwrap(),
            ),
        )
        .unwrap();
    let texture = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::Texture2D(m3g::TextureObjectState {
                transformable: m3g::TransformableState::default(),
                image,
                level_filter: 208,
                image_filter: 209,
                wrap_s: 240,
                wrap_t: 240,
                blending: 225,
                blend_color: 0x0012_3456,
            }),
        )
        .unwrap();
    add_track(
        &mut machine.m3g.runtime,
        texture,
        constant_sequence(&[1.0, 0.5, -1.0]),
        258,
    );
    machine.m3g_animate(texture, 0).unwrap();
    let m3g::ObjectKind::Texture2D(state) = machine.m3g.runtime.kind(texture).unwrap() else {
        panic!("texture changed type");
    };
    assert_eq!(state.blend_color, 0x00ff_8000);
}
