use super::*;

fn buffer(runtime: &mut m3g::Runtime, state: m3g::VertexBufferState) -> m3g::Handle {
    runtime
        .create(
            None,
            m3g::ObjectKind::VertexBuffer {
                state,
                arrays: [None; 5],
            },
        )
        .unwrap()
}

#[test]
fn morph_picking_uses_updated_texture_arrays_and_defers_invalid_target_shapes() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let runtime = &mut machine.m3g.runtime;
    let group = runtime
        .create(
            None,
            m3g::ObjectKind::Group {
                node: m3g::NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    let mut positions = m3g::VertexArrayState::new(3, 3, m3g::VertexComponent::Short).unwrap();
    positions
        .set_shorts(0, 3, &[-2, -2, 0, 2, -2, 0, 0, 2, 0])
        .unwrap();
    let mut base = m3g::VertexBufferState::default();
    base.set_positions(Some(positions), 1.0, [0.0; 3]).unwrap();
    for unit in 0..2 {
        base.set_texture_coordinates(
            unit,
            Some(m3g::VertexArrayState::new(3, 2, m3g::VertexComponent::Short).unwrap()),
            0.25,
            [0.25, 0.5, 0.0],
        )
        .unwrap();
    }
    let base = buffer(runtime, base);
    let mut target = m3g::VertexBufferState::default();
    let mut texture = m3g::VertexArrayState::new(3, 2, m3g::VertexComponent::Short).unwrap();
    texture.set_shorts(0, 3, &[4; 6]).unwrap();
    let source = runtime
        .create(None, m3g::ObjectKind::VertexArray(texture.clone()))
        .unwrap();
    for unit in 0..2 {
        target
            .set_texture_coordinates(unit, Some(texture.clone()), 100.0, [200.0; 3])
            .unwrap();
    }
    let target = buffer(runtime, target);
    runtime
        .set_vertex_buffer_texture_coordinates(
            target,
            0,
            Some(texture),
            Some(source),
            100.0,
            [200.0; 3],
        )
        .unwrap();
    let indices = runtime
        .create(
            None,
            m3g::ObjectKind::TriangleStripArray(
                m3g::TriangleStripArrayState::implicit(0, vec![3]).unwrap(),
            ),
        )
        .unwrap();
    let image = runtime
        .create(
            None,
            m3g::ObjectKind::Image2D(
                m3g::Image2DState::mutable(m3g::ImageFormat::Rgba, 2, 2).unwrap(),
            ),
        )
        .unwrap();
    let texture = runtime
        .create(
            None,
            m3g::ObjectKind::Texture2D(m3g::TextureObjectState {
                transformable: m3g::TransformableState::default(),
                image,
                level_filter: 208,
                image_filter: 210,
                wrap_s: 240,
                wrap_t: 240,
                blending: 227,
                blend_color: 0,
            }),
        )
        .unwrap();
    let appearance = runtime
        .create(
            None,
            m3g::ObjectKind::Appearance(m3g::AppearanceState {
                textures: [Some(texture); 2],
                ..m3g::AppearanceState::default()
            }),
        )
        .unwrap();
    let mesh = runtime
        .create(
            None,
            m3g::ObjectKind::MorphingMesh {
                mesh: m3g::MeshState {
                    node: m3g::NodeState::default(),
                    vertices: base,
                    submeshes: vec![indices],
                    appearances: vec![Some(appearance)],
                },
                targets: vec![target],
                weights: vec![0.5],
            },
        )
        .unwrap();
    runtime.add_child(group, mesh).unwrap();
    let result = runtime
        .create(
            None,
            m3g::ObjectKind::RayIntersection(m3g::RayIntersectionState::default()),
        )
        .unwrap();
    for value in [4, 8] {
        let m3g::ObjectKind::VertexArray(array) = machine.m3g.runtime.kind_mut(source).unwrap()
        else {
            unreachable!()
        };
        array.set_shorts(0, 3, &[value; 6]).unwrap();
        assert!(
            machine
                .m3g_pick_ray(
                    group,
                    u32::MAX,
                    m3g::Vec3::new(0.0, 0.0, 2.0),
                    m3g::Vec3::new(0.0, 0.0, -1.0),
                    Some(result),
                    None
                )
                .unwrap()
        );
        let m3g::ObjectKind::RayIntersection(result) = machine.m3g.runtime.kind(result).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(result.intersected, Some(mesh));
        assert_eq!(result.distance, 2.0);
        assert_eq!(
            result.texture,
            [
                [
                    0.25 + f32::from(value) * 0.125,
                    0.5 + f32::from(value) * 0.125
                ],
                [0.75, 1.0]
            ]
        );
    }
    // Attaching a valid array can invalidate a referring MorphingMesh. The
    // aggregate shape is checked when used, and reports a Java state error.
    let m3g::ObjectKind::VertexBuffer { state, .. } = machine.m3g.runtime.kind_mut(target).unwrap()
    else {
        unreachable!()
    };
    state
        .set_positions(
            Some(m3g::VertexArrayState::new(3, 3, m3g::VertexComponent::Byte).unwrap()),
            1.0,
            [0.0; 3],
        )
        .unwrap();
    let error = machine
        .m3g_pick_ray(
            group,
            u32::MAX,
            m3g::Vec3::new(0.0, 0.0, 2.0),
            m3g::Vec3::new(0.0, 0.0, -1.0),
            Some(result),
            None,
        )
        .unwrap_err();
    assert_eq!(error.code(), "invalid-object-graph");
    assert_eq!(
        java_error_class(&error),
        Some("java/lang/IllegalStateException")
    );
}
