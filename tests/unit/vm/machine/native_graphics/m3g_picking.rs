use super::*;

struct Scene {
    group: m3g::Handle,
    mesh: m3g::Handle,
    vertices: m3g::Handle,
    appearance: m3g::Handle,
    result: m3g::Handle,
}

fn group(runtime: &mut m3g::Runtime) -> m3g::Handle {
    runtime
        .create(
            None,
            m3g::ObjectKind::Group {
                node: m3g::NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap()
}

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

fn normals(values: &[i16; 9]) -> m3g::VertexArrayState {
    let mut array = m3g::VertexArrayState::new(3, 3, m3g::VertexComponent::Short).unwrap();
    array.set_shorts(0, 3, values).unwrap();
    array
}

fn scene(runtime: &mut m3g::Runtime) -> Scene {
    let group = group(runtime);
    let mut state = m3g::VertexBufferState::default();
    state
        .set_positions(
            Some(normals(&[-1, -1, 0, 1, -1, 0, 0, 1, 0])),
            1.0,
            [0.0; 3],
        )
        .unwrap();
    let vertices = buffer(runtime, state);
    let indices = runtime
        .create(
            None,
            m3g::ObjectKind::TriangleStripArray(
                m3g::TriangleStripArrayState::new(vec![2, 0, 1], vec![3]).unwrap(),
            ),
        )
        .unwrap();
    let appearance = runtime
        .create(
            None,
            m3g::ObjectKind::Appearance(m3g::AppearanceState::default()),
        )
        .unwrap();
    let mesh = runtime
        .create(
            None,
            m3g::ObjectKind::Mesh(m3g::MeshState {
                node: m3g::NodeState::default(),
                vertices,
                submeshes: vec![indices],
                appearances: vec![Some(appearance)],
            }),
        )
        .unwrap();
    runtime.add_child(group, mesh).unwrap();
    let result = runtime
        .create(
            None,
            m3g::ObjectKind::RayIntersection(m3g::RayIntersectionState::default()),
        )
        .unwrap();
    Scene {
        group,
        mesh,
        vertices,
        appearance,
        result,
    }
}

fn pick(machine: &mut Machine<'_, '_>, scene: &Scene) -> bool {
    machine
        .m3g_pick_ray(
            scene.group,
            u32::MAX,
            m3g::Vec3::new(0.0, 0.0, 2.0),
            m3g::Vec3::new(0.0, 0.0, -2.0),
            Some(scene.result),
            None,
        )
        .unwrap()
}

fn result(runtime: &m3g::Runtime, handle: m3g::Handle) -> &m3g::RayIntersectionState {
    let m3g::ObjectKind::RayIntersection(state) = runtime.kind(handle).unwrap() else {
        unreachable!()
    };
    state
}

#[test]
fn intersection_defaults_and_misses_preserve_the_documented_ray_and_normal() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine.m3g.runtime);
    let state = result(&machine.m3g.runtime, scene.result);
    assert_eq!(state.ray, [0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
    assert_eq!(state.normal, m3g::Vec3::new(0.0, 0.0, 1.0));
    assert!(pick(&mut machine, &scene));
    let before = result(&machine.m3g.runtime, scene.result).clone();
    machine
        .m3g
        .runtime
        .set_node_state(scene.mesh, true, false, u32::MAX, 1.0)
        .unwrap();
    assert!(!pick(&mut machine, &scene));
    let after = result(&machine.m3g.runtime, scene.result);
    assert_eq!(after.intersected, before.intersected);
    assert_eq!(after.ray, before.ray);
    assert_eq!(after.distance, 1.0);
    assert_eq!(after.normal, before.normal);
    assert_eq!(after.texture, before.texture);
}

#[test]
fn picking_observes_the_root_enable_flag_but_ignores_its_ancestors() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine.m3g.runtime);
    let ancestor = group(&mut machine.m3g.runtime);
    machine
        .m3g
        .runtime
        .add_child(ancestor, scene.group)
        .unwrap();
    machine
        .m3g
        .runtime
        .set_node_state(ancestor, true, false, u32::MAX, 1.0)
        .unwrap();
    assert!(pick(&mut machine, &scene));
    machine
        .m3g
        .runtime
        .set_node_state(scene.group, true, false, u32::MAX, 1.0)
        .unwrap();
    assert!(!pick(&mut machine, &scene));
}

