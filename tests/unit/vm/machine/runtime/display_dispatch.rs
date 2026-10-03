use super::*;

#[test]
fn timer_thread_yield_services_display_turn_without_worker_suspend() {
    let mut yield_method = runtime_method(
        "java/lang/Thread",
        "yield",
        "()V",
        &[],
        0,
        0,
        vec![None],
        true,
    );
    yield_method.is_native = true;
    let idle = runtime_method(
        "javax/microedition/lcdui/Display",
        "__hostIdle",
        "()V",
        &[0xb1],
        0,
        0,
        vec![None],
        true,
    );
    let mut program = Program::new();
    program
        .methods
        .insert(yield_method.key.clone(), yield_method.clone());
    program.methods.insert(idle.key.clone(), idle);

    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let timer_thread = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = timer_thread.to_raw();
    machine.scheduler.dispatching_timer = true;
    machine.scheduler.quantum_remaining = u64::MAX;
    machine
        .classes
        .initialized
        .insert("javax/microedition/lcdui/Display".into());

    let outcome = machine.call(&yield_method, Vec::new(), 1).unwrap();

    assert!(matches!(outcome, CallOutcome::Return(None)));
    assert_eq!(machine.execution.instructions, 1);
    assert!(!machine.scheduler.suspend_requested);
    assert!(!machine.scheduler.dispatching_display);
}

#[test]
fn main_thread_sleep_services_one_deferred_display_turn() {
    let mut program = Program::new();
    let host_idle = runtime_method(
        "javax/microedition/lcdui/Display",
        "__hostIdle",
        "()V",
        &[0xb1],
        0,
        0,
        Vec::new(),
        true,
    );
    program.methods.insert(host_idle.key.clone(), host_idle);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    machine
        .classes
        .initialized
        .insert("javax/microedition/lcdui/Display".to_owned());
    let sleep = runtime_method(
        "java/lang/Thread",
        "sleep",
        "(J)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        true,
    );

    let outcome = machine
        .invoke_vm_native(&sleep, &[Value::Long(0)], 1)
        .unwrap();

    assert!(matches!(outcome, Some(CallOutcome::Return(None))));
    assert_eq!(machine.execution.instructions, 1);
    assert!(!machine.scheduler.dispatching_display);
}

#[test]
fn main_thread_sleep_isolates_deferred_display_failure() {
    let mut program = program_with_exception("DisplayCallbackException");
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("DisplayCallbackException".to_owned())),
    ];
    let host_idle = runtime_method(
        "javax/microedition/lcdui/Display",
        "__hostIdle",
        "()V",
        &[0xbb, 0, 1, 0xbf],
        1,
        0,
        constants,
        true,
    );
    program.methods.insert(host_idle.key.clone(), host_idle);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    machine
        .classes
        .initialized
        .insert("javax/microedition/lcdui/Display".to_owned());
    let sleep = runtime_method(
        "java/lang/Thread",
        "sleep",
        "(J)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        true,
    );

    let outcome = machine
        .invoke_vm_native(&sleep, &[Value::Long(0)], 1)
        .unwrap();

    assert!(matches!(outcome, Some(CallOutcome::Return(None))));
    assert_eq!(
        machine.execution.instructions, 2,
        "the throwing callback must run"
    );
    assert!(!machine.scheduler.dispatching_display);
}

