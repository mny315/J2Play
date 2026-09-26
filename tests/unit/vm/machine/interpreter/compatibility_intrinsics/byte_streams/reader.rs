use super::*;

mod skip;

fn finish_reader_call(machine: &mut Machine<'_, '_>, mut outcome: CallOutcome) -> CallOutcome {
    let mut yields = 0;
    while let CallOutcome::Suspend(continuation) = outcome {
        assert!(yields < 256, "reader failed to make progress");
        let mut roots = machine.roots(&[], &[]);
        continuation.roots(&mut roots);
        machine.collect_heap(roots);
        machine.scheduler.quantum_remaining = 1;
        outcome = machine.resume_suspended_call(continuation, 1).unwrap();
        yields += 1;
    }
    outcome
}

#[test]
fn character_reader_resumes_partial_characters_and_bulk_reads() {
    let mut program = program(false);
    let read = runtime_method(
        "test/ByteStream",
        "read",
        "()I",
        &[0xb8, 0, 7, 0x2a, 0xb7, 0, 1, 0xac],
        1,
        1,
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
            Some(Constant::Utf8(BYTE_STREAM.into())),
            Some(Constant::Utf8("read".into())),
            Some(Constant::Utf8("()I".into())),
            Some(Constant::Methodref {
                class_index: 8,
                name_and_type_index: 9,
            }),
            Some(Constant::Class { name_index: 10 }),
            Some(Constant::NameAndType {
                name_index: 11,
                descriptor_index: 12,
            }),
            Some(Constant::Utf8("java/lang/System".into())),
            Some(Constant::Utf8("gc".into())),
            Some(Constant::Utf8("()V".into())),
        ],
        false,
    );
    program.methods.insert(read.key.clone(), read);
    for (encoding, bytes, expected) in [
        (
            "UTF-8",
            &[0, 0xc3, 0xa9, 0xf0, 0x9f, 0x99, 0x82, 0xc2, b'!', 0xff][..],
            &[0, 0xe9, 0xd83d, 0xde42, 0xfffd, 33, 0xfffd][..],
        ),
        (
            "UTF-16BE",
            &[0, 65, 0xd8, 0x3d, 0xde, 0x42, 0],
            &[65, 0xd83d, 0xde42, 0xfffd],
        ),
        (
            "UTF-16",
            &[0xff, 0xfe, 65, 0, 0x3d, 0xd8, 0x42, 0xde, 0],
            &[65, 0xd83d, 0xde42, 0xfffd],
        ),
        (
            "UTF-16",
            &[0xfe, 0xff, 0, 65, 0xd8, 0x3d, 0xde, 0x42],
            &[65, 0xd83d, 0xde42],
        ),
        ("ISO-8859-1", &[0, 0xe9, 0xff], &[0, 0xe9, 0xff]),
        ("US-ASCII", &[65, 0x80, 66], &[65, 0xfffd, 66]),
    ] {
        for quantum in [0, 1, 2, 3, 4, 64] {
            for bulk in [false, true] {
                let method = &program.methods[&MethodKey {
                    class: "java/io/InputStreamReader".into(),
                    name: "read".into(),
                    descriptor: if bulk { "([CII)I" } else { "()I" }.into(),
                }];
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let input = byte_stream_bytes(&mut machine, "test/ByteStream", bytes);
                let reader = character_reader(&mut machine, input);
                let encoding_name = machine.allocate_dynamic_string(encoding, &[], &[]).unwrap();
                machine
                    .input_stream_reader_init(
                        &[
                            Value::Reference(Some(reader)),
                            Value::Reference(Some(input)),
                            Value::Reference(Some(encoding_name)),
                        ],
                        Some(2),
                    )
                    .unwrap();
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
                machine.scheduler.quantum_remaining = quantum;
                if bulk {
                    let length = i32::try_from(expected.len()).unwrap();
                    let destination = machine
                        .heap
                        .managed
                        .allocate_array(ArrayKind::Char, length + 3)
                        .unwrap();
                    for index in 0..length + 3 {
                        machine
                            .heap
                            .managed
                            .array_set(destination, index, HeapValue::Int(0x1234))
                            .unwrap();
                    }
                    let outcome = machine
                        .call(
                            method,
                            [
                                Value::Reference(Some(reader)),
                                Value::Reference(Some(destination)),
                                Value::Int(1),
                                Value::Int(length + 1),
                            ],
                            1,
                        )
                        .unwrap();
                    assert!(
                        matches!(finish_reader_call(&mut machine, outcome), CallOutcome::Return(Some(Value::Int(count))) if count == length)
                    );
                    for (index, &unit) in (1..).zip(expected) {
                        assert_eq!(
                            machine.heap.managed.array_get(destination, index).unwrap(),
                            HeapValue::Int(unit)
                        );
                    }
                    for index in [0, length + 1, length + 2] {
                        assert_eq!(
                            machine.heap.managed.array_get(destination, index).unwrap(),
                            HeapValue::Int(0x1234)
                        );
                    }
                } else {
                    for &unit in expected.iter().chain([-1, -1].iter()) {
                        let outcome = machine
                            .call(method, [Value::Reference(Some(reader))], 1)
                            .unwrap();
                        assert!(
                            matches!(finish_reader_call(&mut machine, outcome), CallOutcome::Return(Some(Value::Int(value))) if value == unit),
                            "encoding={encoding} quantum={quantum} expected={unit}"
                        );
                    }
                }
                assert_eq!(
                    machine
                        .graphics_int_field(input, "java/io/ByteArrayInputStream.pos:I")
                        .unwrap(),
                    i32::try_from(bytes.len()).unwrap()
                );
            }
        }
    }
}