#[test]
fn retained_picking_preserves_ray_units_at_small_and_large_mesh_scales() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine.m3g.runtime);
    for scale in [1.0, 1.0e-6, 1.0e-18, 1.0e18] {
        machine
            .m3g
            .runtime
            .set_scale(scene.mesh, m3g::Vec3::new(scale, scale, scale))
            .unwrap();
        assert!(
            machine
                .m3g_pick_ray(
                    scene.group,
                    u32::MAX,
                    m3g::Vec3::new(0.0, 0.0, 2.0 * scale),
                    m3g::Vec3::new(0.0, 0.0, -2.0 * scale),
                    Some(scene.result),
                    None,
                )
                .unwrap()
        );
        let hit = result(&machine.m3g.runtime, scene.result);
        assert!((hit.distance - 1.0).abs() < 1.0e-6, "{scale}");
        assert_eq!(hit.ray[5], -2.0 * scale);
    }
}

#[test]
fn picking_interpolates_plain_morphed_and_skinned_normals_in_mesh_coordinates() {
    for deformation in ["plain", "morph", "skin"] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let scene = scene(&mut machine.m3g.runtime);
        let runtime = &mut machine.m3g.runtime;
        let m3g::ObjectKind::VertexBuffer { state, .. } = runtime.kind_mut(scene.vertices).unwrap()
        else {
            unreachable!()
        };
        state
            .set_normals(Some(normals(&[
                -32768, -1, -1, -1, 32767, -1, -1, -1, 32767,
            ])))
            .unwrap();
        let epsilon = 1.0 / 65535.0;
        let mut normal = m3g::Vec3::new(
            -0.25 - 0.75 * epsilon,
            0.25 - 0.75 * epsilon,
            0.5 - 0.5 * epsilon,
        );
        let m3g::ObjectKind::Mesh(mesh) = runtime.kind(scene.mesh).unwrap() else {
            unreachable!()
        };
        let mesh = mesh.clone();
        if deformation == "morph" {
            let mut target = m3g::VertexBufferState::default();
            target
                .set_normals(Some(normals(&[
                    32767, -1, -1, 32767, -1, -1, 32767, -1, -1,
                ])))
                .unwrap();
            let target = buffer(runtime, target);
            *runtime.kind_mut(scene.mesh).unwrap() = m3g::ObjectKind::MorphingMesh {
                mesh,
                targets: vec![target],
                weights: vec![0.5],
            };
            normal = m3g::Vec3::new(
                (normal.x + 1.0) * 0.5,
                (normal.y - epsilon) * 0.5,
                (normal.z - epsilon) * 0.5,
            );
        } else if deformation == "skin" {
            let skeleton = group(runtime);
            *runtime.kind_mut(scene.mesh).unwrap() = m3g::ObjectKind::SkinnedMesh {
                mesh,
                skeleton,
                bones: vec![skeleton],
                bind_transforms: vec![m3g::Mat4::IDENTITY],
                influences: vec![m3g::SkinInfluence {
                    bone: 0,
                    first_vertex: 0,
                    vertex_count: 3,
                    weight: 1,
                }],
            };
            runtime.bind_skin_skeleton(scene.mesh).unwrap();
            let bone = m3g::Mat4::from_array([
                0.0, 2.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ])
            .unwrap();
            runtime.set_transform(skeleton, bone).unwrap();
            normal = m3g::Vec3::new(-normal.y, normal.x * 0.5, normal.z);
        }
        // Rotate the mesh's plane to face +X and give it a nonuniform scale.
        let model = m3g::Mat4::from_array([
            0.0, 0.0, -2.0, 0.0, 0.0, 3.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
        .unwrap();
        runtime.set_transform(scene.mesh, model).unwrap();
        assert!(
            machine
                .m3g_pick_ray(
                    scene.group,
                    u32::MAX,
                    m3g::Vec3::new(2.0, 0.0, 0.0),
                    m3g::Vec3::new(-2.0, 0.0, 0.0),
                    Some(scene.result),
                    None
                )
                .unwrap()
        );
        let expected = normal.normalized().unwrap();
        let actual = result(&machine.m3g.runtime, scene.result).normal;
        for (actual, expected) in [actual.x, actual.y, actual.z]
            .into_iter()
            .zip([expected.x, expected.y, expected.z])
        {
            assert!(
                (actual - expected).abs() < 1.0e-6,
                "{deformation}: {actual} != {expected}"
            );
        }
    }
}

#[test]
fn picking_applies_all_texture_components_and_projective_transforms_before_clamping() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine.m3g.runtime);
    let runtime = &mut machine.m3g.runtime;
    let m3g::ObjectKind::VertexBuffer { state, .. } = runtime.kind_mut(scene.vertices).unwrap()
    else {
        unreachable!()
    };
    for unit in 0..2 {
        state
            .set_texture_coordinates(
                unit,
                Some(normals(&[0, 0, 0, 4, 0, 2, 0, 4, 4])),
                0.5,
                [0.5, 1.0, 2.0],
            )
            .unwrap();
    }
    let image = runtime
        .create(
            None,
            m3g::ObjectKind::Image2D(
                m3g::Image2DState::mutable(m3g::ImageFormat::Rgba, 2, 2).unwrap(),
            ),
        )
        .unwrap();
    let mut textures = [None; 2];
    for slot in &mut textures {
        *slot = Some(
            runtime
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
                .unwrap(),
        );
    }
    let m3g::ObjectKind::Appearance(appearance) = runtime.kind_mut(scene.appearance).unwrap()
    else {
        unreachable!()
    };
    appearance.textures = textures;
    let matrix = [
        2.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 3.0, 0.0, 1.0, 0.0, 1.0, -2.0, 0.0, 1.0,
    ];
    runtime
        .set_scale(textures[1].unwrap(), m3g::Vec3::new(2.0, 3.0, 1.0))
        .unwrap();
    for scale in [1.0, 1.0e-8] {
        machine
            .m3g
            .runtime
            .set_transform(
                textures[0].unwrap(),
                m3g::Mat4::from_array(matrix.map(|value| value * scale)).unwrap(),
            )
            .unwrap();
        assert!(pick(&mut machine, &scene));
        let texture = result(&machine.m3g.runtime, scene.result).texture;
        for (actual, expected) in texture.into_iter().flatten().zip([6.375, 0.0, 2.0, 6.0]) {
            assert!(
                (actual - expected).abs() < 1.0e-6,
                "{scale}: {actual} != {expected}"
            );
        }
    }
}

