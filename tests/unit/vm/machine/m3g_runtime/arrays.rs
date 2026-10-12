use super::*;
use crate::machine::{CallOutcome, DefaultNativeContext, HashMap, Limits, Program};

#[path = "vertex_transfers.rs"]
mod vertex_transfers;

#[test]
fn vertex_array_transfers_validate_types_ranges_and_preserve_destination_suffixes() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for (component_type, kind, descriptor) in [
        (m3g::VertexComponent::Byte, ArrayKind::Byte, "(II[B)V"),
        (m3g::VertexComponent::Short, ArrayKind::Short, "(II[S)V"),
    ] {
        let guest = machine
            .heap
            .managed
            .allocate_object("javax/microedition/m3g/VertexArray", HashMap::new())
            .unwrap();
        machine
            .m3g
            .runtime
            .create(
                Some(guest.to_raw()),
                m3g::ObjectKind::VertexArray(
                    m3g::VertexArrayState::new(1, 3, component_type).unwrap(),
                ),
            )
            .unwrap();
        let values = machine
            .heap
            .managed
            .allocate_array(kind.clone(), 4)
            .unwrap();
        for (index, value) in [-1, 2, 3, 99].into_iter().enumerate() {
            machine
                .heap
                .managed
                .array_set(values, index as i32, HeapValue::Int(value))
                .unwrap();
        }
        let call = |machine: &mut Machine<'_, '_>, name, descriptor, first, count, values| {
            machine.invoke_m3g_object_native(
                "javax/microedition/m3g/VertexArray",
                name,
                descriptor,
                &[
                    Value::Reference(Some(guest)),
                    Value::Int(first),
                    Value::Int(count),
                    Value::Reference(Some(values)),
                ],
            )
        };
        call(&mut machine, "set", descriptor, 0, 1, values).unwrap();
        for name in ["get", "set"] {
            call(&mut machine, name, descriptor, 1, 0, values).unwrap();
            let error = call(&mut machine, name, descriptor, -1, 0, values)
                .err()
                .unwrap();
            assert_eq!(
                crate::machine::java_error_class(&error),
                Some("java/lang/IndexOutOfBoundsException")
            );
            let error = call(&mut machine, name, descriptor, 0, -1, values)
                .err()
                .unwrap();
            assert_eq!(
                crate::machine::java_error_class(&error),
                Some("java/lang/IllegalArgumentException")
            );
            let error = call(&mut machine, name, descriptor, 2, 0, values)
                .err()
                .unwrap();
            assert_eq!(
                crate::machine::java_error_class(&error),
                Some("java/lang/IndexOutOfBoundsException")
            );
        }
        let destination = machine.heap.managed.allocate_array(kind, 4).unwrap();
        machine
            .heap
            .managed
            .array_set(destination, 3, HeapValue::Int(77))
            .unwrap();
        call(&mut machine, "get", descriptor, 0, 1, destination).unwrap();
        for (index, expected) in (0..).zip([-1, 2, 3, 77]) {
            assert_eq!(
                machine.heap.managed.array_get(destination, index).unwrap(),
                HeapValue::Int(expected)
            );
        }
        let wrong_descriptor = if descriptor == "(II[B)V" {
            "(II[S)V"
        } else {
            "(II[B)V"
        };
        for name in ["get", "set"] {
            let error = call(&mut machine, name, wrong_descriptor, 0, 0, values)
                .err()
                .unwrap();
            assert_eq!(
                crate::machine::java_error_class(&error),
                Some("java/lang/IllegalStateException")
            );
        }
    }
}

