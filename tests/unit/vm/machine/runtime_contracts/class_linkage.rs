use super::*;

#[test]
fn abstract_interface_declarations_resolve_before_virtual_dispatch() {
    let interface = ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: vec![
            None,
            Some(Constant::Utf8("Contract".into())),
            Some(Constant::Class { name_index: 1 }),
            Some(Constant::Utf8("member".into())),
            Some(Constant::Utf8("()I".into())),
            Some(Constant::Utf8("java/lang/Object".into())),
            Some(Constant::Class { name_index: 5 }),
        ],
        access_flags: 0x0601,
        this_class: 2,
        super_class: 6,
        interfaces: vec![],
        fields: vec![],
        methods: vec![classfile::Member {
            access_flags: 0x0401,
            name_index: 3,
            descriptor_index: 4,
            attributes: vec![],
        }],
        attributes: vec![],
    };
    let mut program = Program::new();
    program.add_class(&interface, &Limits::default()).unwrap();
    let mut implementation = test_class_definition(None);
    implementation.interfaces.push("Contract".into());
    program.classes.insert("Concrete".into(), implementation);
    let implementation = runtime_method(
        "Concrete",
        "member",
        "()I",
        &[0x10, 42, 0xac],
        1,
        1,
        vec![],
        false,
    );
    program
        .methods
        .insert(implementation.key.clone(), implementation);
    let caller = runtime_method(
        "Caller",
        "run",
        "(LContract;)I",
        &[0x2a, 0xb9, 0, 1, 1, 0, 0xac],
        1,
        1,
        vec![
            None,
            Some(Constant::InterfaceMethodref {
                class_index: 2,
                name_and_type_index: 3,
            }),
            Some(Constant::Class { name_index: 4 }),
            Some(Constant::NameAndType {
                name_index: 5,
                descriptor_index: 6,
            }),
            Some(Constant::Utf8("Contract".into())),
            Some(Constant::Utf8("member".into())),
            Some(Constant::Utf8("()I".into())),
        ],
        true,
    );
    for trace in [false, true] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), trace, &mut context);
        let receiver = machine
            .heap
            .managed
            .allocate_object("Concrete", HashMap::new())
            .unwrap();
        assert!(matches!(
            machine
                .call(&caller, [Value::Reference(Some(receiver))], 1)
                .unwrap(),
            CallOutcome::Return(Some(Value::Int(42)))
        ));
        assert_eq!(
            machine
                .call(&caller, [Value::Reference(None)], 1)
                .err()
                .unwrap()
                .code(),
            "null-pointer-exception"
        );
        let key = MethodKey {
            class: "Contract".into(),
            name: "member".into(),
            descriptor: "()I".into(),
        };
        let declaration = program.methods.get(&key).unwrap();
        assert_eq!(
            machine
                .call(declaration, [Value::Reference(Some(receiver))], 1)
                .err()
                .unwrap()
                .code(),
            "abstract-method"
        );
    }
}

#[test]
fn instance_invocation_linkage_errors_precede_a_null_receiver() {
    for opcode in [0xb6, 0xb7, 0xb9] {
        for missing in [false, true] {
            let mut program = Program::new();
            program.constant_pool_count = 1;
            let mut class = test_class_definition(None);
            class.is_interface = opcode == 0xb9;
            program.classes.insert("Target".into(), class);
            if !missing {
                let method = runtime_method("Target", "member", "()V", &[0xb1], 0, 0, vec![], true);
                program.methods.insert(method.key.clone(), method);
            }
            let reference = if opcode == 0xb9 {
                Constant::InterfaceMethodref {
                    class_index: 2,
                    name_and_type_index: 3,
                }
            } else {
                Constant::Methodref {
                    class_index: 2,
                    name_and_type_index: 3,
                }
            };
            let mut code = vec![0x01, opcode, 0, 1];
            if opcode == 0xb9 {
                code.extend([1, 0]);
            }
            code.push(0xb1);
            let mut caller = runtime_method(
                "Caller",
                "run",
                "()V",
                &code,
                1,
                0,
                vec![
                    None,
                    Some(reference),
                    Some(Constant::Class { name_index: 4 }),
                    Some(Constant::NameAndType {
                        name_index: 5,
                        descriptor_index: 6,
                    }),
                    Some(Constant::Utf8("Target".into())),
                    Some(Constant::Utf8("member".into())),
                    Some(Constant::Utf8("()V".into())),
                ],
                true,
            );
            caller.constant_pool_id = Some(0);
            for trace in [false, true] {
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), trace, &mut context);
                for _ in 0..2 {
                    let error = machine.call(&caller, [], 1).err().unwrap();
                    assert_eq!(
                        error.code(),
                        if missing {
                            "method-not-found"
                        } else {
                            "incompatible-class-change"
                        },
                        "opcode {opcode:#x}"
                    );
                }
            }
        }
    }
}

