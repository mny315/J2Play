use super::*;
mod cancellation;
mod object_to_string;

#[test]
fn string_index_of_int_preserves_the_full_argument_and_cldc_char_semantics() {
    for intrinsic in [false, true] {
        let mut program = Program::new();
        cldc::register_core_natives(program.native_registry_mut()).unwrap();
        for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(1, 1) {
            program.add_class(&entry.class, &Limits::default()).unwrap();
        }
        for method in program.methods.values_mut() {
            if method.key.class == "java/lang/String" && method.key.name == "indexOf" {
                method.compatibility_candidate = Some(intrinsic);
            }
        }
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let string = machine
            .heap
            .managed
            .allocate_object("java/lang/String", HashMap::new())
            .unwrap();
        machine
            .heap
            .string_values
            .insert(string, vec![0, 65, 0xffff, 0xd801, 0xdc00, 65]);
        for (needle, first, after_three) in [
            (0, 0, -1),
            (65, 1, 5),
            (66, -1, -1),
            (0xffff, 2, -1),
            (0xd801, 3, 3),
            (0xdc00, 4, 4),
            (-1, -1, -1),
            (-0x10000, -1, -1),
            (0x10000, -1, -1),
            (0x10041, -1, -1),
            (0x10400, -1, -1),
            (i32::MIN, -1, -1),
            (i32::MAX, -1, -1),
        ] {
            for (from, expected) in [
                (None, first),
                (Some(i32::MIN), first),
                (Some(-1), first),
                (Some(0), first),
                (Some(3), after_three),
                (Some(6), -1),
                (Some(i32::MAX), -1),
            ] {
                let method = &program.methods[&MethodKey {
                    class: "java/lang/String".into(),
                    name: "indexOf".into(),
                    descriptor: if from.is_some() { "(II)I" } else { "(I)I" }.into(),
                }];
                let mut args = vec![Value::Reference(Some(string)), Value::Int(needle)];
                args.extend(from.map(Value::Int));
                let CallOutcome::Return(Some(Value::Int(actual))) =
                    machine.call(method, args, 1).unwrap()
                else {
                    panic!("String.indexOf did not return an index");
                };
                assert_eq!(
                    actual, expected,
                    "intrinsic={intrinsic}, ch={needle}, from={from:?}"
                );
            }
        }
    }
}

#[test]
fn string_region_matches_preserves_character_mapping_and_signed_bounds() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let left_units = [65, 0xdf, 0xfb00, 0x3c2, 0xd800, 0];
    let right_units = [97, 83, 70, 0x3c3, 0xd800, 0];
    let mut strings = Vec::new();
    for units in [left_units, right_units] {
        let handle = machine
            .heap
            .managed
            .allocate_object("java/lang/String", HashMap::new())
            .unwrap();
        machine.heap.string_values.insert(handle, units.to_vec());
        strings.push(handle);
    }
    let matches = |ignore, left, right, length| {
        machine
            .string_region_matches(&[
                Value::Reference(Some(strings[0])),
                Value::Int(ignore),
                Value::Int(left),
                Value::Reference(Some(strings[1])),
                Value::Int(right),
                Value::Int(length),
            ])
            .unwrap()
    };
    for (ignore, left, right, length, expected) in [
        (0, 0, 0, 1, false),
        (1, 0, 0, 1, true),
        (1, 1, 1, 1, false),
        (1, 2, 2, 1, false),
        (1, 3, 3, 1, true),
        (0, 3, 3, 1, false),
        (1, 4, 4, 2, true),
        (0, 4, 4, 2, true),
        (1, 0, 0, 6, false),
        (0, 6, 6, 0, true),
        (0, 7, 6, 0, false),
        (0, 6, 7, 0, false),
        (0, 0, 0, -1, true),
        (1, 7, 7, -1, true),
        (1, 8, 7, -1, false),
        (1, 7, 8, -1, false),
        (1, -1, 0, 0, false),
        (1, 0, -1, 0, false),
        (1, -1, 0, -1, false),
        (1, 0, -1, -1, false),
        (1, i32::MAX, i32::MAX, i32::MIN, true),
        (1, i32::MAX, 0, 1, false),
        (1, 0, i32::MAX, 1, false),
        (1, 0, 0, i32::MAX, false),
    ] {
        assert_eq!(
            matches(ignore, left, right, length),
            expected,
            "ignore={ignore}, left={left}, right={right}, length={length}"
        );
    }
    for length in [0, -1, 1] {
        assert_eq!(
            machine
                .string_region_matches(&[
                    Value::Reference(Some(strings[0])),
                    Value::Int(1),
                    Value::Int(0),
                    Value::Reference(None),
                    Value::Int(0),
                    Value::Int(length),
                ])
                .unwrap_err()
                .code(),
            "null-pointer-exception"
        );
    }
}

