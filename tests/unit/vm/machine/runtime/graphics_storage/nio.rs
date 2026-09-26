//! NIO shared storage, view offsets, allocation and cancellation contracts.

use super::*;

#[test]
fn nio_overflow_can_be_caught_by_guest_code_and_retried() {
    for (class, view_name, array_kind, element) in [
        ("java/nio/IntBuffer", "asIntBuffer", ArrayKind::Int, "I"),
        (
            "java/nio/FloatBuffer",
            "asFloatBuffer",
            ArrayKind::Float,
            "F",
        ),
    ] {
        let mut program = program_with_core_natives();
        program.classes.insert(
            "test/BufferCaller".into(),
            test_class_definition(Some("java/lang/Object")),
        );
        let mut caller = runtime_method(
            "test/BufferCaller",
            "put",
            &format!("(L{class};[{element})I"),
            &[0x2a, 0x2b, 0xb6, 0, 1, 0x57, 0x03, 0xac, 0x57, 0x04, 0xac],
            2,
            2,
            vec![
                None,
                Some(Constant::Methodref {
                    class_index: 2,
                    name_and_type_index: 3,
                }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType {
                    name_index: 5,
                    descriptor_index: 6,
                }),
                Some(Constant::Utf8(class.into())),
                Some(Constant::Utf8("put".into())),
                Some(Constant::Utf8(format!("([{element})L{class};"))),
                Some(Constant::Class { name_index: 8 }),
                Some(Constant::Utf8("java/nio/BufferOverflowException".into())),
            ],
            true,
        );
        Arc::make_mut(&mut caller.exception_table).push(ExceptionHandler {
            start_pc: 0,
            end_pc: 6,
            handler_pc: 8,
            catch_type: 7,
        });
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let bytes = nio_reference_call(
            &mut machine,
            "java/nio/ByteBuffer",
            "allocateDirect",
            "(I)Ljava/nio/ByteBuffer;",
            &[Value::Int(4)],
        );
        let view = nio_reference_call(
            &mut machine,
            "java/nio/ByteBuffer",
            view_name,
            &format!("()L{class};"),
            &[Value::Reference(Some(bytes))],
        );
        let source = machine.heap.managed.allocate_array(array_kind, 1).unwrap();
        for (rewind, expected) in [(false, 0), (false, 1), (true, 0)] {
            if rewind {
                nio_reference_call(
                    &mut machine,
                    "java/nio/Buffer",
                    "rewind",
                    "()Ljava/nio/Buffer;",
                    &[Value::Reference(Some(view))],
                );
            }
            let outcome = machine
                .call(
                    &caller,
                    [Value::Reference(Some(view)), Value::Reference(Some(source))],
                    1,
                )
                .unwrap();
            assert!(
                matches!(outcome, CallOutcome::Return(Some(Value::Int(result))) if result == expected)
            );
            assert_eq!(
                machine
                    .graphics_int_field(view, "java/nio/Buffer.position:I")
                    .unwrap(),
                1
            );
        }
    }
}

