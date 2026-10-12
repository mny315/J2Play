use super::*;

#[test]
fn main_timed_wait_expires_with_a_permanently_runnable_worker() {
    let mut program = Program::new();
    program
        .classes
        .insert("Busy".into(), test_class_definition(None));
    let busy_run = runtime_method("Busy", "run", "()V", &[0xa7, 0, 0], 0, 1, vec![], false);
    program.methods.insert(busy_run.key.clone(), busy_run);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_instructions: 131_072,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let worker = machine
        .heap
        .managed
        .allocate_object("Busy", HashMap::new())
        .unwrap();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Runnable);
    machine.scheduler.runnable_threads.push_back(worker);
    let monitor = machine
        .heap
        .managed
        .allocate_object("Monitor", HashMap::new())
        .unwrap();
    assert!(machine.enter_monitor(monitor, 1).unwrap());
    assert!(machine.enter_monitor(monitor, 1).unwrap());
    let wait = runtime_method("java/lang/Object", "wait", "(J)V", &[], 0, 3, vec![], false);
    assert!(matches!(
        machine.wait_on_monitor(&wait, monitor, 3, 1).unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(machine.pacing_monotonic_millis(), 3);
    assert_eq!(machine.scheduler.monitors[&monitor].owner, MAIN_THREAD_ID);
    assert_eq!(machine.scheduler.monitors[&monitor].depth, 2);
    assert!(!machine.scheduler.monitor_waiters.contains_key(&monitor));
    assert_eq!(
        machine.scheduler.thread_states[&worker],
        ThreadState::Runnable
    );
}

#[test]
fn lifecycle_driver_resumes_main_wait_after_future_host_notifier() {
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
        Some(Constant::Utf8("java/lang/Object".to_owned())),
        Some(Constant::Utf8("wait".to_owned())),
        Some(Constant::Utf8("(J)V".to_owned())),
        Some(Constant::Methodref {
            class_index: 2,
            name_and_type_index: 8,
        }),
        Some(Constant::NameAndType {
            name_index: 9,
            descriptor_index: 10,
        }),
        Some(Constant::Utf8("notifyAll".to_owned())),
        Some(Constant::Utf8("()V".to_owned())),
    ];
    let constructor = runtime_method("App", "<init>", "()V", &[0xb1], 0, 1, Vec::new(), false);
    let mut start = runtime_method(
        "App",
        "start",
        "()V",
        &[
            0x2a, // aload_0
            0x09, // lconst_0
            0xb6, 0, 1,    // invokevirtual Object.wait(J)V
            0xb1, // return
        ],
        3,
        1,
        constants.clone(),
        false,
    );
    start.is_synchronized = true;
    let mut wake = runtime_method(
        "App",
        "wake",
        "()V",
        &[
            0x2a, // aload_0
            0xb6, 0, 7,    // invokevirtual Object.notifyAll()V
            0xb1, // return
        ],
        1,
        1,
        constants,
        false,
    );
    wake.is_synchronized = true;
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
    let mut program = Program::new();
    program
        .classes
        .insert("java/lang/Object".to_owned(), test_class_definition(None));
    program.classes.insert(
        "App".to_owned(),
        test_class_definition(Some("java/lang/Object")),
    );
    for method in [constructor, start, wake, wait, notify_all] {
        program.methods.insert(method.key.clone(), method);
    }
    let calls = [
        InstanceCall {
            target: CallTarget::Instance,
            name: "start".to_owned(),
            descriptor: "()V".to_owned(),
            arguments: Vec::new(),
        },
        InstanceCall {
            target: CallTarget::Instance,
            name: "wake".to_owned(),
            descriptor: "()V".to_owned(),
            arguments: Vec::new(),
        },
    ];
    let mut context = DefaultNativeContext;

    let execution = program
        .execute_instance_sequence_with_context(
            "App",
            &calls,
            Limits::default(),
            false,
            &mut context,
        )
        .unwrap();

    assert_eq!(execution.value, None);
}