#[test]
fn bone_weight_queries_respect_the_profile_transform_limit() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            m3g_max_transforms_per_vertex: 1,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/SkinnedMesh", HashMap::new())
        .unwrap();
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
    let mut bones = Vec::new();
    let mut guests = Vec::new();
    for _ in 0..3 {
        let guest = machine
            .heap
            .managed
            .allocate_object("javax/microedition/m3g/Group", HashMap::new())
            .unwrap();
        let bone = machine
            .m3g
            .runtime
            .create(
                Some(guest.to_raw()),
                m3g::ObjectKind::Group {
                    node: m3g::NodeState::default(),
                    children: Vec::new(),
                },
            )
            .unwrap();
        machine.m3g.runtime.add_child(skeleton, bone).unwrap();
        guests.push(guest);
        bones.push(bone);
    }
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
    machine
        .m3g
        .runtime
        .create(
            Some(guest.to_raw()),
            m3g::ObjectKind::SkinnedMesh {
                mesh: m3g::MeshState {
                    node: m3g::NodeState::default(),
                    vertices,
                    submeshes: Vec::new(),
                    appearances: Vec::new(),
                },
                skeleton,
                bones,
                bind_transforms: vec![m3g::Mat4::IDENTITY; 3],
                influences: [1, 3, 2]
                    .into_iter()
                    .enumerate()
                    .map(|(bone, weight)| m3g::SkinInfluence {
                        bone,
                        first_vertex: 0,
                        vertex_count: 1,
                        weight,
                    })
                    .collect(),
            },
        )
        .unwrap();
    for (bone, expected) in guests.into_iter().zip([0, 1, 0]) {
        let result = machine
            .invoke_m3g_mesh_native(
                "javax/microedition/m3g/SkinnedMesh",
                "getBoneVertices",
                "(Ljavax/microedition/m3g/Node;[I[F)I",
                &[
                    Value::Reference(Some(guest)),
                    Value::Reference(Some(bone)),
                    Value::Reference(None),
                    Value::Reference(None),
                ],
            )
            .unwrap();
        assert!(
            matches!(result, CallOutcome::Return(Some(Value::Int(count))) if count == expected)
        );
    }
}

#[test]
fn skin_ranges_can_be_bound_before_vertex_data_and_queries_validate_destinations() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/SkinnedMesh", HashMap::new())
        .unwrap();
    let bone_guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Group", HashMap::new())
        .unwrap();
    let bone = machine
        .m3g
        .runtime
        .create(
            Some(bone_guest.to_raw()),
            m3g::ObjectKind::Group {
                node: m3g::NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
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
    let native = machine
        .m3g
        .runtime
        .create(
            Some(guest.to_raw()),
            m3g::ObjectKind::SkinnedMesh {
                mesh: m3g::MeshState {
                    node: m3g::NodeState::default(),
                    vertices,
                    submeshes: Vec::new(),
                    appearances: Vec::new(),
                },
                skeleton: bone,
                bones: Vec::new(),
                bind_transforms: Vec::new(),
                influences: Vec::new(),
            },
        )
        .unwrap();
    machine.m3g.runtime.bind_skin_skeleton(native).unwrap();
    let add = |machine: &mut Machine<'_, '_>, weight, first, count| {
        machine.invoke_m3g_mesh_native(
            "javax/microedition/m3g/SkinnedMesh",
            "addTransform",
            "(Ljavax/microedition/m3g/Node;III)V",
            &[
                Value::Reference(Some(guest)),
                Value::Reference(Some(bone_guest)),
                Value::Int(weight),
                Value::Int(first),
                Value::Int(count),
            ],
        )
    };
    for (weight, first, count, expected) in [
        (0, 0, 1, "java/lang/IllegalArgumentException"),
        (-1, 0, 1, "java/lang/IllegalArgumentException"),
        (1, 0, 0, "java/lang/IllegalArgumentException"),
        (1, -1, 1, "java/lang/IndexOutOfBoundsException"),
        (1, 65_535, 1, "java/lang/IndexOutOfBoundsException"),
        (1, i32::MAX, i32::MAX, "java/lang/IndexOutOfBoundsException"),
    ] {
        let error = add(&mut machine, weight, first, count).err().unwrap();
        assert_eq!(crate::machine::java_error_class(&error), Some(expected));
    }
    assert!(matches!(
        add(&mut machine, 1, 65_534, 1).unwrap(),
        CallOutcome::Return(None)
    ));
    let query = |machine: &mut Machine<'_, '_>, indices, weights| {
        machine.invoke_m3g_mesh_native(
            "javax/microedition/m3g/SkinnedMesh",
            "getBoneVertices",
            "(Ljavax/microedition/m3g/Node;[I[F)I",
            &[
                Value::Reference(Some(guest)),
                Value::Reference(Some(bone_guest)),
                Value::Reference(indices),
                Value::Reference(weights),
            ],
        )
    };
    let empty = float_array(&mut machine, 0, &[]);
    assert!(matches!(
        query(&mut machine, None, Some(empty)).unwrap(),
        CallOutcome::Return(Some(Value::Int(1)))
    ));
    let indices = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 2)
        .unwrap();
    machine.m3g_write_int_array(indices, &[42, 43]).unwrap();
    assert!(query(&mut machine, Some(indices), Some(empty)).is_err());
    assert_eq!(machine.m3g_int_array(indices).unwrap(), [42, 43]);
    let weights = float_array(&mut machine, 2, &[0.0, 7.0]);
    assert!(matches!(
        query(&mut machine, Some(indices), Some(weights)).unwrap(),
        CallOutcome::Return(Some(Value::Int(1)))
    ));
    assert_eq!(machine.m3g_int_array(indices).unwrap(), [65_534, 43]);
    assert_eq!(machine.m3g_float_array(weights, 0).unwrap(), [1.0, 7.0]);
    let m3g::ObjectKind::SkinnedMesh { influences, .. } = machine.m3g.runtime.kind(native).unwrap()
    else {
        panic!("expected skin");
    };
    assert_eq!(influences.len(), 1);
    let error = m3g::SkinInfluence::validate_all(influences, 1, 1).unwrap_err();
    assert_eq!(
        crate::machine::java_error_class(&error),
        Some("java/lang/IllegalStateException")
    );
}

