use super::*;

struct DecoderFields {
    probabilities: String,
    range: String,
    code: String,
    position: String,
    input: String,
}

fn range_decoder_fixture() -> (Method, DecoderFields) {
    fn push_constant(constants: &mut Vec<Option<Constant>>, value: Constant) -> u16 {
        let index = u16::try_from(constants.len()).unwrap();
        constants.push(Some(value));
        index
    }
    fn push_field(
        constants: &mut Vec<Option<Constant>>,
        class_index: u16,
        name: &str,
        descriptor: &str,
    ) -> u16 {
        let name_index = push_constant(constants, Constant::Utf8(name.to_owned()));
        let descriptor_index = push_constant(constants, Constant::Utf8(descriptor.to_owned()));
        let name_and_type_index = push_constant(
            constants,
            Constant::NameAndType {
                name_index,
                descriptor_index,
            },
        );
        push_constant(
            constants,
            Constant::Fieldref {
                class_index,
                name_and_type_index,
            },
        )
    }
    fn access(code: &mut Vec<u8>, opcode: u8, index: u16) {
        code.push(opcode);
        code.extend_from_slice(&index.to_be_bytes());
    }

    let mut constants = vec![None];
    let class_name = push_constant(&mut constants, Constant::Utf8("Decoder".to_owned()));
    let class_index = push_constant(
        &mut constants,
        Constant::Class {
            name_index: class_name,
        },
    );
    let probabilities = push_field(&mut constants, class_index, "a", "[S");
    let range = push_field(&mut constants, class_index, "b", "J");
    let code_field = push_field(&mut constants, class_index, "c", "J");
    let position = push_field(&mut constants, class_index, "d", "I");
    let input = push_field(&mut constants, class_index, "f", "[B");
    let threshold = push_constant(&mut constants, Constant::Long(16_777_216));
    let mut direct_code = Vec::new();
    access(&mut direct_code, 0xb2, range);
    direct_code.extend_from_slice(&[0x10, 0x0b, 0x7b]);
    access(&mut direct_code, 0xb2, probabilities);
    direct_code.extend_from_slice(&[0x1a, 0x35, 0x85, 0x69, 0x40]);
    access(&mut direct_code, 0xb2, code_field);
    direct_code.extend_from_slice(&[0x1f, 0x94, 0x9c, 0x00, 0x4c, 0x1f]);
    access(&mut direct_code, 0xb3, range);
    access(&mut direct_code, 0xb2, probabilities);
    direct_code.extend_from_slice(&[0x1a, 0x5c, 0x35, 0x11, 0x08, 0x00]);
    access(&mut direct_code, 0xb2, probabilities);
    direct_code.extend_from_slice(&[0x1a, 0x35, 0x64, 0x08, 0x7a, 0x60, 0x93, 0x56]);
    access(&mut direct_code, 0xb2, range);
    access(&mut direct_code, 0x14, threshold);
    direct_code.extend_from_slice(&[0x94, 0x9c, 0x00, 0x28]);
    access(&mut direct_code, 0xb2, code_field);
    direct_code.extend_from_slice(&[0x10, 0x08, 0x79]);
    access(&mut direct_code, 0xb2, input);
    access(&mut direct_code, 0xb2, position);
    direct_code.extend_from_slice(&[0x59, 0x04, 0x60]);
    access(&mut direct_code, 0xb3, position);
    direct_code.extend_from_slice(&[0x33, 0x11, 0x00, 0xff, 0x7e, 0x85, 0x81]);
    access(&mut direct_code, 0xb3, code_field);
    access(&mut direct_code, 0xb2, range);
    direct_code.extend_from_slice(&[0x10, 0x08, 0x79]);
    access(&mut direct_code, 0xb3, range);
    direct_code.extend_from_slice(&[0x03, 0xac]);
    access(&mut direct_code, 0xb2, range);
    direct_code.extend_from_slice(&[0x1f, 0x65]);
    access(&mut direct_code, 0xb3, range);
    access(&mut direct_code, 0xb2, code_field);
    direct_code.extend_from_slice(&[0x1f, 0x65]);
    access(&mut direct_code, 0xb3, code_field);
    access(&mut direct_code, 0xb2, probabilities);
    direct_code.extend_from_slice(&[0x1a, 0x5c, 0x35]);
    access(&mut direct_code, 0xb2, probabilities);
    direct_code.extend_from_slice(&[0x1a, 0x35, 0x08, 0x7a, 0x64, 0x93, 0x56]);
    access(&mut direct_code, 0xb2, range);
    access(&mut direct_code, 0x14, threshold);
    direct_code.extend_from_slice(&[0x94, 0x9c, 0x00, 0x28]);
    access(&mut direct_code, 0xb2, code_field);
    direct_code.extend_from_slice(&[0x10, 0x08, 0x79]);
    access(&mut direct_code, 0xb2, input);
    access(&mut direct_code, 0xb2, position);
    direct_code.extend_from_slice(&[0x59, 0x04, 0x60]);
    access(&mut direct_code, 0xb3, position);
    direct_code.extend_from_slice(&[0x33, 0x11, 0x00, 0xff, 0x7e, 0x85, 0x81]);
    access(&mut direct_code, 0xb3, code_field);
    access(&mut direct_code, 0xb2, range);
    direct_code.extend_from_slice(&[0x10, 0x08, 0x79]);
    access(&mut direct_code, 0xb3, range);
    direct_code.extend_from_slice(&[0x04, 0xac]);
    let direct_method = runtime_method(
        "Decoder",
        "another_obfuscated_name",
        "(I)I",
        &direct_code,
        6,
        3,
        constants,
        true,
    );
    (
        direct_method,
        DecoderFields {
            probabilities: "Decoder.a:[S".into(),
            range: "Decoder.b:J".into(),
            code: "Decoder.c:J".into(),
            position: "Decoder.d:I".into(),
            input: "Decoder.f:[B".into(),
        },
    )
}