#[test]
fn main_thread_indefinite_wait_services_deferred_display_notifier() {
    const DISPLAY_CLASS: &str = "javax/microedition/lcdui/Display";
    const MONITOR_FIELD: &str =
        "javax/microedition/lcdui/Display.deferredMonitor:Ljava/lang/Object;";
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
        Some(Constant::Utf8(DISPLAY_CLASS.to_owned())),
        Some(Constant::Utf8("deferredMonitor".to_owned())),
        Some(Constant::Utf8("Ljava/lang/Object;".to_owned())),
        Some(Constant::Methodref {
            class_index: 8,
            name_and_type_index: 9,
        }),
        Some(Constant::Class { name_index: 10 }),
        Some(Constant::NameAndType {
            name_index: 11,
            descriptor_index: 12,
        }),
        Some(Constant::Utf8("java/lang/Object".to_owned())),
        Some(Constant::Utf8("notifyAll".to_owned())),
        Some(Constant::Utf8("()V".to_owned())),
    ];
    let host_idle = runtime_method(
        DISPLAY_CLASS,
        "__hostIdle",
        "()V",
        &[
            0xb2, 0, 1,    // getstatic deferredMonitor
            0x59, // dup
            0x4b, // astore_0
            0xc2, // monitorenter
            0x2a, // aload_0
            0xb6, 0, 7,    // invokevirtual Object.notifyAll
            0x2a, // aload_0
            0xc3, // monitorexit
            0xb1, // return
        ],
        2,
        1,
        constants,
        true,
    );
    let mut notify_all = runtime_method(
        "java/lang/Object",
        "notifyAll",
        "()V",
        &[],
        0,
        1,
        Vec::new(),
        false,
    );
    notify_all.is_native = true;
    let mut wait = runtime_method(
        "java/lang/Object",
        "wait",
        "(J)V",
        &[],
        0,
        3,
        Vec::new(),
        false,
    );
    wait.is_native = true;
    let mut display = test_class_definition(Some("java/lang/Object"));
    display.fields.push(Field {
        key: MONITOR_FIELD.into(),
        declaring_class: DISPLAY_CLASS.to_owned(),
        kind: ValueKind::Reference,
        is_static: true,
        field_token: FieldToken::new(),
        instance_slot: None,
        initial: Value::Reference(None),
        constant_string: None,
    });
    let mut program = Program::new();
    program
        .classes
        .insert("java/lang/Object".to_owned(), test_class_definition(None));
    program.classes.insert(DISPLAY_CLASS.to_owned(), display);
    program.methods.insert(host_idle.key.clone(), host_idle);
    program.methods.insert(notify_all.key.clone(), notify_all);

    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    machine.classes.initialized.insert(DISPLAY_CLASS.to_owned());
    let monitor = machine
        .heap
        .managed
        .allocate_object("java/lang/Object", HashMap::new())
        .unwrap();
    machine
        .classes
        .static_fields
        .insert(MONITOR_FIELD.to_owned(), Value::Reference(Some(monitor)));
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: MAIN_THREAD_ID,
            depth: 2,
        },
    );

    let outcome = machine.wait_on_monitor(&wait, monitor, 0, 1).unwrap();

    assert!(matches!(outcome, CallOutcome::Return(None)));
    assert_eq!(machine.scheduler.monitors.get(&monitor).unwrap().depth, 2);
    assert!(!machine.scheduler.monitor_waiters.contains_key(&monitor));
    assert!(!machine.scheduler.notified_threads.contains(&MAIN_THREAD_ID));
    assert!(!machine.scheduler.dispatching_display);
}