fn float_array(machine: &mut Machine<'_, '_>, length: usize, prefix: &[f32]) -> Handle {
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, i32::try_from(length).unwrap())
        .unwrap();
    machine.m3g_write_float_array(array, prefix).unwrap();
    array
}

#[test]
fn byte_array_prefixes_bound_copies_without_padding_short_inputs() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let bytes = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 65_536)
        .unwrap();
    machine
        .m3g_write_primitive_int_array(bytes, &ArrayKind::Byte, &[i8::MIN, -1, 0, i8::MAX])
        .unwrap();
    for maximum in [0, 1, 4, 1_024, 65_536, usize::MAX] {
        let copy = machine.m3g_byte_array_prefix(bytes, maximum).unwrap();
        assert_eq!(copy.len(), maximum.min(65_536));
        let prefix = [128, 255, 0, 127];
        assert_eq!(&copy[..copy.len().min(4)], &prefix[..copy.len().min(4)]);
        assert!(copy.iter().skip(4).all(|byte| *byte == 0));
    }
    let empty = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 0)
        .unwrap();
    assert!(machine.m3g_byte_array_prefix(empty, 4).unwrap().is_empty());
    let wrong = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 4)
        .unwrap();
    for maximum in [0, 4] {
        assert!(machine.m3g_byte_array_prefix(wrong, maximum).is_err());
    }
}

#[test]
fn primitive_array_transfers_preserve_signed_values_and_validate_before_writes() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let bytes = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 4)
        .unwrap();
    let shorts = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Short, 4)
        .unwrap();
    machine
        .m3g_write_primitive_int_array(bytes, &ArrayKind::Byte, &[i8::MIN, -1, 0, i8::MAX])
        .unwrap();
    machine
        .m3g_write_primitive_int_array(shorts, &ArrayKind::Short, &[i16::MIN, -1, 0, i16::MAX])
        .unwrap();
    assert_eq!(machine.m3g_byte_array(bytes).unwrap(), [128, 255, 0, 127]);
    assert_eq!(
        machine
            .m3g_primitive_int_array(shorts, &ArrayKind::Short, usize::MAX, |value| value as i16)
            .unwrap(),
        [i16::MIN, -1, 0, i16::MAX]
    );
    assert!(
        machine
            .m3g_write_primitive_int_array(bytes, &ArrayKind::Byte, &[1; 5])
            .is_err()
    );
    assert!(
        machine
            .m3g_write_primitive_int_array(shorts, &ArrayKind::Short, &[1; 5])
            .is_err()
    );
    assert!(
        machine
            .m3g_write_primitive_int_array(bytes, &ArrayKind::Short, &[1])
            .is_err()
    );
    assert!(
        machine
            .m3g_write_primitive_int_array(shorts, &ArrayKind::Byte, &[1])
            .is_err()
    );
    assert_eq!(machine.m3g_byte_array(bytes).unwrap(), [128, 255, 0, 127]);
    assert_eq!(
        machine
            .m3g_primitive_int_array(shorts, &ArrayKind::Short, usize::MAX, |value| value as i16)
            .unwrap(),
        [i16::MIN, -1, 0, i16::MAX]
    );
}

