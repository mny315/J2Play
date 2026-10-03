use super::*;

#[test]
fn worker_class_initializer_resumes_before_becoming_initialized() {
    let (mut program, _) = resumable_initializer_program(&[0xb1]);
    program
        .classes
        .get_mut("Deferred")
        .unwrap()
        .fields
        .push(Field {
            key: "Deferred.value:I".into(),
            declaring_class: "Deferred".into(),
            kind: ValueKind::Int,
            is_static: true,
            field_token: FieldToken::new(),
            instance_slot: None,
            initial: Value::Int(7),
            constant_string: None,
        });
    let trigger = runtime_method(
        "Worker",
        "read",
        "()I",
        &[0xb2, 0, 1, 0xac],
        1,
        0,
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
            Some(Constant::Utf8("Deferred".into())),
            Some(Constant::Utf8("value".into())),
            Some(Constant::Utf8("I".into())),
        ],
        true,
    );
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
    // The triggering getstatic consumes the final instruction in this
    // quantum; <clinit> itself must therefore become the saved child.
    machine.scheduler.quantum_remaining = 1;

    let CallOutcome::Suspend(continuation) = machine.call(&trigger, Vec::new(), 1).unwrap() else {
        panic!("the exhausted worker quantum must suspend <clinit>");
    };
    assert_eq!(
        machine.classes.initializing.get("Deferred"),
        Some(&worker.to_raw())
    );
    assert!(!machine.classes.initialized.contains("Deferred"));

    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    let CallOutcome::Return(Some(Value::Int(value))) =
        machine.resume_suspended_call(continuation, 1).unwrap()
    else {
        panic!("getstatic must be retried after initialization");
    };
    assert_eq!(value, 7);
    assert!(machine.classes.initialized.contains("Deferred"));
    assert!(!machine.classes.initializing.contains_key("Deferred"));
}

#[test]
fn subclass_initialization_continues_after_suspended_parent() {
    let (mut program, trigger) = resumable_initializer_program(&[0xb1]);
    program.classes.insert(
        "DeferredChild".into(),
        Class {
            super_name: Some("Deferred".into()),
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
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
        panic!("the child must retain its phase while its parent is suspended");
    };
    assert_eq!(
        machine.classes.initializing.get("Deferred"),
        Some(&worker.to_raw())
    );
    assert_eq!(
        machine.classes.initializing.get("DeferredChild"),
        Some(&worker.to_raw())
    );

    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    assert!(matches!(
        machine.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Return(None)
    ));
    assert!(machine.classes.initialized.contains("Deferred"));
    assert!(machine.classes.initialized.contains("DeferredChild"));
    assert!(machine.classes.initializing.is_empty());
}

#[test]
fn competing_worker_waits_for_class_initializer_owner() {
    let (program, trigger) = resumable_initializer_program(&[0xb1]);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
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
        panic!("the owner must suspend <clinit>");
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
        panic!("a competing worker must wait for the owner");
    };
    assert_eq!(
        machine.scheduler.thread_states.get(&waiter),
        Some(&ThreadState::Sleeping)
    );
    assert_eq!(
        machine.classes.initialization_waiters.get("Deferred"),
        Some(&vec![waiter])
    );

    let mut interrupt = runtime_method(
        "java/lang/Thread",
        "interrupt",
        "()V",
        &[0xb1],
        0,
        1,
        Vec::new(),
        false,
    );
    interrupt.is_native = true;
    machine.scheduler.current_thread = owner.to_raw();
    assert!(matches!(
        machine
            .invoke_vm_native(&interrupt, &[Value::Reference(Some(waiter))], 1,)
            .unwrap(),
        Some(CallOutcome::Return(None))
    ));
    assert_eq!(
        machine.scheduler.thread_states.get(&waiter),
        Some(&ThreadState::Runnable)
    );

    machine
        .scheduler
        .thread_states
        .insert(owner, ThreadState::Running);
    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    assert!(matches!(
        machine
            .resume_suspended_call(owner_continuation, 1)
            .unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(
        machine.scheduler.thread_states.get(&waiter),
        Some(&ThreadState::Runnable)
    );
    assert!(
        !machine
            .classes
            .initialization_waiters
            .contains_key("Deferred")
    );
    assert_eq!(
        machine
            .scheduler
            .runnable_threads
            .iter()
            .filter(|candidate| **candidate == waiter)
            .count(),
        1
    );
    assert_eq!(machine.scheduler.runnable_threads.pop_front(), Some(waiter));

    machine.scheduler.current_thread = waiter.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiter, ThreadState::Running);
    assert!(matches!(
        machine
            .resume_suspended_call(waiter_continuation, 1)
            .unwrap(),
        CallOutcome::Return(None)
    ));
}

#[test]
fn suspended_initializer_failure_marks_class_erroneous() {
    let (program, trigger) = resumable_initializer_program(&[0x03, 0xac]);
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
        .request_class_initialization("Deferred", 1, Some(&trigger))
        .unwrap()
    else {
        panic!("the owner must suspend <clinit>");
    };

    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    let error = match machine.resume_suspended_call(continuation, 1) {
        Err(error) => error,
        Ok(_) => panic!("the invalid initializer return must fail"),
    };
    assert_eq!(error.code(), "type-mismatch");
    assert!(!machine.classes.initializing.contains_key("Deferred"));
    assert!(machine.classes.failed_initialization["Deferred"].contains("type-mismatch"));

    machine.scheduler.current_thread = MAIN_THREAD_ID;
    let repeated = machine.initialize_class("Deferred", 1).unwrap_err();
    assert_eq!(repeated.code(), "class-not-found");
    assert!(repeated.message().contains("type-mismatch"));
}

#[test]
fn class_initializer_can_initialize_subclass_of_current_class() {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("Child".into())),
    ];
    let initializer = runtime_method(
        "Parent",
        "<clinit>",
        "()V",
        &[0xbb, 0, 1, 0x57, 0xb1],
        1,
        0,
        constants,
        true,
    );
    let mut program = Program::new();
    program.methods.insert(initializer.key.clone(), initializer);
    program.classes.insert(
        "Parent".into(),
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
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);

    machine.initialize_class("Parent", 1).unwrap();

    assert!(machine.classes.initialized.contains("Parent"));
    assert!(machine.classes.initialized.contains("Child"));
}
