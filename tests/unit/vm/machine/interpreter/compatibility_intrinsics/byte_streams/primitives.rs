use super::*;

fn data_input(machine: &mut Machine<'_, '_>, input: Handle) -> Handle {
    machine
        .heap
        .managed
        .allocate_object(
            "java/io/DataInputStream",
            HashMap::from([(
                "java/io/DataInputStream.in:Ljava/io/InputStream;".into(),
                Value::Reference(Some(input)),
            )]),
        )
        .unwrap()
}

#[test]
fn primitive_reads_match_bootstrap_values_exceptions_and_cursor_side_effects() {
    for (class, override_read) in [
        (BYTE_STREAM, false),
        ("test/ByteStream", false),
        ("test/ByteStream", true),
    ] {
        let program = program(override_read);
        for (name, descriptor) in [
            ("readBoolean", "()Z"),
            ("readByte", "()B"),
            ("readUnsignedShort", "()I"),
            ("readShort", "()S"),
            ("readChar", "()C"),
            ("readInt", "()I"),
            ("readLong", "()J"),
        ] {
            for (position, count) in [
                (0, 8),
                (1, 8),
                (6, 8),
                (7, 8),
                (8, 8),
                (9, 8),
                (0, 0),
                (0, 1),
                (0, 3),
                (0, 10),
                (7, 10),
                (8, 10),
                (-1, 8),
                (i32::MIN, i32::MAX),
                (i32::MAX, i32::MAX),
            ] {
                for null_buffer in [false, true] {
                    let mut results = Vec::new();
                    for intrinsic in [false, true] {
                        let mut method = program.methods[&MethodKey {
                            class: "java/io/DataInputStream".into(),
                            name: name.into(),
                            descriptor: descriptor.into(),
                        }]
                            .clone();
                        method.compatibility_candidate = Some(intrinsic);
                        let mut host = DefaultNativeContext;
                        let mut machine = program.machine(Limits::default(), false, &mut host);
                        let input = byte_stream_bytes(
                            &mut machine,
                            class,
                            &[0x80, 0x11, 0xff, 0x22, 0x7f, 0, 0xaa, 0x55],
                        );
                        for (field, value) in [("pos", position), ("count", count)] {
                            machine
                                .heap
                                .managed
                                .set_field(
                                    input,
                                    &format!("{BYTE_STREAM}.{field}:I"),
                                    Value::Int(value),
                                )
                                .unwrap();
                        }
                        if null_buffer {
                            machine
                                .heap
                                .managed
                                .set_field(
                                    input,
                                    &format!("{BYTE_STREAM}.buf:[B"),
                                    Value::Reference(None),
                                )
                                .unwrap();
                        }
                        let stream = data_input(&mut machine, input);
                        let result =
                            match machine.call(&method, [Value::Reference(Some(stream))], 1) {
                                Ok(CallOutcome::Return(Some(value))) => format!("return:{value:?}"),
                                Ok(CallOutcome::Throw(exception)) => {
                                    machine.object_class(exception).unwrap().into_owned()
                                }
                                Err(error) => format!("vm-error:{}", error.code()),
                                _ => panic!("unexpected primitive read result"),
                            };
                        let cursor = machine
                            .graphics_int_field(input, &format!("{BYTE_STREAM}.pos:I"))
                            .unwrap();
                        results.push((result, cursor));
                    }
                    assert_eq!(
                        results[0], results[1],
                        "{class}.{name}, override={override_read}, pos={position}, count={count}, null={null_buffer}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "manual DataInput primitive throughput measurement"]
fn data_input_primitive_throughput() {
    let program = program(false);
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let input = byte_stream_bytes(
        &mut machine,
        BYTE_STREAM,
        &[0x80, 0x11, 0xff, 0x22, 0x7f, 0, 0xaa, 0x55],
    );
    let stream = data_input(&mut machine, input);
    for (width, signed) in [
        (1, false),
        (1, true),
        (2, false),
        (2, true),
        (4, true),
        (8, true),
    ] {
        let mut checksum = 0_u64;
        let started = std::time::Instant::now();
        for _ in 0..32_768 {
            machine
                .heap
                .managed
                .set_field(input, "java/io/ByteArrayInputStream.pos:I", Value::Int(0))
                .unwrap();
            let outcome = machine
                .data_input_stream_primitive(
                    std::hint::black_box(&[Value::Reference(Some(stream))]),
                    std::hint::black_box(width),
                    std::hint::black_box(signed),
                )
                .unwrap()
                .unwrap();
            let value = match outcome {
                CallOutcome::Return(Some(Value::Int(value))) => u64::from(value.cast_unsigned()),
                CallOutcome::Return(Some(Value::Long(value))) => value.cast_unsigned(),
                _ => panic!("expected primitive value"),
            };
            checksum = checksum.wrapping_add(value);
        }
        eprintln!(
            "data-input width={width} signed={signed} elapsed={:?} checksum={checksum:016x}",
            started.elapsed()
        );
    }
}
