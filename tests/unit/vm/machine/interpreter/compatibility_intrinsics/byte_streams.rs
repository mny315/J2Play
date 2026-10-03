use super::*;

mod output;
mod primitives;
mod reader;

const BYTE_STREAM: &str = "java/io/ByteArrayInputStream";

#[test]
fn data_input_read_utf_cancels_between_intrinsic_byte_reads() {
    use std::{cell::Cell, rc::Rc};
    let program = program(false);
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(2));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at,
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let mut bytes = vec![b'A'; usize::from(u16::MAX) + 2];
    bytes[..2].copy_from_slice(&u16::MAX.to_be_bytes());
    let stream = byte_stream_bytes(&mut machine, BYTE_STREAM, &bytes);
    let input = machine
        .heap
        .managed
        .allocate_object(
            "java/io/DataInputStream",
            HashMap::from([(
                "java/io/DataInputStream.in:Ljava/io/InputStream;".into(),
                HeapValue::Reference(Some(stream)),
            )]),
        )
        .unwrap();
    let method = &program.methods[&MethodKey {
        class: "java/io/DataInputStream".into(),
        name: "readUTF".into(),
        descriptor: "(Ljava/io/DataInput;)Ljava/lang/String;".into(),
    }];
    let result = machine.call(method, [Value::Reference(Some(input))], 1);
    let Err(error) = result else {
        panic!("readUTF ignored cancellation during intrinsic byte reads");
    };
    assert_eq!(error.code(), "execution-cancelled");
    assert_eq!(checks.get(), 2);
    let consumed = machine
        .graphics_int_field(stream, "java/io/ByteArrayInputStream.pos:I")
        .unwrap();
    assert!((2..=2_050).contains(&consumed), "consumed {consumed} bytes");
}

#[test]
fn data_input_read_utf_decodes_cldc_groups_and_preserves_the_following_byte() {
    let program = program(false);
    let read_utf = &program.methods[&MethodKey {
        class: "java/io/DataInputStream".into(),
        name: "readUTF".into(),
        descriptor: "()Ljava/lang/String;".into(),
    }];
    let read_byte = &program.methods[&MethodKey {
        class: "java/io/DataInputStream".into(),
        name: "readUnsignedByte".into(),
        descriptor: "()I".into(),
    }];
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for (payload, expected) in [
        (&[0xc1, 0x81, 0xe0, 0x80, 0x80][..], Some(vec![65, 0])),
        (
            &[0xed, 0xa0, 0x81, 0xed, 0xb0, 0x80],
            Some(vec![0xd801, 0xdc00]),
        ),
        (&[0xc0, 0x80, 0, b'A'], Some(vec![0, 0, 65])),
        (&[0xc1], None),
        (&[0xe0, 0x80], None),
        (&[0xe0, 0x80, 0xff], None),
        (&[0xf0, 0x90, 0x80, 0x80], None),
    ] {
        let length = u16::try_from(payload.len()).unwrap().to_be_bytes();
        let bytes = length
            .into_iter()
            .chain(payload.iter().copied())
            .chain([0x55])
            .collect::<Vec<_>>();
        let stream = byte_stream_bytes(&mut machine, BYTE_STREAM, &bytes);
        let input = machine
            .heap
            .managed
            .allocate_object(
                "java/io/DataInputStream",
                HashMap::from([(
                    "java/io/DataInputStream.in:Ljava/io/InputStream;".into(),
                    HeapValue::Reference(Some(stream)),
                )]),
            )
            .unwrap();
        let result = machine
            .call(read_utf, [Value::Reference(Some(input))], 1)
            .unwrap();
        match (result, expected) {
            (CallOutcome::Return(Some(Value::Reference(Some(string)))), Some(expected)) => {
                assert_eq!(machine.heap.string_values[&string], expected)
            }
            (CallOutcome::Throw(exception), None) => assert_eq!(
                machine.object_class(exception).unwrap(),
                "java/io/UTFDataFormatException"
            ),
            _ => panic!("unexpected DataInputStream.readUTF outcome for {payload:?}"),
        }
        assert!(matches!(
            machine
                .call(read_byte, [Value::Reference(Some(input))], 1)
                .unwrap(),
            CallOutcome::Return(Some(Value::Int(0x55)))
        ));
    }
}

