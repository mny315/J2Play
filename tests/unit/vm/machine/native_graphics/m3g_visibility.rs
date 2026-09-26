use super::m3g_support::{object, setup_renderer};
use super::*;

struct Scene {
    root: m3g::Handle,
    node: m3g::Handle,
    appearance: m3g::Handle,
    vertices: m3g::Handle,
    pick: m3g::Handle,
    projection: m3g::Mat4,
}

fn scene(machine: &mut Machine<'_, '_>, mode: &str) -> Scene {
    let projection = m3g::Mat4::from_row_major([
        0.25, 0.0, 0.0, 0.0, 0.0, 0.25, 0.0, 0.0, 0.0, 0.0, 0.25, 0.0, 0.0, 0.0, 0.0, 1.0,
    ])
    .unwrap();
    setup_renderer(machine, projection, m3g::Mat4::IDENTITY);
    let runtime = &mut machine.m3g.runtime;
    let root = runtime
        .create(
            None,
            m3g::ObjectKind::Group {
                node: m3g::NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    let appearance = runtime
        .create(
            None,
            m3g::ObjectKind::Appearance(m3g::AppearanceState::default()),
        )
        .unwrap();
    let mut array = m3g::VertexArrayState::new(3, 3, m3g::VertexComponent::Short).unwrap();
    array
        .set_shorts(0, 3, &[-2, -2, 0, 2, -2, 0, 0, 2, 0])
        .unwrap();
    let mut buffer = m3g::VertexBufferState::default();
    buffer.set_positions(Some(array), 1.0, [0.0; 3]).unwrap();
    let vertices = runtime
        .create(
            None,
            m3g::ObjectKind::VertexBuffer {
                state: buffer,
                arrays: [None; 5],
            },
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
    let mesh = m3g::MeshState {
        node: m3g::NodeState::default(),
        vertices,
        submeshes: vec![indices, indices],
        appearances: vec![None, Some(appearance)],
    };
    let kind = match mode {
        "morph" => m3g::ObjectKind::MorphingMesh {
            mesh,
            targets: vec![vertices],
            weights: vec![0.5],
        },
        "skin" => {
            let skeleton = runtime
                .create(
                    None,
                    m3g::ObjectKind::Group {
                        node: m3g::NodeState::default(),
                        children: Vec::new(),
                    },
                )
                .unwrap();
            m3g::ObjectKind::SkinnedMesh {
                mesh,
                skeleton,
                bones: Vec::new(),
                bind_transforms: Vec::new(),
                influences: Vec::new(),
            }
        }
        "sprite" => {
            let image = runtime
                .create(
                    None,
                    m3g::ObjectKind::Image2D(
                        m3g::Image2DState::from_argb(m3g::ImageFormat::Rgba, 2, 2, &[u32::MAX; 4])
                            .unwrap(),
                    ),
                )
                .unwrap();
            m3g::ObjectKind::Sprite3D(m3g::SpriteState {
                node: m3g::NodeState::default(),
                scaled: true,
                image,
                appearance: Some(appearance),
                crop: [0, 0, 2, 2],
            })
        }
        _ => m3g::ObjectKind::Mesh(mesh),
    };
    let node = runtime.create(None, kind).unwrap();
    if mode == "skin" {
        runtime.bind_skin_skeleton(node).unwrap();
    }
    runtime.add_child(root, node).unwrap();
    let pick = runtime
        .create(
            None,
            m3g::ObjectKind::RayIntersection(m3g::RayIntersectionState::default()),
        )
        .unwrap();
    Scene {
        root,
        node,
        appearance,
        vertices,
        pick,
        projection,
    }
}

fn render(machine: &mut Machine<'_, '_>, scene: &Scene) -> Result<u32, EmuError> {
    machine.m3g.graphics.renderer.clear(Some(0xff00_0000), true);
    machine.m3g_publish_bound_target()?;
    machine.m3g_render_retained(scene.root, None, false)?;
    Ok(machine.m3g.graphics.renderer.pixels()[16 * 32 + 16])
}

fn pick(machine: &mut Machine<'_, '_>, scene: &Scene) -> Result<bool, EmuError> {
    machine.m3g_pick_ray(
        scene.root,
        u32::MAX,
        m3g::Vec3::new(0.0, 0.0, 2.0),
        m3g::Vec3::new(0.0, 0.0, -1.0),
        Some(scene.pick),
        Some(M3gSpritePickContext {
            projection: scene.projection,
            group_to_camera: m3g::Mat4::IDENTITY,
            viewport_point: [0.5, 0.5],
        }),
    )
}

#[test]
fn null_appearances_disable_rendering_picking_and_deferred_geometry_checks() {
    for mode in ["mesh", "morph", "skin", "sprite"] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let scene = scene(&mut machine, mode);
        assert_eq!(render(&mut machine, &scene).unwrap(), u32::MAX, "{mode}");
        assert!(pick(&mut machine, &scene).unwrap(), "{mode}");
        let m3g::ObjectKind::RayIntersection(result) =
            machine.m3g.runtime.kind(scene.pick).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(result.submesh, i32::from(mode != "sprite"), "{mode}");
        match machine.m3g.runtime.kind_mut(scene.node).unwrap() {
            m3g::ObjectKind::Mesh(mesh)
            | m3g::ObjectKind::MorphingMesh { mesh, .. }
            | m3g::ObjectKind::SkinnedMesh { mesh, .. } => mesh.appearances.fill(None),
            m3g::ObjectKind::Sprite3D(sprite) => sprite.appearance = None,
            _ => unreachable!(),
        }
        assert_eq!(render(&mut machine, &scene).unwrap(), 0xff00_0000, "{mode}");
        assert!(!pick(&mut machine, &scene).unwrap(), "{mode}");
        let m3g::ObjectKind::VertexBuffer { state, .. } =
            machine.m3g.runtime.kind_mut(scene.vertices).unwrap()
        else {
            unreachable!()
        };
        state.set_positions(None, 1.0, [0.0; 3]).unwrap();
        assert_eq!(render(&mut machine, &scene).unwrap(), 0xff00_0000, "{mode}");
        assert!(!pick(&mut machine, &scene).unwrap(), "{mode}");
    }
}

#[test]
fn sprite_appearance_does_not_activate_its_mesh_textures() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine, "sprite");
    let runtime = &mut machine.m3g.runtime;
    let image = runtime
        .create(
            None,
            m3g::ObjectKind::Image2D(
                m3g::Image2DState::from_argb(m3g::ImageFormat::Rgba, 1, 1, &[0xffff_0000]).unwrap(),
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
                wrap_s: 241,
                wrap_t: 241,
                blending: 228,
                blend_color: 0,
            }),
        )
        .unwrap();
    let m3g::ObjectKind::Appearance(appearance) = runtime.kind_mut(scene.appearance).unwrap()
    else {
        unreachable!()
    };
    appearance.textures[1] = Some(texture);
    // The geometry components do not affect a Sprite, even when its Appearance
    // was prepared for a mesh requiring a larger texturing capacity.
    machine.limits.m3g_num_texture_units = 1;
    assert_eq!(render(&mut machine, &scene).unwrap(), u32::MAX);
    assert!(pick(&mut machine, &scene).unwrap());
}

