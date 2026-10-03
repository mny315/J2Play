use crate::machine::{DefaultNativeContext, Limits, Program, java_error_class};
use std::{cell::Cell, rc::Rc};

#[path = "animation/targets.rs"]
mod targets;

struct CancellableAnimation(Rc<Cell<usize>>);

impl natives::HostServices for CancellableAnimation {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, diagnostics::EmuError> {
        Ok(None)
    }
    fn execution_cancelled(&self) -> bool {
        let Some(remaining) = self.0.get().checked_sub(1) else {
            return true;
        };
        self.0.set(remaining);
        false
    }
}

#[test]
fn animation_cancellation_keeps_the_whole_scene_and_allows_retry() {
    for (node_count, track_count) in [(32, 1), (1, 192)] {
        for allowed_polls in [0, 1, 3, 7] {
            let remaining = Rc::new(Cell::new(allowed_polls));
            let mut host = CancellableAnimation(Rc::clone(&remaining));
            let program = Program::new();
            let mut machine = program.machine(Limits::default(), false, &mut host);
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
            let mut nodes = Vec::new();
            for _ in 0..node_count {
                let mut sequence =
                    m3g::KeyframeSequenceState::new(1, 1, m3g::Interpolation::Step).unwrap();
                sequence.set_duration(100).unwrap();
                sequence.set_keyframe(0, 0, &[0.001]).unwrap();
                let node = animated_node(runtime, sequence);
                let track = runtime.animation_tracks(node).unwrap()[0];
                let track = runtime.kind(track).unwrap().clone();
                for _ in 1..track_count {
                    let track = runtime.create(None, track.clone()).unwrap();
                    runtime.add_animation_track(node, track).unwrap();
                }
                runtime.add_child(root, node).unwrap();
                nodes.push(node);
            }
            let error = machine.m3g_animate(root, 50).unwrap_err();
            assert_eq!(error.code(), "execution-cancelled");
            assert_eq!(java_error_class(&error), None);
            assert_eq!(machine.m3g.metrics.animation_samples, 0);
            for &node in &nodes {
                assert_eq!(machine.m3g.runtime.node_state(node).unwrap().3, 1.0);
            }
            remaining.set(usize::MAX);
            assert_eq!(machine.m3g_animate(root, 50).unwrap(), 1);
            assert_eq!(machine.m3g.metrics.animation_samples, node_count as u64);
            for node in nodes {
                assert!(machine.m3g.runtime.node_state(node).unwrap().3 < 0.5);
            }
        }
    }
}

fn animated_node(runtime: &mut m3g::Runtime, sequence: m3g::KeyframeSequenceState) -> m3g::Handle {
    let node = runtime
        .create(None, m3g::ObjectKind::Node(m3g::NodeState::default()))
        .unwrap();
    add_track(runtime, node, sequence, 256);
    node
}

fn add_track(
    runtime: &mut m3g::Runtime,
    target: m3g::Handle,
    sequence: m3g::KeyframeSequenceState,
    property: i32,
) {
    let sequence = runtime
        .create(None, m3g::ObjectKind::KeyframeSequence(sequence))
        .unwrap();
    let controller = runtime
        .create(
            None,
            m3g::ObjectKind::AnimationController(m3g::AnimationControllerState::default()),
        )
        .unwrap();
    let track = runtime
        .create(
            None,
            m3g::ObjectKind::AnimationTrack {
                sequence,
                controller: Some(controller),
                property,
            },
        )
        .unwrap();
    runtime.add_animation_track(target, track).unwrap();
}

#[test]
fn invalid_orientation_target_keeps_earlier_animation_properties_unchanged() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let runtime = &mut machine.m3g.runtime;
    let color = 0xff12_3456;
    let background = runtime
        .create(None, m3g::ObjectKind::Node(m3g::NodeState::default()))
        .unwrap();
    for (property, value) in [(256, &[0.5][..]), (268, &[0.0, 0.0, 0.0, 1.0][..])] {
        let mut sequence =
            m3g::KeyframeSequenceState::new(1, value.len(), m3g::Interpolation::Step).unwrap();
        sequence.set_duration(100).unwrap();
        sequence.set_keyframe(0, 0, value).unwrap();
        add_track(runtime, background, sequence, property);
    }
    // Normal attachment rejects this target. Corrupt the native type to check
    // that the animation transaction also protects against inconsistent state.
    *runtime.kind_mut(background).unwrap() = m3g::ObjectKind::Background(m3g::BackgroundState {
        color,
        ..m3g::BackgroundState::default()
    });
    let error = machine.m3g_animate(background, 50).unwrap_err();
    assert_eq!(error.code(), "invalid-animation-target");
    let m3g::ObjectKind::Background(state) = machine.m3g.runtime.kind(background).unwrap() else {
        panic!("expected background");
    };
    assert_eq!(state.color, color);
    assert_eq!(machine.m3g.metrics.animation_samples, 0);
}

