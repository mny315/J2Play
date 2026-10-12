use super::*;

#[test]
fn main_and_timer_threads_cannot_be_started_again() {
    let mut program = program_with_exception("java/lang/IllegalThreadStateException");
    program
        .classes
        .insert("java/lang/Thread".into(), thread_class());
    let mut start = runtime_method(
        "java/lang/Thread",
        "start",
        "()V",
        &[],
        0,
        1,
        vec![None],
        false,
    );
    start.is_native = true;
    let mut cancel = runtime_method(
        "java/util/Timer",
        "cancel0",
        "()V",
        &[],
        0,
        1,
        vec![None],
        false,
    );
    cancel.is_native = true;
    program
        .native_registry_mut()
        .register(
            NativeSignature::new("java/util/Timer", "cancel0", "()V"),
            |context, args| {
                let [NativeValue::Reference(Some(timer))] = args else {
                    panic!("expected Timer receiver");
                };
                context.cancel_timer(*timer);
                Ok(None)
            },
        )
        .unwrap();

    for case in ["main", "timer", "cancelled timer", "collected timer"] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let thread = if case == "main" {
            machine.current_thread_handle().unwrap()
        } else {
            let timer = machine
                .heap
                .managed
                .allocate_object("java/util/Timer", HashMap::new())
                .unwrap();
            let thread = machine.timer_thread(timer).unwrap();
            if case == "cancelled timer" {
                assert!(matches!(
                    machine
                        .call(&cancel, [Value::Reference(Some(timer))], 1)
                        .unwrap(),
                    CallOutcome::Return(None)
                ));
            } else if case == "collected timer" {
                let roots = machine.roots(&[Some(Value::Reference(Some(thread)))], &[]);
                machine.collect_heap(roots);
                assert!(machine.heap.managed.get(timer).is_err());
            }
            thread
        };
        let outcome = machine
            .call(&start, [Value::Reference(Some(thread))], 1)
            .unwrap();
        let CallOutcome::Throw(exception) = outcome else {
            panic!("{case} was accepted as a new thread");
        };
        assert_eq!(
            machine.object_class(exception).unwrap(),
            "java/lang/IllegalThreadStateException",
            "{case}"
        );
        assert!(machine.scheduler.runnable_threads.is_empty(), "{case}");
    }
}

#[test]
fn active_count_includes_idle_timers_and_counts_each_running_timer_once() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let count = runtime_method(
        "java/lang/Thread",
        "activeCount",
        "()I",
        &[],
        0,
        0,
        vec![None],
        true,
    );
    let timer = machine
        .heap
        .managed
        .allocate_object("java/util/Timer", HashMap::new())
        .unwrap();
    let timer_thread = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let task = machine
        .heap
        .managed
        .allocate_object("java/util/TimerTask", HashMap::new())
        .unwrap();
    machine.scheduler.timer_threads.insert(timer, timer_thread);
    machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
        timer,
        task,
        deadline: 0,
        period: 0,
        fixed_rate: false,
    });
    for running in [false, true] {
        if running {
            machine
                .scheduler
                .thread_states
                .insert(timer_thread, ThreadState::Running);
        }
        assert!(
            matches!(
                machine.invoke_vm_native(&count, &[], 1).unwrap(),
                Some(CallOutcome::Return(Some(Value::Int(2))))
            ),
            "running={running}"
        );
    }
    machine.scheduler.thread_states.remove(&timer_thread);
    machine.scheduler.scheduled_tasks.clear();
    assert!(matches!(
        machine.invoke_vm_native(&count, &[], 1).unwrap(),
        Some(CallOutcome::Return(Some(Value::Int(2))))
    ));
    machine.scheduler.cancel_timer(timer);
    assert!(matches!(
        machine.invoke_vm_native(&count, &[], 1).unwrap(),
        Some(CallOutcome::Return(Some(Value::Int(1))))
    ));
}