#[test]
fn immediate_render_respects_camera_scope_before_validating_unused_geometry() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let target = setup_renderer(&mut machine, m3g::Mat4::IDENTITY, m3g::Mat4::IDENTITY);
    let camera = machine
        .m3g_handle(machine.m3g.graphics.camera.unwrap().0)
        .unwrap();
    // The Camera's rendering flag is irrelevant.
    machine
        .m3g
        .runtime
        .set_node_state(camera, false, true, 1, 1.0)
        .unwrap();
    let (vertices, _) = object(
        &mut machine,
        "javax/microedition/m3g/VertexBuffer",
        m3g::ObjectKind::VertexBuffer {
            state: m3g::VertexBufferState::default(),
            arrays: [None; 5],
        },
    );
    let (indices, _) = object(
        &mut machine,
        "javax/microedition/m3g/TriangleStripArray",
        m3g::ObjectKind::TriangleStripArray(
            m3g::TriangleStripArrayState::implicit(0, vec![3]).unwrap(),
        ),
    );
    let (appearance, _) = object(
        &mut machine,
        "javax/microedition/m3g/Appearance",
        m3g::ObjectKind::Appearance(m3g::AppearanceState::default()),
    );
    for scope in [2, 1] {
        let result = machine.invoke_m3g_graphics3d_native("javax/microedition/m3g/Graphics3D", "render", "(Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;Ljavax/microedition/m3g/Transform;I)V", &[
            Value::Reference(Some(target)), Value::Reference(Some(vertices)), Value::Reference(Some(indices)), Value::Reference(Some(appearance)), Value::Reference(None), Value::Int(scope),
        ]);
        if scope == 2 {
            assert!(matches!(result, Ok(CallOutcome::Return(None))));
        } else {
            assert_eq!(result.err().unwrap().code(), "missing-positions");
        }
    }
}

