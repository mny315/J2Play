use super::*;

#[test]
pub(crate) fn bytecode_static_array_accessors_preserve_nested_calls_and_suspension() {
    fn push_member(
        constants: &mut Vec<Option<Constant>>,
        class: u16,
        name: &str,
        descriptor: &str,
        field: bool,
    ) -> u16 {
        let name_and_type_index = push_name_and_type(constants, name, descriptor);
        if field {
            push_constant(
                constants,
                Constant::Fieldref {
                    class_index: class,
                    name_and_type_index,
                },
            )
        } else {
            push_constant(
                constants,
                Constant::Methodref {
                    class_index: class,
                    name_and_type_index,
                },
            )
        }
    }
    let mut constants = vec![None];
    let class = push_class(&mut constants, "PackedCells");
    let matrix = push_member(&mut constants, class, "matrix", "[[I", true);
    let mut bit_code = Vec::new();
    reference(&mut bit_code, 0xb2, matrix);
    bit_code.extend_from_slice(&[
        0x1a, 0x32, 0x10, 15, 0x2e, 0x1b, 0x7e, 0x99, 0, 5, 0x04, 0xac, 0x03, 0xac,
    ]);
    let mut bit_test = runtime_method(
        "PackedCells",
        "hasFlag",
        "(II)Z",
        &bit_code,
        2,
        2,
        constants,
        true,
    );

    let mut constants = vec![None];
    let class = push_class(&mut constants, "PackedCells");
    let matrix = push_member(&mut constants, class, "matrix", "[[I", true);
    let limits = push_member(&mut constants, class, "limits", "[I", true);
    let bit_ref = push_member(&mut constants, class, "hasFlag", "(II)Z", false);
    let mut mirror_code = vec![0x1a, 0x10, 8];
    reference(&mut mirror_code, 0xb8, bit_ref);
    mirror_code.extend_from_slice(&[0x99, 0, 20]);
    reference(&mut mirror_code, 0xb2, matrix);
    mirror_code.extend_from_slice(&[0x1a, 0x32, 0x10, 6, 0x2e]);
    reference(&mut mirror_code, 0xb2, limits);
    mirror_code.extend_from_slice(&[0x04, 0x2e, 0x64, 0x1b, 0x64, 0xac, 0x1b, 0xac]);
    let mut mirror = runtime_method(
        "PackedCells",
        "mirror",
        "(II)I",
        &mirror_code,
        3,
        2,
        constants,
        true,
    );

    let mut constants = vec![None];
    let class = push_class(&mut constants, "PackedCells");
    let cube = push_member(&mut constants, class, "cube", "[[[B", true);
    let wide_reader = push_member(&mut constants, class, "readWide", "([BI)S", false);
    let mask = push_constant(&mut constants, Constant::Integer(65_535));
    let mask = u8::try_from(mask).unwrap();
    let mut cube_code = vec![0x1d, 0x99, 0, 20];
    reference(&mut cube_code, 0xb2, cube);
    cube_code.extend_from_slice(&[0x1a, 0x32, 0x1b, 0x32, 0x1c, 0x04, 0x78]);
    reference(&mut cube_code, 0xb8, wide_reader);
    cube_code.extend_from_slice(&[0x12, mask, 0x7e, 0xac]);
    reference(&mut cube_code, 0xb2, cube);
    cube_code.extend_from_slice(&[
        0x1a, 0x32, 0x1b, 0x32, 0x1c, 0x33, 0x11, 0, 0xff, 0x7e, 0xac,
    ]);
    let mut cube_accessor = runtime_method(
        "PackedCells",
        "cell",
        "(IIIZ)I",
        &cube_code,
        3,
        4,
        constants,
        true,
    );

    let mut constants = vec![None];
    let class = push_class(&mut constants, "PackedCells");
    let dimensions = push_member(&mut constants, class, "matrix", "[[I", true);
    let cube_ref = push_member(&mut constants, class, "cell", "(IIIZ)I", false);
    let mut flat_code = vec![0x1a, 0x1b, 0x1c, 0x1d, 0x03, 0x3b, 0x3e, 0x3d, 0x3c, 0x3b];
    reference(&mut flat_code, 0xb2, dimensions);
    flat_code.extend_from_slice(&[
        0x1a, 0x32, 0x05, 0x2e, 0x36, 4, 0x1a, 0x1b, 0x1d, 0x15, 4, 0x68, 0x1c, 0x60, 0x03,
    ]);
    reference(&mut flat_code, 0xb8, cube_ref);
    flat_code.push(0xac);
    let mut flat_accessor = runtime_method(
        "PackedCells",
        "flatCell",
        "(IIII)I",
        &flat_code,
        5,
        5,
        constants,
        true,
    );

    let mut constants = vec![None];
    let class = push_class(&mut constants, "PackedCells");
    let mirror_ref = push_member(&mut constants, class, "mirror", "(II)I", false);
    let flat_ref = push_member(&mut constants, class, "flatCell", "(IIII)I", false);
    let mut mapped_code = vec![0x1a, 0x1c];
    reference(&mut mapped_code, 0xb8, mirror_ref);
    mapped_code.extend_from_slice(&[0x3d, 0x1a, 0x03, 0x1b, 0x1c]);
    reference(&mut mapped_code, 0xb8, flat_ref);
    mapped_code.extend_from_slice(&[0x59, 0x3b, 0xac]);
    let mut mapped_accessor = runtime_method(
        "PackedCells",
        "mappedCell",
        "(III)I",
        &mapped_code,
        4,
        3,
        constants,
        true,
    );

    let mut wide_reader = runtime_method(
        "PackedCells",
        "readWide",
        "([BI)S",
        &[0x2a, 0x1b, 0x33, 0x93, 0xac],
        2,
        2,
        vec![None],
        true,
    );
    let program = bytecode_program(&mut [
        &mut bit_test,
        &mut mirror,
        &mut cube_accessor,
        &mut flat_accessor,
        &mut mapped_accessor,
        &mut wide_reader,
    ]);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);

    machine.initialize_class("PackedCells", 1).unwrap();
    let matrix_row = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 16)
        .unwrap();
    for (index, value) in [(2, 3), (6, 10), (15, 8)] {
        machine
            .heap
            .managed
            .array_set(matrix_row, index, HeapValue::Int(value))
            .unwrap();
    }
    let matrix = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Reference("[I".into()), 1)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(matrix, 0, HeapValue::Reference(Some(matrix_row)))
        .unwrap();
    let limits = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 2)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(limits, 1, HeapValue::Int(2))
        .unwrap();
    let bytes = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 30)
        .unwrap();
    for (index, value) in [(0, -2), (7, -5), (23, -7)] {
        machine
            .heap
            .managed
            .array_set(bytes, index, HeapValue::Int(value))
            .unwrap();
    }
    let plane = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Reference("[B".into()), 1)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(plane, 0, HeapValue::Reference(Some(bytes)))
        .unwrap();
    let cube = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Reference("[[B".into()), 1)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(cube, 0, HeapValue::Reference(Some(plane)))
        .unwrap();
    machine.classes.static_fields.extend([
        (
            "PackedCells.matrix:[[I".to_owned(),
            Value::Reference(Some(matrix)),
        ),
        (
            "PackedCells.limits:[I".to_owned(),
            Value::Reference(Some(limits)),
        ),
        (
            "PackedCells.cube:[[[B".to_owned(),
            Value::Reference(Some(cube)),
        ),
    ]);

    macro_rules! result {
        ($method:expr, $args:expr) => {{
            let outcome = machine.call($method, $args, 1).unwrap();
            let CallOutcome::Return(Some(Value::Int(value))) = outcome else {
                panic!("structural array accessor did not return an int");
            };
            value
        }};
    }
    assert_eq!(result!(&bit_test, &[Value::Int(0), Value::Int(8)]), 1);
    assert_eq!(result!(&bit_test, &[Value::Int(0), Value::Int(4)]), 0);
    assert_eq!(result!(&mirror, &[Value::Int(0), Value::Int(3)]), 5);
    assert_eq!(
        result!(
            &cube_accessor,
            &[Value::Int(0), Value::Int(0), Value::Int(7), Value::Int(0),]
        ),
        251
    );
    assert_eq!(
        result!(
            &cube_accessor,
            &[Value::Int(0), Value::Int(0), Value::Int(0), Value::Int(1),]
        ),
        65_534
    );
    assert_eq!(
        result!(
            &flat_accessor,
            &[Value::Int(0), Value::Int(0), Value::Int(1), Value::Int(2),]
        ),
        251
    );
    assert_eq!(
        result!(
            &mapped_accessor,
            &[Value::Int(0), Value::Int(2), Value::Int(1)]
        ),
        249
    );
    assert!(matches!(
        machine
            .call(
                &cube_accessor,
                [Value::Int(0), Value::Int(0), Value::Int(30), Value::Int(0),],
                1
            )
            .unwrap(),
        CallOutcome::Throw(_)
    ));

    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = worker.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Running);
    machine.scheduler.quantum_remaining = 0;
    let CallOutcome::Suspend(child) = machine
        .call(
            &cube_accessor,
            [Value::Int(0), Value::Int(0), Value::Int(0), Value::Int(1)],
            1,
        )
        .unwrap()
    else {
        panic!("wide accessor must preserve a suspended guest reader");
    };
    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    assert!(matches!(
        machine.resume_suspended_call(child, 1).unwrap(),
        CallOutcome::Return(Some(Value::Int(65_534)))
    ));

    assert!(machine.execution.instructions > 0);
}