#[derive(Debug, PartialEq)]
struct DecoderResult {
    value: Result<Option<Value>, String>,
    range: Value,
    code: Value,
    position: Value,
    probability: HeapValue,
}

fn execute_decoder(
    batching: bool,
    range: i64,
    code: i64,
    probability_index: i32,
    null_probabilities: bool,
    input_length: Option<i32>,
    position: i32,
) -> DecoderResult {
    let (method, fields) = range_decoder_fixture();
    let mut program = Program::new();
    let mut class = test_class_definition(None);
    for (key, kind, initial) in [
        (
            &fields.probabilities,
            ValueKind::Reference,
            Value::Reference(None),
        ),
        (&fields.range, ValueKind::Long, Value::Long(0)),
        (&fields.code, ValueKind::Long, Value::Long(0)),
        (&fields.position, ValueKind::Int, Value::Int(0)),
        (&fields.input, ValueKind::Reference, Value::Reference(None)),
    ] {
        class.fields.push(Field {
            key: key.clone().into(),
            declaring_class: "Decoder".into(),
            kind,
            is_static: true,
            field_token: FieldToken::new(),
            instance_slot: None,
            initial,
            constant_string: None,
        });
    }
    program.classes.insert("Decoder".into(), class);
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    machine.execution.profiling = !batching;
    machine.initialize_class("Decoder", 1).unwrap();
    let probabilities = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Short, 1)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(probabilities, 0, HeapValue::Int(1_024))
        .unwrap();
    let input = input_length.map(|length| {
        let input = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Byte, length)
            .unwrap();
        if length != 0 {
            machine
                .heap
                .managed
                .array_set(input, 0, HeapValue::Int(-128))
                .unwrap();
        }
        input
    });
    machine.classes.static_fields.extend([
        (
            fields.probabilities.clone(),
            Value::Reference((!null_probabilities).then_some(probabilities)),
        ),
        (fields.range.clone(), Value::Long(range)),
        (fields.code.clone(), Value::Long(code)),
        (fields.position.clone(), Value::Int(position)),
        (fields.input.clone(), Value::Reference(input)),
    ]);
    let value = match machine.call(&method, [Value::Int(probability_index)], 1) {
        Ok(CallOutcome::Return(value)) => Ok(value),
        Err(error) => Err(error.code().to_owned()),
        _ => panic!("decoder must return a bit or a controlled error"),
    };
    DecoderResult {
        value,
        range: *machine.classes.static_fields.get(&fields.range).unwrap(),
        code: *machine.classes.static_fields.get(&fields.code).unwrap(),
        position: *machine.classes.static_fields.get(&fields.position).unwrap(),
        probability: machine.heap.managed.array_get(probabilities, 0).unwrap(),
    }
}

#[test]
fn range_decoder_keeps_bytecode_exception_and_partial_write_order() {
    for range in [1 << 20, 1 << 32] {
        for code in [0, 1 << 20, 1 << 32] {
            for probability_index in [0, -1, 1] {
                for null_probabilities in [false, true] {
                    for input_length in [Some(0), None, Some(1)] {
                        for position in [-1, 0, 1, i32::MAX] {
                            let run = |intrinsic| {
                                execute_decoder(
                                    intrinsic,
                                    range,
                                    code,
                                    probability_index,
                                    null_probabilities,
                                    input_length,
                                    position,
                                )
                            };
                            let expected = run(false);
                            assert!(
                                match &expected.value {
                                    Ok(Some(Value::Int(0 | 1))) => true,
                                    Err(error) =>
                                        error == "null-pointer-exception"
                                            || error == "array-index-out-of-bounds-exception",
                                    _ => false,
                                },
                                "unexpected bytecode result: {expected:?}"
                            );
                            assert_eq!(
                                run(true),
                                expected,
                                "range={range} code={code} probability_index={probability_index} \
                                 null_probabilities={null_probabilities} input_length={input_length:?} position={position}"
                            );
                        }
                    }
                }
            }
        }
    }
}