#[test]
fn static_access_to_instance_members_does_not_initialize_the_declaring_class() {
    for (opcode, code) in [
        (0xb2, vec![0xb2, 0, 1, 0x57, 0xb1]),
        (0xb3, vec![0x03, 0xb3, 0, 1, 0xb1]),
        (0xb8, vec![0xb8, 0, 1, 0xb1]),
    ] {
        for initializer_code in [vec![0xb1], vec![0x03, 0x03, 0x6c, 0x57, 0xb1]] {
            let mut class = test_class_definition(None);
            class.fields.push(Field {
                key: "Deferred.member:I".into(),
                declaring_class: "Deferred".into(),
                kind: ValueKind::Int,
                is_static: false,
                field_token: FieldToken::new(),
                instance_slot: None,
                initial: Value::Int(0),
                constant_string: None,
            });
            let mut program = Program::new();
            program.constant_pool_count = 1;
            program.classes.insert("Deferred".into(), class);
            for method in [
                runtime_method(
                    "Deferred",
                    "<clinit>",
                    "()V",
                    &initializer_code,
                    2,
                    0,
                    vec![],
                    true,
                ),
                runtime_method("Deferred", "member", "()V", &[0xb1], 0, 1, vec![], false),
            ] {
                program.methods.insert(method.key.clone(), method);
            }
            let reference = if opcode == 0xb8 {
                Constant::Methodref {
                    class_index: 2,
                    name_and_type_index: 3,
                }
            } else {
                Constant::Fieldref {
                    class_index: 2,
                    name_and_type_index: 3,
                }
            };
            let mut caller = runtime_method(
                "Caller",
                "run",
                "()V",
                &code,
                1,
                0,
                vec![
                    None,
                    Some(reference),
                    Some(Constant::Class { name_index: 4 }),
                    Some(Constant::NameAndType {
                        name_index: 5,
                        descriptor_index: 6,
                    }),
                    Some(Constant::Utf8("Deferred".into())),
                    Some(Constant::Utf8("member".into())),
                    Some(Constant::Utf8(
                        if opcode == 0xb8 { "()V" } else { "I" }.into(),
                    )),
                ],
                true,
            );
            caller.constant_pool_id = Some(0);
            for trace in [false, true] {
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), trace, &mut context);
                // Repeat with the same machine to include cached resolution.
                for _ in 0..2 {
                    let error = machine.call(&caller, [], 1).err().unwrap();
                    assert_eq!(
                        error.code(),
                        "incompatible-class-change",
                        "opcode {opcode:#x}"
                    );
                    assert!(!machine.classes.initialized.contains("Deferred"));
                    assert!(
                        !machine
                            .classes
                            .failed_initialization
                            .contains_key("Deferred")
                    );
                }
            }
        }
    }
}