#[test]
fn nio_overflow_exception_has_a_working_no_argument_constructor() {
    let program = program_with_core_natives();
    let constructor = &program.methods[&MethodKey {
        class: "java/nio/BufferOverflowException".into(),
        name: "<init>".into(),
        descriptor: "()V".into(),
    }];
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let exception = machine
        .allocate_native_instance("java/nio/BufferOverflowException", &[])
        .unwrap();
    assert!(matches!(
        machine
            .call(constructor, [Value::Reference(Some(exception))], 1)
            .unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(
        machine
            .heap
            .managed
            .field(
                exception,
                "java/lang/Throwable.detailMessage:Ljava/lang/String;"
            )
            .unwrap(),
        HeapValue::Reference(None)
    );
}

#[test]
fn nio_bulk_operations_and_pointer_access_honor_cancellation() {
    let program = program_with_core_natives();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let bytes = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "allocateDirect",
        "(I)Ljava/nio/ByteBuffer;",
        &[Value::Int(32_768)],
    );
    let view = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "asFloatBuffer",
        "()Ljava/nio/FloatBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    let source = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, 8_192)
        .unwrap();
    for index in 0..8_192 {
        machine
            .heap
            .managed
            .array_set(source, index, HeapValue::Float(1.0))
            .unwrap();
    }
    for operation in 0..3 {
        let run = |machine: &mut Machine<'_, '_>| {
            machine
                .heap
                .managed
                .set_field(view, "java/nio/Buffer.position:I", HeapValue::Int(0))
                .unwrap();
            match operation {
                0 => machine.nio_put_values(view, source, &ArrayKind::Float),
                1 => {
                    let pointer = machine.jsr239_float_pointer(view, 2, 0)?;
                    machine
                        .jsr239_float_pointer_words(&pointer, 0..pointer.components())
                        .map(|_| ())
                }
                _ => machine.jsr239_upload_texture(64, 128, bytes),
            }
        };
        checks.set(0);
        cancel_at.set(usize::MAX);
        run(&mut machine).unwrap();
        let total = checks.get();
        if operation == 1 {
            assert_eq!(
                total, 1,
                "borrowing a GL pointer must not scan its contents"
            );
        } else {
            assert!(total > 4, "long NIO operation did not poll during its copy");
        }
        for point in 1..=total {
            checks.set(0);
            cancel_at.set(point);
            assert_eq!(run(&mut machine).unwrap_err().code(), "execution-cancelled");
            checks.set(0);
            cancel_at.set(usize::MAX);
            run(&mut machine).unwrap();
        }
    }
    assert_eq!(nio_float_values(&machine, view).unwrap(), vec![1.0; 8_192]);
}

#[test]
fn nio_views_share_storage_and_keep_independent_positions() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let bytes = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "allocateDirect",
        "(I)Ljava/nio/ByteBuffer;",
        &[Value::Int(12)],
    );
    let first = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "asFloatBuffer",
        "()Ljava/nio/FloatBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    let floats = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, 3)
        .unwrap();
    for (index, value) in (0..).zip([1.0, -2.0, 0.5]) {
        machine
            .heap
            .managed
            .array_set(floats, index, HeapValue::Float(value))
            .unwrap();
    }
    assert_eq!(
        nio_reference_call(
            &mut machine,
            "java/nio/FloatBuffer",
            "put",
            "([F)Ljava/nio/FloatBuffer;",
            &[
                Value::Reference(Some(first)),
                Value::Reference(Some(floats))
            ],
        ),
        first
    );
    let second = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "asFloatBuffer",
        "()Ljava/nio/FloatBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    assert_eq!(
        nio_float_values(&machine, second).unwrap(),
        [1.0, -2.0, 0.5]
    );
    assert_eq!(
        machine
            .graphics_int_field(bytes, "java/nio/Buffer.position:I")
            .unwrap(),
        0
    );
    assert_eq!(
        machine
            .graphics_int_field(first, "java/nio/Buffer.position:I")
            .unwrap(),
        3
    );
    assert_eq!(
        machine
            .graphics_int_field(second, "java/nio/Buffer.position:I")
            .unwrap(),
        0
    );

    nio_reference_call(
        &mut machine,
        "java/nio/Buffer",
        "rewind",
        "()Ljava/nio/Buffer;",
        &[Value::Reference(Some(first))],
    );
    let ints = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "asIntBuffer",
        "()Ljava/nio/IntBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    let source = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 3)
        .unwrap();
    let expected = [3.0_f32, 4.0, -0.0];
    for (index, value) in (0..).zip(expected) {
        machine
            .heap
            .managed
            .array_set(source, index, HeapValue::Int(value.to_bits().cast_signed()))
            .unwrap();
    }
    nio_reference_call(
        &mut machine,
        "java/nio/IntBuffer",
        "put",
        "([I)Ljava/nio/IntBuffer;",
        &[Value::Reference(Some(ints)), Value::Reference(Some(source))],
    );
    for view in [first, second] {
        assert_eq!(
            nio_float_values(&machine, view)
                .unwrap()
                .into_iter()
                .map(f32::to_bits)
                .collect::<Vec<_>>(),
            expected.map(f32::to_bits)
        );
    }
    machine.collect_heap(vec![second]);
    assert!(machine.heap.managed.get(bytes).is_err());
    assert_eq!(
        nio_float_values(&machine, second)
            .unwrap()
            .into_iter()
            .map(f32::to_bits)
            .collect::<Vec<_>>(),
        expected.map(f32::to_bits)
    );
}

