use super::*;

#[test]
pub(crate) fn writing_second_slot_invalidates_category_two_local() {
    let mut locals = vec![None; 2];
    set_local(&mut locals, 0, Value::Long(7)).unwrap();
    set_local(&mut locals, 1, Value::Int(3)).unwrap();
    assert!(get_local(&locals, 0).is_err());
    assert_eq!(get_local(&locals, 1).unwrap(), Value::Int(3));
}

#[test]
pub(crate) fn add_class_rolls_back_all_methods_on_duplicate() {
    let code = classfile::CodeAttribute {
        name_index: 5,
        max_stack: 1,
        max_locals: 0,
        code: vec![0x03, 0xac],
        exception_table: Vec::new(),
        attributes: Vec::new(),
    };
    let member = classfile::Member {
        access_flags: 0x0008,
        name_index: 3,
        descriptor_index: 4,
        attributes: vec![Attribute::Code(code)],
    };
    let class = ClassFile {
        minor_version: 0,
        major_version: 50,
        constant_pool: vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("T".into())),
            Some(Constant::Utf8("main".into())),
            Some(Constant::Utf8("()I".into())),
            Some(Constant::Utf8("Code".into())),
        ],
        access_flags: 0x0021,
        this_class: 1,
        super_class: 0,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![member.clone(), member],
        attributes: Vec::new(),
    };
    let mut program = Program::new();
    assert_eq!(
        program
            .add_class(&class, &Limits::default())
            .unwrap_err()
            .code(),
        "duplicate-method"
    );
    assert!(program.methods.is_empty());
    assert_eq!(program.runtime_bytes, 0);
    assert_eq!(program.constant_pool_count, 0);
}

#[test]
pub(crate) fn table_switch_selects_exact_case() {
    let mut code = [
        0x05, 0xaa, 0, 0, 0, 0, 0, 29, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 23, 0, 0, 0, 26, 0x10, 10,
        0xac, 0x10, 20, 0xac, 0x02, 0xac,
    ];
    for padding in [0, 0xab, 0xff] {
        code[2..4].fill(padding);
        for (key, expected) in [(0x04, 10), (0x05, 20), (0x06, -1)] {
            code[0] = key;
            assert_eq!(run(&code, 1, 0).unwrap(), Some(Value::Int(expected)));
        }
    }
}

#[test]
pub(crate) fn lookup_switch_selects_exact_case() {
    let mut code = [
        0x05, 0xab, 0, 0, 0, 0, 0, 33, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 27, 0, 0, 0, 2, 0, 0, 0,
        30, 0x10, 10, 0xac, 0x10, 20, 0xac, 0x02, 0xac,
    ];
    for padding in [0, 0xab, 0xff] {
        code[2..4].fill(padding);
        for (key, expected) in [(0x04, 10), (0x05, 20), (0x06, -1)] {
            code[0] = key;
            assert_eq!(run(&code, 1, 0).unwrap(), Some(Value::Int(expected)));
        }
    }
}

#[test]
pub(crate) fn wide_load_and_store_use_u16_local_index() {
    let code = [0x10, 7, 0xc4, 0x36, 1, 0, 0xc4, 0x15, 1, 0, 0xac];
    assert_eq!(run(&code, 1, 257).unwrap(), Some(Value::Int(7)));
}