fn program(override_read: bool) -> Program {
    let mut program = Program::new();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(1, 1) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    program.classes.insert(
        "test/ByteStream".into(),
        Class {
            super_name: Some(BYTE_STREAM.into()),
            fields: Vec::new(),
            ..program.classes[BYTE_STREAM].clone()
        },
    );
    if override_read {
        let method = runtime_method(
            "test/ByteStream",
            "read",
            "()I",
            &[0x10, 9, 0xac],
            1,
            1,
            vec![None],
            false,
        );
        program.methods.insert(method.key.clone(), method);
    }
    program
}

fn byte_stream(machine: &mut Machine<'_, '_>, class: &str) -> Handle {
    byte_stream_bytes(machine, class, &[0x11, 0x22, 0x33])
}

fn byte_stream_bytes(machine: &mut Machine<'_, '_>, class: &str, bytes: &[u8]) -> Handle {
    let buffer = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, i32::try_from(bytes.len()).unwrap())
        .unwrap();
    for (index, byte) in bytes.iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(buffer, index as i32, HeapValue::Int(i32::from(*byte)))
            .unwrap();
    }
    machine
        .heap
        .managed
        .allocate_object(
            class,
            HashMap::from([
                (
                    format!("{BYTE_STREAM}.buf:[B"),
                    HeapValue::Reference(Some(buffer)),
                ),
                (format!("{BYTE_STREAM}.pos:I"), HeapValue::Int(0)),
                (
                    format!("{BYTE_STREAM}.count:I"),
                    HeapValue::Int(i32::try_from(bytes.len()).unwrap()),
                ),
            ]),
        )
        .unwrap()
}

#[test]
fn data_input_fast_reads_respect_overrides_and_keep_inherited_reads_fast() {
    for (class, override_read) in [
        (BYTE_STREAM, false),
        ("test/ByteStream", false),
        ("test/ByteStream", true),
    ] {
        for (name, expected, consumed) in [("read", 0x11, 1), ("readUnsignedShort", 0x1122, 2)] {
            let program = program(override_read);
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let input = byte_stream(&mut machine, class);
            assert_eq!(
                machine
                    .byte_array_input_read_is_unmodified(input, "()I")
                    .unwrap(),
                !override_read
            );
            let stream = machine
                .heap
                .managed
                .allocate_object(
                    "java/io/DataInputStream",
                    HashMap::from([(
                        "java/io/DataInputStream.in:Ljava/io/InputStream;".into(),
                        HeapValue::Reference(Some(input)),
                    )]),
                )
                .unwrap();
            let method = program.methods[&MethodKey {
                class: "java/io/DataInputStream".into(),
                name: name.into(),
                descriptor: "()I".into(),
            }]
                .clone();
            let result = machine
                .call(&method, [Value::Reference(Some(stream))], 1)
                .unwrap();
            let expected = if override_read {
                if name == "read" { 9 } else { 0x0909 }
            } else {
                expected
            };
            assert!(
                matches!(result, CallOutcome::Return(Some(Value::Int(value))) if value == expected)
            );
            assert_eq!(
                machine
                    .graphics_int_field(input, &format!("{BYTE_STREAM}.pos:I"))
                    .unwrap(),
                if override_read { 0 } else { consumed }
            );
        }
    }
}