#[test]
fn character_reader_only_recovers_io_exceptions_after_partial_reads() {
    for (exception_class, recoverable) in [
        ("java/io/IOException", true),
        ("java/io/EOFException", true),
        ("java/lang/RuntimeException", false),
        ("java/lang/Error", false),
        ("java/lang/OutOfMemoryError", false),
    ] {
        let mut program = program(false);
        let read = runtime_method(
            "test/ByteStream",
            "read",
            "()I",
            &[
                0x2a, 0xb7, 0, 1, 0x59, 0x9a, 0, 12, 0x57, 0xbb, 0, 7, 0x59, 0xb7, 0, 9, 0xbf, 0xac,
            ],
            2,
            1,
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
                Some(Constant::Utf8(BYTE_STREAM.into())),
                Some(Constant::Utf8("read".into())),
                Some(Constant::Utf8("()I".into())),
                Some(Constant::Class { name_index: 8 }),
                Some(Constant::Utf8(exception_class.into())),
                Some(Constant::Methodref {
                    class_index: 7,
                    name_and_type_index: 10,
                }),
                Some(Constant::NameAndType {
                    name_index: 11,
                    descriptor_index: 12,
                }),
                Some(Constant::Utf8("<init>".into())),
                Some(Constant::Utf8("()V".into())),
            ],
            false,
        );
        program.methods.insert(read.key.clone(), read);
        let read_reader = &program.methods[&MethodKey {
            class: "java/io/InputStreamReader".into(),
            name: "read".into(),
            descriptor: "([CII)I".into(),
        }];
        for (bytes, quantum) in [
            (&b"A\0"[..], 0),
            (&b"\0"[..], 0),
            (&b"A\0"[..], 1000),
            (&b"\0"[..], 1000),
        ] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let input = byte_stream_bytes(&mut machine, "test/ByteStream", bytes);
            let reader = character_reader(&mut machine, input);
            let destination = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Char, 6)
                .unwrap();
            for index in 0..6 {
                machine
                    .heap
                    .managed
                    .array_set(destination, index, HeapValue::Int(0x1234))
                    .unwrap();
            }
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
            machine.scheduler.quantum_remaining = quantum;
            let outcome = machine
                .call(
                    read_reader,
                    [
                        Value::Reference(Some(reader)),
                        Value::Reference(Some(destination)),
                        Value::Int(1),
                        Value::Int(4),
                    ],
                    1,
                )
                .unwrap();
            assert_eq!(matches!(outcome, CallOutcome::Suspend(_)), quantum == 0);
            let outcome = finish_reader_call(&mut machine, outcome);
            let partial = bytes.len() == 2 && recoverable;
            if partial {
                assert!(matches!(outcome, CallOutcome::Return(Some(Value::Int(1)))));
            } else {
                let CallOutcome::Throw(exception) = outcome else {
                    panic!(
                        "{exception_class} must propagate (partial={}, quantum={quantum})",
                        bytes.len() == 2
                    );
                };
                assert_eq!(machine.object_class(exception).unwrap(), exception_class);
            }
            for index in 0..6 {
                assert_eq!(
                    machine.heap.managed.array_get(destination, index).unwrap(),
                    HeapValue::Int(if partial && index == 1 {
                        i32::from(b'A')
                    } else {
                        0x1234
                    })
                );
            }
            assert_eq!(
                machine
                    .graphics_int_field(input, "java/io/ByteArrayInputStream.pos:I")
                    .unwrap(),
                i32::try_from(bytes.len()).unwrap()
            );
        }
    }
}