#[test]
fn array_assignability_preserves_rank_covariance_and_primitive_types() {
    let mut program = Program::new();
    program.classes.insert(
        "Parent".into(),
        test_class_definition(Some("java/lang/Object")),
    );
    program
        .classes
        .insert("Child".into(), test_class_definition(Some("Parent")));
    let mut host = DefaultNativeContext;
    let machine = program.machine(Limits::default(), false, &mut host);
    for (actual, target, expected) in [
        ("[LChild;", "[LParent;", true),
        ("[LParent;", "[LChild;", false),
        ("[I", "[I", true),
        ("[I", "[J", false),
        ("[I", "[Ljava/lang/Object;", false),
        ("[[I", "[Ljava/lang/Object;", true),
        ("[[I", "[Ljava/lang/Cloneable;", true),
        ("[[I", "[Ljava/io/Serializable;", true),
        ("[[I", "[LIface;", false),
        ("[Ljava/lang/Object;", "[[I", false),
    ] {
        // Each common outer dimension preserves component assignability.
        // Exercise ordinary arrays and the classfile limit of 255 dimensions.
        for depth in [0, 1, 253] {
            let prefix = "[".repeat(depth);
            let actual = format!("{prefix}{actual}");
            let target = format!("{prefix}{target}");
            assert_eq!(
                machine.is_instance(&actual, &target),
                expected,
                "{actual} -> {target}"
            );
        }
    }
    for target in [
        "java/lang/Object",
        "java/lang/Cloneable",
        "java/io/Serializable",
    ] {
        assert!(machine.is_instance("[I", target));
        assert!(!machine.is_instance(target, "[I"));
    }
}

#[test]
fn duplicate_fields_are_rejected_before_linking_the_class() {
    let field = classfile::Member {
        access_flags: 1,
        name_index: 3,
        descriptor_index: 4,
        attributes: Vec::new(),
    };
    let class = ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: vec![
            None,
            Some(Constant::Utf8("test/Fields".into())),
            Some(Constant::Class { name_index: 1 }),
            Some(Constant::Utf8("value".into())),
            Some(Constant::Utf8("I".into())),
            Some(Constant::Utf8("J".into())),
        ],
        access_flags: 1,
        this_class: 2,
        super_class: 0,
        interfaces: Vec::new(),
        fields: vec![field.clone(), field],
        methods: Vec::new(),
        attributes: Vec::new(),
    };
    for access_flags in [1, 9] {
        let mut class = class.clone();
        class.fields[1].access_flags = access_flags;
        let mut program = Program::new();
        assert_eq!(
            program
                .add_class(&class, &Limits::default())
                .unwrap_err()
                .code(),
            "duplicate-field",
        );
        assert!(!program.contains_class("test/Fields"));
        assert_eq!(program.runtime_bytes, 0);
        assert_eq!(program.constant_pool_count, 0);

        // The descriptor distinguishes fields that share a name.
        class.fields[1].descriptor_index = 5;
        program.add_class(&class, &Limits::default()).unwrap();
        assert_eq!(program.classes["test/Fields"].fields.len(), 2);
    }
}

#[test]
fn method_parameter_slots_include_the_receiver_before_linking_the_class() {
    for (parameters, slots) in [
        ("I".repeat(254), 254),
        ("I".repeat(255), 255),
        ("I".repeat(256), 256),
        ("J".repeat(127), 254),
        (format!("{}I", "J".repeat(127)), 255),
        ("D".repeat(128), 256),
    ] {
        for flags in [0x0101, 0x0109] {
            let class = ClassFile {
                minor_version: 0,
                major_version: 48,
                constant_pool: vec![
                    None,
                    Some(Constant::Utf8("test/Parameters".into())),
                    Some(Constant::Class { name_index: 1 }),
                    Some(Constant::Utf8("method".into())),
                    Some(Constant::Utf8(format!("({parameters})V"))),
                ],
                access_flags: 1,
                this_class: 2,
                super_class: 0,
                interfaces: Vec::new(),
                fields: Vec::new(),
                methods: vec![classfile::Member {
                    access_flags: flags,
                    name_index: 3,
                    descriptor_index: 4,
                    attributes: Vec::new(),
                }],
                attributes: Vec::new(),
            };
            let mut program = Program::new();
            let result = program.add_class(&class, &Limits::default());
            assert_eq!(result.is_ok(), slots + usize::from(flags & 8 == 0) <= 255);
            if let Err(error) = result {
                assert_eq!(error.code(), "invalid-descriptor");
                assert!(!program.contains_class("test/Parameters"));
                assert_eq!(program.runtime_bytes, 0);
                assert_eq!(program.constant_pool_count, 0);
            }
        }
    }
}

