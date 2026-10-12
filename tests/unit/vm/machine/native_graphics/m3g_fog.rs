use super::m3g_support::{object, setup_renderer};
use super::*;

#[test]
fn fog_uses_camera_distance_for_immediate_retained_deformed_and_sprite_geometry() {
    for mode in [
        "immediate",
        "immediate-lit",
        "mesh",
        "mesh-lit",
        "morph",
        "morph-lit",
        "skin",
        "skin-lit",
        "sprite",
    ] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        // This valid projection flattens clip Z and is deliberately singular:
        // fog must retain eye distance before projection, not invert it later.
        let projection = m3g::Mat4::from_row_major([
            0.25, 0.0, 0.0, 0.0, 0.0, 0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
        .unwrap();
        let target = setup_renderer(
            &mut machine,
            projection,
            m3g::Mat4::translation(0.0, 0.0, 5.0).unwrap(),
        );
        let (_, fog) = object(
            &mut machine,
            "javax/microedition/m3g/Fog",
            m3g::ObjectKind::Fog(m3g::FogState {
                color: 0x0000_00ff,
                mode: m3g::FogMode::Linear,
                near: 2.0,
                far: 6.0,
                density: 1.0,
            }),
        );
        let mut positions = m3g::VertexArrayState::new(3, 3, m3g::VertexComponent::Short).unwrap();
        positions
            .set_shorts(0, 3, &[-2, -2, 0, 2, -2, 0, 0, 2, 0])
            .unwrap();
        let mut buffer = m3g::VertexBufferState::default();
        buffer
            .set_positions(Some(positions), 1.0, [0.0; 3])
            .unwrap();
        buffer.set_default_color(0xffff_0000);
        let material = if mode.ends_with("-lit") {
            // Lighting must replace the vertex color, including on MorphingMesh.
            buffer.set_default_color(0xff00_00ff);
            let mut normals =
                m3g::VertexArrayState::new(3, 3, m3g::VertexComponent::Short).unwrap();
            normals
                .set_shorts(0, 3, &[0, 0, 1, 0, 0, 1, 0, 0, 1])
                .unwrap();
            buffer.set_normals(Some(normals)).unwrap();
            let (_, material) = object(
                &mut machine,
                "javax/microedition/m3g/Material",
                m3g::ObjectKind::Material(m3g::MaterialObjectState {
                    material: m3g::MaterialState {
                        emissive: 0x00ff_0000,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            );
            Some(material)
        } else {
            None
        };
        let (appearance_guest, appearance) = object(
            &mut machine,
            "javax/microedition/m3g/Appearance",
            m3g::ObjectKind::Appearance(m3g::AppearanceState {
                fog: Some(fog),
                material,
                ..Default::default()
            }),
        );
        let (vertices_guest, vertices) = object(
            &mut machine,
            "javax/microedition/m3g/VertexBuffer",
            m3g::ObjectKind::VertexBuffer {
                state: buffer,
                arrays: [None; 5],
            },
        );
        let (indices_guest, indices) = object(
            &mut machine,
            "javax/microedition/m3g/TriangleStripArray",
            m3g::ObjectKind::TriangleStripArray(
                m3g::TriangleStripArrayState::implicit(0, vec![3]).unwrap(),
            ),
        );
        let transform = m3g::Mat4::translation(
            0.0,
            0.0,
            if mode.starts_with("morph") || mode.starts_with("skin") {
                0.0
            } else {
                1.0
            },
        )
        .unwrap();
        if mode.starts_with("immediate") {
            let (transform, _) = object(
                &mut machine,
                "javax/microedition/m3g/Transform",
                m3g::ObjectKind::Transform(transform),
            );
            let method = runtime_method(
                "javax/microedition/m3g/Graphics3D",
                "render",
                "(Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;Ljavax/microedition/m3g/Transform;)V",
                &[0xb1],
                0,
                5,
                Vec::new(),
                false,
            );
            machine
                .invoke_m3g_native(
                    &method,
                    &[
                        Value::Reference(Some(target)),
                        Value::Reference(Some(vertices_guest)),
                        Value::Reference(Some(indices_guest)),
                        Value::Reference(Some(appearance_guest)),
                        Value::Reference(Some(transform)),
                    ],
                    1,
                )
                .unwrap();
        } else {
            let mesh = m3g::MeshState {
                node: m3g::NodeState::default(),
                vertices,
                submeshes: vec![indices],
                appearances: vec![Some(appearance)],
            };
            let kind = match mode {
                "morph" | "morph-lit" => {
                    let mut target = machine
                        .m3g
                        .runtime
                        .resolved_vertex_buffer(vertices)
                        .unwrap();
                    let mut positions = target.positions().unwrap().0.clone();
                    positions
                        .set_shorts(0, 3, &[-2, -2, 2, 2, -2, 2, 0, 2, 2])
                        .unwrap();
                    target
                        .set_positions(Some(positions), 1.0, [0.0; 3])
                        .unwrap();
                    let (_, target) = object(
                        &mut machine,
                        "javax/microedition/m3g/VertexBuffer",
                        m3g::ObjectKind::VertexBuffer {
                            state: target,
                            arrays: [None; 5],
                        },
                    );
                    m3g::ObjectKind::MorphingMesh {
                        mesh,
                        targets: vec![target],
                        weights: vec![0.5],
                    }
                }
                "skin" | "skin-lit" => {
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
                    m3g::ObjectKind::SkinnedMesh {
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
                    }
                }
                "sprite" => {
                    let (_, image) = object(
                        &mut machine,
                        "javax/microedition/m3g/Image2D",
                        m3g::ObjectKind::Image2D(
                            m3g::Image2DState::from_argb(
                                m3g::ImageFormat::Rgba,
                                2,
                                2,
                                &[0xffff_0000; 4],
                            )
                            .unwrap(),
                        ),
                    );
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
            let node = machine.m3g.runtime.create(None, kind).unwrap();
            if mode.starts_with("skin") {
                machine.m3g.runtime.bind_skin_skeleton(node).unwrap();
                let m3g::ObjectKind::SkinnedMesh { skeleton, .. } =
                    machine.m3g.runtime.kind(node).unwrap()
                else {
                    unreachable!()
                };
                let skeleton = *skeleton;
                machine
                    .m3g
                    .runtime
                    .set_translation(skeleton, m3g::Vec3::new(0.0, 0.0, 1.0))
                    .unwrap();
            }
            machine
                .m3g_render_retained(node, Some(transform), false)
                .unwrap();
        }
        assert_eq!(
            machine.m3g.graphics.renderer.pixels()[16 * 32 + 16],
            0xff80_0080,
            "{mode}"
        );
    }
}
