use super::*;

#[test]
fn class_for_name_resolves_only_the_requested_binary_name() {
    for default_package in [false, true] {
        let mut program = program_with_core_natives();
        for class in ["test/Caller", "test/Target", "other/Target"] {
            program.classes.insert(
                class.into(),
                test_class_definition(Some("java/lang/Object")),
            );
        }
        if default_package {
            program.classes.insert(
                "Target".into(),
                test_class_definition(Some("java/lang/Object")),
            );
        }
        let caller = runtime_method(
            "test/Caller",
            "lookup",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[0x2a, 0xb8, 0, 1, 0xb0],
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
                Some(Constant::Utf8("java/lang/Class".into())),
                Some(Constant::Utf8("forName".into())),
                Some(Constant::Utf8(
                    "(Ljava/lang/String;)Ljava/lang/Class;".into(),
                )),
            ],
            true,
        );
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        for (name, expected) in [
            ("Target", default_package.then_some("Target")),
            ("test.Target", Some("test/Target")),
            ("other.Target", Some("other/Target")),
            ("java.lang.String", Some("java/lang/String")),
            ("test/Target", None),
            ("test.example/Target", None),
            ("test..Target", None),
            (".test.Target", None),
            ("test.Target.", None),
            ("", None),
            ("[Ljava.lang.String;", None),
        ] {
            let string = machine.allocate_dynamic_string(name, &[], &[]).unwrap();
            let actual = match machine
                .call(&caller, [Value::Reference(Some(string))], 1)
                .unwrap()
            {
                CallOutcome::Return(Some(Value::Reference(Some(class)))) => {
                    Some(machine.class_name_for_handle(class).unwrap())
                }
                CallOutcome::Throw(exception) => {
                    assert_eq!(
                        machine.object_class(exception).unwrap(),
                        "java/lang/ClassNotFoundException"
                    );
                    None
                }
                _ => panic!("Class.forName did not return or throw"),
            };
            assert_eq!(
                actual.as_deref(),
                expected,
                "name={name}, default_package={default_package}"
            );
        }
    }
}

#[test]
fn class_for_name_does_not_replace_unpaired_utf16() {
    let mut program = program_with_core_natives();
    program.classes.insert(
        "test/�Target".into(),
        test_class_definition(Some("java/lang/Object")),
    );
    let method = &program.methods[&MethodKey {
        class: "java/lang/Class".into(),
        name: "forName".into(),
        descriptor: "(Ljava/lang/String;)Ljava/lang/Class;".into(),
    }];
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let name = machine
        .allocate_dynamic_string("test.�Target", &[], &[])
        .unwrap();
    let mut units = machine.heap.string_values.get(&name).unwrap().clone();
    units[5] = 0xd800;
    machine.heap.string_values.insert(name, units);
    let CallOutcome::Throw(exception) = machine
        .call(method, [Value::Reference(Some(name))], 1)
        .unwrap()
    else {
        panic!("Class.forName replaced an unpaired surrogate and loaded a different class");
    };
    assert_eq!(
        machine.object_class(exception).unwrap(),
        "java/lang/ClassNotFoundException"
    );
    assert!(!machine.classes.initialized.contains("test/�Target"));
    machine
        .heap
        .string_values
        .insert(name, "test.�Target".encode_utf16().collect());
    let CallOutcome::Return(Some(Value::Reference(Some(class)))) = machine
        .call(method, [Value::Reference(Some(name))], 1)
        .unwrap()
    else {
        panic!("Class.forName rejected a valid Unicode class name");
    };
    assert_eq!(
        machine.class_name_for_handle(class).unwrap(),
        "test/�Target"
    );
}

#[test]
fn class_new_instance_rejects_array_classes_with_instantiation_exception() {
    let program = program_with_core_natives();
    let get_class = &program.methods[&MethodKey {
        class: "java/lang/Object".into(),
        name: "getClass".into(),
        descriptor: "()Ljava/lang/Class;".into(),
    }];
    let new_instance = &program.methods[&MethodKey {
        class: "java/lang/Class".into(),
        name: "newInstance".into(),
        descriptor: "()Ljava/lang/Object;".into(),
    }];
    for kind in [
        ArrayKind::Boolean,
        ArrayKind::Byte,
        ArrayKind::Char,
        ArrayKind::Short,
        ArrayKind::Int,
        ArrayKind::Long,
        ArrayKind::Float,
        ArrayKind::Double,
        ArrayKind::Reference("java/lang/Object".into()),
        ArrayKind::Reference("[I".into()),
        ArrayKind::Reference("[Ljava/lang/String;".into()),
    ] {
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let array = machine.heap.managed.allocate_array(kind, 0).unwrap();
        let CallOutcome::Return(Some(class)) = machine
            .call(get_class, [Value::Reference(Some(array))], 1)
            .unwrap()
        else {
            panic!("array.getClass() did not return its Class");
        };
        let CallOutcome::Throw(exception) = machine.call(new_instance, [class], 1).unwrap() else {
            panic!("Class.newInstance accepted an array class");
        };
        assert_eq!(
            machine.object_class(exception).unwrap(),
            "java/lang/InstantiationException"
        );
    }
}