#[test]
fn transform_accepts_an_empty_vector_array() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Transform", HashMap::new())
        .unwrap();
    machine
        .m3g
        .runtime
        .create(
            Some(guest.to_raw()),
            m3g::ObjectKind::Transform(m3g::Mat4::IDENTITY),
        )
        .unwrap();
    let empty = float_array(&mut machine, 0, &[]);
    let outcome = machine
        .invoke_m3g_scene_graph_native(
            "javax/microedition/m3g/Transform",
            "transform",
            "([F)V",
            &[Value::Reference(Some(guest)), Value::Reference(Some(empty))],
        )
        .unwrap();
    assert!(matches!(outcome, CallOutcome::Return(None)));
    assert_eq!(machine.heap.managed.array_length(empty).unwrap(), 0);
}

#[test]
fn singular_relative_transform_throws_arithmetic_exception_without_changing_output() {
    let program = crate::machine::tests::program_with_exception("java/lang/ArithmeticException");
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let nodes: Vec<_> = (0..2)
        .map(|_| {
            let guest = machine
                .heap
                .managed
                .allocate_object("javax/microedition/m3g/Group", HashMap::new())
                .unwrap();
            let native = machine
                .m3g
                .runtime
                .create(
                    Some(guest.to_raw()),
                    m3g::ObjectKind::Group {
                        node: m3g::NodeState::default(),
                        children: Vec::new(),
                    },
                )
                .unwrap();
            (guest, native)
        })
        .collect();
    machine
        .m3g
        .runtime
        .add_child(nodes[0].1, nodes[1].1)
        .unwrap();
    machine
        .m3g
        .runtime
        .set_scale(nodes[1].1, m3g::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let output = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Transform", HashMap::new())
        .unwrap();
    let original = m3g::Mat4::translation(3.0, 2.0, 1.0).unwrap();
    let output_native = machine
        .m3g
        .runtime
        .create(Some(output.to_raw()), m3g::ObjectKind::Transform(original))
        .unwrap();
    let outcome = machine
        .invoke_m3g_scene_graph_native(
            "javax/microedition/m3g/Node",
            "getTransformTo",
            "(Ljavax/microedition/m3g/Node;Ljavax/microedition/m3g/Transform;)Z",
            &[
                Value::Reference(Some(nodes[0].0)),
                Value::Reference(Some(nodes[1].0)),
                Value::Reference(Some(output)),
            ],
        )
        .unwrap();
    let CallOutcome::Throw(exception) = outcome else {
        panic!("expected ArithmeticException");
    };
    assert_eq!(
        machine.object_class(exception).unwrap(),
        "java/lang/ArithmeticException"
    );
    assert_eq!(
        machine.m3g.runtime.transform_value(output_native).unwrap(),
        original
    );
}

#[test]
fn morph_weights_ignore_unused_source_values_and_preserve_state_on_short_input() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/MorphingMesh", HashMap::new())
        .unwrap();
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
    let native = machine
        .m3g
        .runtime
        .create(
            Some(guest.to_raw()),
            m3g::ObjectKind::MorphingMesh {
                mesh: m3g::MeshState {
                    node: m3g::NodeState::default(),
                    vertices,
                    submeshes: Vec::new(),
                    appearances: Vec::new(),
                },
                targets: vec![vertices; 2],
                weights: vec![0.0; 2],
            },
        )
        .unwrap();
    let invoke = |machine: &mut Machine<'_, '_>, source| {
        machine.invoke_m3g_mesh_native(
            "javax/microedition/m3g/MorphingMesh",
            "setWeights",
            "([F)V",
            &[
                Value::Reference(Some(guest)),
                Value::Reference(Some(source)),
            ],
        )
    };
    let source = float_array(&mut machine, 65_536, &[0.5, -0.25, f32::NAN, f32::INFINITY]);
    assert!(matches!(
        invoke(&mut machine, source).unwrap(),
        CallOutcome::Return(None)
    ));
    let short = float_array(&mut machine, 1, &[1.0]);
    assert_eq!(
        invoke(&mut machine, short).err().unwrap().code(),
        "illegal-argument-exception"
    );
    let m3g::ObjectKind::MorphingMesh { weights, .. } = machine.m3g.runtime.kind(native).unwrap()
    else {
        panic!("expected a morphing mesh");
    };
    assert_eq!(weights, &[0.5, -0.25]);
}

