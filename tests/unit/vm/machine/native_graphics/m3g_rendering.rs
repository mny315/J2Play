use super::*;

#[test]
pub(crate) fn m3g_immediate_geometry_reaches_the_midp_target_before_release() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let object = |machine: &mut Machine<'_, '_>, class: &str| {
        machine
            .heap
            .managed
            .allocate_object(class, HashMap::new())
            .unwrap()
    };
    let graphics3d = object(&mut machine, "javax/microedition/m3g/Graphics3D");
    let camera = object(&mut machine, "javax/microedition/m3g/Camera");
    let vertex_array = object(&mut machine, "javax/microedition/m3g/VertexArray");
    let vertex_buffer = object(&mut machine, "javax/microedition/m3g/VertexBuffer");
    let strips = object(&mut machine, "javax/microedition/m3g/TriangleStripArray");
    let appearance = object(&mut machine, "javax/microedition/m3g/Appearance");

    let construct = |machine: &mut Machine<'_, '_>, class, descriptor, receiver, tail| {
        let method = runtime_method(
            class,
            "<init>",
            descriptor,
            &[0xb1],
            0,
            8,
            Vec::new(),
            false,
        );
        let mut arguments = vec![Value::Reference(Some(receiver))];
        arguments.extend(tail);
        machine.invoke_m3g_native(&method, &arguments, 1).unwrap();
    };
    construct(
        &mut machine,
        "javax/microedition/m3g/Camera",
        "()V",
        camera,
        vec![],
    );
    construct(
        &mut machine,
        "javax/microedition/m3g/VertexArray",
        "(III)V",
        vertex_array,
        vec![Value::Int(3), Value::Int(3), Value::Int(2)],
    );
    construct(
        &mut machine,
        "javax/microedition/m3g/VertexBuffer",
        "()V",
        vertex_buffer,
        vec![],
    );
    construct(
        &mut machine,
        "javax/microedition/m3g/Appearance",
        "()V",
        appearance,
        vec![],
    );
    let lengths = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(lengths, 0, HeapValue::Int(3))
        .unwrap();
    construct(
        &mut machine,
        "javax/microedition/m3g/TriangleStripArray",
        "(I[I)V",
        strips,
        vec![Value::Int(0), Value::Reference(Some(lengths))],
    );
    let coordinates = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Short, 9)
        .unwrap();
    for (index, value) in [-1, -1, 0, 1, -1, 0, 0, 1, 0].into_iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(coordinates, index as i32, HeapValue::Int(value))
            .unwrap();
    }
    let set_vertices = runtime_method(
        "javax/microedition/m3g/VertexArray",
        "set",
        "(II[S)V",
        &[0xb1],
        0,
        4,
        Vec::new(),
        false,
    );
    let set_positions = runtime_method(
        "javax/microedition/m3g/VertexBuffer",
        "setPositions",
        "(Ljavax/microedition/m3g/VertexArray;F[F)V",
        &[0xb1],
        0,
        4,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &set_positions,
            &[
                Value::Reference(Some(vertex_buffer)),
                Value::Reference(Some(vertex_array)),
                Value::Float(0.75),
                Value::Reference(None),
            ],
            1,
        )
        .unwrap();
    // VertexBuffer retains VertexArray by reference. Mutating the array
    // after attachment must change the geometry rendered below.
    machine
        .invoke_m3g_native(
            &set_vertices,
            &[
                Value::Reference(Some(vertex_array)),
                Value::Int(0),
                Value::Int(3),
                Value::Reference(Some(coordinates)),
            ],
            1,
        )
        .unwrap();

    let (graphics, target_pixels) = graphics_support::direct_graphics_target(&mut machine, 32, 32);
    machine
        .graphics_int_array_mut(target_pixels)
        .unwrap()
        .fill(HeapValue::Int(0xff00_0000_u32.cast_signed()));
    for (name, descriptor, values) in [
        (
            "bindTarget",
            "(Ljava/lang/Object;)V",
            vec![Value::Reference(Some(graphics))],
        ),
        (
            "setCamera",
            "(Ljavax/microedition/m3g/Camera;Ljavax/microedition/m3g/Transform;)V",
            vec![Value::Reference(Some(camera)), Value::Reference(None)],
        ),
        (
            "render",
            "(Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;Ljavax/microedition/m3g/Transform;)V",
            vec![
                Value::Reference(Some(vertex_buffer)),
                Value::Reference(Some(strips)),
                Value::Reference(Some(appearance)),
                Value::Reference(None),
            ],
        ),
    ] {
        let method = runtime_method(
            "javax/microedition/m3g/Graphics3D",
            name,
            descriptor,
            &[0xb1],
            0,
            8,
            Vec::new(),
            false,
        );
        let mut arguments = vec![Value::Reference(Some(graphics3d))];
        arguments.extend(values);
        machine.invoke_m3g_native(&method, &arguments, 1).unwrap();
    }
    let pixels = machine.graphics_int_array_snapshot(target_pixels).unwrap();
    assert!(
        pixels
            .iter()
            .all(|pixel| pixel.cast_unsigned() >> 24 == 0xff)
    );
    assert!(
        pixels
            .iter()
            .any(|pixel| *pixel != 0xff00_0000_u32.cast_signed())
    );
    assert!(machine.m3g.graphics.target.is_some());

    let release = runtime_method(
        "javax/microedition/m3g/Graphics3D",
        "releaseTarget",
        "()V",
        &[0xb1],
        0,
        1,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(&release, &[Value::Reference(Some(graphics3d))], 1)
        .unwrap();
    assert!(machine.m3g.graphics.target.is_none());
}