#[test]
fn byte_array_bulk_read_reports_eof_even_for_zero_length_in_both_execution_paths() {
    // CLDC 1.1 ByteArrayInputStream.read checks pos == count before choosing
    // min(len, count - pos). Bounds/null checks still precede EOF handling.
    let program = program(false);
    for intrinsic in [false, true] {
        let mut method = program.methods[&MethodKey {
            class: BYTE_STREAM.into(),
            name: "read".into(),
            descriptor: "([BII)I".into(),
        }]
            .clone();
        method.compatibility_candidate = Some(intrinsic);
        for (position, length, expected) in [(0, 0, 0), (0, 1, 1), (3, 0, -1), (3, 1, -1)] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let stream = byte_stream(&mut machine, BYTE_STREAM);
            machine
                .heap
                .managed
                .set_field(
                    stream,
                    &format!("{BYTE_STREAM}.pos:I"),
                    HeapValue::Int(position),
                )
                .unwrap();
            let destination = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Byte, 1)
                .unwrap();
            machine
                .heap
                .managed
                .array_set(destination, 0, HeapValue::Int(99))
                .unwrap();
            let outcome = machine
                .call(
                    &method,
                    [
                        Value::Reference(Some(stream)),
                        Value::Reference(Some(destination)),
                        Value::Int(0),
                        Value::Int(length),
                    ],
                    1,
                )
                .unwrap();
            assert!(
                matches!(outcome, CallOutcome::Return(Some(Value::Int(value))) if value == expected)
            );
            assert_eq!(
                machine.heap.managed.array_get(destination, 0).unwrap(),
                HeapValue::Int(if expected == 1 { 0x11 } else { 99 })
            );
            assert_eq!(
                machine
                    .graphics_int_field(stream, &format!("{BYTE_STREAM}.pos:I"))
                    .unwrap(),
                position + expected.max(0)
            );
        }
    }
}

#[test]
fn byte_array_bulk_read_rejects_overflowing_protected_bounds_without_panicking() {
    let program = program(false);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let stream = byte_stream(&mut machine, "test/ByteStream");
    machine
        .heap
        .managed
        .set_field(
            stream,
            &format!("{BYTE_STREAM}.pos:I"),
            HeapValue::Int(i32::MIN),
        )
        .unwrap();
    machine
        .heap
        .managed
        .set_field(
            stream,
            &format!("{BYTE_STREAM}.count:I"),
            HeapValue::Int(i32::MAX),
        )
        .unwrap();
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 1)
        .unwrap();
    assert!(
        machine
            .byte_array_input_read_slice(&[
                Value::Reference(Some(stream)),
                Value::Reference(Some(destination)),
                Value::Int(0),
                Value::Int(1),
            ])
            .is_err()
    );
    assert_eq!(
        machine.heap.managed.array_get(destination, 0).unwrap(),
        HeapValue::Int(0)
    );
}

fn array_reader(name: &str) -> Method {
    runtime_method(
        "test/ArrayReader",
        "readBytes",
        "(Ljava/io/InputStream;I[B)[B",
        &[
            0x1b, 0x9d, 0, 5, 0x2c, 0xb0, // nonpositive length returns the fallback
            0x1b, 0xbc, 8, 0x4d, 3, 0x3e, // allocate byte[length], index = 0
            0x1d, 0x1b, 0xa2, 0, 16, 0x2c, 0x1d, 0x2a, 0xb6, 0, 6, 0x54, 0x84, 3, 1, 0xa7, 0xff,
            0xf1, 0x2c, 0xb0,
        ],
        3,
        4,
        vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("java/io/InputStream".into())),
            Some(Constant::NameAndType {
                name_index: 4,
                descriptor_index: 5,
            }),
            Some(Constant::Utf8(name.into())),
            Some(Constant::Utf8("()I".into())),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 3,
            }),
        ],
        true,
    )
}

