use super::*;

#[test]
pub(crate) fn bytecode_accessors_preserve_unsigned_and_nullable_results() {
    fn field_constants(name: &str, descriptor: &str) -> (Vec<Option<Constant>>, u16) {
        let constants = vec![
            None,
            Some(Constant::Utf8("Accessors".to_owned())),
            Some(Constant::Class { name_index: 1 }),
            Some(Constant::Utf8(name.to_owned())),
            Some(Constant::Utf8(descriptor.to_owned())),
            Some(Constant::NameAndType {
                name_index: 3,
                descriptor_index: 4,
            }),
            Some(Constant::Fieldref {
                class_index: 2,
                name_and_type_index: 5,
            }),
            Some(Constant::Integer(65_535)),
        ];
        (constants, 6)
    }

    let (byte_constants, byte_field) = field_constants("bytes", "[B");
    let mut byte_code = vec![0x2a, 0xb4, 0, 0, 0x1b, 0x33, 0x11, 0, 0xff, 0x7e, 0xac];
    byte_code[2..4].copy_from_slice(&byte_field.to_be_bytes());
    let mut byte_getter = runtime_method(
        "Accessors",
        "unsignedByte",
        "(I)I",
        &byte_code,
        2,
        2,
        byte_constants,
        false,
    );
    let (short_constants, short_field) = field_constants("shorts", "[S");
    let mut short_code = vec![0x2a, 0xb4, 0, 0, 0x1b, 0x35, 0x12, 7, 0x7e, 0xac];
    short_code[2..4].copy_from_slice(&short_field.to_be_bytes());
    let mut short_getter = runtime_method(
        "Accessors",
        "unsignedShort",
        "(I)I",
        &short_code,
        2,
        2,
        short_constants,
        false,
    );
    let (length_constants, byte_field) = field_constants("bytes", "[B");
    let mut length_code = vec![
        0x2a, 0xb4, 0, 0, 0xc7, 0, 5, 0x03, 0xac, 0x2a, 0xb4, 0, 0, 0xbe, 0xac,
    ];
    for offset in [1, 10] {
        length_code[offset + 1..offset + 3].copy_from_slice(&byte_field.to_be_bytes());
    }
    let mut length_getter = runtime_method(
        "Accessors",
        "nullableLength",
        "()I",
        &length_code,
        1,
        1,
        length_constants,
        false,
    );
    let program = bytecode_program(&mut [&mut byte_getter, &mut short_getter, &mut length_getter]);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let bytes = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 2)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(bytes, 0, HeapValue::Int(-1))
        .unwrap();
    let shorts = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Short, 1)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(shorts, 0, HeapValue::Int(-2))
        .unwrap();
    let receiver = machine
        .heap
        .managed
        .allocate_object(
            "Accessors",
            HashMap::from([
                (
                    "Accessors.bytes:[B".to_owned(),
                    HeapValue::Reference(Some(bytes)),
                ),
                (
                    "Accessors.shorts:[S".to_owned(),
                    HeapValue::Reference(Some(shorts)),
                ),
            ]),
        )
        .unwrap();

    for (method, args, expected) in [
        (
            &byte_getter,
            vec![Value::Reference(Some(receiver)), Value::Int(0)],
            255,
        ),
        (
            &short_getter,
            vec![Value::Reference(Some(receiver)), Value::Int(0)],
            65_534,
        ),
        (&length_getter, vec![Value::Reference(Some(receiver))], 2),
    ] {
        let outcome = machine.call(method, &args, 1).unwrap();
        assert!(matches!(
            outcome,
            CallOutcome::Return(Some(Value::Int(value))) if value == expected
        ));
    }
    machine
        .heap
        .managed
        .set_field(receiver, "Accessors.bytes:[B", HeapValue::Reference(None))
        .unwrap();
    assert!(matches!(
        machine
            .call(&length_getter, [Value::Reference(Some(receiver))], 1,)
            .unwrap(),
        CallOutcome::Return(Some(Value::Int(0)))
    ));
    assert!(matches!(
        machine
            .call(
                &byte_getter,
                [Value::Reference(Some(receiver)), Value::Int(0)],
                1
            )
            .unwrap(),
        CallOutcome::Throw(_)
    ));
    assert!(machine.execution.instructions > 0);
}