#[test]
fn fixed_float_inputs_accept_extra_values_and_preserve_shape_validation() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let source = float_array(&mut machine, 65_536, &[1.0, 2.0, 3.0, f32::NAN]);
    let mut values = [0.0; 3];
    machine.m3g_read_float_array(source, &mut values).unwrap();
    assert_eq!(values, [1.0, 2.0, 3.0]);
    let short = float_array(&mut machine, 2, &[1.0, 2.0]);
    let wrong_type = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 3)
        .unwrap();
    for invalid in [short, wrong_type] {
        assert_eq!(
            machine
                .m3g_read_float_array(invalid, &mut values)
                .unwrap_err()
                .code(),
            "illegal-argument-exception"
        );
    }

    let guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Transform", HashMap::new())
        .unwrap();
    let native = machine
        .m3g
        .runtime
        .create(
            Some(guest.to_raw()),
            m3g::ObjectKind::Transform(m3g::Mat4::IDENTITY),
        )
        .unwrap();
    let expected = m3g::Mat4::translation(2.0, 3.0, 4.0).unwrap();
    let source = float_array(&mut machine, 65_536, &expected.to_row_major());
    machine
        .heap
        .managed
        .array_set(source, 16, HeapValue::Float(f32::NAN))
        .unwrap();
    let invoke = |machine: &mut Machine<'_, '_>, source| {
        machine.invoke_m3g_scene_graph_native(
            "javax/microedition/m3g/Transform",
            "set",
            "([F)V",
            &[
                Value::Reference(Some(guest)),
                Value::Reference(Some(source)),
            ],
        )
    };
    assert!(matches!(
        invoke(&mut machine, source).unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(
        machine.m3g.runtime.transform_value(native).unwrap(),
        expected
    );
    let short = float_array(&mut machine, 15, &[0.0; 15]);
    let Err(error) = invoke(&mut machine, short) else {
        panic!("short transform source must be rejected");
    };
    assert_eq!(error.code(), "illegal-argument-exception");
    assert_eq!(
        machine.m3g.runtime.transform_value(native).unwrap(),
        expected
    );
}

#[test]
fn keyframe_native_consumes_exactly_the_declared_components() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for components in 1..=16 {
        let guest = machine
            .heap
            .managed
            .allocate_object("javax/microedition/m3g/KeyframeSequence", HashMap::new())
            .unwrap();
        let sequence =
            m3g::KeyframeSequenceState::new(1, components, m3g::Interpolation::Linear).unwrap();
        let native = machine
            .m3g
            .runtime
            .create(
                Some(guest.to_raw()),
                m3g::ObjectKind::KeyframeSequence(sequence),
            )
            .unwrap();
        let values = (0..components)
            .map(|component| component as f32 + 0.5)
            .collect::<Vec<_>>();
        let source = float_array(&mut machine, 128, &values);
        machine
            .heap
            .managed
            .array_set(source, components as i32, HeapValue::Float(f32::NAN))
            .unwrap();
        let invoke = |machine: &mut Machine<'_, '_>, source| {
            machine.invoke_m3g_animation_native(
                "javax/microedition/m3g/KeyframeSequence",
                "setKeyframe",
                "(II[F)V",
                &[
                    Value::Reference(Some(guest)),
                    Value::Int(0),
                    Value::Int(7),
                    Value::Reference(Some(source)),
                ],
            )
        };
        assert!(matches!(
            invoke(&mut machine, source).unwrap(),
            CallOutcome::Return(None)
        ));
        let short = float_array(&mut machine, components - 1, &values[..components - 1]);
        assert!(invoke(&mut machine, short).is_err());
        let m3g::ObjectKind::KeyframeSequence(sequence) = machine.m3g.runtime.kind(native).unwrap()
        else {
            panic!("expected a keyframe sequence");
        };
        assert_eq!(sequence.keyframe(0).unwrap(), (7, values.as_slice()));
    }
}