#[test]
fn guest_bytewise_reads_preserve_symbols_overrides_and_eof() {
    for (override_read, override_bulk, symbol, expected) in [
        (false, false, "read", [0x11, 0x22, 0x33, -1, -1]),
        (true, false, "read", [9; 5]),
        (false, true, "read", [0x11, 0x22, 0x33, -1, -1]),
        (false, false, "available", [3; 5]),
    ] {
        let mut program = program(override_read);
        if override_bulk {
            let bulk = runtime_method(
                "test/ByteStream",
                "read",
                "([BII)I",
                &[3, 0xac],
                1,
                4,
                vec![None],
                false,
            );
            program.methods.insert(bulk.key.clone(), bulk);
        }
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let input = byte_stream(&mut machine, "test/ByteStream");
        let method = array_reader(symbol);
        let outcome = machine
            .call(
                &method,
                [
                    Value::Reference(Some(input)),
                    Value::Int(5),
                    Value::Reference(None),
                ],
                1,
            )
            .unwrap();
        let CallOutcome::Return(Some(Value::Reference(Some(output)))) = outcome else {
            panic!("expected byte array");
        };
        let values = (0..5)
            .map(|index| machine.heap.managed.array_get(output, index).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values, expected.map(HeapValue::Int));
        assert!(machine.execution.instructions > 0);
    }
}

#[test]
fn guest_bytewise_reads_keep_null_fallback_and_execute_handlers() {
    let program = program(false);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let mut method = array_reader("read");
    for length in [0, -1] {
        let outcome = machine
            .call(
                &method,
                [
                    Value::Reference(None),
                    Value::Int(length),
                    Value::Reference(None),
                ],
                1,
            )
            .unwrap();
        assert!(matches!(
            outcome,
            CallOutcome::Return(Some(Value::Reference(None)))
        ));
    }
    method.exception_table = Arc::new(vec![ExceptionHandler {
        start_pc: 0,
        end_pc: 30,
        handler_pc: 30,
        catch_type: 0,
    }]);
    let outcome = machine
        .call(
            &method,
            [
                Value::Reference(None),
                Value::Int(1),
                Value::Reference(None),
            ],
            1,
        )
        .unwrap();
    let CallOutcome::Return(Some(Value::Reference(Some(output)))) = outcome else {
        panic!("the guest handler must return its partially filled array");
    };
    assert_eq!(machine.heap.managed.array_length(output).unwrap(), 1);
    assert_eq!(
        machine.heap.managed.array_get(output, 0).unwrap(),
        HeapValue::Int(0)
    );
}

#[test]
fn byte_stream_fast_reads_match_bootstrap_after_subclass_changes_protected_state() {
    let program = program(false);
    for bulk in [false, true] {
        for (position, count, null_buffer) in [
            (-1, 3, false),
            (i32::MIN, i32::MAX, false),
            (0, 5, false),
            (3, 5, false),
            (0, 3, true),
            (3, 3, true),
        ] {
            let requests: &[i32] = if bulk { &[0, 1, 4] } else { &[0] };
            for &requested in requests {
                let mut results = Vec::new();
                for intrinsic in [false, true] {
                    let mut method = program.methods[&MethodKey {
                        class: BYTE_STREAM.into(),
                        name: "read".into(),
                        descriptor: if bulk { "([BII)I" } else { "()I" }.into(),
                    }]
                        .clone();
                    method.compatibility_candidate = Some(intrinsic);
                    let mut host = DefaultNativeContext;
                    let mut machine = program.machine(Limits::default(), false, &mut host);
                    let stream = byte_stream(&mut machine, "test/ByteStream");
                    for (name, value) in [("pos", position), ("count", count)] {
                        machine
                            .heap
                            .managed
                            .set_field(
                                stream,
                                &format!("{BYTE_STREAM}.{name}:I"),
                                Value::Int(value),
                            )
                            .unwrap();
                    }
                    if null_buffer {
                        machine
                            .heap
                            .managed
                            .set_field(
                                stream,
                                &format!("{BYTE_STREAM}.buf:[B"),
                                Value::Reference(None),
                            )
                            .unwrap();
                    }
                    let destination = machine
                        .heap
                        .managed
                        .allocate_array(ArrayKind::Byte, 4)
                        .unwrap();
                    let mut args = vec![Value::Reference(Some(stream))];
                    if bulk {
                        args.extend([
                            Value::Reference(Some(destination)),
                            Value::Int(0),
                            Value::Int(requested),
                        ]);
                    }
                    let result = match machine.call(&method, &args, 1) {
                        Ok(CallOutcome::Return(Some(Value::Int(value)))) => {
                            format!("return:{value}")
                        }
                        Ok(CallOutcome::Throw(handle)) => {
                            machine.object_class(handle).unwrap().into_owned()
                        }
                        Err(error) => format!("vm-error:{}", error.code()),
                        _ => panic!("unexpected stream result"),
                    };
                    let cursor = machine
                        .graphics_int_field(stream, &format!("{BYTE_STREAM}.pos:I"))
                        .unwrap();
                    let output: Vec<_> = (0..4)
                        .map(|i| machine.heap.managed.array_get(destination, i).unwrap())
                        .collect();
                    results.push((result, cursor, output));
                }
                assert_eq!(
                    results[0], results[1],
                    "bulk={bulk}, pos={position}, count={count}, null={null_buffer}, requested={requested}"
                );
            }
        }
    }
}

