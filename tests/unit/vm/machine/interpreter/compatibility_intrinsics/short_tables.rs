use super::*;

#[test]
pub(crate) fn bytecode_short_table_lookups_preserve_results_and_bounds() {
    fn push_field(
        constants: &mut Vec<Option<Constant>>,
        class: u16,
        name: &str,
        descriptor: &str,
    ) -> u16 {
        let name_and_type_index = push_name_and_type(constants, name, descriptor);
        push_constant(
            constants,
            Constant::Fieldref {
                class_index: class,
                name_and_type_index,
            },
        )
    }
    fn push_method(
        constants: &mut Vec<Option<Constant>>,
        class: u16,
        name: &str,
        descriptor: &str,
    ) -> u16 {
        let name_and_type_index = push_name_and_type(constants, name, descriptor);
        push_constant(
            constants,
            Constant::Methodref {
                class_index: class,
                name_and_type_index,
            },
        )
    }
    fn short_array(machine: &mut Machine<'_, '_>, values: &[i32]) -> Handle {
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Short, i32::try_from(values.len()).unwrap())
            .unwrap();
        for (index, value) in values.iter().copied().enumerate() {
            machine
                .heap
                .managed
                .array_set(array, i32::try_from(index).unwrap(), HeapValue::Int(value))
                .unwrap();
        }
        array
    }

    let mut constants = vec![None];
    let class = push_class(&mut constants, "LegacyFont");
    let table = push_field(&mut constants, class, "glyphBuckets", "[[S");
    let bucket_count = push_field(&mut constants, class, "bucketCount", "S");
    let mut hash_code = vec![
        0x2a, 0xb4, 0, 0, 0xc7, 0, 5, 0x03, 0xac, 0x1b, 0x2a, 0xb4, 0, 0, 0x70, 0x3d, 0x2a, 0xb4,
        0, 0, 0x1c, 0x32, 0x03, 0x35, 0x1b, 0xa0, 0, 12, 0x2a, 0xb4, 0, 0, 0x1c, 0x32, 0x04, 0x35,
        0xac, 0x05, 0x3e, 0x2a, 0xb4, 0, 0, 0x1c, 0x32, 0xbe, 0x36, 4, 0x1d, 0x15, 4, 0xa2, 0, 21,
        0x2a, 0xb4, 0, 0, 0x1c, 0x32, 0x1d, 0x35, 0x1b, 0x9f, 0, 9, 0x84, 3, 2, 0xa7, 0xff, 0xeb,
        0x1d, 0x15, 4, 0xa1, 0, 5, 0x04, 0xac, 0x2a, 0xb4, 0, 0, 0x1c, 0x32, 0x1d, 0x04, 0x60,
        0x35, 0xac,
    ];
    for offset in [1, 17, 29, 40, 55, 81] {
        hash_code[offset + 1..offset + 3].copy_from_slice(&table.to_be_bytes());
    }
    hash_code[12..14].copy_from_slice(&bucket_count.to_be_bytes());
    let mut hash_method = runtime_method(
        "LegacyFont",
        "lookup",
        "(I)I",
        &hash_code,
        3,
        5,
        constants,
        false,
    );
    let mut offset_constants = vec![None];
    let class = push_class(&mut offset_constants, "LegacyFont");
    let offsets = push_field(&mut offset_constants, class, "offsets", "[S");
    let values = push_field(&mut offset_constants, class, "values", "[S");
    let mut offset_code = vec![
        0x2a, 0x2a, 0xb4, 0, 0, 0x1b, 0x35, 0x1c, 0x60, 0x3c, 0x59, 0x4b, 0xb4, 0, 0, 0x1b, 0x35,
        0xac,
    ];
    offset_code[3..5].copy_from_slice(&offsets.to_be_bytes());
    offset_code[13..15].copy_from_slice(&values.to_be_bytes());
    let mut offset_method = runtime_method(
        "LegacyFont",
        "offsetValue",
        "(II)I",
        &offset_code,
        3,
        3,
        offset_constants,
        false,
    );
    let mut scaled_constants = vec![None];
    let class = push_class(&mut scaled_constants, "LegacyFont");
    let scale = push_field(&mut scaled_constants, class, "scale", "I");
    let accessor = push_method(&mut scaled_constants, class, "offsetValue", "(II)I");
    let mut scaled_code = vec![
        0x2a, 0xb4, 0, 0, 0x9b, 0, 18, 0x2a, 0x1b, 0x03, 0xb7, 0, 0, 0x2a, 0xb4, 0, 0, 0x68, 0x10,
        8, 0x7a, 0xac, 0x2a, 0x1b, 0x03, 0xb7, 0, 0, 0xac,
    ];
    for offset in [1, 14] {
        scaled_code[offset + 1..offset + 3].copy_from_slice(&scale.to_be_bytes());
    }
    for offset in [10, 25] {
        scaled_code[offset + 1..offset + 3].copy_from_slice(&accessor.to_be_bytes());
    }
    let mut scaled_method = runtime_method(
        "LegacyFont",
        "scaledOffsetValue",
        "(I)I",
        &scaled_code,
        3,
        2,
        scaled_constants,
        false,
    );
    let program = bytecode_program(&mut [&mut hash_method, &mut offset_method, &mut scaled_method]);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let rows = [
        short_array(&mut machine, &[0, 2]),
        short_array(&mut machine, &[1, 3]),
        short_array(&mut machine, &[65, 7, 68, 9]),
    ];
    let table_array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Reference("[S".into()), 3)
        .unwrap();
    for (index, row) in rows.into_iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(
                table_array,
                i32::try_from(index).unwrap(),
                HeapValue::Reference(Some(row)),
            )
            .unwrap();
    }
    let offsets_array = short_array(&mut machine, &[2, 4]);
    let values_array = short_array(&mut machine, &[10, 11, 12, 13, 14, 15]);
    let receiver = machine
        .heap
        .managed
        .allocate_object(
            "LegacyFont",
            HashMap::from([
                (
                    "LegacyFont.glyphBuckets:[[S".to_owned(),
                    HeapValue::Reference(Some(table_array)),
                ),
                ("LegacyFont.bucketCount:S".to_owned(), HeapValue::Int(3)),
                (
                    "LegacyFont.offsets:[S".to_owned(),
                    HeapValue::Reference(Some(offsets_array)),
                ),
                (
                    "LegacyFont.values:[S".to_owned(),
                    HeapValue::Reference(Some(values_array)),
                ),
                ("LegacyFont.scale:I".to_owned(), HeapValue::Int(512)),
            ]),
        )
        .unwrap();

    for (key, expected) in [(65, 7), (68, 9), (71, 1)] {
        let outcome = machine
            .call(
                &hash_method,
                [Value::Reference(Some(receiver)), Value::Int(key)],
                1,
            )
            .unwrap();
        assert!(matches!(
            outcome,
            CallOutcome::Return(Some(Value::Int(value))) if value == expected
        ));
    }
    let outcome = machine
        .call(
            &offset_method,
            [
                Value::Reference(Some(receiver)),
                Value::Int(1),
                Value::Int(-1),
            ],
            1,
        )
        .unwrap();
    assert!(matches!(outcome, CallOutcome::Return(Some(Value::Int(13)))));
    let outcome = machine
        .call(
            &scaled_method,
            [Value::Reference(Some(receiver)), Value::Int(1)],
            1,
        )
        .unwrap();
    assert!(matches!(outcome, CallOutcome::Return(Some(Value::Int(28)))));
    machine
        .heap
        .managed
        .set_field(receiver, "LegacyFont.scale:I", HeapValue::Int(-1))
        .unwrap();
    let outcome = machine
        .call(
            &scaled_method,
            [Value::Reference(Some(receiver)), Value::Int(1)],
            1,
        )
        .unwrap();
    assert!(matches!(outcome, CallOutcome::Return(Some(Value::Int(14)))));
    assert!(matches!(
        machine
            .call(
                &offset_method,
                [
                    Value::Reference(Some(receiver)),
                    Value::Int(9),
                    Value::Int(0),
                ],
                1
            )
            .unwrap(),
        CallOutcome::Throw(_)
    ));
    assert!(machine.execution.instructions > 0);
}