#[test]
fn display_turn_waits_for_worker_owned_m3g_target() {
    let mut program = Program::new();
    let host_idle = runtime_method(
        "javax/microedition/lcdui/Display",
        "__hostIdle",
        "()V",
        &[0xb1],
        0,
        0,
        Vec::new(),
        true,
    );
    program.methods.insert(host_idle.key.clone(), host_idle);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    machine
        .classes
        .initialized
        .insert("javax/microedition/lcdui/Display".to_owned());
    let target = machine
        .heap
        .managed
        .allocate_object("javax/microedition/lcdui/Graphics", HashMap::new())
        .unwrap();
    machine.m3g.graphics.target = Some(target);

    assert!(matches!(
        machine.run_display_event_turn(1).unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(machine.execution.instructions, 0);
}

#[test]
fn display_transition_deferral_covers_midlet_startup_and_workers() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let target = machine
        .heap
        .managed
        .allocate_object("javax/microedition/lcdui/Canvas", HashMap::new())
        .unwrap();
    let should_defer = runtime_method(
        "javax/microedition/lcdui/Display",
        "__shouldDeferCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)Z",
        &[0x03, 0xac],
        1,
        2,
        Vec::new(),
        false,
    );
    let arguments = [Value::Reference(None), Value::Reference(Some(target))];

    assert!(matches!(
        machine
            .invoke_compatibility_intrinsic(&should_defer, &arguments, 1)
            .unwrap(),
        Some(CallOutcome::Return(Some(Value::Int(0))))
    ));

    machine.execution.call_stack.push(ActiveJavaStackFrame {
        key: ActiveJavaStackKey::Owned(Arc::new(MethodKey {
            class: "game/MainMidlet".to_owned(),
            name: "startApp".to_owned(),
            descriptor: "()V".to_owned(),
        })),
        bytecode_pc: 0,
    });
    assert!(matches!(
        machine
            .invoke_compatibility_intrinsic(&should_defer, &arguments, 1)
            .unwrap(),
        Some(CallOutcome::Return(Some(Value::Int(1))))
    ));

    machine.execution.call_stack.clear();
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = worker.to_raw();
    assert!(matches!(
        machine
            .invoke_compatibility_intrinsic(&should_defer, &arguments, 1)
            .unwrap(),
        Some(CallOutcome::Return(Some(Value::Int(1))))
    ));
}

#[test]
fn realtime_sleep_inside_canvas_paint_does_not_run_a_worker_recursively() {
    let sleep_constants = vec![
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
        Some(Constant::Utf8("java/lang/Thread".to_owned())),
        Some(Constant::Utf8("sleep".to_owned())),
        Some(Constant::Utf8("(J)V".to_owned())),
    ];
    let paint = runtime_method(
        "PaintCanvas",
        "paint",
        "(Ljavax/microedition/lcdui/Graphics;)V",
        &[0x10, 25, 0x85, 0xb8, 0, 1, 0xb1],
        2,
        2,
        sleep_constants,
        false,
    );
    let mut sleep = runtime_method(
        "java/lang/Thread",
        "sleep",
        "(J)V",
        &[],
        0,
        2,
        vec![None],
        true,
    );
    sleep.is_native = true;
    let mut program = Program::new();
    for (name, super_name) in [
        ("java/lang/Thread", None),
        ("javax/microedition/lcdui/Canvas", None),
        (
            "PaintCanvas",
            Some("javax/microedition/lcdui/Canvas".to_owned()),
        ),
    ] {
        program.classes.insert(
            name.to_owned(),
            Class {
                super_name,
                interfaces: Vec::new(),
                fields: Vec::new(),
                is_public: true,
                is_abstract: false,
                is_interface: false,
                no_arg_constructor: NoArgConstructor::Public,
            },
        );
    }
    program.methods.insert(sleep.key.clone(), sleep);
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: None,
        yield_25_millis: false,
        random_seed: None,
    };
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let worker = machine
            .heap
            .managed
            .allocate_object("worker", HashMap::new())
            .unwrap();
        machine
            .scheduler
            .thread_states
            .insert(worker, ThreadState::Runnable);
        machine.scheduler.runnable_threads.push_back(worker);
        machine.scheduler.host_driver_active = true;
        machine.scheduler.dispatching_display = true;
        let canvas = machine
            .heap
            .managed
            .allocate_object("PaintCanvas", HashMap::new())
            .unwrap();
        let graphics = machine
            .heap
            .managed
            .allocate_object("javax/microedition/lcdui/Graphics", HashMap::new())
            .unwrap();

        let outcome = machine
            .call(
                &paint,
                vec![
                    Value::Reference(Some(canvas)),
                    Value::Reference(Some(graphics)),
                ],
                1,
            )
            .unwrap();

        assert!(matches!(outcome, CallOutcome::Return(None)));
        assert_eq!(machine.scheduler.runnable_threads.front(), Some(&worker));
        assert_eq!(
            machine.scheduler.thread_states.get(&worker),
            Some(&ThreadState::Runnable)
        );
        assert!(!machine.scheduler.suspend_requested);
        assert!(machine.scheduler.driver_host_poll_yield);
        assert!(!machine.scheduler.dispatching_canvas_paint);
    }
    assert_eq!(context.paced_millis, 25);
    assert_eq!(context.now_millis, 125);
}

