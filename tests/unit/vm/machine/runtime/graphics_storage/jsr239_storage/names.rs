use super::*;

#[test]
fn jsr239_texture_names_exclude_bound_names_and_wrap_without_duplicates() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let gl = machine
        .allocate_native_instance("javax/microedition/khronos/opengles/GLImpl", &[])
        .unwrap();
    machine.jsr239.gl = Some(gl);
    for texture in [1, 3, i32::MAX, -1] {
        gl_set(&mut machine, "glBindTexture", &[0x0de1, texture]);
    }
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 5)
        .unwrap();
    for (start, expected) in [
        (1, [2, 4, 5]),
        (i32::MAX, [i32::MIN, i32::MIN + 1, i32::MIN + 2]),
        (-1, [6, 7, 8]),
    ] {
        machine.jsr239.next_texture = start;
        assert!(matches!(
            jsr239_call(
                &mut machine,
                "glGenTextures",
                "(I[II)V",
                &[
                    Value::Reference(Some(gl)),
                    Value::Int(3),
                    Value::Reference(Some(destination)),
                    Value::Int(1),
                ],
            )
            .unwrap(),
            CallOutcome::Return(None)
        ));
        for (index, value) in [0].into_iter().chain(expected).chain([0]).enumerate() {
            assert_eq!(
                machine
                    .heap
                    .managed
                    .array_get(destination, i32::try_from(index).unwrap())
                    .unwrap(),
                HeapValue::Int(value)
            );
        }
    }
}

#[test]
fn jsr239_texture_name_generation_validates_the_entire_destination_before_writing() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let gl = machine
        .allocate_native_instance("javax/microedition/khronos/opengles/GLImpl", &[])
        .unwrap();
    machine.jsr239.gl = Some(gl);
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 3)
        .unwrap();
    for (count, offset, array) in [
        (4, 0, Some(destination)),
        (2, 2, Some(destination)),
        (1, i32::MAX, Some(destination)),
        (i32::MAX, 1, Some(destination)),
        (1, -1, Some(destination)),
        (0, 4, Some(destination)),
        (1, 0, None),
    ] {
        for index in 0..3 {
            machine
                .heap
                .managed
                .array_set(destination, index, HeapValue::Int(0x3456))
                .unwrap();
        }
        let next = machine.jsr239.next_texture;
        let outcome = jsr239_call(
            &mut machine,
            "glGenTextures",
            "(I[II)V",
            &[
                Value::Reference(Some(gl)),
                Value::Int(count),
                Value::Reference(array),
                Value::Int(offset),
            ],
        )
        .unwrap();
        let CallOutcome::Throw(exception) = outcome else {
            panic!("invalid destination must throw before changing texture names");
        };
        assert_eq!(
            machine.object_class(exception).unwrap(),
            "java/lang/IllegalArgumentException"
        );
        assert_eq!(machine.jsr239.next_texture, next);
        for index in 0..3 {
            assert_eq!(
                machine.heap.managed.array_get(destination, index).unwrap(),
                HeapValue::Int(0x3456)
            );
        }
    }
    assert!(matches!(
        jsr239_call(
            &mut machine,
            "glGenTextures",
            "(I[II)V",
            &[
                Value::Reference(Some(gl)),
                Value::Int(0),
                Value::Reference(Some(destination)),
                Value::Int(3),
            ],
        )
        .unwrap(),
        CallOutcome::Return(None)
    ));
}

