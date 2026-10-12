use super::*;

pub(crate) fn resumable_initializer_program(initializer_code: &[u8]) -> (Program, Method) {
    let initializer = runtime_method(
        "Deferred",
        "<clinit>",
        "()V",
        initializer_code,
        1,
        0,
        Vec::new(),
        true,
    );
    let trigger = runtime_method("Worker", "run", "()V", &[0xb1], 0, 1, Vec::new(), false);
    let mut program = Program::new();
    program.classes.insert(
        "Deferred".into(),
        Class {
            super_name: Some("java/lang/Object".into()),
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    program.methods.insert(initializer.key.clone(), initializer);
    (program, trigger)
}

fn throwable_initializer_program(throwable_class: &str) -> (Program, Method) {
    let constants = vec![
        None,
        Some(Constant::Fieldref {
            class_index: 2,
            name_and_type_index: 3,
        }),
        Some(Constant::Class { name_index: 4 }),
        Some(Constant::NameAndType {
            name_index: 5,
            descriptor_index: 6,
        }),
        Some(Constant::Utf8("Deferred".into())),
        Some(Constant::Utf8("failure".into())),
        Some(Constant::Utf8("Ljava/lang/Throwable;".into())),
    ];
    let initializer = runtime_method(
        "Deferred",
        "<clinit>",
        "()V",
        // Clear the static slot before athrow so the original exception
        // is rooted only by the wrapper-allocation path under test.
        &[0xb2, 0, 1, 0x01, 0xb3, 0, 1, 0xbf],
        2,
        0,
        constants,
        true,
    );
    let trigger = runtime_method("Worker", "run", "()V", &[0xb1], 0, 1, Vec::new(), false);
    let mut deferred = test_class_definition(Some("java/lang/Object"));
    deferred.fields.push(Field {
        key: "Deferred.failure:Ljava/lang/Throwable;".into(),
        declaring_class: "Deferred".into(),
        kind: ValueKind::Reference,
        is_static: true,
        field_token: FieldToken::new(),
        instance_slot: None,
        initial: Value::Reference(None),
        constant_string: None,
    });
    let mut program = program_with_exception(throwable_class);
    program.classes.insert("Deferred".into(), deferred);
    program.classes.insert(
        "java/lang/Error".into(),
        test_class_definition(Some("java/lang/Throwable")),
    );
    program.classes.insert(
        "java/lang/ExceptionInInitializerError".into(),
        test_class_definition(Some("java/lang/Error")),
    );
    program.classes.insert(
        "java/lang/NoClassDefFoundError".into(),
        test_class_definition(Some("java/lang/Error")),
    );
    program.methods.insert(initializer.key.clone(), initializer);
    (program, trigger)
}

#[test]
fn exception_initializer_owner_gets_eiie_and_waiter_gets_ncdfe() {
    let (program, trigger) = throwable_initializer_program("java/lang/RuntimeException");
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let original = machine
        .heap
        .managed
        .allocate_object("java/lang/RuntimeException", HashMap::new())
        .unwrap();
    machine.classes.static_fields.insert(
        "Deferred.failure:Ljava/lang/Throwable;".into(),
        Value::Reference(Some(original)),
    );
    let owner = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let waiter = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = owner.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(owner, ThreadState::Running);
    machine.scheduler.quantum_remaining = 0;
    let ClassInitializationOutcome::Suspend(owner_continuation) = machine
        .request_class_initialization("Deferred", 1, Some(&trigger))
        .unwrap()
    else {
        panic!("the owner must suspend before executing <clinit>");
    };

    machine.scheduler.current_thread = waiter.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiter, ThreadState::Running);
    let ClassInitializationOutcome::Suspend(waiter_continuation) = machine
        .request_class_initialization("Deferred", 1, Some(&trigger))
        .unwrap()
    else {
        panic!("the competing worker must wait for initialization");
    };

    machine.scheduler.current_thread = owner.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(owner, ThreadState::Running);
    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    let CallOutcome::Throw(owner_failure) = machine
        .resume_suspended_call(owner_continuation, 1)
        .unwrap()
    else {
        panic!("the owner must receive ExceptionInInitializerError");
    };
    assert_ne!(owner_failure, original);
    assert_eq!(
        machine.object_class(owner_failure).unwrap(),
        "java/lang/ExceptionInInitializerError"
    );
    assert!(machine.heap.managed.get(original).is_ok());
    assert!(
        machine
            .classes
            .failed_initialization
            .contains_key("Deferred")
    );
    assert_eq!(
        machine.scheduler.thread_states.get(&waiter),
        Some(&ThreadState::Runnable)
    );

    machine.scheduler.current_thread = waiter.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiter, ThreadState::Running);
    let CallOutcome::Throw(waiter_failure) = machine
        .resume_suspended_call(waiter_continuation, 1)
        .unwrap()
    else {
        panic!("a waiter must observe NoClassDefFoundError");
    };
    assert_eq!(
        machine.object_class(waiter_failure).unwrap(),
        "java/lang/NoClassDefFoundError"
    );
}

