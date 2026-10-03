use super::*;

fn timer_exception_program() -> Program {
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
        Some(Constant::Utf8("ThrowingTimerTask".into())),
        Some(Constant::Utf8("failure".into())),
        Some(Constant::Utf8("Ljava/lang/Throwable;".into())),
    ];
    let throwing = runtime_method(
        "ThrowingTimerTask",
        "run",
        "()V",
        &[0x2a, 0xb4, 0, 1, 0xbf],
        1,
        1,
        constants,
        false,
    );
    let surviving = runtime_method(
        "SurvivingTimerTask",
        "run",
        "()V",
        &[0xb1],
        0,
        1,
        Vec::new(),
        false,
    );
    let mut throwing_class = test_class_definition(Some("java/util/TimerTask"));
    throwing_class.fields.push(Field {
        key: "ThrowingTimerTask.failure:Ljava/lang/Throwable;".into(),
        declaring_class: "ThrowingTimerTask".into(),
        kind: ValueKind::Reference,
        is_static: false,
        field_token: FieldToken::new(),
        instance_slot: None,
        initial: Value::Reference(None),
        constant_string: None,
    });
    let mut program = Program::new();
    program
        .classes
        .insert("ThrowingTimerTask".into(), throwing_class);
    program.classes.insert(
        "SurvivingTimerTask".into(),
        test_class_definition(Some("java/util/TimerTask")),
    );
    program.classes.insert(
        "java/util/TimerTask".into(),
        test_class_definition(Some("java/lang/Object")),
    );
    program.classes.insert(
        "java/lang/Throwable".into(),
        test_class_definition(Some("java/lang/Object")),
    );
    program.classes.insert(
        "java/lang/Exception".into(),
        test_class_definition(Some("java/lang/Throwable")),
    );
    program.classes.insert(
        "java/lang/RuntimeException".into(),
        test_class_definition(Some("java/lang/Exception")),
    );
    program.classes.insert(
        "java/lang/NullPointerException".into(),
        test_class_definition(Some("java/lang/RuntimeException")),
    );
    program.classes.insert(
        "java/lang/Error".into(),
        test_class_definition(Some("java/lang/Throwable")),
    );
    program.methods.insert(throwing.key.clone(), throwing);
    program.methods.insert(surviving.key.clone(), surviving);
    program
}

#[test]
fn due_timer_callback_is_deferred_for_a_foreign_running_initializer_owner() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let owner = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let dispatcher = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let timer = machine
        .heap
        .managed
        .allocate_object("java/util/Timer", HashMap::new())
        .unwrap();
    let task = machine
        .heap
        .managed
        .allocate_object("Deferred", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = dispatcher.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(owner, ThreadState::Running);
    machine
        .scheduler
        .thread_states
        .insert(dispatcher, ThreadState::Running);
    machine
        .classes
        .initializing
        .insert("Deferred".into(), owner.to_raw());
    machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
        timer,
        task,
        deadline: 0,
        period: 0,
        fixed_rate: false,
    });

    machine.run_due_timer_tasks(1, &[], &[]).unwrap();

    assert_eq!(machine.scheduler.scheduled_tasks.len(), 1);
    assert_eq!(machine.scheduler.scheduled_tasks[0].task, task);
    assert!(!machine.scheduler.dispatching_timer);
    assert!(!machine.scheduler.timer_poll_requested);
    assert_eq!(
        machine.scheduler.timer_poll_countdown,
        machine.scheduler.timer_poll_instructions
    );
}

#[test]
fn due_timer_callback_runs_when_a_foreign_initializer_owner_is_runnable() {
    let callback = runtime_method("Deferred", "run", "()V", &[0xb1], 0, 1, Vec::new(), false);
    let mut program = Program::new();
    program
        .classes
        .insert("Deferred".into(), test_class_definition(None));
    program.methods.insert(callback.key.clone(), callback);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let owner = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let dispatcher = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let timer_thread = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let timer = machine
        .heap
        .managed
        .allocate_object("java/util/Timer", HashMap::new())
        .unwrap();
    let task = machine
        .heap
        .managed
        .allocate_object(
            "Deferred",
            HashMap::from([
                ("java/util/TimerTask.cancelled:Z".into(), HeapValue::Int(0)),
                (
                    "java/util/TimerTask.executionTime:J".into(),
                    HeapValue::Long(0),
                ),
            ]),
        )
        .unwrap();
    machine.scheduler.current_thread = dispatcher.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(owner, ThreadState::Runnable);
    machine
        .scheduler
        .thread_states
        .insert(dispatcher, ThreadState::Running);
    machine
        .classes
        .initializing
        .insert("Deferred".into(), owner.to_raw());
    machine.scheduler.timer_threads.insert(timer, timer_thread);
    machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
        timer,
        task,
        deadline: 0,
        period: 0,
        fixed_rate: false,
    });

    machine.run_due_timer_tasks(1, &[], &[]).unwrap();

    assert!(machine.scheduler.scheduled_tasks.is_empty());
    assert_eq!(
        machine
            .heap
            .managed
            .field(task, "java/util/TimerTask.cancelled:Z")
            .unwrap(),
        HeapValue::Int(1)
    );
    assert!(!machine.scheduler.dispatching_timer);
}

