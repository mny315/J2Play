use super::m3g_support::{object, setup_renderer};
use super::*;
use std::cell::Cell;
use std::rc::Rc;

struct Scene {
    root: m3g::Handle,
    mesh: m3g::Handle,
}

fn scene(machine: &mut Machine<'_, '_>, mesh_count: usize, submeshes: usize) -> Scene {
    setup_renderer(machine, m3g::Mat4::IDENTITY, m3g::Mat4::IDENTITY);
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
    let mut positions = m3g::VertexArrayState::new(3, 3, m3g::VertexComponent::Short).unwrap();
    positions
        .set_shorts(0, 3, &[-1, -1, 0, 1, -1, 0, 0, 1, 0])
        .unwrap();
    let mut vertices = m3g::VertexBufferState::default();
    vertices
        .set_positions(Some(positions), 1.0, [0.0; 3])
        .unwrap();
    let vertices = runtime
        .create(
            None,
            m3g::ObjectKind::VertexBuffer {
                state: vertices,
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
    let appearance = runtime
        .create(
            None,
            m3g::ObjectKind::Appearance(m3g::AppearanceState::default()),
        )
        .unwrap();
    let mut first = None;
    for _ in 0..mesh_count {
        let mesh = runtime
            .create(
                None,
                m3g::ObjectKind::Mesh(m3g::MeshState {
                    node: m3g::NodeState::default(),
                    vertices,
                    submeshes: vec![indices; submeshes],
                    appearances: vec![Some(appearance); submeshes],
                }),
            )
            .unwrap();
        runtime.add_child(root, mesh).unwrap();
        first.get_or_insert(mesh);
    }
    Scene {
        root,
        mesh: first.unwrap(),
    }
}

fn run(machine: &mut Machine<'_, '_>, scene: &Scene, picking: bool) -> Result<(), EmuError> {
    if picking {
        machine
            .m3g_pick_ray(
                scene.root,
                u32::MAX,
                m3g::Vec3::new(0.0, 0.0, 2.0),
                m3g::Vec3::new(0.0, 0.0, -1.0),
                None,
                None,
            )
            .map(|_| ())
    } else {
        machine.m3g_render_retained(scene.root, None, false)
    }
}

#[test]
fn retained_operations_bound_cumulative_vertex_work_and_can_be_retried() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine, 3, 1);
    for picking in [false, true] {
        machine.limits.m3g_vertices = 5;
        let error = run(&mut machine, &scene, picking).unwrap_err();
        assert_eq!(error.code(), "render-budget");
        assert_eq!(java_error_class(&error), Some("java/lang/OutOfMemoryError"));
        machine.limits.m3g_vertices = 9;
        run(&mut machine, &scene, picking).unwrap();
    }
}

#[test]
fn immediate_rendering_obeys_vertex_and_index_work_limits() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine, 1, 1);
    let m3g::ObjectKind::Mesh(mesh) = machine.m3g.runtime.kind(scene.mesh).unwrap() else {
        unreachable!()
    };
    let mut args = vec![Value::Reference(machine.m3g.graphics.target)];
    let handles = [
        mesh.vertices,
        mesh.submeshes[0],
        mesh.appearances[0].unwrap(),
    ];
    for (handle, class) in handles.into_iter().zip([
        "javax/microedition/m3g/VertexBuffer",
        "javax/microedition/m3g/TriangleStripArray",
        "javax/microedition/m3g/Appearance",
    ]) {
        let kind = machine.m3g.runtime.kind(handle).unwrap().clone();
        let (guest, _) = object(&mut machine, class, kind);
        args.push(Value::Reference(Some(guest)));
    }
    args.push(Value::Reference(None));
    for (vertices, triangles, expected) in [
        (2, 1, Some("render-budget")),
        (3, 0, Some("render-budget")),
        (3, 1, None),
    ] {
        machine.limits.m3g_vertices = vertices;
        machine.limits.m3g_render.triangles = triangles;
        let result = machine.invoke_m3g_graphics3d_native(
            "javax/microedition/m3g/Graphics3D",
            "render",
            "(Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;Ljavax/microedition/m3g/Transform;)V",
            &args,
        );
        assert_eq!(result.err().as_ref().map(EmuError::code), expected);
    }
}

#[test]
fn shared_unlit_submeshes_reuse_prepared_vertices_within_the_work_budget() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine, 1, 8);
    machine.limits.m3g_vertices = 3;
    run(&mut machine, &scene, false).unwrap();
    assert_eq!(machine.m3g.graphics.renderer.stats().submitted_triangles, 8);
    run(&mut machine, &scene, true).unwrap();
}

#[test]
fn degenerate_index_streams_still_consume_bounded_native_work() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine, 1, 1);
    let indices = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::TriangleStripArray(
                m3g::TriangleStripArrayState::new(vec![0; 6], vec![6]).unwrap(),
            ),
        )
        .unwrap();
    let m3g::ObjectKind::Mesh(mesh) = machine.m3g.runtime.kind_mut(scene.mesh).unwrap() else {
        unreachable!()
    };
    mesh.submeshes[0] = indices;
    machine.limits.m3g_render.triangles = 1;
    for picking in [false, true] {
        assert_eq!(
            run(&mut machine, &scene, picking).unwrap_err().code(),
            "render-budget"
        );
    }
}

#[test]
fn retained_rendering_and_picking_observe_cancellation_during_scene_work() {
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let program = Program::new();
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine, 3, 2);
    for picking in [false, true] {
        checks.set(0);
        cancel_at.set(usize::MAX);
        run(&mut machine, &scene, picking).unwrap();
        let count = checks.get();
        assert!(count >= 2, "scene work needs cancellation points");
        for at in 1..=count {
            checks.set(0);
            cancel_at.set(at);
            assert_eq!(
                run(&mut machine, &scene, picking).unwrap_err().code(),
                "execution-cancelled"
            );
        }
        cancel_at.set(usize::MAX);
        run(&mut machine, &scene, picking).unwrap();
    }
}

#[test]
fn picking_visits_renderable_children_of_a_skin_skeleton() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = scene(&mut machine, 1, 1);
    let runtime = &mut machine.m3g.runtime;
    let m3g::ObjectKind::Mesh(mesh) = runtime.kind(scene.mesh).unwrap() else {
        unreachable!()
    };
    let mut parent_mesh = mesh.clone();
    parent_mesh.node = m3g::NodeState::default();
    parent_mesh.appearances.fill(None);
    let skeleton = runtime
        .create(
            None,
            m3g::ObjectKind::Group {
                node: m3g::NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    runtime.remove_child(scene.root, scene.mesh).unwrap();
    runtime.add_child(skeleton, scene.mesh).unwrap();
    let parent = runtime
        .create(
            None,
            m3g::ObjectKind::SkinnedMesh {
                mesh: parent_mesh,
                skeleton,
                bones: Vec::new(),
                bind_transforms: Vec::new(),
                influences: Vec::new(),
            },
        )
        .unwrap();
    runtime.bind_skin_skeleton(parent).unwrap();
    runtime.add_child(scene.root, parent).unwrap();
    let picked = machine
        .m3g_pick_ray(
            scene.root,
            u32::MAX,
            m3g::Vec3::new(0.0, 0.0, 2.0),
            m3g::Vec3::new(0.0, 0.0, -1.0),
            None,
            None,
        )
        .unwrap();
    assert!(picked);
}