#[test]
fn character_reader_close_resumes_without_repeating_or_losing_the_close() {
    let mut program = program(false);
    let close = runtime_method(
        "test/ByteStream",
        "close",
        "()V",
        &[0x2a, 0x59, 0xb4, 0, 1, 0x04, 0x60, 0xb5, 0, 1, 0x00, 0xb1],
        3,
        1,
        vec![
            None,
            Some(Constant::Fieldref {
                class_index: 2,
                name_and_type_index: 3,
            }),
            Some(Constant::Class { name_index: 4 }),
            Some(Constant::NameAndType {
                name_index: 5,
                descriptor_index: 6,
            }),
            Some(Constant::Utf8(BYTE_STREAM.into())),
            Some(Constant::Utf8("pos".into())),
            Some(Constant::Utf8("I".into())),
        ],
        false,
    );
    program.methods.insert(close.key.clone(), close);
    let close_reader = &program.methods[&MethodKey {
        class: "java/io/InputStreamReader".into(),
        name: "close".into(),
        descriptor: "()V".into(),
    }];
    for quantum in 0..=12 {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let stream = byte_stream(&mut machine, "test/ByteStream");
        let reader = character_reader(&mut machine, stream);
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
        machine.scheduler.quantum_remaining = quantum;
        let mut outcome = machine
            .call(close_reader, [Value::Reference(Some(reader))], 1)
            .unwrap();
        let mut yields = 0;
        while let CallOutcome::Suspend(continuation) = outcome {
            assert!(yields < 16);
            assert_eq!(
                machine
                    .graphics_int_field(reader, "java/io/InputStreamReader.closed:Z")
                    .unwrap(),
                0
            );
            let mut roots = machine.roots(&[], &[]);
            continuation.roots(&mut roots);
            machine.collect_heap(roots);
            assert!(machine.heap.managed.get(reader).is_ok());
            machine.scheduler.quantum_remaining = 1;
            outcome = machine.resume_suspended_call(continuation, 1).unwrap();
            yields += 1;
        }
        assert!(matches!(outcome, CallOutcome::Return(None)));
        assert_eq!(
            machine
                .graphics_int_field(reader, "java/io/InputStreamReader.closed:Z")
                .unwrap(),
            1
        );
        assert_eq!(
            machine
                .graphics_int_field(stream, "java/io/ByteArrayInputStream.pos:I")
                .unwrap(),
            1
        );
        assert!(matches!(
            machine
                .call(close_reader, [Value::Reference(Some(reader))], 1)
                .unwrap(),
            CallOutcome::Return(None)
        ));
        assert_eq!(
            machine
                .graphics_int_field(stream, "java/io/ByteArrayInputStream.pos:I")
                .unwrap(),
            1
        );
    }
}

#[test]
fn character_reader_close_propagates_an_exception_after_resume() {
    let mut program = program(false);
    let close = runtime_method(
        "test/ByteStream",
        "close",
        "()V",
        &[0x00, 0x01, 0xbf],
        1,
        1,
        vec![],
        false,
    );
    program.methods.insert(close.key.clone(), close);
    let close_reader = &program.methods[&MethodKey {
        class: "java/io/InputStreamReader".into(),
        name: "close".into(),
        descriptor: "()V".into(),
    }];
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let stream = byte_stream(&mut machine, "test/ByteStream");
    let reader = character_reader(&mut machine, stream);
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
    let CallOutcome::Suspend(continuation) = machine
        .call(close_reader, [Value::Reference(Some(reader))], 1)
        .unwrap()
    else {
        panic!("close must yield before entering the guest stream");
    };
    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    let CallOutcome::Throw(exception) = machine.resume_suspended_call(continuation, 1).unwrap()
    else {
        panic!("close must preserve the guest exception");
    };
    assert_eq!(
        machine.object_class(exception).unwrap(),
        "java/lang/NullPointerException"
    );
    assert_eq!(
        machine
            .graphics_int_field(reader, "java/io/InputStreamReader.closed:Z")
            .unwrap(),
        0
    );
}

