use super::*;

#[test]
fn byte_output_copies_overlapping_sources_and_grows_across_collection() {
    let program = program(false);
    for (capacity, count, offset, length, alias) in [
        (4, 3, 1, 3, true),
        (8, 1, 0, 5, true),
        (0, 0, 0, 4, false),
        (4, 2, 1, 3, false),
    ] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                max_heap_bytes: 512,
                ..Limits::default()
            },
            false,
            &mut context,
        );
        let original = (1..=capacity).map(|value| value as u8).collect::<Vec<_>>();
        let backing = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Byte, capacity)
            .unwrap();
        for (index, byte) in original.iter().enumerate() {
            machine
                .heap
                .managed
                .array_set(backing, index as i32, HeapValue::Int(i32::from(*byte)))
                .unwrap();
        }
        let stream = machine
            .heap
            .managed
            .allocate_object(
                "java/io/ByteArrayOutputStream",
                HashMap::from([
                    (
                        "java/io/ByteArrayOutputStream.buf:[B".to_owned(),
                        HeapValue::Reference(Some(backing)),
                    ),
                    (
                        "java/io/ByteArrayOutputStream.count:I".to_owned(),
                        HeapValue::Int(count),
                    ),
                ]),
            )
            .unwrap();
        let (source, source_bytes) = if alias {
            (backing, original.clone())
        } else {
            let bytes = vec![0x80, 0xff, 0, 0x7f, 42];
            let source = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Byte, bytes.len() as i32)
                .unwrap();
            for (index, byte) in bytes.iter().enumerate() {
                machine
                    .heap
                    .managed
                    .array_set(source, index as i32, HeapValue::Int(i32::from(*byte)))
                    .unwrap();
            }
            (source, bytes)
        };
        let error = machine
            .byte_array_output_write_slice(&[
                Value::Reference(Some(stream)),
                Value::Reference(Some(source)),
                Value::Int(i32::MAX),
                Value::Int(1),
            ])
            .unwrap_err();
        assert_eq!(error.code(), "array-index-out-of-bounds-exception");
        assert_eq!(
            machine
                .graphics_int_field(stream, "java/io/ByteArrayOutputStream.count:I")
                .unwrap(),
            count
        );
        let garbage = machine
            .heap
            .managed
            .allocate_array(
                ArrayKind::Byte,
                (512 - machine.heap.managed.bytes() - 24) as i32,
            )
            .unwrap();
        machine
            .byte_array_output_write_slice(&[
                Value::Reference(Some(stream)),
                Value::Reference(Some(source)),
                Value::Int(offset),
                Value::Int(length),
            ])
            .unwrap();
        if count + length > capacity {
            assert!(machine.heap.managed.get(garbage).is_err());
        }
        machine
            .byte_array_output_write_byte(&[Value::Reference(Some(stream)), Value::Int(i32::MAX)])
            .unwrap();
        let mut expected = original[..count as usize].to_vec();
        expected.extend_from_slice(&source_bytes[offset as usize..(offset + length) as usize]);
        expected.push(0xff);
        let output = machine
            .graphics_reference_field(stream, "java/io/ByteArrayOutputStream.buf:[B")
            .unwrap();
        assert_eq!(
            machine
                .graphics_int_field(stream, "java/io/ByteArrayOutputStream.count:I")
                .unwrap(),
            expected.len() as i32
        );
        for (index, expected) in expected.iter().enumerate() {
            assert_eq!(
                machine
                    .heap
                    .managed
                    .array_get(output, index as i32)
                    .unwrap(),
                HeapValue::Int(i32::from(*expected as i8))
            );
        }
    }
}

const BYTE_OUTPUT: &str = "java/io/ByteArrayOutputStream";

fn byte_output_program() -> Program {
    let mut program = program(false);
    program.classes.insert(
        "test/ByteOutput".into(),
        Class {
            super_name: Some(BYTE_OUTPUT.into()),
            fields: Vec::new(),
            ..program.classes[BYTE_OUTPUT].clone()
        },
    );
    // Override write(int) by incrementing the byte before calling super.write.
    let write = runtime_method(
        "test/ByteOutput",
        "write",
        "(I)V",
        &[0x2a, 0x1b, 0x04, 0x60, 0xb7, 0, 1, 0xb1],
        3,
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
            Some(Constant::Utf8(BYTE_OUTPUT.into())),
            Some(Constant::Utf8("write".into())),
            Some(Constant::Utf8("(I)V".into())),
        ],
        false,
    );
    program.methods.insert(write.key.clone(), write);
    program
}