#[test]
fn sprite_distance_intersects_the_camera_plane_away_from_the_sprite_center() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine.m3g.runtime);
    let runtime = &mut machine.m3g.runtime;
    runtime
        .set_node_state(scene.mesh, true, false, u32::MAX, 1.0)
        .unwrap();
    let image = runtime
        .create(
            None,
            m3g::ObjectKind::Image2D(
                m3g::Image2DState::mutable(m3g::ImageFormat::Rgb, 4, 4).unwrap(),
            ),
        )
        .unwrap();
    let sprite = runtime
        .create(
            None,
            m3g::ObjectKind::Sprite3D(m3g::SpriteState {
                node: m3g::NodeState::default(),
                scaled: true,
                image,
                appearance: Some(scene.appearance),
                crop: [0, 0, 4, 4],
            }),
        )
        .unwrap();
    runtime.add_child(scene.group, sprite).unwrap();
    runtime
        .set_translation(sprite, m3g::Vec3::new(0.0, 0.0, -3.0))
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
                scene.group,
                u32::MAX,
                m3g::Vec3::new(0.2, 0.0, -1.0),
                m3g::Vec3::new(1.8, 0.0, -9.0),
                Some(scene.result),
                Some(M3gSpritePickContext {
                    viewport_point: [0.6, 0.5],
                    projection,
                    group_to_camera: m3g::Mat4::IDENTITY,
                })
            )
            .unwrap()
    );
    let state = result(&machine.m3g.runtime, scene.result);
    assert_eq!(state.intersected, Some(sprite));
    assert!(
        (state.distance - 2.0 / 9.0).abs() < 1.0e-6,
        "{}",
        state.distance
    );
    assert!((state.ray[2] + state.ray[5] * state.distance + 3.0).abs() < 1.0e-6);
}