#[test]
fn interrupting_a_realtime_sleep_throws_on_native_resume() {
    let program = program_with_interrupted_exception();
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: None,
        yield_25_millis: false,
        random_seed: None,
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let sleeping = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let interrupter = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = sleeping.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(sleeping, ThreadState::Running);
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
    let Some(CallOutcome::Suspend(continuation)) = machine
        .invoke_vm_native(&sleep, &[Value::Long(1_000)], 1)
        .unwrap()
    else {
        panic!("a realtime worker sleep must suspend");
    };

    let interrupt = runtime_method(
        "java/lang/Thread",
        "interrupt",
        "()V",
        &[0xb1],
        0,
        1,
        Vec::new(),
        false,
    );
    machine.scheduler.current_thread = interrupter.to_raw();
    assert!(matches!(
        machine
            .invoke_vm_native(&interrupt, &[Value::Reference(Some(sleeping))], 1)
            .unwrap(),
        Some(CallOutcome::Return(None))
    ));
    assert_eq!(
        machine.scheduler.runnable_threads.pop_front(),
        Some(sleeping)
    );
    machine.scheduler.current_thread = sleeping.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(sleeping, ThreadState::Running);
    let CallOutcome::Throw(exception) = machine.resume_suspended_call(continuation, 1).unwrap()
    else {
        panic!("an interrupted sleep must throw on resume");
    };
    assert_eq!(
        machine.object_class(exception).unwrap(),
        "java/lang/InterruptedException"
    );
    assert!(
        !machine
            .scheduler
            .interrupted_threads
            .contains(&sleeping.to_raw())
    );
    assert!(!machine.scheduler.sleeping_threads.contains_key(&sleeping));
}

#[test]
fn thread_constructors_and_runtime_threads_always_have_names() {
    let mut program = Program::new();
    program
        .classes
        .insert("java/lang/Thread".to_owned(), thread_class());
    program.classes.insert(
        "java/lang/NullPointerException".to_owned(),
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
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);

    let main = machine.current_thread_handle().unwrap();
    assert_eq!(machine_thread_name(&machine, main), "main");
    assert_eq!(
        machine
            .heap
            .managed
            .field(main, "java/lang/Thread.priority:I")
            .unwrap(),
        HeapValue::Int(5)
    );

    let mut constructor = runtime_method(
        "java/lang/Thread",
        "<init>",
        "(Ljava/lang/Runnable;)V",
        &[],
        0,
        2,
        Vec::new(),
        false,
    );
    constructor.is_native = true;
    let mut workers = Vec::new();
    for expected in ["Thread-0", "Thread-1"] {
        let fields = machine
            .instance_fields("java/lang/Thread")
            .unwrap()
            .into_iter()
            .map(|field| (field.key.to_string(), field.initial))
            .collect();
        let worker = machine
            .allocate_object("java/lang/Thread", fields, &[], &[])
            .unwrap();
        assert!(matches!(
            machine
                .call(
                    &constructor,
                    vec![Value::Reference(Some(worker)), Value::Reference(None)],
                    1,
                )
                .unwrap(),
            CallOutcome::Return(None)
        ));
        assert_eq!(machine_thread_name(&machine, worker), expected);
        workers.push(worker);
    }
    assert_ne!(
        machine_thread_name(&machine, workers[0]),
        machine_thread_name(&machine, workers[1])
    );

    let mut named_constructor = runtime_method(
        "java/lang/Thread",
        "<init>",
        "(Ljava/lang/String;)V",
        &[],
        0,
        2,
        Vec::new(),
        false,
    );
    named_constructor.is_native = true;
    let fields = machine
        .instance_fields("java/lang/Thread")
        .unwrap()
        .into_iter()
        .map(|field| (field.key.to_string(), field.initial))
        .collect();
    let invalid = machine
        .allocate_object("java/lang/Thread", fields, &[], &[])
        .unwrap();
    let CallOutcome::Throw(exception) = machine
        .call(
            &named_constructor,
            vec![Value::Reference(Some(invalid)), Value::Reference(None)],
            1,
        )
        .unwrap()
    else {
        panic!("an explicit null thread name must throw");
    };
    assert_eq!(
        machine.object_class(exception).unwrap(),
        "java/lang/NullPointerException"
    );
}

#[test]
fn terminated_thread_bookkeeping_and_failure_history_are_bounded() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        max_threads: 2,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    let thread = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine
        .scheduler
        .thread_states
        .insert(thread, ThreadState::Terminated);

    machine.collect_heap(vec![thread]);
    assert_eq!(
        machine.scheduler.thread_states.get(&thread),
        Some(&ThreadState::Terminated)
    );
    for thread_id in 1..=3 {
        machine.record_thread_failure(ThreadFailure {
            thread_id,
            exception_class: "java/lang/RuntimeException".to_owned(),
            exception_message: None,
            stack_trace: vec!["T::run()V pc=0".to_owned()],
            managed_heap_limit: false,
        });
    }
    assert_eq!(
        machine
            .execution
            .thread_failures
            .iter()
            .map(|failure| failure.thread_id)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(machine.execution.thread_failure_count, 3);

    machine.collect_heap(Vec::new());
    assert!(!machine.scheduler.thread_states.contains_key(&thread));
    assert!(machine.heap.managed.get(thread).is_err());
}