#[test]
pub(crate) fn m3g_retained_morph_skin_camera_scope_and_picking_reach_the_framebuffer() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let camera_guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Camera", HashMap::new())
        .unwrap();
    let (target, _) = m3g_support::object(
        &mut machine,
        "javax/microedition/m3g/Image2D",
        m3g::ObjectKind::Image2D(
            m3g::Image2DState::mutable(m3g::ImageFormat::Rgba, 32, 32).unwrap(),
        ),
    );

    let world = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::World {
                node: m3g::NodeState::default(),
                children: Vec::new(),
                camera: None,
                background: None,
            },
        )
        .unwrap();
    let camera = machine
        .m3g
        .runtime
        .create(
            Some(camera_guest.to_raw()),
            m3g::ObjectKind::Camera {
                node: m3g::NodeState::default(),
                projection: m3g::CameraProjection::Perspective {
                    field_of_view: 90.0,
                    aspect_ratio: 1.0,
                    near: 1.0,
                    far: 10.0,
                },
            },
        )
        .unwrap();
    machine.m3g.runtime.add_child(world, camera).unwrap();
    machine.m3g.runtime.set_world_camera(world, camera).unwrap();

    let positions = |values: &[i16]| {
        let mut array = m3g::VertexArrayState::new(3, 3, m3g::VertexComponent::Short).unwrap();
        array.set_shorts(0, 3, values).unwrap();
        let mut buffer = m3g::VertexBufferState::default();
        buffer.set_positions(Some(array), 0.75, [0.0; 3]).unwrap();
        buffer
    };
    let base = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::VertexBuffer {
                state: positions(&[-1, -1, 0, 1, -1, 0, 0, 1, 0]),
                arrays: [None; 5],
            },
        )
        .unwrap();
    let morph_target = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::VertexBuffer {
                state: positions(&[-1, -1, 0, 1, -1, 0, 1, 1, 0]),
                arrays: [None; 5],
            },
        )
        .unwrap();
    let strips = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::TriangleStripArray(
                m3g::TriangleStripArrayState::implicit(0, vec![3]).unwrap(),
            ),
        )
        .unwrap();
    let appearance = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::Appearance(m3g::AppearanceState::default()),
        )
        .unwrap();
    let mesh_state = || m3g::MeshState {
        node: m3g::NodeState::default(),
        vertices: base,
        submeshes: vec![strips],
        appearances: vec![Some(appearance)],
    };
    let morph = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::MorphingMesh {
                mesh: mesh_state(),
                targets: vec![morph_target],
                weights: vec![0.0],
            },
        )
        .unwrap();
    machine
        .m3g
        .runtime
        .set_translation(morph, m3g::Vec3::new(-0.5, 0.0, -3.0))
        .unwrap();
    machine.m3g.runtime.add_child(world, morph).unwrap();

    let skeleton = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::Group {
                node: m3g::NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    let bone = machine
        .m3g
        .runtime
        .create(None, m3g::ObjectKind::Node(m3g::NodeState::default()))
        .unwrap();
    machine.m3g.runtime.add_child(skeleton, bone).unwrap();
    machine
        .m3g
        .runtime
        .set_translation(bone, m3g::Vec3::new(0.25, 0.0, 0.0))
        .unwrap();
    let skinned = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::SkinnedMesh {
                mesh: mesh_state(),
                skeleton,
                bones: vec![bone],
                bind_transforms: Vec::new(),
                influences: vec![m3g::SkinInfluence {
                    bone: 0,
                    first_vertex: 0,
                    vertex_count: 3,
                    weight: 1,
                }],
            },
        )
        .unwrap();
    machine.m3g.runtime.bind_skin_skeleton(skinned).unwrap();
    machine
        .m3g
        .runtime
        .set_translation(skinned, m3g::Vec3::new(0.5, 0.0, -3.0))
        .unwrap();
    machine.m3g.runtime.add_child(world, skinned).unwrap();

    let sprite_image = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::Image2D(
                m3g::Image2DState::from_bytes(
                    m3g::ImageFormat::Rgba,
                    2,
                    2,
                    &[
                        255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
                    ],
                )
                .unwrap(),
            ),
        )
        .unwrap();
    let sprite = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::Sprite3D(m3g::SpriteState {
                node: m3g::NodeState::default(),
                scaled: true,
                image: sprite_image,
                appearance: Some(appearance),
                crop: [0, 0, 2, 2],
            }),
        )
        .unwrap();
    machine
        .m3g
        .runtime
        .set_translation(sprite, m3g::Vec3::new(0.0, 0.0, -2.0))
        .unwrap();
    machine.m3g.runtime.add_child(world, sprite).unwrap();

    let mut sequence_state =
        m3g::KeyframeSequenceState::new(2, 1, m3g::Interpolation::Linear).unwrap();
    sequence_state.set_keyframe(0, 0, &[0.0]).unwrap();
    sequence_state.set_keyframe(1, 100, &[1.0]).unwrap();
    sequence_state.set_duration(100).unwrap();
    let sequence = machine
        .m3g
        .runtime
        .create(None, m3g::ObjectKind::KeyframeSequence(sequence_state))
        .unwrap();
    let inactive_track = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::AnimationTrack {
                sequence,
                controller: None,
                property: 256,
            },
        )
        .unwrap();
    machine
        .m3g
        .runtime
        .add_animation_track(sprite, inactive_track)
        .unwrap();
    let controller = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::AnimationController(m3g::AnimationControllerState::default()),
        )
        .unwrap();
    let track = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::AnimationTrack {
                sequence,
                controller: Some(controller),
                property: 266,
            },
        )
        .unwrap();
    machine
        .m3g
        .runtime
        .add_animation_track(morph, track)
        .unwrap();

    machine.m3g.graphics.target = Some(target);
    machine.m3g.graphics.viewport = [0, 0, 32, 32];
    machine.m3g.graphics.renderer =
        m3g::SoftwareRenderer::new(32, 32, m3g::RenderLimits::default()).unwrap();
    assert_eq!(machine.m3g_animate(world, 0).unwrap(), 1);
    assert_eq!(machine.m3g.runtime.node_state(sprite).unwrap().3, 1.0);
    machine.m3g_render_retained(world, None, true).unwrap();
    let first_frame = machine.m3g.graphics.renderer.pixels().to_vec();
    assert_eq!(machine.m3g_animate(world, 100).unwrap(), 1);
    assert_eq!(machine.m3g.runtime.node_state(sprite).unwrap().3, 1.0);
    machine.m3g_render_retained(world, None, true).unwrap();
    assert_ne!(first_frame, machine.m3g.graphics.renderer.pixels());
    assert_eq!(machine.m3g.metrics.animation_samples, 2);
    assert_eq!(machine.m3g.metrics.render_calls, 2);
    assert!(machine.m3g.metrics.render_time_nanos > 0);
    assert!(machine.m3g.metrics.max_render_time_nanos <= machine.m3g.metrics.render_time_nanos);
    let frame_hash = machine
        .m3g
        .graphics
        .renderer
        .pixels()
        .iter()
        .flat_map(|pixel| pixel.to_le_bytes())
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    assert_eq!(frame_hash, 16_765_427_475_008_229_793);

    let stats = machine.m3g.graphics.renderer.stats();
    assert_eq!(stats.submitted_triangles, 4);
    assert_eq!(stats.rasterized_triangles, 4);
    assert!(stats.shaded_fragments > 0);
    assert!(
        machine
            .m3g
            .graphics
            .renderer
            .pixels()
            .contains(&0xffff_0000)
    );

    // Degenerate triangles are legal no-hit geometry for picking. They
    // must not abort a search that can still hit another mesh.
    let degenerate_vertices = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::VertexBuffer {
                state: positions(&[0, 0, 0, 0, 0, 0, 0, 0, 0]),
                arrays: [None; 5],
            },
        )
        .unwrap();
    let degenerate_mesh = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::Mesh(m3g::MeshState {
                node: m3g::NodeState::default(),
                vertices: degenerate_vertices,
                submeshes: vec![strips],
                appearances: vec![Some(appearance)],
            }),
        )
        .unwrap();
    machine
        .m3g
        .runtime
        .set_node_state(degenerate_mesh, true, true, 0x02, 1.0)
        .unwrap();
    machine
        .m3g
        .runtime
        .add_child(world, degenerate_mesh)
        .unwrap();
    assert!(
        machine
            .m3g_pick_ray(
                world,
                u32::MAX,
                m3g::Vec3::new(-0.5, 0.0, 0.0),
                m3g::Vec3::new(0.0, 0.0, -1.0),
                None,
                None,
            )
            .unwrap()
    );
    let ray_result = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::RayIntersection(m3g::RayIntersectionState::default()),
        )
        .unwrap();
    let projection = m3g::CameraProjection::Perspective {
        field_of_view: 90.0,
        aspect_ratio: 1.0,
        near: 1.0,
        far: 10.0,
    }
    .render_matrix()
    .unwrap();
    assert!(
        machine
            .m3g_pick_ray(
                world,
                u32::MAX,
                m3g::Vec3::new(0.0, 0.0, -1.0),
                m3g::Vec3::new(0.0, 0.0, -9.0),
                Some(ray_result),
                Some(M3gSpritePickContext {
                    viewport_point: [0.5, 0.5],
                    projection,
                    group_to_camera: m3g::Mat4::IDENTITY,
                }),
            )
            .unwrap()
    );
    let m3g::ObjectKind::RayIntersection(ray_result) =
        machine.m3g.runtime.kind(ray_result).unwrap()
    else {
        panic!("RayIntersection state changed type");
    };
    assert_eq!(ray_result.intersected, Some(sprite));
    assert_eq!(ray_result.submesh, 0);
    assert_eq!(ray_result.normal, m3g::Vec3::new(0.0, 0.0, 1.0));
    assert!((ray_result.distance - 1.0 / 9.0).abs() < 1.0e-6);

    // Scope is not inherited from Group nodes: a zero-scope World must
    // still expose its matching child, while the SkinnedMesh and Sprite3D
    // in a disjoint scope are culled by the active Camera.
    machine
        .m3g
        .runtime
        .set_node_state(world, true, true, 0, 1.0)
        .unwrap();
    machine
        .m3g
        .runtime
        .set_node_state(camera, true, true, 0x01, 1.0)
        .unwrap();
    machine
        .m3g
        .runtime
        .set_node_state(morph, true, true, 0x01, 1.0)
        .unwrap();
    machine
        .m3g
        .runtime
        .set_node_state(skinned, true, true, 0x02, 1.0)
        .unwrap();
    machine
        .m3g
        .runtime
        .set_node_state(sprite, true, true, 0x02, 1.0)
        .unwrap();
    machine.m3g_render_retained(world, None, true).unwrap();
    let scope_stats = machine.m3g.graphics.renderer.stats();
    assert_eq!(scope_stats.submitted_triangles, 1);
    assert_eq!(scope_stats.rasterized_triangles, 1);
    assert!(
        !machine
            .m3g
            .graphics
            .renderer
            .pixels()
            .contains(&0xffff_0000)
    );
}
