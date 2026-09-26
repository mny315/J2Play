use super::*;

#[test]
fn string_buffer_delete_mutates_utf16_and_checks_ranges() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let value = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Char, 8)
        .unwrap();
    for (index, unit) in "abcd".encode_utf16().enumerate() {
        machine
            .heap
            .managed
            .array_set(
                value,
                i32::try_from(index).unwrap(),
                HeapValue::Int(i32::from(unit)),
            )
            .unwrap();
    }
    let receiver = machine
        .heap
        .managed
        .allocate_object(
            "java/lang/StringBuffer",
            HashMap::from([
                (
                    "java/lang/StringBuffer.value:[C".to_owned(),
                    HeapValue::Reference(Some(value)),
                ),
                (
                    "java/lang/StringBuffer.count:I".to_owned(),
                    HeapValue::Int(4),
                ),
            ]),
        )
        .unwrap();

    machine.string_buffer_delete(receiver, 1, 3, false).unwrap();
    assert_eq!(
        machine
            .heap
            .managed
            .field(receiver, "java/lang/StringBuffer.count:I")
            .unwrap(),
        HeapValue::Int(2)
    );
    assert_eq!(
        machine.heap.managed.array_get(value, 0).unwrap(),
        HeapValue::Int('a' as i32)
    );
    assert_eq!(
        machine.heap.managed.array_get(value, 1).unwrap(),
        HeapValue::Int('d' as i32)
    );
    assert_eq!(
        machine.heap.managed.array_get(value, 2).unwrap(),
        HeapValue::Int(0)
    );
    let string = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    machine
        .string_from_buffer(&[
            Value::Reference(Some(string)),
            Value::Reference(Some(receiver)),
        ])
        .unwrap();
    assert_eq!(
        machine.heap.string_values[&string],
        "ad".encode_utf16().collect::<Vec<_>>()
    );
    assert_eq!(
        machine
            .string_buffer_delete(receiver, 2, 3, true)
            .unwrap_err()
            .code(),
        "string-index"
    );
    assert_eq!(
        machine
            .string_buffer_delete(receiver, -1, 0, false)
            .unwrap_err()
            .code(),
        "string-index"
    );
}

#[test]
fn integer_parse_int_matches_guest_code_for_digits_null_radix_sign_and_overflow() {
    for intrinsic in [false, true] {
        let mut program = Program::new();
        cldc::register_core_natives(program.native_registry_mut()).unwrap();
        for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(1, 1) {
            program.add_class(&entry.class, &Limits::default()).unwrap();
        }
        for method in program.methods.values_mut() {
            if method.key.class == "java/lang/Integer" && method.key.name == "parseInt" {
                method.compatibility_candidate = Some(intrinsic);
            }
        }
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        for (text, radix, expected) in [
            ("-2147483648", 10, Some(i32::MIN)),
            ("+7fffffff", 16, Some(i32::MAX)),
            ("2147483648", 10, None),
            ("١٢٣", 10, Some(123)),
            ("-१२७", 10, Some(-127)),
            ("７fffffff", 16, Some(i32::MAX)),
            ("-२१४७४८३६४८", 10, Some(i32::MIN)),
            ("４2", 10, Some(42)),
            ("१A", 16, Some(26)),
            ("१२", 2, None),
            ("١٢٣", 1, None),
            ("२१४७४८३६४८", 10, None),
            ("²", 10, None),
            ("Ⅳ", 10, None),
            ("８", 8, None),
        ]
        .into_iter()
        .map(|(text, radix, expected)| (Some(text), radix, expected))
        .chain([(None, 10, None)])
        {
            let string = text.map(|text| machine.allocate_dynamic_string(text, &[], &[]).unwrap());
            for explicit_radix in [false, true] {
                if !explicit_radix && radix != 10 {
                    continue;
                }
                let method = &program.methods[&MethodKey {
                    class: "java/lang/Integer".into(),
                    name: "parseInt".into(),
                    descriptor: if explicit_radix {
                        "(Ljava/lang/String;I)I"
                    } else {
                        "(Ljava/lang/String;)I"
                    }
                    .into(),
                }];
                let mut args = vec![Value::Reference(string)];
                if explicit_radix {
                    args.push(Value::Int(radix));
                }
                let actual = match machine.call(method, args, 1).unwrap() {
                    CallOutcome::Return(Some(Value::Int(value))) => Some(value),
                    CallOutcome::Throw(exception) => {
                        assert_eq!(
                            machine.object_class(exception).unwrap(),
                            "java/lang/NumberFormatException"
                        );
                        None
                    }
                    _ => panic!("Integer.parseInt did not return or throw"),
                };
                assert_eq!(
                    actual, expected,
                    "intrinsic={intrinsic}, text={text:?}, radix={radix}, explicit={explicit_radix}"
                );
            }
        }
    }
}