#[test]
pub(crate) fn string_comparison_batches_preserve_resolution_limits_and_fallback() {
    let mut caller = runtime_method(
        "StringCaller",
        "compare",
        "(Ljava/lang/String;Ljava/lang/String;)I",
        &[0x2a, 0x2b, 0xb6, 0, 6, 0xac],
        2,
        2,
        vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("java/lang/String".to_owned())),
            Some(Constant::NameAndType {
                name_index: 4,
                descriptor_index: 5,
            }),
            Some(Constant::Utf8("equalsIgnoreCase".to_owned())),
            Some(Constant::Utf8("(Ljava/lang/String;)Z".to_owned())),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 3,
            }),
        ],
        true,
    );
    caller.constant_pool_id = Some(0);
    let callee = runtime_method(
        "java/lang/String",
        "equalsIgnoreCase",
        "(Ljava/lang/String;)Z",
        &[0x03, 0xac],
        1,
        2,
        vec![None],
        false,
    );
    let mut program = Program::new();
    program.constant_pool_count = 1;
    program.methods.insert(callee.key.clone(), callee);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let mut handles = Vec::new();
    for units in [
        "Alpha".encode_utf16().collect::<Vec<_>>(),
        "aLPHA".encode_utf16().collect(),
        vec![65; 257],
    ] {
        let handle = machine
            .heap
            .managed
            .allocate_object("java/lang/String", HashMap::new())
            .unwrap();
        machine.heap.string_values.insert(handle, units);
        handles.push(handle);
    }
    let args = [
        Value::Reference(Some(handles[0])),
        Value::Reference(Some(handles[1])),
    ];
    // A cold invocation must resolve through the ordinary VM first.
    let mut stack = args.to_vec();
    let cold = crate::machine::interpreter_batch::execute_with_strings(
        &caller,
        &mut [],
        &mut stack,
        2,
        2,
        2,
        1,
        &mut machine.heap.managed,
        &mut machine.classes,
        Some(&machine.heap.string_values),
    );
    assert_eq!((cold.pc, cold.instructions), (2, 0));
    assert_eq!(stack, args);
    assert!(matches!(
        machine.call(&caller, args, 1).unwrap(),
        CallOutcome::Return(Some(Value::Int(1)))
    ));
    for wide in [false, true] {
        let prefix = if wide { vec![Value::Long(7)] } else { vec![] };
        for (other, expected) in [(Some(handles[1]), 1), (None, 0)] {
            let mut stack = prefix.clone();
            stack.extend([args[0], Value::Reference(other)]);
            let slots = if wide { 4 } else { 2 };
            let result = crate::machine::interpreter_batch::execute_with_strings(
                &caller,
                &mut [],
                &mut stack,
                2,
                slots,
                slots,
                1,
                &mut machine.heap.managed,
                &mut machine.classes,
                Some(&machine.heap.string_values),
            );
            assert_eq!(
                (result.pc, result.last_pc, result.instructions, result.slots),
                (5, 2, 1, slots - 1)
            );
            assert_eq!(stack.last(), Some(&Value::Int(expected)));
            assert_eq!(&stack[..stack.len() - 1], prefix);
        }
    }
    let non_string = machine
        .heap
        .managed
        .allocate_object("Other", HashMap::new())
        .unwrap();
    for (values, budget, allow_strings) in [
        (args, 0, true),
        (args, 1, false),
        ([Value::Reference(None), args[1]], 1, true),
        ([Value::Int(1), args[1]], 1, true),
        ([Value::Reference(Some(non_string)), args[1]], 1, true),
        ([args[0], Value::Reference(Some(non_string))], 1, true),
        ([Value::Reference(Some(handles[2])); 2], 1, true),
    ] {
        let mut stack = values.to_vec();
        let result = crate::machine::interpreter_batch::execute_with_strings(
            &caller,
            &mut [],
            &mut stack,
            2,
            2,
            2,
            budget,
            &mut machine.heap.managed,
            &mut machine.classes,
            allow_strings.then_some(&machine.heap.string_values),
        );
        assert_eq!((result.pc, result.instructions, result.slots), (2, 0, 2));
        assert_eq!(stack, values);
    }
    // A warmed call must still fail at the same Java frame bound.
    machine.limits.max_frames = 1;
    let Err(error) = machine.call(&caller, args, 1) else {
        panic!("frame bound bypassed")
    };
    assert_eq!(error.code(), "stack-overflow");
}

