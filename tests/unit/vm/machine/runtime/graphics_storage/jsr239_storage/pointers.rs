use super::*;

#[test]
fn jsr239_pointers_retain_only_active_buffers_and_read_only_requested_components() {
    let program = program_with_core_natives();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let gl = machine
        .allocate_native_instance("javax/microedition/khronos/opengles/GLImpl", &[])
        .unwrap();
    machine.jsr239.gl = Some(gl);
    let mut buffers = Vec::new();
    for name in ["glVertexPointer", "glTexCoordPointer", "glVertexPointer"] {
        let bytes = nio_reference_call(
            &mut machine,
            "java/nio/ByteBuffer",
            "allocateDirect",
            "(I)Ljava/nio/ByteBuffer;",
            &[Value::Int(32_768)],
        );
        let buffer = nio_reference_call(
            &mut machine,
            "java/nio/ByteBuffer",
            "asFloatBuffer",
            "()Ljava/nio/FloatBuffer;",
            &[Value::Reference(Some(bytes))],
        );
        let method = &program.methods[&MethodKey {
            class: "javax/microedition/khronos/opengles/GLImpl".into(),
            name: name.into(),
            descriptor: "(IIILjava/nio/Buffer;)V".into(),
        }];
        checks.set(0);
        cancel_at.set(1);
        assert!(matches!(
            machine
                .invoke_jsr239_native(
                    method,
                    &[
                        Value::Reference(Some(gl)),
                        Value::Int(2),
                        Value::Int(0x1406),
                        Value::Int(0),
                        Value::Reference(Some(buffer))
                    ]
                )
                .unwrap(),
            CallOutcome::Return(None)
        ));
        assert_eq!(checks.get(), 0, "binding must not copy the buffer contents");
        cancel_at.set(usize::MAX);
        assert_eq!(
            machine
                .jsr239_float_pointer(bytes, 2, 0)
                .unwrap_err()
                .code(),
            "illegal-argument-exception"
        );
        buffers.push(buffer);
        let roots = machine.roots(&[], &[]);
        machine.collect_heap(roots);
        let pointer = if name == "glVertexPointer" {
            machine.jsr239.vertex_pointer.as_ref().unwrap()
        } else {
            machine.jsr239.texture_pointer.as_ref().unwrap()
        };
        checks.set(0);
        assert_eq!(
            nio_float_words(machine.jsr239_float_pointer_words(pointer, 2..8).unwrap()).unwrap(),
            [0.0; 6]
        );
        assert_eq!(
            checks.get(),
            1,
            "a short draw must not scan the rest of the large buffer"
        );
        assert_eq!(
            machine
                .jsr239_float_pointer_words(pointer, 8_191..8_193)
                .unwrap_err()
                .code(),
            "illegal-argument-exception"
        );
        let invalid = machine
            .invoke_jsr239_native(
                method,
                &[
                    Value::Reference(Some(gl)),
                    Value::Int(2),
                    Value::Int(0x1404),
                    Value::Int(0),
                    Value::Reference(Some(buffer)),
                ],
            )
            .unwrap();
        assert!(
            matches!(invalid, CallOutcome::Throw(exception) if machine.object_class(exception).unwrap() == "java/lang/IllegalArgumentException")
        );
        let pointer = if name == "glVertexPointer" {
            machine.jsr239.vertex_pointer.as_ref().unwrap()
        } else {
            machine.jsr239.texture_pointer.as_ref().unwrap()
        };
        assert_eq!(pointer.buffer, buffer);
    }
    assert!(machine.heap.managed.get(buffers[0]).is_err());
    assert!(machine.heap.managed.get(buffers[1]).is_ok());
    assert!(machine.heap.managed.get(buffers[2]).is_ok());
}

#[test]
fn jsr239_draw_reads_updated_buffer_data_at_the_captured_position() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let bytes = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "allocateDirect",
        "(I)Ljava/nio/ByteBuffer;",
        &[Value::Int(32)],
    );
    let view = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "asFloatBuffer",
        "()Ljava/nio/FloatBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    machine
        .heap
        .managed
        .set_field(view, "java/nio/Buffer.position:I", HeapValue::Int(2))
        .unwrap();
    let gl = machine
        .allocate_native_instance("javax/microedition/khronos/opengles/GLImpl", &[])
        .unwrap();
    machine.jsr239.gl = Some(gl);
    jsr239_call(
        &mut machine,
        "glEnableClientState",
        "(I)V",
        &[Value::Reference(Some(gl)), Value::Int(0x8074)],
    )
    .unwrap();
    let pointer_method = &program.methods[&MethodKey {
        class: "javax/microedition/khronos/opengles/GLImpl".into(),
        name: "glVertexPointer".into(),
        descriptor: "(IIILjava/nio/Buffer;)V".into(),
    }];
    machine
        .invoke_jsr239_native(
            pointer_method,
            &[
                Value::Reference(Some(gl)),
                Value::Int(2),
                Value::Int(0x1406),
                Value::Int(0),
                Value::Reference(Some(view)),
            ],
        )
        .unwrap();
    let pixels = jsr239_canvas_target(&mut machine);
    let source = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, 8)
        .unwrap();
    for (coordinates, visible) in [
        ([99.0, 99.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0], true),
        ([99.0; 8], false),
    ] {
        for (index, value) in (0..).zip(coordinates) {
            machine
                .heap
                .managed
                .array_set(source, index, HeapValue::Float(value))
                .unwrap();
        }
        nio_reference_call(
            &mut machine,
            "java/nio/Buffer",
            "rewind",
            "()Ljava/nio/Buffer;",
            &[Value::Reference(Some(view))],
        );
        nio_reference_call(
            &mut machine,
            "java/nio/FloatBuffer",
            "put",
            "([F)Ljava/nio/FloatBuffer;",
            &[Value::Reference(Some(view)), Value::Reference(Some(source))],
        );
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(0));
        machine.jsr239_draw_arrays(5, 0, 3).unwrap();
        assert_eq!(
            machine
                .graphics_int_array_snapshot(pixels)
                .unwrap()
                .iter()
                .any(|&pixel| pixel != 0),
            visible
        );
        let roots = machine.roots(&[Some(Value::Reference(Some(source)))], &[]);
        machine.collect_heap(roots);
        assert!(
            machine.heap.managed.get(view).is_ok(),
            "the active GL pointer must retain its buffer"
        );
    }
}
