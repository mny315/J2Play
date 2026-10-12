use super::m3g_support::object;
use super::*;

fn floats(machine: &mut Machine<'_, '_>, values: &[f32]) -> Handle {
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, values.len() as i32)
        .unwrap();
    for (index, value) in values.iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(array, index as i32, HeapValue::Float(*value))
            .unwrap();
    }
    array
}

#[test]
fn vertex_buffer_can_disconnect_positions_with_an_empty_bias_array() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let mut state = m3g::VertexBufferState::default();
    state
        .set_positions(
            Some(m3g::VertexArrayState::new(1, 3, m3g::VertexComponent::Short).unwrap()),
            1.0,
            [0.0; 3],
        )
        .unwrap();
    let (guest, native) = object(
        &mut machine,
        "javax/microedition/m3g/VertexBuffer",
        m3g::ObjectKind::VertexBuffer {
            state,
            arrays: [None; 5],
        },
    );
    let bias = floats(&mut machine, &[]);
    machine
        .invoke_m3g_object_native(
            "javax/microedition/m3g/VertexBuffer",
            "setPositions",
            "(Ljavax/microedition/m3g/VertexArray;F[F)V",
            &[
                Value::Reference(Some(guest)),
                Value::Reference(None),
                Value::Float(1.0),
                Value::Reference(Some(bias)),
            ],
        )
        .unwrap();
    assert_eq!(
        machine
            .m3g
            .runtime
            .resolved_vertex_buffer(native)
            .unwrap()
            .vertex_count(),
        0
    );
}

#[test]
fn vertex_buffer_position_getter_checks_destination_length_even_without_positions() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (guest, _) = object(
        &mut machine,
        "javax/microedition/m3g/VertexBuffer",
        m3g::ObjectKind::VertexBuffer {
            state: m3g::VertexBufferState::default(),
            arrays: [None; 5],
        },
    );
    let output = floats(&mut machine, &[99.0; 3]);
    let error = machine
        .invoke_m3g_object_native(
            "javax/microedition/m3g/VertexBuffer",
            "getPositions",
            "([F)Ljavax/microedition/m3g/VertexArray;",
            &[
                Value::Reference(Some(guest)),
                Value::Reference(Some(output)),
            ],
        )
        .err()
        .unwrap();
    assert_eq!(
        java_error_class(&error),
        Some("java/lang/IllegalArgumentException")
    );
    assert_eq!(
        machine.heap.managed.array_get(output, 0).unwrap(),
        HeapValue::Float(99.0)
    );
}

#[test]
fn vertex_buffer_scale_bias_transfers_preserve_suffixes_for_positions_and_texture_dimensions() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (buffer, _) = object(
        &mut machine,
        "javax/microedition/m3g/VertexBuffer",
        m3g::ObjectKind::VertexBuffer {
            state: m3g::VertexBufferState::default(),
            arrays: [None; 5],
        },
    );
    for (positions, components, output_length) in [
        (true, 3, 4),
        (true, 3, 5),
        (false, 2, 3),
        (false, 2, 5),
        (false, 3, 4),
        (false, 3, 5),
    ] {
        let (array, _) = object(
            &mut machine,
            "javax/microedition/m3g/VertexArray",
            m3g::ObjectKind::VertexArray(
                m3g::VertexArrayState::new(1, components, m3g::VertexComponent::Short).unwrap(),
            ),
        );
        let bias = floats(&mut machine, &[10.0, 20.0, 30.0][..components]);
        let mut args = vec![Value::Reference(Some(buffer))];
        if !positions {
            args.push(Value::Int(0));
        }
        args.extend([
            Value::Reference(Some(array)),
            Value::Float(2.0),
            Value::Reference(Some(bias)),
        ]);
        machine
            .invoke_m3g_object_native(
                "javax/microedition/m3g/VertexBuffer",
                if positions {
                    "setPositions"
                } else {
                    "setTexCoords"
                },
                if positions {
                    "(Ljavax/microedition/m3g/VertexArray;F[F)V"
                } else {
                    "(ILjavax/microedition/m3g/VertexArray;F[F)V"
                },
                &args,
            )
            .unwrap();
        let output = floats(&mut machine, &vec![99.0; output_length]);
        let mut args = vec![Value::Reference(Some(buffer))];
        if !positions {
            args.push(Value::Int(0));
        }
        args.push(Value::Reference(Some(output)));
        let result = machine
            .invoke_m3g_object_native(
                "javax/microedition/m3g/VertexBuffer",
                if positions {
                    "getPositions"
                } else {
                    "getTexCoords"
                },
                if positions {
                    "([F)Ljavax/microedition/m3g/VertexArray;"
                } else {
                    "(I[F)Ljavax/microedition/m3g/VertexArray;"
                },
                &args,
            )
            .unwrap();
        assert!(
            matches!(result, CallOutcome::Return(Some(Value::Reference(Some(value)))) if value == array)
        );
        for (index, expected) in [
            2.0,
            10.0,
            20.0,
            if components == 3 { 30.0 } else { 99.0 },
            99.0,
        ]
        .into_iter()
        .take(output_length)
        .enumerate()
        {
            assert_eq!(
                machine
                    .heap
                    .managed
                    .array_get(output, index as i32)
                    .unwrap(),
                HeapValue::Float(expected)
            );
        }
    }
}

#[test]
pub(crate) fn m3g_triangle_strip_array_accepts_an_unused_index_suffix() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/TriangleStripArray", HashMap::new())
        .unwrap();
    let indices = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 4)
        .unwrap();
    for (index, value) in [0, 1, 2, 2].into_iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(indices, index as i32, HeapValue::Int(value))
            .unwrap();
    }
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
    let constructor = runtime_method(
        "javax/microedition/m3g/TriangleStripArray",
        "<init>",
        "([I[I)V",
        &[0xb1],
        0,
        3,
        Vec::new(),
        false,
    );

    assert!(matches!(
        machine
            .invoke_m3g_native(
                &constructor,
                &[
                    Value::Reference(Some(guest)),
                    Value::Reference(Some(indices)),
                    Value::Reference(Some(lengths)),
                ],
                1,
            )
            .unwrap(),
        CallOutcome::Return(None)
    ));
    let native = machine.m3g_handle(guest).unwrap();
    let m3g::ObjectKind::TriangleStripArray(state) = machine.m3g.runtime.kind(native).unwrap()
    else {
        panic!("constructor did not create a triangle strip array");
    };
    assert_eq!(state.indices(), [0, 1, 2]);
}
