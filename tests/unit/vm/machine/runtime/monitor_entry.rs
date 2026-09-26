use super::*;

#[test]
fn resumed_synchronized_entry_checks_frame_limit_before_acquiring_monitor() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_frames: 1,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let receiver = machine
        .heap
        .managed
        .allocate_object("T", HashMap::new())
        .unwrap();
    let mut locked = runtime_method("T", "locked", "()V", &[0xb1], 0, 1, vec![], false);
    locked.is_synchronized = true;
    let CallOutcome::Suspend(call) =
        suspended_monitor_entry(&locked, receiver, vec![Value::Reference(Some(receiver))])
    else {
        unreachable!()
    };
    machine
        .execution
        .call_stack
        .push(locked.active_stack_frame(0));
    let result = machine.resume_suspended_call(call, 1);
    assert_eq!(result.err().unwrap().code(), "stack-overflow");
    assert!(!machine.scheduler.monitors.contains_key(&receiver));
    assert_eq!(machine.execution.call_stack.len(), 1);
}

#[test]
fn scheduler_worker_contended_synchronized_call_parks_until_monitor_is_released() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let receiver = machine
        .heap
        .managed
        .allocate_object("T", HashMap::new())
        .unwrap();
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
    let mut locked = runtime_method("T", "locked", "()I", &[0x08, 0xac], 1, 1, Vec::new(), false);
    locked.is_synchronized = true;
    machine
        .scheduler
        .thread_states
        .insert(owner, ThreadState::Running);
    machine
        .scheduler
        .thread_states
        .insert(waiter, ThreadState::Running);
    machine.scheduler.monitors.insert(
        receiver,
        Monitor {
            owner: owner.to_raw(),
            depth: 1,
        },
    );
    machine.scheduler.current_thread = waiter.to_raw();
    machine.scheduler.quantum_remaining = 100;

    let CallOutcome::Suspend(continuation) = machine
        .call(&locked, vec![Value::Reference(Some(receiver))], 1)
        .unwrap()
    else {
        panic!("a worker blocked at synchronized method entry must suspend");
    };
    assert_eq!(
        machine.scheduler.thread_states.get(&waiter),
        Some(&ThreadState::Sleeping)
    );
    assert_eq!(
        machine.scheduler.monitor_entry_waiters.get(&receiver),
        Some(&VecDeque::from([waiter]))
    );
    assert_eq!(
        machine.scheduler.monitors.get(&receiver).unwrap().owner,
        owner.to_raw()
    );

    machine.scheduler.current_thread = owner.to_raw();
    machine.exit_monitor(receiver).unwrap();
    assert_eq!(machine.scheduler.runnable_threads.pop_front(), Some(waiter));
    machine.scheduler.current_thread = waiter.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiter, ThreadState::Running);
    assert!(matches!(
        machine.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Return(Some(Value::Int(5)))
    ));
    assert!(!machine.scheduler.monitors.contains_key(&receiver));
    assert!(
        !machine
            .scheduler
            .monitor_entry_waiters
            .contains_key(&receiver)
    );
}

#[test]
fn scheduler_worker_monitorenter_retries_after_owner_releases() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
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
    let locked = runtime_method(
        "T",
        "locked",
        "(Ljava/lang/Object;)I",
        &[0x2a, 0xc2, 0x2a, 0xc3, 0x08, 0xac],
        1,
        1,
        Vec::new(),
        true,
    );
    machine
        .scheduler
        .thread_states
        .insert(owner, ThreadState::Running);
    machine
        .scheduler
        .thread_states
        .insert(waiter, ThreadState::Running);
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: owner.to_raw(),
            depth: 1,
        },
    );
    machine.scheduler.current_thread = waiter.to_raw();
    machine.scheduler.quantum_remaining = 100;

    let CallOutcome::Suspend(continuation) = machine
        .call(&locked, vec![Value::Reference(Some(monitor))], 1)
        .unwrap()
    else {
        panic!("a worker blocked at monitorenter must suspend");
    };
    assert_eq!(
        machine.scheduler.thread_states.get(&waiter),
        Some(&ThreadState::Sleeping)
    );
    assert_eq!(
        machine.scheduler.monitor_entry_waiters.get(&monitor),
        Some(&VecDeque::from([waiter]))
    );

    machine.scheduler.current_thread = owner.to_raw();
    machine.exit_monitor(monitor).unwrap();
    assert_eq!(machine.scheduler.runnable_threads.pop_front(), Some(waiter));
    machine.scheduler.current_thread = waiter.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiter, ThreadState::Running);
    assert!(matches!(
        machine.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Return(Some(Value::Int(5)))
    ));
    assert!(!machine.scheduler.monitors.contains_key(&monitor));
    assert!(
        !machine
            .scheduler
            .monitor_entry_waiters
            .contains_key(&monitor)
    );
}

#[test]
fn inline_monitor_waiter_prevents_worker_reacquire_starvation() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let run = runtime_method(
        "Worker",
        "run",
        "()V",
        &[
            0x2b, 0xc3, // aload_1; monitorexit
            0x2b, 0xc2, // aload_1; monitorenter
            0x2b, 0xc3, // aload_1; monitorexit
            0xb1, // return
        ],
        1,
        2,
        Vec::new(),
        false,
    );
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: worker.to_raw(),
            depth: 1,
        },
    );
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Runnable);
    machine.scheduler.runnable_threads.push_back(worker);
    machine.scheduler.thread_continuations.insert(
        worker,
        Box::new(SuspendedCall {
            method: run,
            locals: vec![
                Some(Value::Reference(Some(worker))),
                Some(Value::Reference(Some(monitor))),
            ],
            stack: Vec::new(),
            pc: 0,
            synchronized_monitor: None,
            monitor_entry: None,
            pending: None,
            native_resume: None,
            class_initialization: None,
        }),
    );

    assert!(machine.enter_monitor(monitor, 1).unwrap());
    assert_eq!(
        machine
            .scheduler
            .monitors
            .get(&monitor)
            .map(|state| state.owner),
        Some(MAIN_THREAD_ID)
    );
    assert_eq!(
        machine.scheduler.thread_states.get(&worker),
        Some(&ThreadState::Sleeping)
    );
    assert_eq!(
        machine.scheduler.monitor_entry_waiters.get(&monitor),
        Some(&VecDeque::from([worker]))
    );

    machine.exit_monitor(monitor).unwrap();
    assert_eq!(
        machine.scheduler.thread_states.get(&worker),
        Some(&ThreadState::Runnable)
    );
    assert!(machine.run_one_thread(1).unwrap());
    assert_eq!(
        machine.scheduler.thread_states.get(&worker),
        Some(&ThreadState::Terminated)
    );
    assert!(!machine.scheduler.monitors.contains_key(&monitor));
    assert!(
        !machine
            .scheduler
            .monitor_entry_waiters
            .contains_key(&monitor)
    );
}