#[test]
fn jsr239_texture_name_reservations_are_bounded_and_rebinding_keeps_the_slot() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut limits = Limits::default();
    limits.m3g_arena.objects = 1;
    let mut machine = program.machine(limits, false, &mut context);
    let gl = machine
        .allocate_native_instance("javax/microedition/khronos/opengles/GLImpl", &[])
        .unwrap();
    machine.jsr239.gl = Some(gl);
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    let generate = [
        Value::Reference(Some(gl)),
        Value::Int(1),
        Value::Reference(Some(destination)),
        Value::Int(0),
    ];
    assert!(matches!(
        jsr239_call(&mut machine, "glGenTextures", "(I[II)V", &generate).unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(
        machine.heap.managed.array_get(destination, 0).unwrap(),
        HeapValue::Int(1)
    );
    assert!(machine.jsr239.textures[&1].image.is_none());
    let next = machine.jsr239.next_texture;
    assert_eq!(
        jsr239_call(&mut machine, "glGenTextures", "(I[II)V", &generate)
            .err()
            .unwrap()
            .code(),
        "resource-limit"
    );
    assert_eq!(machine.jsr239.next_texture, next);
    assert_eq!(
        machine.heap.managed.array_get(destination, 0).unwrap(),
        HeapValue::Int(1)
    );
    for texture in [1, 0, 1] {
        gl_set(&mut machine, "glBindTexture", &[0x0de1, texture]);
    }
    assert_eq!(
        jsr239_call(
            &mut machine,
            "glBindTexture",
            "(II)V",
            &[
                Value::Reference(Some(gl)),
                Value::Int(0x0de1),
                Value::Int(2),
            ]
        )
        .err()
        .unwrap()
        .code(),
        "resource-limit"
    );
    assert_eq!(machine.jsr239.bound_texture, 1);
    let invalid = jsr239_call(
        &mut machine,
        "glBindTexture",
        "(II)V",
        &[Value::Reference(Some(gl)), Value::Int(0), Value::Int(0)],
    )
    .unwrap();
    assert!(
        matches!(invalid, CallOutcome::Throw(exception) if machine.object_class(exception).unwrap() == "java/lang/IllegalArgumentException")
    );
    assert_eq!(machine.jsr239.bound_texture, 1);
    assert_eq!(machine.jsr239.textures.len(), 1);

    let bytes = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "allocateDirect",
        "(I)Ljava/nio/ByteBuffer;",
        &[Value::Int(4)],
    );
    for _ in 0..2 {
        machine.jsr239_upload_texture(1, 1, bytes).unwrap();
        assert_eq!(machine.jsr239.textures.len(), 1);
        assert_eq!(machine.jsr239.texture_bytes, 4);
    }
}

#[test]
fn jsr239_texture_name_generation_can_be_cancelled_without_reserving_or_writing_names() {
    let program = program_with_core_natives();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut limits = Limits::default();
    limits.m3g_arena.objects = 8_192;
    let mut machine = program.machine(limits, false, &mut context);
    let gl = machine
        .allocate_native_instance("javax/microedition/khronos/opengles/GLImpl", &[])
        .unwrap();
    machine.jsr239.gl = Some(gl);
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 4_098)
        .unwrap();
    let generate = [
        Value::Reference(Some(gl)),
        Value::Int(4_096),
        Value::Reference(Some(destination)),
        Value::Int(1),
    ];
    for point in 1..=4 {
        checks.set(0);
        cancel_at.set(point);
        assert_eq!(
            jsr239_call(&mut machine, "glGenTextures", "(I[II)V", &generate)
                .err()
                .unwrap()
                .code(),
            "execution-cancelled"
        );
        assert_eq!(checks.get(), point);
        assert_eq!(machine.jsr239.next_texture, 1);
        assert!(machine.jsr239.textures.is_empty());
        assert!(
            machine
                .graphics_int_array_mut(destination)
                .unwrap()
                .iter()
                .all(|value| *value == HeapValue::Int(0))
        );
    }
    checks.set(0);
    cancel_at.set(usize::MAX);
    assert!(matches!(
        jsr239_call(&mut machine, "glGenTextures", "(I[II)V", &generate).unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(checks.get(), 4);
    assert_eq!(machine.jsr239.textures.len(), 4_096);
    assert_eq!(
        machine.heap.managed.array_get(destination, 0).unwrap(),
        HeapValue::Int(0)
    );
    for index in 1..=4_096 {
        assert_eq!(
            machine.heap.managed.array_get(destination, index).unwrap(),
            HeapValue::Int(index)
        );
    }
    assert_eq!(
        machine.heap.managed.array_get(destination, 4_097).unwrap(),
        HeapValue::Int(0)
    );
}