#[test]
pub(crate) fn string_equals_ignore_case_intrinsic_preserves_utf16_and_null() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let method = runtime_method(
        "java/lang/String",
        "equalsIgnoreCase",
        "(Ljava/lang/String;)Z",
        &[0x03, 0xac],
        1,
        2,
        vec![None],
        false,
    );
    let cases: &[(&[u16], &[u16], bool)] = &[
        (&[], &[], true),
        (&[65, 98, 47, 48], &[97, 66, 47, 48], true),
        (&[65], &[65, 65], false),
        (&[65, 98], &[97, 67], false),
        (&[0xc4, 0x3a3, 0x3c2], &[0xe4, 0x3c2, 0x3c3], true),
        (&[0x130], &[0x69], true),
        (&[0x130], &[0x49], true),
        (&[0x1f80], &[0x1f88], true),
        (&[0xdf], &[83], false),
        (&[0xdf], &[83, 83], false),
        (&[0xfb00], &[70], false),
        (&[0xd800, 65], &[0xd800, 97], true),
        (&[0xd800], &[0xd801], false),
        (&[0xd801, 0xdc00], &[0xd801, 0xdc28], false),
        (&[0, 65], &[0, 97], true),
    ];
    for &(left, right, expected) in cases {
        let mut strings = Vec::new();
        for units in [left, right] {
            let handle = machine
                .heap
                .managed
                .allocate_object("java/lang/String", HashMap::new())
                .unwrap();
            machine.heap.string_values.insert(handle, units.to_vec());
            strings.push(handle);
        }
        for (other, expected) in [
            (Some(strings[1]), expected),
            (Some(strings[0]), true),
            (None, false),
        ] {
            let outcome = machine
                .call(
                    &method,
                    [Value::Reference(Some(strings[0])), Value::Reference(other)],
                    1,
                )
                .unwrap();
            assert!(
                matches!(outcome,
                    CallOutcome::Return(Some(Value::Int(value))) if value == i32::from(expected)
                ),
                "{left:?} vs {right:?}, other={other:?}"
            );
        }
    }
    assert_eq!(machine.execution.instructions, 0);
    let result = machine.call(&method, [Value::Reference(None), Value::Reference(None)], 1);
    assert!(result.is_err());
}

#[test]
pub(crate) fn string_slice_and_uppercase_intrinsics_preserve_identity_and_bounds() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let string = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    machine
        .heap
        .string_values
        .insert(string, "abΩ1ß".encode_utf16().collect());

    let substring = runtime_method(
        "java/lang/String",
        "substring",
        "(II)Ljava/lang/String;",
        &[0xb0],
        1,
        3,
        vec![None],
        false,
    );
    let full = machine
        .invoke_compatibility_intrinsic(
            &substring,
            &[Value::Reference(Some(string)), Value::Int(0), Value::Int(5)],
            1,
        )
        .unwrap();
    assert!(matches!(
        full,
        Some(CallOutcome::Return(Some(Value::Reference(Some(value))))) if value == string
    ));
    let slice = machine
        .invoke_compatibility_intrinsic(
            &substring,
            &[Value::Reference(Some(string)), Value::Int(1), Value::Int(3)],
            1,
        )
        .unwrap();
    let Some(CallOutcome::Return(Some(Value::Reference(Some(slice))))) = slice else {
        panic!("String.substring intrinsic did not return a String");
    };
    assert_eq!(
        machine.heap.string_values.get(&slice).unwrap(),
        &"bΩ".encode_utf16().collect::<Vec<_>>()
    );
    let Err(error) = machine.invoke_compatibility_intrinsic(
        &substring,
        &[Value::Reference(Some(string)), Value::Int(3), Value::Int(2)],
        1,
    ) else {
        panic!("invalid String substring range was accepted");
    };
    assert_eq!(error.code(), "string-index");

    let uppercase = runtime_method(
        "java/lang/String",
        "toUpperCase",
        "()Ljava/lang/String;",
        &[0xb0],
        1,
        1,
        vec![None],
        false,
    );
    let upper = machine
        .invoke_compatibility_intrinsic(&uppercase, &[Value::Reference(Some(string))], 1)
        .unwrap();
    let Some(CallOutcome::Return(Some(Value::Reference(Some(upper))))) = upper else {
        panic!("String.toUpperCase intrinsic did not return a String");
    };
    assert_eq!(
        machine.heap.string_values.get(&upper).unwrap(),
        &"ABΩ1ß".encode_utf16().collect::<Vec<_>>()
    );
    let already_upper = machine
        .invoke_compatibility_intrinsic(&uppercase, &[Value::Reference(Some(upper))], 1)
        .unwrap();
    assert!(matches!(
        already_upper,
        Some(CallOutcome::Return(Some(Value::Reference(Some(value))))) if value == upper
    ));
    assert_eq!(machine.execution.instructions, 0);
}