#[test]
fn invalid_animation_sequences_report_java_state_errors_without_partial_scene_updates() {
    for invalid in 0..3 {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let mut valid = m3g::KeyframeSequenceState::new(1, 1, m3g::Interpolation::Step).unwrap();
        valid.set_duration(100).unwrap();
        valid.set_keyframe(0, 0, &[0.5]).unwrap();
        let good = animated_node(&mut machine.m3g.runtime, valid);
        let mut invalid_sequence =
            m3g::KeyframeSequenceState::new(3, 1, m3g::Interpolation::Linear).unwrap();
        invalid_sequence.set_keyframe(0, 10, &[0.2]).unwrap();
        invalid_sequence.set_keyframe(2, 30, &[0.6]).unwrap();
        if invalid != 2 {
            invalid_sequence.set_keyframe(1, 20, &[0.4]).unwrap();
        }
        if invalid != 0 {
            invalid_sequence
                .set_duration(if invalid == 1 { 20 } else { 100 })
                .unwrap();
        }
        assert_eq!(
            java_error_class(&invalid_sequence.set_duration(-1).unwrap_err()),
            Some("java/lang/IllegalArgumentException")
        );
        let bad = animated_node(&mut machine.m3g.runtime, invalid_sequence);
        let root = machine
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
        machine.m3g.runtime.add_child(root, good).unwrap();
        machine.m3g.runtime.add_child(root, bad).unwrap();
        let error = machine.m3g_animate(root, 15).unwrap_err();
        assert_eq!(
            java_error_class(&error),
            Some("java/lang/IllegalStateException")
        );
        assert_eq!(machine.m3g.runtime.node_state(good).unwrap().3, 1.0);
        assert_eq!(machine.m3g.runtime.node_state(bad).unwrap().3, 1.0);
    }
}

#[test]
fn controller_weights_scale_node_alpha_and_zero_weight_leaves_the_node_unchanged() {
    let tiny = f32::EPSILON * 0.25;
    for (samples, expected) in [
        (vec![(0.8, 0.25)], 0.2),
        (vec![(0.2, 1.0), (0.4, 1.0)], 0.6),
        (vec![(0.5, tiny)], tiny * 0.5),
        (vec![(0.25, f32::MAX), (0.25, f32::MAX)], 1.0),
        (vec![(0.8, 0.0)], 1.0),
    ] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let runtime = &mut machine.m3g.runtime;
        let node = runtime
            .create(None, m3g::ObjectKind::Node(m3g::NodeState::default()))
            .unwrap();
        for (value, weight) in samples {
            let mut sequence =
                m3g::KeyframeSequenceState::new(1, 1, m3g::Interpolation::Step).unwrap();
            sequence.set_duration(100).unwrap();
            sequence.set_keyframe(0, 0, &[value]).unwrap();
            let sequence = runtime
                .create(None, m3g::ObjectKind::KeyframeSequence(sequence))
                .unwrap();
            let mut controller = m3g::AnimationControllerState::default();
            controller.set_weight(weight).unwrap();
            let controller = runtime
                .create(None, m3g::ObjectKind::AnimationController(controller))
                .unwrap();
            let track = runtime
                .create(
                    None,
                    m3g::ObjectKind::AnimationTrack {
                        sequence,
                        controller: Some(controller),
                        property: 256,
                    },
                )
                .unwrap();
            runtime.add_animation_track(node, track).unwrap();
        }
        machine.m3g_animate(node, 50).unwrap();
        assert_eq!(machine.m3g.runtime.node_state(node).unwrap().3, expected);
    }
}