#[test]
fn nio_texture_upload_reads_shared_bytes_without_requiring_an_int_view() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let bytes = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "allocateDirect",
        "(I)Ljava/nio/ByteBuffer;",
        &[Value::Int(4)],
    );
    machine.jsr239_upload_texture(1, 1, bytes).unwrap();
    assert_eq!(jsr239_texture(&machine, 0).shade(0.0, 0.0, u32::MAX), 0);
    let first = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "asIntBuffer",
        "()Ljava/nio/IntBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    let source = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(
            source,
            0,
            HeapValue::Int(i32::from_ne_bytes([0x12, 0x34, 0x56, 0x78])),
        )
        .unwrap();
    nio_reference_call(
        &mut machine,
        "java/nio/IntBuffer",
        "put",
        "([I)Ljava/nio/IntBuffer;",
        &[
            Value::Reference(Some(first)),
            Value::Reference(Some(source)),
        ],
    );
    let second = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "asIntBuffer",
        "()Ljava/nio/IntBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    machine.jsr239_upload_texture(1, 1, bytes).unwrap();
    assert_eq!(
        jsr239_texture(&machine, 0).shade(0.0, 0.0, u32::MAX),
        0x7812_3456
    );
    machine
        .heap
        .managed
        .array_set(
            source,
            0,
            HeapValue::Int(i32::from_ne_bytes([0xab, 0xcd, 0xef, 0xff])),
        )
        .unwrap();
    nio_reference_call(
        &mut machine,
        "java/nio/IntBuffer",
        "put",
        "([I)Ljava/nio/IntBuffer;",
        &[
            Value::Reference(Some(second)),
            Value::Reference(Some(source)),
        ],
    );
    machine.jsr239_upload_texture(1, 1, bytes).unwrap();
    assert_eq!(
        jsr239_texture(&machine, 0).shade(0.0, 0.0, u32::MAX),
        0xffab_cdef
    );
    // A failed upload must preserve the previous texture and its budget.
    machine
        .heap
        .managed
        .set_field(bytes, "java/nio/Buffer.position:I", HeapValue::Int(1))
        .unwrap();
    let retained = machine.jsr239.texture_bytes;
    assert_eq!(
        machine
            .jsr239_upload_texture(1, 1, bytes)
            .unwrap_err()
            .code(),
        "illegal-argument-exception"
    );
    assert_eq!(machine.jsr239.texture_bytes, retained);
    assert_eq!(
        jsr239_texture(&machine, 0).shade(0.0, 0.0, u32::MAX),
        0xffab_cdef
    );
}

#[test]
fn nio_failed_allocation_does_not_retain_a_temporary_gc_root() {
    let program = program_with_core_natives();
    let method = &program.methods[&MethodKey {
        class: "java/nio/ByteBuffer".into(),
        name: "allocateDirect".into(),
        descriptor: "(I)Ljava/nio/ByteBuffer;".into(),
    }];
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 88,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    for _ in 0..3 {
        // The 64-byte array fits; allocation of its wrapper must fail.
        let error = machine
            .invoke_nio_native(method, &[Value::Int(64)])
            .err()
            .unwrap();
        assert_eq!(error.code(), MANAGED_HEAP_LIMIT_CODE);
        assert_eq!(machine.heap.managed.bytes(), 88);
        assert!(machine.heap.temporary_roots.is_empty());
        let roots = machine.roots(&[], &[]);
        machine.collect_heap(roots);
        assert_eq!(machine.heap.managed.bytes(), 0);
    }
}

#[test]
fn nio_views_fit_without_allocating_a_second_backing_array() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 8_192,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let bytes = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "allocateDirect",
        "(I)Ljava/nio/ByteBuffer;",
        &[Value::Int(4_096)],
    );
    let first = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "asFloatBuffer",
        "()Ljava/nio/FloatBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    let second = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "asIntBuffer",
        "()Ljava/nio/IntBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    machine.collect_heap(vec![first, second]);
    assert_eq!(nio_float_values(&machine, first).unwrap(), vec![0.0; 1_024]);
    assert_eq!(
        machine.nio_buffer_bytes(second, 4).unwrap(),
        vec![HeapValue::Int(0); 4_096]
    );
}

#[test]
fn nio_views_honor_byte_offsets_and_reject_overflow_before_writing() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let bytes = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "allocateDirect",
        "(I)Ljava/nio/ByteBuffer;",
        &[Value::Int(10)],
    );
    machine
        .heap
        .managed
        .set_field(bytes, "java/nio/Buffer.position:I", HeapValue::Int(1))
        .unwrap();
    let view = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "asIntBuffer",
        "()Ljava/nio/IntBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    assert_eq!(
        machine
            .graphics_int_field(view, "java/nio/Buffer.capacity:I")
            .unwrap(),
        2
    );
    let source = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 2)
        .unwrap();
    for (index, value) in (0..).zip([0x1234_5678, 0x1020_3040]) {
        machine
            .heap
            .managed
            .array_set(source, index, HeapValue::Int(value))
            .unwrap();
    }
    machine
        .nio_put_values(view, source, &ArrayKind::Int)
        .unwrap();
    let expected = [0x1234_5678_i32, 0x1020_3040]
        .into_iter()
        .flat_map(i32::to_ne_bytes)
        .chain([0])
        .map(|byte| HeapValue::Int(i32::from(byte.cast_signed())))
        .collect::<Vec<_>>();
    assert_eq!(machine.nio_buffer_bytes(bytes, 1).unwrap(), expected);
    assert!(machine.nio_buffer_bytes(view, 4).unwrap().is_empty());
    assert_eq!(
        machine
            .nio_put_values(view, source, &ArrayKind::Int)
            .unwrap_err()
            .code(),
        "buffer-overflow"
    );
    assert_eq!(machine.nio_buffer_bytes(bytes, 1).unwrap(), expected);
    assert_eq!(
        machine
            .graphics_int_field(view, "java/nio/Buffer.position:I")
            .unwrap(),
        2
    );
}