#[test]
fn bulk_reads_into_the_stream_buffer_preserve_overlapping_copy_semantics() {
    let program = program(false);
    for (position, offset, length) in [(0, 1, 2), (1, 0, 2), (0, 0, 3), (3, 0, 0)] {
        let mut results = Vec::new();
        for intrinsic in [false, true] {
            let mut method = program.methods[&MethodKey {
                class: BYTE_STREAM.into(),
                name: "read".into(),
                descriptor: "([BII)I".into(),
            }]
                .clone();
            method.compatibility_candidate = Some(intrinsic);
            let mut host = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut host);
            let stream = byte_stream(&mut machine, BYTE_STREAM);
            let buffer = machine
                .graphics_reference_field(stream, "java/io/ByteArrayInputStream.buf:[B")
                .unwrap();
            machine
                .heap
                .managed
                .set_field(
                    stream,
                    "java/io/ByteArrayInputStream.pos:I",
                    Value::Int(position),
                )
                .unwrap();
            let CallOutcome::Return(Some(Value::Int(copied))) = machine
                .call(
                    &method,
                    [
                        Value::Reference(Some(stream)),
                        Value::Reference(Some(buffer)),
                        Value::Int(offset),
                        Value::Int(length),
                    ],
                    1,
                )
                .unwrap()
            else {
                panic!("bulk read must return a byte count");
            };
            let cursor = machine
                .graphics_int_field(stream, "java/io/ByteArrayInputStream.pos:I")
                .unwrap();
            let bytes: Vec<_> = (0..3)
                .map(|i| machine.heap.managed.array_get(buffer, i).unwrap())
                .collect();
            results.push((copied, cursor, bytes));
        }
        assert_eq!(
            results[0], results[1],
            "pos={position}, offset={offset}, len={length}"
        );
    }
}

#[test]
#[ignore = "manual byte stream throughput measurement"]
fn byte_array_bulk_read_throughput() {
    let program = program(false);
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let stream = byte_stream(&mut machine, BYTE_STREAM);
    let buffer = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 4096)
        .unwrap();
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 4096)
        .unwrap();
    machine
        .heap
        .managed
        .set_field(
            stream,
            &format!("{BYTE_STREAM}.buf:[B"),
            Value::Reference(Some(buffer)),
        )
        .unwrap();
    machine
        .heap
        .managed
        .set_field(stream, &format!("{BYTE_STREAM}.count:I"), Value::Int(4096))
        .unwrap();
    for length in [0, 1, 64, 4096] {
        let started = std::time::Instant::now();
        for _ in 0..20_000 {
            machine
                .heap
                .managed
                .set_field(stream, "java/io/ByteArrayInputStream.pos:I", Value::Int(0))
                .unwrap();
            let copied = machine
                .byte_array_input_read_slice(&[
                    Value::Reference(Some(stream)),
                    Value::Reference(Some(destination)),
                    Value::Int(0),
                    Value::Int(std::hint::black_box(length)),
                ])
                .unwrap();
            assert_eq!(copied, length);
        }
        eprintln!("bulk bytes={length} elapsed={:?}", started.elapsed());
    }
}