#[test]
fn error_from_initializer_is_propagated_without_wrapping() {
    let (program, trigger) = throwable_initializer_program("java/lang/Error");
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let original = machine
        .heap
        .managed
        .allocate_object("java/lang/Error", HashMap::new())
        .unwrap();
    machine.classes.static_fields.insert(
        "Deferred.failure:Ljava/lang/Throwable;".into(),
        Value::Reference(Some(original)),
    );
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = worker.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Running);
    machine.scheduler.quantum_remaining = WORKER_QUANTUM;

    let ClassInitializationOutcome::Throw(failure) = machine
        .request_class_initialization("Deferred", 1, Some(&trigger))
        .unwrap()
    else {
        panic!("java/lang/Error must escape <clinit>");
    };

    assert_eq!(failure, original);
    assert!(
        machine
            .classes
            .failed_initialization
            .contains_key("Deferred")
    );
}

#[test]
fn suspended_parent_failure_marks_child_erroneous_without_rewrapping() {
    let (mut program, trigger) = throwable_initializer_program("java/lang/RuntimeException");
    program.classes.insert(
        "DeferredChild".into(),
        test_class_definition(Some("Deferred")),
    );
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let original = machine
        .heap
        .managed
        .allocate_object("java/lang/RuntimeException", HashMap::new())
        .unwrap();
    machine.classes.static_fields.insert(
        "Deferred.failure:Ljava/lang/Throwable;".into(),
        Value::Reference(Some(original)),
    );
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = worker.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Running);
    machine.scheduler.quantum_remaining = 0;
    let ClassInitializationOutcome::Suspend(continuation) = machine
        .request_class_initialization("DeferredChild", 1, Some(&trigger))
        .unwrap()
    else {
        panic!("the child must retain its suspended-parent phase");
    };

    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    let CallOutcome::Throw(first_failure) = machine.resume_suspended_call(continuation, 1).unwrap()
    else {
        panic!("the parent's first-failure throwable must escape the child request");
    };

    assert_eq!(
        machine.object_class(first_failure).unwrap(),
        "java/lang/ExceptionInInitializerError"
    );
    assert!(
        machine
            .classes
            .failed_initialization
            .contains_key("Deferred")
    );
    assert!(
        machine
            .classes
            .failed_initialization
            .contains_key("DeferredChild")
    );
    assert!(machine.classes.initializing.is_empty());
}