#[test]
fn class_is_interface_distinguishes_interfaces_from_classes_and_arrays() {
    let program = program_with_core_natives();
    let method = program
        .methods
        .get(&MethodKey {
            class: "java/lang/Class".into(),
            name: "isInterface".into(),
            descriptor: "()Z".into(),
        })
        .expect("Class.isInterface must be available");
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    for (name, expected) in [
        ("java/lang/Object", false),
        ("java/lang/String", false),
        ("java/lang/Runnable", true),
        ("java/util/Enumeration", true),
        ("[I", false),
        ("[[Ljava/lang/Runnable;", false),
    ] {
        let class = machine.intern_java_class(name).unwrap();
        assert!(
            matches!(
                machine.call(method, [Value::Reference(Some(class))], 1).unwrap(),
                CallOutcome::Return(Some(Value::Int(value))) if value == i32::from(expected)
            ),
            "{name}"
        );
    }
}

#[test]
fn class_to_string_describes_the_represented_type() {
    let program = program_with_core_natives();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let key = machine
        .resolve_virtual("java/lang/Class", "toString", "()Ljava/lang/String;")
        .unwrap();
    let method = &program.methods[&key];
    for (name, expected) in [
        ("java/lang/Object", "class java.lang.Object"),
        ("java/lang/String", "class java.lang.String"),
        ("java/lang/Runnable", "interface java.lang.Runnable"),
        ("java/util/Enumeration", "interface java.util.Enumeration"),
        ("[I", "class [I"),
        ("[[Ljava/lang/Runnable;", "class [[Ljava.lang.Runnable;"),
    ] {
        let class = machine.intern_java_class(name).unwrap();
        let CallOutcome::Return(Some(Value::Reference(Some(string)))) = machine
            .call(method, [Value::Reference(Some(class))], 1)
            .unwrap()
        else {
            panic!("Class.toString did not return a string");
        };
        assert_eq!(
            machine.heap.string_values.get(&string),
            Some(&expected.encode_utf16().collect::<Vec<_>>())
        );
    }
}

#[test]
fn class_for_name_resumes_suspended_initialization() {
    let (mut program, _) = resumable_initializer_program(&[0xb1]);
    program
        .classes
        .insert("java/lang/Class".into(), test_class_definition(None));
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let name = machine
        .allocate_dynamic_string("Deferred", &[], &[])
        .unwrap();
    let mut for_name = runtime_method(
        "java/lang/Class",
        "forName",
        "(Ljava/lang/String;)Ljava/lang/Class;",
        &[0xb0],
        1,
        1,
        Vec::new(),
        true,
    );
    for_name.is_native = true;
    machine.scheduler.current_thread = worker.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Running);
    machine.scheduler.quantum_remaining = 0;

    let CallOutcome::Suspend(continuation) = machine
        .call(&for_name, vec![Value::Reference(Some(name))], 1)
        .unwrap()
    else {
        panic!("Class.forName must retain a suspended initialization");
    };
    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    let CallOutcome::Return(Some(Value::Reference(Some(class_object)))) =
        machine.resume_suspended_call(continuation, 1).unwrap()
    else {
        panic!("Class.forName must return its Class after resume");
    };
    assert_eq!(machine.classes.objects["Deferred"], class_object);
    assert!(machine.classes.initialized.contains("Deferred"));
}

#[test]
fn class_new_instance_resumes_suspended_initialization() {
    let (mut program, _) = resumable_initializer_program(&[0xb1]);
    program
        .classes
        .insert("java/lang/Object".into(), test_class_definition(None));
    let constructor = runtime_method(
        "Deferred",
        "<init>",
        "()V",
        &[0xb1],
        0,
        1,
        Vec::new(),
        false,
    );
    program.methods.insert(constructor.key.clone(), constructor);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let class_object = machine
        .heap
        .managed
        .allocate_object("java/lang/Class", HashMap::new())
        .unwrap();
    machine
        .classes
        .objects
        .insert("Deferred".into(), class_object);
    let mut new_instance = runtime_method(
        "java/lang/Class",
        "newInstance",
        "()Ljava/lang/Object;",
        &[0xb0],
        1,
        1,
        Vec::new(),
        false,
    );
    new_instance.is_native = true;
    machine.scheduler.current_thread = worker.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Running);
    machine.scheduler.quantum_remaining = 0;

    let CallOutcome::Suspend(continuation) = machine
        .call(&new_instance, vec![Value::Reference(Some(class_object))], 1)
        .unwrap()
    else {
        panic!("Class.newInstance must retain a suspended initialization");
    };
    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    let CallOutcome::Return(Some(Value::Reference(Some(instance)))) =
        machine.resume_suspended_call(continuation, 1).unwrap()
    else {
        panic!("Class.newInstance must construct the object after resume");
    };
    assert_eq!(machine.object_class(instance).unwrap(), "Deferred");
    assert!(machine.classes.initialized.contains("Deferred"));
}