fn character_reader(machine: &mut Machine<'_, '_>, input: Handle) -> Handle {
    let class = "java/io/InputStreamReader";
    let fields = machine
        .instance_fields(class)
        .unwrap()
        .into_iter()
        .map(|field| (field.key.to_string(), field.initial))
        .collect();
    let reader = machine.allocate_object(class, fields, &[], &[]).unwrap();
    machine
        .input_stream_reader_init(
            &[
                Value::Reference(Some(reader)),
                Value::Reference(Some(input)),
            ],
            None,
        )
        .unwrap();
    reader
}

fn read_character(machine: &mut Machine<'_, '_>, reader: Handle) -> Result<CallOutcome, EmuError> {
    let method = machine.program.methods[&MethodKey {
        class: "java/io/InputStreamReader".into(),
        name: "read".into(),
        descriptor: "()I".into(),
    }]
        .clone();
    machine.input_stream_reader_read(&method, &[Value::Reference(Some(reader))], 0)
}

#[test]
fn character_reader_rejects_invalid_stream_results_before_buffering_them() {
    for value in [i32::MAX, i32::MIN, 256, -2] {
        let mut program = program(true);
        let read = runtime_method(
            "test/ByteStream",
            "read",
            "()I",
            &[0x12, 1, 0xac],
            1,
            1,
            vec![None, Some(Constant::Integer(value))],
            false,
        );
        program.methods.insert(read.key.clone(), read);
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let input = byte_stream(&mut machine, "test/ByteStream");
        let reader = character_reader(&mut machine, input);
        for pending in [0, -195] {
            // -195 retains byte 0xc2 after an earlier malformed prefix. A
            // following invalid read must not become another cached byte.
            machine
                .heap
                .managed
                .set_field(
                    reader,
                    "java/io/InputStreamReader.pending:I",
                    HeapValue::Int(pending),
                )
                .unwrap();
            let error = match read_character(&mut machine, reader) {
                Err(error) => error,
                Ok(_) => panic!("invalid stream result {value} was accepted"),
            };
            assert_eq!(error.code(), "reader-io");
            assert_eq!(
                machine
                    .graphics_int_field(reader, "java/io/InputStreamReader.pending:I")
                    .unwrap(),
                0
            );
        }
    }
}

#[test]
fn character_reader_rejects_corrupt_pending_state_without_arithmetic_overflow() {
    let program = program(false);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let input = byte_stream(&mut machine, BYTE_STREAM);
    let reader = character_reader(&mut machine, input);
    for pending in [i32::MIN, -257, 0x1_0000, i32::MAX] {
        machine
            .heap
            .managed
            .set_field(
                reader,
                "java/io/InputStreamReader.pending:I",
                HeapValue::Int(pending),
            )
            .unwrap();
        let error = match read_character(&mut machine, reader) {
            Err(error) => error,
            Ok(_) => panic!("invalid pending value {pending} was accepted"),
        };
        assert_eq!(error.code(), "reader-io");
    }
}

#[test]
fn character_reader_preserves_utf16_pairs_and_delimiters_after_malformed_utf8() {
    let program = program(false);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let input = byte_stream_bytes(
        &mut machine,
        BYTE_STREAM,
        &[0, 0xc3, 0xa9, 0xf0, 0x9f, 0x99, 0x82, 0xc2, b'!', 0xff],
    );
    let reader = character_reader(&mut machine, input);
    for expected in [0, 0x00e9, 0xd83d, 0xde42, 0xfffd, 33, 0xfffd, -1, -1] {
        assert!(matches!(
            read_character(&mut machine, reader).unwrap(),
            CallOutcome::Return(Some(Value::Int(value))) if value == expected
        ));
    }
}