#[test]
fn static_slots_are_prepared_before_suspended_parent_finishes() {
    let (mut program, trigger) = resumable_initializer_program(&[0xb1]);
    let mut child = test_class_definition(Some("Deferred"));
    child.fields.extend([
        Field {
            key: "DeferredChild.defaultValue:I".into(),
            declaring_class: "DeferredChild".into(),
            kind: ValueKind::Int,
            is_static: true,
            field_token: FieldToken::new(),
            instance_slot: None,
            initial: Value::Int(0),
            constant_string: None,
        },
        Field {
            key: "DeferredChild.constantValue:I".into(),
            declaring_class: "DeferredChild".into(),
            kind: ValueKind::Int,
            is_static: true,
            field_token: FieldToken::new(),
            instance_slot: None,
            initial: Value::Int(9),
            constant_string: None,
        },
        Field {
            key: "DeferredChild.text:Ljava/lang/String;".into(),
            declaring_class: "DeferredChild".into(),
            kind: ValueKind::Reference,
            is_static: true,
            field_token: FieldToken::new(),
            instance_slot: None,
            initial: Value::Reference(None),
            constant_string: Some("prepared".encode_utf16().collect()),
        },
    ]);
    program.classes.insert("DeferredChild".into(), child);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = worker.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Running);
    machine.scheduler.quantum_remaining = 0;

    let ClassInitializationOutcome::Suspend(continuation) = machine
        .request_class_initialization("DeferredChild", 1, Some(&trigger))
        .unwrap()
    else {
        panic!("the parent initializer must suspend");
    };
    assert_eq!(
        machine
            .classes
            .static_fields
            .get("DeferredChild.defaultValue:I"),
        Some(&Value::Int(0))
    );
    assert_eq!(
        machine
            .classes
            .static_fields
            .get("DeferredChild.constantValue:I"),
        Some(&Value::Int(9))
    );
    let Some(Value::Reference(Some(text))) = machine
        .classes
        .static_fields
        .get("DeferredChild.text:Ljava/lang/String;")
        .copied()
    else {
        panic!("the String ConstantValue must be interned during preparation");
    };
    assert_eq!(
        String::from_utf16_lossy(&machine.heap.string_values[&text]),
        "prepared"
    );
    assert!(!machine.classes.initialized.contains("DeferredChild"));

    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    assert!(matches!(
        machine.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Return(None)
    ));
    assert!(machine.classes.initialized.contains("DeferredChild"));
}

#[test]
fn string_constant_preparation_keeps_triggering_frame_roots() {
    let mut class = test_class_definition(Some("java/lang/Object"));
    class.fields.push(Field {
        key: "Prepared.text:Ljava/lang/String;".into(),
        declaring_class: "Prepared".into(),
        kind: ValueKind::Reference,
        is_static: true,
        field_token: FieldToken::new(),
        instance_slot: None,
        initial: Value::Reference(None),
        constant_string: Some("x".encode_utf16().collect()),
    });
    let read = runtime_method(
        "Worker",
        "read",
        "(Ljava/lang/Object;)Ljava/lang/String;",
        &[0xb2, 0, 1, 0xb0],
        1,
        1,
        vec![
            None,
            Some(Constant::Fieldref {
                class_index: 2,
                name_and_type_index: 3,
            }),
            Some(Constant::Class { name_index: 4 }),
            Some(Constant::NameAndType {
                name_index: 5,
                descriptor_index: 6,
            }),
            Some(Constant::Utf8("Prepared".into())),
            Some(Constant::Utf8("text".into())),
            Some(Constant::Utf8("Ljava/lang/String;".into())),
        ],
        true,
    );
    let mut program = Program::new();
    program.classes.insert("Prepared".into(), class);
    let mut context = DefaultNativeContext;
    let limits = Limits {
        max_heap_bytes: 40,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    let live = machine
        .heap
        .managed
        .allocate_object("Live", HashMap::new())
        .unwrap();
    let garbage = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 8)
        .unwrap();

    let CallOutcome::Return(Some(Value::Reference(Some(text)))) = machine
        .call(&read, vec![Value::Reference(Some(live))], 1)
        .unwrap()
    else {
        panic!("getstatic must return the prepared String ConstantValue");
    };

    assert!(machine.heap.managed.get(live).is_ok());
    assert!(machine.heap.managed.get(text).is_ok());
    assert_eq!(
        machine.heap.managed.get(garbage).unwrap_err(),
        HeapError::InvalidHandle
    );
}
