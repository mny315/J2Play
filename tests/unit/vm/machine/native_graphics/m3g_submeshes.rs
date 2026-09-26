use super::m3g_support::{object, setup_renderer};
use super::*;

pub(super) struct Scene {
    pub(super) root: m3g::Handle,
    materials: Vec<m3g::Handle>,
}

pub(super) fn scene(
    machine: &mut Machine<'_, '_>,
    triangles: usize,
    submeshes: usize,
    materials: usize,
    split: bool,
) -> Scene {
    setup_renderer(machine, m3g::Mat4::IDENTITY, m3g::Mat4::IDENTITY);
    let (light, _) = object(
        machine,
        "javax/microedition/m3g/Light",
        m3g::ObjectKind::Light {
            node: m3g::NodeState::default(),
            light: m3g::LightState::default(),
        },
    );
    machine.m3g.graphics.lights = vec![(light, m3g::Mat4::IDENTITY)];
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
    let count = triangles * 3;
    let mut positions = m3g::VertexArrayState::new(count, 3, m3g::VertexComponent::Short).unwrap();
    let mut normals = m3g::VertexArrayState::new(count, 3, m3g::VertexComponent::Short).unwrap();
    let mut coordinates = Vec::with_capacity(count * 3);
    for triangle in 0..triangles {
        let x = (triangle % 16) as i16 * 8 - 64;
        let y = (triangle / 16 % 16) as i16 * 8 - 64;
        coordinates.extend([x, y, 0, x + 6, y, 0, x, y + 6, 0]);
    }
    positions.set_shorts(0, count, &coordinates).unwrap();
    normals
        .set_shorts(0, count, &[0, 0, i16::MAX].repeat(count))
        .unwrap();
    let mut state = m3g::VertexBufferState::default();
    state
        .set_positions(Some(positions), 1.0 / 64.0, [0.0; 3])
        .unwrap();
    state.set_normals(Some(normals)).unwrap();
    let vertices = runtime
        .create(
            None,
            m3g::ObjectKind::VertexBuffer {
                state,
                arrays: [None; 5],
            },
        )
        .unwrap();
    let appearance_period = materials.max(1);
    let materials = (0..materials.min(2))
        .map(|index| {
            runtime
                .create(
                    None,
                    m3g::ObjectKind::Material(m3g::MaterialObjectState {
                        material: m3g::MaterialState {
                            diffuse: [0xfff0_6040, 0xff40_f060][index % 2],
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut indices = Vec::new();
    let mut appearances = Vec::new();
    for index in 0..submeshes {
        indices.push(
            runtime
                .create(
                    None,
                    m3g::ObjectKind::TriangleStripArray(
                        m3g::TriangleStripArrayState::implicit(
                            (index * count / submeshes) as u32,
                            vec![3; triangles / submeshes],
                        )
                        .unwrap(),
                    ),
                )
                .unwrap(),
        );
        appearances.push(Some(
            runtime
                .create(
                    None,
                    m3g::ObjectKind::Appearance(m3g::AppearanceState {
                        material: materials.get(index % appearance_period).copied(),
                        ..Default::default()
                    }),
                )
                .unwrap(),
        ));
    }
    for index in 0..if split { submeshes } else { 1 } {
        let range = if split {
            index..index + 1
        } else {
            0..submeshes
        };
        let mesh = runtime
            .create(
                None,
                m3g::ObjectKind::Mesh(m3g::MeshState {
                    node: m3g::NodeState::default(),
                    vertices,
                    submeshes: indices[range.clone()].to_vec(),
                    appearances: appearances[range].to_vec(),
                }),
            )
            .unwrap();
        runtime.add_child(root, mesh).unwrap();
    }
    Scene { root, materials }
}

pub(super) fn render(machine: &mut Machine<'_, '_>, root: m3g::Handle) {
    machine.m3g.graphics.renderer.clear(Some(0xff00_0000), true);
    machine.m3g_publish_bound_target().unwrap();
    machine.m3g_render_retained(root, None, false).unwrap();
}

#[test]
fn submesh_lighting_matches_separate_meshes_and_observes_material_changes() {
    // Pattern three alternates two materials with unlit submeshes.
    for materials in [0, 1, 2, 3] {
        let program = Program::new();
        let mut batched_context = DefaultNativeContext;
        let mut split_context = DefaultNativeContext;
        let mut batched = program.machine(Limits::default(), false, &mut batched_context);
        let mut split = program.machine(Limits::default(), false, &mut split_context);
        let batch_scene = scene(&mut batched, 32, 8, materials, false);
        let split_scene = scene(&mut split, 32, 8, materials, true);
        let mut previous_frame = None;
        for color in [0xfff0_6040, 0xff40_60f0] {
            for (machine, scene) in [(&mut batched, &batch_scene), (&mut split, &split_scene)] {
                if let Some(&material) = scene.materials.first() {
                    let m3g::ObjectKind::Material(material) =
                        machine.m3g.runtime.kind_mut(material).unwrap()
                    else {
                        panic!("expected material");
                    };
                    material.material.diffuse = color;
                }
                render(machine, scene.root);
            }
            assert!(
                batched
                    .m3g
                    .graphics
                    .renderer
                    .pixels()
                    .iter()
                    .any(|pixel| *pixel != 0xff00_0000)
            );
            assert_eq!(
                batched.m3g.graphics.renderer.pixels(),
                split.m3g.graphics.renderer.pixels()
            );
            assert_eq!(
                batched.m3g.graphics.renderer.depth(),
                split.m3g.graphics.renderer.depth()
            );
            assert_eq!(
                batched.m3g.graphics.renderer.stats(),
                split.m3g.graphics.renderer.stats()
            );
            if materials != 0 {
                let frame = batched.m3g.graphics.renderer.pixels().to_vec();
                if let Some(previous) = previous_frame.replace(frame.clone()) {
                    assert_ne!(frame, previous);
                }
            }
        }
    }
}

#[test]
fn shared_lit_submeshes_reuse_vertices_within_the_work_budget() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine, 32, 8, 1, false);
    machine.limits.m3g_vertices = 32 * 3;
    render(&mut machine, scene.root);
    assert_eq!(
        machine.m3g.graphics.renderer.stats().submitted_triangles,
        32
    );
    machine.limits.m3g_vertices -= 1;
    assert_eq!(
        machine
            .m3g_render_retained(scene.root, None, false)
            .unwrap_err()
            .code(),
        "render-budget"
    );
}

#[test]
#[ignore = "manual release throughput comparison"]
fn submesh_lighting_throughput() {
    use std::{hint::black_box, time::Instant};

    for triangles in [32, 512] {
        for submeshes in [1, 8] {
            for materials in [0, 1, 2] {
                let program = Program::new();
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let scene = scene(&mut machine, triangles, submeshes, materials, false);
                let start = Instant::now();
                for _ in 0..128 {
                    render(&mut machine, black_box(scene.root));
                }
                let elapsed = start.elapsed();
                let checksum = machine
                    .m3g
                    .graphics
                    .renderer
                    .pixels()
                    .iter()
                    .fold(0_u64, |sum, &pixel| {
                        sum.wrapping_mul(31).wrapping_add(u64::from(pixel))
                    });
                eprintln!(
                    "submesh-lighting-triangles={triangles}-submeshes={submeshes}-materials={materials}: elapsed={elapsed:?} checksum={checksum} stats={:?}",
                    machine.m3g.graphics.renderer.stats()
                );
            }
        }
    }
}