#[test]
fn realtime_sleep_inside_non_paint_display_callback_runs_a_worker() {
    let run = runtime_method("Worker", "run", "()V", &[0xb1], 0, 1, Vec::new(), false);
    let mut program = Program::new();
    program.classes.insert(
        "Worker".to_owned(),
        Class {
            super_name: None,
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    program.methods.insert(run.key.clone(), run);
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: None,
        yield_25_millis: false,
        random_seed: None,
    };
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let worker = machine
            .heap
            .managed
            .allocate_object("Worker", HashMap::new())
            .unwrap();
        machine
            .scheduler
            .thread_states
            .insert(worker, ThreadState::Runnable);
        machine.scheduler.runnable_threads.push_back(worker);
        machine.scheduler.host_driver_active = true;
        machine.scheduler.dispatching_display = true;
        let mut sleep = runtime_method(
            "java/lang/Thread",
            "sleep",
            "(J)V",
            &[0xb1],
            0,
            2,
            Vec::new(),
            true,
        );
        sleep.is_native = true;

        let outcome = machine
            .invoke_vm_native(&sleep, &[Value::Long(25)], 1)
            .unwrap();

        assert!(matches!(outcome, Some(CallOutcome::Return(None))));
        assert!(machine.scheduler.runnable_threads.is_empty());
        assert_eq!(
            machine.scheduler.thread_states.get(&worker),
            Some(&ThreadState::Terminated)
        );
        assert!(!machine.scheduler.suspend_requested);
        assert!(machine.scheduler.driver_host_poll_yield);
    }
    assert_eq!(context.paced_millis, 25);
    assert_eq!(context.now_millis, 125);
}

#[test]
fn top_level_display_idle_finishes_before_returning_to_the_driver() {
    let sleep_constants = vec![
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
        Some(Constant::Utf8("java/lang/Thread".to_owned())),
        Some(Constant::Utf8("sleep".to_owned())),
        Some(Constant::Utf8("(J)V".to_owned())),
    ];
    let idle = runtime_method(
        "javax/microedition/lcdui/Display",
        "__hostIdle",
        "()V",
        &[0x0a, 0xb8, 0, 1, 0xb1],
        2,
        0,
        sleep_constants,
        true,
    );
    let mut sleep = runtime_method(
        "java/lang/Thread",
        "sleep",
        "(J)V",
        &[],
        0,
        2,
        vec![None],
        true,
    );
    sleep.is_native = true;
    let run = runtime_method("Worker", "run", "()V", &[0xb1], 0, 1, Vec::new(), false);
    let mut program = Program::new();
    program.classes.insert(
        "java/lang/Thread".to_owned(),
        Class {
            super_name: None,
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    program.methods.insert(sleep.key.clone(), sleep);
    program.classes.insert(
        "Worker".to_owned(),
        Class {
            super_name: None,
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    program.methods.insert(run.key.clone(), run);
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: None,
        yield_25_millis: false,
        random_seed: None,
    };
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let worker = machine
            .heap
            .managed
            .allocate_object("Worker", HashMap::new())
            .unwrap();
        machine
            .scheduler
            .thread_states
            .insert(worker, ThreadState::Runnable);
        machine.scheduler.runnable_threads.push_back(worker);
        machine.scheduler.host_driver_active = true;

        let outcome = machine.call(&idle, Vec::new(), 1).unwrap();

        assert!(matches!(outcome, CallOutcome::Return(None)));
        assert!(machine.scheduler.runnable_threads.is_empty());
        assert_eq!(
            machine.scheduler.thread_states.get(&worker),
            Some(&ThreadState::Terminated)
        );
        assert!(!machine.scheduler.dispatching_display);
        assert!(!machine.scheduler.suspend_requested);
        assert!(!machine.scheduler.driver_host_poll_yield);
    }
    assert_eq!(context.paced_millis, 1);
    assert_eq!(context.now_millis, 101);
}