fn byte_output(machine: &mut Machine<'_, '_>, class: &str) -> Handle {
    let fields = machine
        .instance_fields(class)
        .unwrap()
        .into_iter()
        .map(|field| (field.key.to_string(), field.initial))
        .collect();
    let stream = machine.allocate_object(class, fields, &[], &[]).unwrap();
    let init = machine.program.methods[&MethodKey {
        class: BYTE_OUTPUT.into(),
        name: "<init>".into(),
        descriptor: "()V".into(),
    }]
        .clone();
    assert!(matches!(
        machine
            .call(&init, [Value::Reference(Some(stream))], 1)
            .unwrap(),
        CallOutcome::Return(None)
    ));
    stream
}

#[test]
fn byte_output_bulk_write_and_base_output_write_keep_distinct_dispatch() {
    let program = byte_output_program();
    for intrinsic in [false, true] {
        for (owner, descriptor, expected) in [
            ("java/io/OutputStream", "([BII)V", [2, 129, 0]),
            ("test/ByteOutput", "([BII)V", [1, 128, 255]),
            ("test/ByteOutput", "([B)V", [1, 128, 255]),
        ] {
            let mut host = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut host);
            let stream = byte_output(&mut machine, "test/ByteOutput");
            let source = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Byte, 3)
                .unwrap();
            for (index, value) in [1, 128, 255].into_iter().enumerate() {
                machine
                    .heap
                    .managed
                    .array_set(source, index as i32, Value::Int(value))
                    .unwrap();
            }
            let key = machine.resolve_virtual(owner, "write", descriptor).unwrap();
            let mut method = program.methods[&key].clone();
            method.compatibility_candidate = Some(intrinsic);
            let mut args = vec![
                Value::Reference(Some(stream)),
                Value::Reference(Some(source)),
            ];
            if descriptor == "([BII)V" {
                args.extend([Value::Int(0), Value::Int(3)]);
            }
            assert!(matches!(
                machine.call(&method, args, 1).unwrap(),
                CallOutcome::Return(None)
            ));
            assert_eq!(
                machine
                    .graphics_int_field(stream, &format!("{BYTE_OUTPUT}.count:I"))
                    .unwrap(),
                3
            );
            let buffer = machine
                .graphics_reference_field(stream, &format!("{BYTE_OUTPUT}.buf:[B"))
                .unwrap();
            for (index, value) in expected.into_iter().enumerate() {
                assert_eq!(
                    machine
                        .heap
                        .managed
                        .array_get(buffer, index as i32)
                        .unwrap(),
                    Value::Int(i32::from(value as u8 as i8)),
                    "owner={owner}, descriptor={descriptor}, intrinsic={intrinsic}, index={index}"
                );
            }
        }
    }
}

struct ByteOutputEncodingContext(&'static str);

impl HostServices for ByteOutputEncodingContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, name: &str) -> Option<&str> {
        (name == "microedition.encoding").then_some(self.0)
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}

#[test]
fn byte_output_to_string_decodes_only_the_written_bytes_and_survives_reset() {
    let program = byte_output_program();
    for encoding in ["UTF-8", "ISO-8859-1"] {
        for bytes in [
            b"".as_slice(),
            b"A\0B",
            "Aé🙂".as_bytes(),
            &[0xff, 0xc2, b'!'],
        ] {
            let mut host = ByteOutputEncodingContext(encoding);
            let mut machine = program.machine(Limits::default(), false, &mut host);
            let stream = byte_output(&mut machine, "test/ByteOutput");
            for byte in bytes.iter().chain(b"unused") {
                machine
                    .byte_array_output_write_byte(&[
                        Value::Reference(Some(stream)),
                        Value::Int(i32::from(*byte)),
                    ])
                    .unwrap();
            }
            machine
                .heap
                .managed
                .set_field(
                    stream,
                    &format!("{BYTE_OUTPUT}.count:I"),
                    Value::Int(bytes.len() as i32),
                )
                .unwrap();
            let key = machine
                .resolve_virtual("test/ByteOutput", "toString", "()Ljava/lang/String;")
                .unwrap();
            let method = program.methods[&key].clone();
            let CallOutcome::Return(Some(Value::Reference(Some(string)))) = machine
                .call(&method, [Value::Reference(Some(stream))], 1)
                .unwrap()
            else {
                panic!("byte output did not return a string");
            };
            let expected: Vec<_> = if encoding == "UTF-8" {
                String::from_utf8_lossy(bytes).encode_utf16().collect()
            } else {
                bytes.iter().map(|byte| u16::from(*byte)).collect()
            };
            assert_eq!(machine.heap.string_values.get(&string), Some(&expected));
            let reset = program.methods[&MethodKey {
                class: BYTE_OUTPUT.into(),
                name: "reset".into(),
                descriptor: "()V".into(),
            }]
                .clone();
            assert!(matches!(
                machine
                    .call(&reset, [Value::Reference(Some(stream))], 1)
                    .unwrap(),
                CallOutcome::Return(None)
            ));
            machine
                .byte_array_output_write_byte(&[Value::Reference(Some(stream)), Value::Int(42)])
                .unwrap();
            assert_eq!(machine.heap.string_values.get(&string), Some(&expected));
        }
    }
}