#[test]
fn disabled_immediate_lights_do_not_illuminate_meshes() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (guest, light) = object(
        &mut machine,
        "javax/microedition/m3g/Light",
        m3g::ObjectKind::Light {
            node: m3g::NodeState::default(),
            light: m3g::LightState::default(),
        },
    );
    machine
        .m3g
        .graphics
        .lights
        .push((guest, m3g::Mat4::IDENTITY));
    for rendering in [true, false, true] {
        machine
            .m3g
            .runtime
            .set_node_state(light, rendering, true, 1, 1.0)
            .unwrap();
        assert_eq!(
            machine.m3g_light_sources(1).unwrap().len(),
            usize::from(rendering)
        );
        assert!(machine.m3g_light_sources(2).unwrap().is_empty());
    }
}

#[test]
fn zero_node_alpha_still_writes_color_and_depth_with_replace_compositing() {
    for mode in ["mesh", "morph", "skin", "sprite"] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let scene = scene(&mut machine, mode);
        for (parent_alpha, local_alpha) in [(1.0, 0.0), (0.0, 1.0), (0.0, 0.0)] {
            for (node, alpha) in [(scene.root, parent_alpha), (scene.node, local_alpha)] {
                machine
                    .m3g
                    .runtime
                    .set_node_state(node, true, true, u32::MAX, alpha)
                    .unwrap();
            }
            assert_eq!(render(&mut machine, &scene).unwrap(), 0x00ff_ffff, "{mode}");
            assert!(
                machine.m3g.graphics.renderer.depth()[16 * 32 + 16] < 0x00ff_ffff,
                "{mode}"
            );
            assert!(pick(&mut machine, &scene).unwrap(), "{mode}");
        }
        machine
            .m3g
            .runtime
            .set_node_state(scene.root, false, true, u32::MAX, 0.0)
            .unwrap();
        assert_eq!(render(&mut machine, &scene).unwrap(), 0xff00_0000, "{mode}");
    }
}

#[test]
fn world_lights_ignore_their_own_and_ancestor_alpha_but_inherit_rendering_enable() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine, "mesh");
    let camera = machine
        .m3g_handle(machine.m3g.graphics.camera.unwrap().0)
        .unwrap();
    let light_group = machine
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
    let (_, light) = object(
        &mut machine,
        "javax/microedition/m3g/Light",
        m3g::ObjectKind::Light {
            node: m3g::NodeState::default(),
            light: m3g::LightState {
                mode: 128,
                ..Default::default()
            },
        },
    );
    let runtime = &mut machine.m3g.runtime;
    let root = runtime.kind_mut(scene.root).unwrap();
    let m3g::ObjectKind::Group { node, children } = root else {
        unreachable!()
    };
    *root = m3g::ObjectKind::World {
        node: node.clone(),
        children: std::mem::take(children),
        camera: None,
        background: None,
    };
    runtime.add_child(scene.root, camera).unwrap();
    runtime.set_world_camera(scene.root, camera).unwrap();
    runtime
        .set_node_state(camera, false, true, u32::MAX, 0.0)
        .unwrap();
    runtime.add_child(scene.root, light_group).unwrap();
    runtime.add_child(light_group, light).unwrap();
    let mut normals = m3g::VertexArrayState::new(3, 3, m3g::VertexComponent::Byte).unwrap();
    normals
        .set_bytes(0, 3, &[0, 0, 127, 0, 0, 127, 0, 0, 127])
        .unwrap();
    let m3g::ObjectKind::VertexBuffer { state, .. } = runtime.kind_mut(scene.vertices).unwrap()
    else {
        unreachable!()
    };
    state.set_normals(Some(normals)).unwrap();
    let material = runtime
        .create(
            None,
            m3g::ObjectKind::Material(m3g::MaterialObjectState {
                material: m3g::MaterialState {
                    ambient: 0x00ff_ffff,
                    diffuse: 0xff00_0000,
                    ..Default::default()
                },
                ..Default::default()
            }),
        )
        .unwrap();
    let m3g::ObjectKind::Appearance(appearance) = runtime.kind_mut(scene.appearance).unwrap()
    else {
        unreachable!()
    };
    appearance.material = Some(material);
    for (parent_alpha, local_alpha, enabled, expected) in [
        (1.0, 1.0, true, u32::MAX),
        (0.0, 1.0, true, u32::MAX),
        (1.0, 0.0, true, u32::MAX),
        (0.0, 0.0, true, u32::MAX),
        (1.0, 1.0, false, 0xff00_0000),
    ] {
        machine
            .m3g
            .runtime
            .set_node_state(light_group, enabled, true, u32::MAX, parent_alpha)
            .unwrap();
        machine
            .m3g
            .runtime
            .set_node_state(light, true, true, u32::MAX, local_alpha)
            .unwrap();
        machine.m3g_render_retained(scene.root, None, true).unwrap();
        assert_eq!(
            machine.m3g.graphics.renderer.pixels()[16 * 32 + 16],
            expected,
            "group alpha={parent_alpha}, light alpha={local_alpha}, enabled={enabled}"
        );
    }
}