#[test]
fn compact_locals_preserve_every_value_type_and_slot() {
    let cases = [
        (
            0x04,
            [0x3b, 0x3c, 0x3d, 0x3e],
            [0x1a, 0x1b, 0x1c, 0x1d],
            "()I",
            0xac,
            Value::Int(1),
        ),
        (
            0x0a,
            [0x3f, 0x40, 0x41, 0x42],
            [0x1e, 0x1f, 0x20, 0x21],
            "()J",
            0xad,
            Value::Long(1),
        ),
        (
            0x0c,
            [0x43, 0x44, 0x45, 0x46],
            [0x22, 0x23, 0x24, 0x25],
            "()F",
            0xae,
            Value::Float(1.0),
        ),
        (
            0x0f,
            [0x47, 0x48, 0x49, 0x4a],
            [0x26, 0x27, 0x28, 0x29],
            "()D",
            0xaf,
            Value::Double(1.0),
        ),
        (
            0x01,
            [0x4b, 0x4c, 0x4d, 0x4e],
            [0x2a, 0x2b, 0x2c, 0x2d],
            "()Ljava/lang/Object;",
            0xb0,
            Value::Reference(None),
        ),
    ];
    for (constant, stores, _, _, _, value) in cases {
        for (_, _, loads, descriptor, return_op, expected) in cases {
            for (store, load) in stores.into_iter().zip(loads) {
                let method = runtime_method(
                    "Locals",
                    "roundtrip",
                    descriptor,
                    &[constant, store, load, return_op],
                    2,
                    5,
                    vec![None],
                    true,
                );
                let mut program = Program::new();
                program.methods.insert(method.key.clone(), method);
                for tracing in [false, true] {
                    let result = program.execute(
                        "Locals",
                        "roundtrip",
                        descriptor,
                        Limits::default(),
                        tracing,
                    );
                    if value == expected {
                        assert_eq!(result.unwrap().value, Some(value));
                    } else {
                        assert_eq!(
                            result.unwrap_err().code(),
                            "type-mismatch",
                            "store {store:x}, load {load:x}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
pub(crate) fn value_categories_are_enforced() {
    for (code, max_stack, max_locals) in [
        (&[0x09, 0x59, 0xad][..], 4, 0),
        (&[0x03, 0xad][..], 2, 0),
        (&[0x03, 0x3f, 0x03, 0xac][..], 2, 2),
        (&[0x09, 0x57, 0x03, 0xac][..], 2, 0),
    ] {
        assert_eq!(
            run(code, max_stack, max_locals).unwrap_err().code(),
            "type-mismatch"
        );
    }
}

#[test]
pub(crate) fn invokestatic_recurses_with_bounded_frames() {
    let constants = vec![
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
        Some(Constant::Utf8("T".into())),
        Some(Constant::Utf8("fact".into())),
        Some(Constant::Utf8("(I)I".into())),
    ];
    // 34! forces a synchronous call chain deeper than the old 32-frame
    // compatibility cap. Java int multiplication wraps to zero here.
    let main_code = vec![0x10, 34, 0xb8, 0, 1, 0xac];
    let fact_code = vec![
        0x1a, 0x04, 0xa3, 0, 5, 0x04, 0xac, 0x1a, 0x1a, 0x04, 0x64, 0xb8, 0, 1, 0x68, 0xac,
    ];
    let mut program = Program::new();
    program.constant_pool_count = 1;
    for mut method in [
        runtime_method(
            "T",
            "main",
            "()I",
            &main_code,
            1,
            0,
            constants.clone(),
            true,
        ),
        runtime_method("T", "fact", "(I)I", &fact_code, 3, 1, constants, true),
    ] {
        method.constant_pool_id = Some(0);
        program.methods.insert(method.key.clone(), method);
    }
    assert_eq!(
        program
            .execute("T", "main", "()I", Limits::default(), false)
            .unwrap()
            .value,
        Some(Value::Int(0))
    );
    assert_eq!(
        program
            .execute(
                "T",
                "main",
                "()I",
                Limits {
                    max_frames: 3,
                    ..Limits::default()
                },
                false
            )
            .unwrap_err()
            .code(),
        "stack-overflow"
    );
}

#[test]
pub(crate) fn objects_constructors_fields_and_virtual_dispatch_execute() {
    let cp = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("Child".into())),
        Some(Constant::Methodref {
            class_index: 1,
            name_and_type_index: 4,
        }),
        Some(Constant::NameAndType {
            name_index: 5,
            descriptor_index: 9,
        }),
        Some(Constant::Utf8("<init>".into())),
        Some(Constant::Methodref {
            class_index: 14,
            name_and_type_index: 7,
        }),
        Some(Constant::NameAndType {
            name_index: 8,
            descriptor_index: 13,
        }),
        Some(Constant::Utf8("value".into())),
        Some(Constant::Utf8("()V".into())),
        Some(Constant::Fieldref {
            class_index: 14,
            name_and_type_index: 11,
        }),
        Some(Constant::NameAndType {
            name_index: 12,
            descriptor_index: 15,
        }),
        Some(Constant::Utf8("x".into())),
        Some(Constant::Utf8("()I".into())),
        Some(Constant::Class { name_index: 16 }),
        Some(Constant::Utf8("I".into())),
        Some(Constant::Utf8("Parent".into())),
    ];
    let mut program = Program::new();
    program.classes.insert(
        "Parent".into(),
        Class {
            super_name: Some("java/lang/Object".into()),
            interfaces: Vec::new(),
            fields: vec![Field {
                key: "Parent.x:I".into(),
                declaring_class: "Parent".into(),
                kind: ValueKind::Int,
                is_static: false,
                field_token: FieldToken::new(),
                instance_slot: None,
                initial: Value::Int(0),
                constant_string: None,
            }],
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    program.classes.insert(
        "Child".into(),
        Class {
            super_name: Some("Parent".into()),
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    for mut method in [
        runtime_method(
            "T",
            "main",
            "()I",
            &[0xbb, 0, 1, 0x59, 0xb7, 0, 3, 0xb6, 0, 6, 0xac],
            2,
            0,
            cp.clone(),
            true,
        ),
        runtime_method(
            "Child",
            "<init>",
            "()V",
            &[0x2a, 0x10, 41, 0xb5, 0, 10, 0xb1],
            2,
            1,
            cp.clone(),
            false,
        ),
        runtime_method(
            "Parent",
            "value",
            "()I",
            &[0x2a, 0xb4, 0, 10, 0xac],
            1,
            1,
            cp.clone(),
            false,
        ),
        runtime_method(
            "Child",
            "value",
            "()I",
            &[0x2a, 0xb4, 0, 10, 0x04, 0x60, 0xac],
            2,
            1,
            cp.clone(),
            false,
        ),
    ] {
        method.constant_pool_id = Some(match method.key.class.as_str() {
            "T" => 0,
            "Child" => 1,
            "Parent" => 2,
            _ => unreachable!(),
        });
        program.methods.insert(method.key.clone(), method);
    }
    program.constant_pool_count = 3;
    assert_eq!(
        program
            .execute("T", "main", "()I", Limits::default(), false)
            .unwrap()
            .value,
        Some(Value::Int(42))
    );
}

#[test]
pub(crate) fn primitive_arrays_load_store_and_length_execute() {
    let code = [
        0x05, 0xbc, 10, 0x4b, 0x2a, 0x04, 0x10, 9, 0x4f, 0x2a, 0x04, 0x2e, 0x2a, 0xbe, 0x60, 0xac,
    ];
    assert_eq!(run(&code, 3, 1).unwrap(), Some(Value::Int(11)));
}

#[test]
pub(crate) fn athrow_uses_matching_exception_table_handler() {
    let cp = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("E".into())),
    ];
    let mut main = runtime_method(
        "T",
        "main",
        "()I",
        &[0xbb, 0, 1, 0xbf, 0x57, 0x10, 7, 0xac],
        1,
        0,
        cp,
        true,
    );
    Arc::make_mut(&mut main.exception_table).push(ExceptionHandler {
        start_pc: 0,
        end_pc: 4,
        handler_pc: 4,
        catch_type: 1,
    });
    let mut program = program_with_exception("E");
    program.methods.insert(main.key.clone(), main);
    assert_eq!(
        program
            .execute("T", "main", "()I", Limits::default(), false)
            .unwrap()
            .value,
        Some(Value::Int(7))
    );

    // An implicit exception enters its handler with exactly the throwable on
    // the operand stack. Keep the cached slot count synchronized with that
    // real stack so malformed bytecode cannot evade max_stack after a catch.
    let cp = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("java/lang/NullPointerException".into())),
    ];
    let mut overflow = runtime_method(
        "OverflowAfterCatch",
        "main",
        "()I",
        &[0x01, 0xbe, 0x03, 0xac, 0x04, 0x57, 0x57, 0x10, 7, 0xac],
        1,
        0,
        cp,
        true,
    );
    Arc::make_mut(&mut overflow.exception_table).push(ExceptionHandler {
        start_pc: 0,
        end_pc: 2,
        handler_pc: 4,
        catch_type: 1,
    });
    let mut program = program_with_exception("java/lang/NullPointerException");
    program.methods.insert(overflow.key.clone(), overflow);
    let error = program
        .execute(
            "OverflowAfterCatch",
            "main",
            "()I",
            Limits::default(),
            false,
        )
        .unwrap_err();
    assert_eq!(error.code(), "operand-stack-overflow");
}

#[test]
pub(crate) fn interfaces_instanceof_and_checkcast_use_runtime_hierarchy() {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("Impl".into())),
        Some(Constant::Class { name_index: 4 }),
        Some(Constant::Utf8("Iface".into())),
    ];
    let main = runtime_method(
        "T",
        "main",
        "()I",
        &[0xbb, 0, 1, 0x59, 0xc0, 0, 3, 0x57, 0xc1, 0, 3, 0xac],
        2,
        0,
        constants,
        true,
    );
    let mut program = Program::new();
    program.classes.insert(
        "Iface".into(),
        Class {
            super_name: Some("java/lang/Object".into()),
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: true,
            is_interface: true,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    program.classes.insert(
        "Impl".into(),
        Class {
            super_name: Some("java/lang/Object".into()),
            interfaces: vec!["Iface".into()],
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    program.methods.insert(main.key.clone(), main);
    assert_eq!(
        program
            .execute("T", "main", "()I", Limits::default(), false)
            .unwrap()
            .value,
        Some(Value::Int(1))
    );
}

#[test]
pub(crate) fn multidimensional_primitive_array_is_an_object_array() {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("[[S".into())),
        Some(Constant::Class { name_index: 4 }),
        Some(Constant::Utf8("[Ljava/lang/Object;".into())),
    ];
    let code = [0x04, 0x04, 0xc5, 0, 1, 2, 0xc0, 0, 3, 0x57, 0x04, 0xac];
    let main = runtime_method("T", "main", "()I", &code, 2, 0, constants, true);
    let mut program = Program::new();
    program.methods.insert(main.key.clone(), main);
    assert_eq!(
        program
            .execute("T", "main", "()I", Limits::default(), false)
            .unwrap()
            .value,
        Some(Value::Int(1))
    );
}

#[test]
pub(crate) fn negative_nested_array_size_is_catchable() {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("[[I".into())),
        Some(Constant::Class { name_index: 4 }),
        Some(Constant::Utf8(
            "java/lang/NegativeArraySizeException".into(),
        )),
    ];
    let mut main = runtime_method(
        "T",
        "main",
        "()I",
        &[
            0x04, 0x02, 0xc5, 0, 1, 2, 0x57, 0x03, 0xac, 0x57, 0x04, 0xac,
        ],
        2,
        0,
        constants,
        true,
    );
    Arc::make_mut(&mut main.exception_table).push(ExceptionHandler {
        start_pc: 0,
        end_pc: 7,
        handler_pc: 9,
        catch_type: 3,
    });
    let mut program = program_with_exception("java/lang/NegativeArraySizeException");
    program.methods.insert(main.key.clone(), main);

    assert_eq!(
        program
            .execute("T", "main", "()I", Limits::default(), false)
            .unwrap()
            .value,
        Some(Value::Int(1))
    );
}

#[test]
pub(crate) fn multianewarray_constructs_every_requested_dimension() {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("[[I".into())),
    ];
    let code = [
        0x05, 0x06, 0xc5, 0, 1, 2, 0x4b, 0x2a, 0xbe, 0x2a, 0x03, 0x32, 0xbe, 0x60, 0xac,
    ];
    let main = runtime_method("T", "main", "()I", &code, 3, 1, constants, true);
    let mut program = Program::new();
    program.methods.insert(main.key.clone(), main);
    assert_eq!(
        program
            .execute("T", "main", "()I", Limits::default(), false)
            .unwrap()
            .value,
        Some(Value::Int(5))
    );
}

#[test]
fn multianewarray_preserves_reference_class_names_starting_with_l() {
    for name in ["L", "Llama", "LLNode", "pkg/Llama"] {
        for dimensions in 1..=3_u8 {
            let constants = vec![
                None,
                Some(Constant::Class { name_index: 2 }),
                Some(Constant::Utf8(format!(
                    "{}L{name};",
                    "[".repeat(usize::from(dimensions))
                ))),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::Utf8(format!("[L{name};"))),
            ];
            let mut code = vec![0x04; usize::from(dimensions)];
            code.extend([0xc5, 0, 1, dimensions]);
            for _ in 1..dimensions {
                code.extend([0x03, 0x32]);
            }
            code.extend([0xc1, 0, 3, 0xac]);
            let main = runtime_method("T", "main", "()I", &code, 3, 0, constants, true);
            let mut program = Program::new();
            program
                .classes
                .insert(name.into(), test_class_definition(Some("java/lang/Object")));
            program.methods.insert(main.key.clone(), main);
            assert_eq!(
                program
                    .execute("T", "main", "()I", Limits::default(), false)
                    .unwrap()
                    .value,
                Some(Value::Int(1)),
                "{dimensions}-dimensional array of {name} lost its component type"
            );
        }
    }
}