#[test]
pub(crate) fn m3g_alpha_animation_updates_every_compatible_object_kind() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);

    let mut sequence_state =
        m3g::KeyframeSequenceState::new(2, 1, m3g::Interpolation::Linear).unwrap();
    sequence_state.set_keyframe(0, 0, &[-0.5]).unwrap();
    sequence_state.set_keyframe(1, 100, &[1.5]).unwrap();
    sequence_state.set_duration(100).unwrap();
    let sequence = machine
        .m3g
        .runtime
        .create(None, m3g::ObjectKind::KeyframeSequence(sequence_state))
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
                property: 256,
            },
        )
        .unwrap();

    let node = machine
        .m3g
        .runtime
        .create(None, m3g::ObjectKind::Node(m3g::NodeState::default()))
        .unwrap();
    let background_state = m3g::BackgroundState {
        color: 0xaa11_2233,
        ..m3g::BackgroundState::default()
    };
    let background = machine
        .m3g
        .runtime
        .create(None, m3g::ObjectKind::Background(background_state))
        .unwrap();
    let mut material_state = m3g::MaterialObjectState::default();
    material_state.material.diffuse = 0xaa44_5566;
    let material = machine
        .m3g
        .runtime
        .create(None, m3g::ObjectKind::Material(material_state))
        .unwrap();
    let mut vertex_state = m3g::VertexBufferState::default();
    vertex_state.set_default_color(0xaa77_8899);
    let vertex_buffer = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::VertexBuffer {
                state: vertex_state,
                arrays: [None; 5],
            },
        )
        .unwrap();
    let targets = [node, background, material, vertex_buffer];
    for target in targets {
        machine
            .m3g
            .runtime
            .add_animation_track(target, track)
            .unwrap();
        assert_eq!(machine.m3g_animate(target, 0).unwrap(), 1);
    }

    assert_eq!(machine.m3g.runtime.node_state(node).unwrap().3, 0.0);
    let m3g::ObjectKind::Background(state) = machine.m3g.runtime.kind(background).unwrap() else {
        panic!("background target changed type");
    };
    assert_eq!(state.color, 0x0011_2233);
    let m3g::ObjectKind::Material(state) = machine.m3g.runtime.kind(material).unwrap() else {
        panic!("material target changed type");
    };
    assert_eq!(state.material.diffuse, 0x0044_5566);
    let m3g::ObjectKind::VertexBuffer { state, .. } =
        machine.m3g.runtime.kind(vertex_buffer).unwrap()
    else {
        panic!("vertex buffer target changed type");
    };
    assert_eq!(state.default_color(), 0x0077_8899);

    for target in targets {
        assert_eq!(machine.m3g_animate(target, 100).unwrap(), 1);
    }
    assert_eq!(machine.m3g.runtime.node_state(node).unwrap().3, 1.0);
    let m3g::ObjectKind::Background(state) = machine.m3g.runtime.kind(background).unwrap() else {
        panic!("background target changed type");
    };
    assert_eq!(state.color, 0xff11_2233);
    let m3g::ObjectKind::Material(state) = machine.m3g.runtime.kind(material).unwrap() else {
        panic!("material target changed type");
    };
    assert_eq!(state.material.diffuse, 0xff44_5566);
    let m3g::ObjectKind::VertexBuffer { state, .. } =
        machine.m3g.runtime.kind(vertex_buffer).unwrap()
    else {
        panic!("vertex buffer target changed type");
    };
    assert_eq!(state.default_color(), 0xff77_8899);
}

#[test]
pub(crate) fn m3g_camera_clip_distance_animation_follows_projection_semantics() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);

    let mut sequence_state =
        m3g::KeyframeSequenceState::new(2, 1, m3g::Interpolation::Linear).unwrap();
    sequence_state.set_keyframe(0, 0, &[-5.0]).unwrap();
    sequence_state.set_keyframe(1, 100, &[25.0]).unwrap();
    sequence_state.set_duration(100).unwrap();
    let sequence = machine
        .m3g
        .runtime
        .create(None, m3g::ObjectKind::KeyframeSequence(sequence_state))
        .unwrap();
    let controller = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::AnimationController(m3g::AnimationControllerState::default()),
        )
        .unwrap();
    let far_track = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::AnimationTrack {
                sequence,
                controller: Some(controller),
                property: 263,
            },
        )
        .unwrap();
    let near_track = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::AnimationTrack {
                sequence,
                controller: Some(controller),
                property: 267,
            },
        )
        .unwrap();
    let camera = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::Camera {
                node: m3g::NodeState::default(),
                projection: m3g::CameraProjection::Perspective {
                    field_of_view: 60.0,
                    aspect_ratio: 1.0,
                    near: 1.0,
                    far: 10.0,
                },
            },
        )
        .unwrap();
    machine
        .m3g
        .runtime
        .add_animation_track(camera, far_track)
        .unwrap();
    machine
        .m3g
        .runtime
        .add_animation_track(camera, near_track)
        .unwrap();

    assert_eq!(machine.m3g_animate(camera, 0).unwrap(), 1);
    let m3g::ObjectKind::Camera {
        projection: m3g::CameraProjection::Perspective { near, far, .. },
        ..
    } = machine.m3g.runtime.kind(camera).unwrap()
    else {
        panic!("camera projection changed type");
    };
    assert_eq!((*near, *far), (f32::from_bits(1), f32::from_bits(1)));

    let m3g::ObjectKind::Camera { projection, .. } = machine.m3g.runtime.kind_mut(camera).unwrap()
    else {
        panic!("camera changed type");
    };
    *projection = m3g::CameraProjection::Parallel {
        height: 2.0,
        aspect_ratio: 1.0,
        near: 1.0,
        far: 10.0,
    };
    assert_eq!(machine.m3g_animate(camera, 0).unwrap(), 1);
    let m3g::ObjectKind::Camera {
        projection: m3g::CameraProjection::Parallel { near, far, .. },
        ..
    } = machine.m3g.runtime.kind(camera).unwrap()
    else {
        panic!("camera projection changed type");
    };
    assert_eq!((*near, *far), (-5.0, -5.0));

    let m3g::ObjectKind::Camera { projection, .. } = machine.m3g.runtime.kind_mut(camera).unwrap()
    else {
        panic!("camera changed type");
    };
    *projection = m3g::CameraProjection::Generic(m3g::Mat4::IDENTITY);
    assert_eq!(machine.m3g_animate(camera, 100).unwrap(), 1);
    assert!(matches!(
        machine.m3g.runtime.kind(camera).unwrap(),
        m3g::ObjectKind::Camera {
            projection: m3g::CameraProjection::Generic(matrix),
            ..
        } if *matrix == m3g::Mat4::IDENTITY
    ));
}