#[test]
fn timer_task_exception_cancels_only_the_failing_task() {
    let program = timer_exception_program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let timer_thread = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let timer = machine
        .heap
        .managed
        .allocate_object(
            "java/util/Timer",
            HashMap::from([("java/util/Timer.cancelled:Z".into(), HeapValue::Int(0))]),
        )
        .unwrap();
    let failure = machine
        .heap
        .managed
        .allocate_object("java/lang/NullPointerException", HashMap::new())
        .unwrap();
    let throwing_task = machine
        .heap
        .managed
        .allocate_object(
            "ThrowingTimerTask",
            HashMap::from([
                (
                    "ThrowingTimerTask.failure:Ljava/lang/Throwable;".into(),
                    HeapValue::Reference(Some(failure)),
                ),
                ("java/util/TimerTask.cancelled:Z".into(), HeapValue::Int(0)),
                (
                    "java/util/TimerTask.executionTime:J".into(),
                    HeapValue::Long(0),
                ),
            ]),
        )
        .unwrap();
    let surviving_task = machine
        .heap
        .managed
        .allocate_object(
            "SurvivingTimerTask",
            HashMap::from([
                ("java/util/TimerTask.cancelled:Z".into(), HeapValue::Int(0)),
                (
                    "java/util/TimerTask.executionTime:J".into(),
                    HeapValue::Long(0),
                ),
            ]),
        )
        .unwrap();
    machine.scheduler.timer_threads.insert(timer, timer_thread);
    machine.scheduler.scheduled_tasks.extend([
        ScheduledJavaTask {
            timer,
            task: throwing_task,
            deadline: 0,
            period: 40,
            fixed_rate: false,
        },
        ScheduledJavaTask {
            timer,
            task: surviving_task,
            deadline: 0,
            period: 40,
            fixed_rate: false,
        },
    ]);

    machine.run_due_timer_tasks(1, &[], &[]).unwrap();

    assert_eq!(
        machine
            .heap
            .managed
            .field(throwing_task, "java/util/TimerTask.cancelled:Z")
            .unwrap(),
        HeapValue::Int(1)
    );
    assert_eq!(
        machine
            .heap
            .managed
            .field(timer, "java/util/Timer.cancelled:Z")
            .unwrap(),
        HeapValue::Int(0)
    );
    assert_eq!(machine.scheduler.scheduled_tasks.len(), 1);
    assert_eq!(machine.scheduler.scheduled_tasks[0].task, surviving_task);
    assert_eq!(machine.scheduler.scheduled_tasks[0].deadline, 40);
    assert_eq!(
        machine.scheduler.timer_threads.get(&timer),
        Some(&timer_thread)
    );
    assert_eq!(machine.execution.thread_failure_count, 0);
}

#[test]
fn timer_task_error_terminates_the_timer_thread() {
    let program = timer_exception_program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let timer_thread = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let timer = machine
        .heap
        .managed
        .allocate_object(
            "java/util/Timer",
            HashMap::from([("java/util/Timer.cancelled:Z".into(), HeapValue::Int(0))]),
        )
        .unwrap();
    let failure = machine
        .heap
        .managed
        .allocate_object("java/lang/Error", HashMap::new())
        .unwrap();
    let throwing_task = machine
        .heap
        .managed
        .allocate_object(
            "ThrowingTimerTask",
            HashMap::from([
                (
                    "ThrowingTimerTask.failure:Ljava/lang/Throwable;".into(),
                    HeapValue::Reference(Some(failure)),
                ),
                ("java/util/TimerTask.cancelled:Z".into(), HeapValue::Int(0)),
                (
                    "java/util/TimerTask.executionTime:J".into(),
                    HeapValue::Long(0),
                ),
            ]),
        )
        .unwrap();
    let queued_task = machine
        .heap
        .managed
        .allocate_object(
            "SurvivingTimerTask",
            HashMap::from([("java/util/TimerTask.cancelled:Z".into(), HeapValue::Int(0))]),
        )
        .unwrap();
    machine.scheduler.timer_threads.insert(timer, timer_thread);
    machine.scheduler.scheduled_tasks.extend([
        ScheduledJavaTask {
            timer,
            task: throwing_task,
            deadline: 0,
            period: 40,
            fixed_rate: false,
        },
        ScheduledJavaTask {
            timer,
            task: queued_task,
            deadline: 100,
            period: 40,
            fixed_rate: false,
        },
    ]);

    machine.run_due_timer_tasks(1, &[], &[]).unwrap();

    assert_eq!(
        machine
            .heap
            .managed
            .field(timer, "java/util/Timer.cancelled:Z")
            .unwrap(),
        HeapValue::Int(1)
    );
    assert!(machine.scheduler.scheduled_tasks.is_empty());
    assert!(!machine.scheduler.timer_threads.contains_key(&timer));
    assert_eq!(machine.execution.thread_failure_count, 1);
    assert_eq!(
        machine.execution.thread_failures[0].exception_class,
        "java/lang/Error"
    );
}