#[test]
pub(crate) fn reference_return_and_wide_reference_locals_execute() {
    let code = [0x01, 0xc4, 0x3a, 1, 0, 0xc4, 0x19, 1, 0, 0xb0];
    let main = runtime_method(
        "T",
        "main",
        "()Ljava/lang/Object;",
        &code,
        1,
        257,
        vec![None],
        true,
    );
    let mut program = Program::new();
    program.methods.insert(main.key.clone(), main);
    assert_eq!(
        program
            .execute(
                "T",
                "main",
                "()Ljava/lang/Object;",
                Limits::default(),
                false
            )
            .unwrap()
            .value,
        Some(Value::Reference(None))
    );
}

#[test]
fn reference_array_store_checks_null_then_bounds_then_component_type() {
    let store = runtime_method(
        "T",
        "store",
        "([Ljava/lang/Object;ILjava/lang/Object;)V",
        &[0x2a, 0x1b, 0x2c, 0x53, 0xb1],
        3,
        3,
        vec![None],
        true,
    );
    let mut program = Program::new();
    for class in ["Expected", "Other"] {
        program
            .classes
            .insert(class.into(), test_class_definition(None));
    }
    for tracing in [false, true] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), tracing, &mut context);
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Reference("Expected".into()), 1)
            .unwrap();
        let other = machine
            .heap
            .managed
            .allocate_object("Other", HashMap::new())
            .unwrap();
        for (reference, index, value, expected) in [
            (None, -1, Some(other), "null-pointer-exception"),
            (
                Some(array),
                -1,
                Some(other),
                "array-index-out-of-bounds-exception",
            ),
            (
                Some(array),
                1,
                Some(other),
                "array-index-out-of-bounds-exception",
            ),
            (
                Some(array),
                i32::MAX,
                None,
                "array-index-out-of-bounds-exception",
            ),
            (Some(array), 0, Some(other), "array-store-exception"),
        ] {
            let error = machine
                .call(
                    &store,
                    [
                        Value::Reference(reference),
                        Value::Int(index),
                        Value::Reference(value),
                    ],
                    1,
                )
                .err()
                .expect("invalid array store");
            assert_eq!(error.code(), expected, "index={index}, tracing={tracing}");
            assert_eq!(
                machine.heap.managed.array_get(array, 0).unwrap(),
                Value::Reference(None)
            );
        }
    }
}

#[test]
pub(crate) fn concrete_array_opcode_must_match_component_kind() {
    let error = run(&[0x04, 0xbc, 8, 0x03, 0x2e, 0xac], 2, 0).unwrap_err();
    assert_eq!(error.code(), "type-mismatch");
}

#[test]
pub(crate) fn stack_rearrangement_opcode_forms_preserve_values() {
    for (code, expected, max_stack) in [
        (&[0x04, 0x05, 0x5f, 0x64, 0xac][..], 1, 2),
        (&[0x04, 0x05, 0x5a, 0x60, 0x60, 0xac][..], 5, 3),
        (&[0x04, 0x05, 0x5c, 0x60, 0x60, 0x60, 0xac][..], 6, 4),
    ] {
        assert_eq!(run(code, max_stack, 0).unwrap(), Some(Value::Int(expected)));
    }
}