#[test]
fn linkage_error_getfield_reports_missing_field_with_call_site() {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("FieldOwner".into())),
        Some(Constant::Fieldref {
            class_index: 1,
            name_and_type_index: 4,
        }),
        Some(Constant::NameAndType {
            name_index: 5,
            descriptor_index: 6,
        }),
        Some(Constant::Utf8("missing".into())),
        Some(Constant::Utf8("I".into())),
    ];
    let main = runtime_method(
        "T",
        "main",
        "()I",
        &[0xbb, 0, 1, 0x4b, 0x2a, 0xb4, 0, 3, 0x57, 0x03, 0xac],
        2,
        1,
        constants,
        true,
    );
    let mut program = Program::new();
    program.methods.insert(main.key.clone(), main);
    program
        .classes
        .insert("FieldOwner".into(), test_class_definition(None));

    let error = program
        .execute("T", "main", "()I", Limits::default(), false)
        .unwrap_err();

    assert_eq!(error.code(), "field-not-found");
    let message = error.message();
    assert!(message.contains("unresolved=FieldOwner::missing:I"));
    assert!(message.contains("call-site=T::main()I pc=5 opcode=getfield"));
    assert!(!message.contains('/'));
}

#[test]
fn field_resolution_searches_superinterfaces_before_superclasses() {
    let mut program = Program::new();
    program.classes.insert(
        "FieldInterface".into(),
        Class {
            super_name: Some("java/lang/Object".into()),
            interfaces: Vec::new(),
            fields: vec![Field {
                key: "FieldInterface.c:Ljavax/microedition/lcdui/Font;".into(),
                declaring_class: "FieldInterface".into(),
                kind: ValueKind::Reference,
                is_static: true,
                field_token: FieldToken::new(),
                instance_slot: None,
                initial: Value::Reference(None),
                constant_string: None,
            }],
            is_public: true,
            is_abstract: true,
            is_interface: true,
            no_arg_constructor: NoArgConstructor::Missing,
        },
    );
    program.classes.insert(
        "FieldOwner".into(),
        Class {
            super_name: Some("FieldParent".into()),
            interfaces: vec!["FieldInterface".into()],
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    let mut parent = test_class_definition(Some("java/lang/Object"));
    let mut field = program.classes["FieldInterface"].fields[0].clone();
    field.key = "FieldParent.c:Ljavax/microedition/lcdui/Font;".into();
    field.declaring_class = "FieldParent".into();
    field.field_token = FieldToken::new();
    parent.fields.push(field);
    program.classes.insert("FieldParent".into(), parent);
    let mut context = DefaultNativeContext;
    for expected in ["FieldInterface", "FieldParent"] {
        let machine = program.machine(Limits::default(), false, &mut context);
        let field = machine
            .find_field(&FieldRef {
                class: "FieldOwner".into(),
                name: "c".into(),
                descriptor: "Ljavax/microedition/lcdui/Font;".into(),
            })
            .unwrap();
        assert_eq!(field.declaring_class, expected);
        drop(machine);
        program
            .classes
            .get_mut("FieldOwner")
            .unwrap()
            .interfaces
            .clear();
    }
}

#[test]
fn linkage_error_virtual_reports_missing_method_with_call_site() {
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
        Some(Constant::Utf8("Abstract".into())),
        Some(Constant::Utf8("missing".into())),
        Some(Constant::Utf8("()V".into())),
    ];
    let main = runtime_method(
        "T",
        "main",
        "()I",
        &[0xbb, 0, 2, 0x4b, 0x2a, 0xb6, 0, 1, 0x03, 0xac],
        2,
        1,
        constants,
        true,
    );
    let mut program = Program::new();
    program.methods.insert(main.key.clone(), main);
    program.classes.insert(
        "Abstract".into(),
        test_class_definition(Some("java/lang/Object")),
    );

    let error = program
        .execute("T", "main", "()I", Limits::default(), false)
        .unwrap_err();

    assert_eq!(error.code(), "method-not-found");
    let message = error.message();
    assert!(message.contains("unresolved=Abstract::missing()V"));
    assert!(message.contains("call-site=T::main()I pc=5 opcode=invokevirtual"));
    assert!(!message.contains('/'));
}

#[test]
fn array_virtual_dispatch_starts_at_java_lang_object() {
    let object_method = runtime_method(
        "java/lang/Object",
        "getClass",
        "()Ljava/lang/Class;",
        &[0x01, 0xb0],
        1,
        1,
        Vec::new(),
        false,
    );
    let expected = object_method.key.clone();
    let mut program = Program::new();
    program
        .methods
        .insert(object_method.key.clone(), object_method);
    let mut context = DefaultNativeContext;
    let machine = program.machine(Limits::default(), false, &mut context);

    assert_eq!(
        machine
            .resolve_virtual("[Ljava/lang/String;", "getClass", "()Ljava/lang/Class;")
            .unwrap(),
        expected
    );
    assert_eq!(
        machine
            .resolve_symbolic_method(&MethodKey {
                class: "[Ljava/lang/String;".into(),
                name: "getClass".into(),
                descriptor: "()Ljava/lang/Class;".into(),
            })
            .unwrap(),
        expected
    );
}

#[test]
fn linkage_error_incompatible_class_change_reports_receiver_mismatch() {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("Actual".into())),
        Some(Constant::Class { name_index: 4 }),
        Some(Constant::Utf8("Expected".into())),
        Some(Constant::Methodref {
            class_index: 3,
            name_and_type_index: 6,
        }),
        Some(Constant::NameAndType {
            name_index: 7,
            descriptor_index: 8,
        }),
        Some(Constant::Utf8("answer".into())),
        Some(Constant::Utf8("()I".into())),
    ];
    let main = runtime_method(
        "T",
        "main",
        "()I",
        &[0xbb, 0, 1, 0x4b, 0x2a, 0xb6, 0, 5, 0xac],
        2,
        1,
        constants,
        true,
    );
    let mut program = Program::new();
    program.methods.insert(main.key.clone(), main);
    program.classes.insert(
        "Expected".into(),
        test_class_definition(Some("java/lang/Object")),
    );
    program.classes.insert(
        "Actual".into(),
        test_class_definition(Some("java/lang/Object")),
    );
    let expected_method = runtime_method(
        "Expected",
        "answer",
        "()I",
        &[0x10, 42, 0xac],
        1,
        1,
        vec![None],
        false,
    );
    program
        .methods
        .insert(expected_method.key.clone(), expected_method);

    let error = program
        .execute("T", "main", "()I", Limits::default(), false)
        .unwrap_err();

    assert_eq!(error.code(), "incompatible-class-change");
    let message = error.message();
    assert!(message.contains("unresolved=Expected::answer()I"));
    assert!(message.contains("call-site=T::main()I pc=5 opcode=invokevirtual"));
    assert!(!message.contains('/'));

    // The same instance method and call site must work for a compatible receiver.
    program.classes.get_mut("Actual").unwrap().super_name = Some("Expected".into());
    assert_eq!(
        program
            .execute("T", "main", "()I", Limits::default(), false)
            .unwrap()
            .value,
        Some(Value::Int(42))
    );
}
