use super::*;

#[test]
fn worker_releasing_a_contended_monitor_yields_before_reacquiring_it() {
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
    let method =
        |code: &[u8]| runtime_method("Worker", "run", "()V", code, 1, 2, Vec::new(), false);
    let run = method(&[
        0x2b, 0xc3, // release
        0x2b, 0xc2, // immediate reacquire must not overtake the queued worker
        0x2b, 0xc3, 0xb1,
    ]);
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: owner.to_raw(),
            depth: 2,
        },
    );
    machine.scheduler.current_thread = waiter.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiter, ThreadState::Running);
    assert!(!machine.enter_monitor(monitor, 1).unwrap());
    machine.scheduler.current_thread = owner.to_raw();
    machine.exit_monitor(monitor).unwrap();
    assert!(!machine.scheduler.suspend_requested);
    assert_eq!(
        machine.scheduler.thread_states[&waiter],
        ThreadState::Sleeping
    );
    machine.scheduler.current_thread = MAIN_THREAD_ID;
    machine
        .scheduler
        .thread_states
        .insert(owner, ThreadState::Runnable);
    machine.scheduler.runnable_threads.push_back(owner);
    for (thread, run) in [
        (owner, run),
        (waiter, method(&[0x2b, 0xc2, 0x2b, 0xc3, 0xb1])),
    ] {
        machine.scheduler.thread_continuations.insert(
            thread,
            Box::new(SuspendedCall {
                method: run,
                locals: vec![
                    Some(Value::Reference(Some(thread))),
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
    }
    machine.run_one_thread(1).unwrap();
    assert_eq!(machine.scheduler.runnable_threads.front(), Some(&waiter));
    assert_eq!(
        machine.scheduler.thread_states[&owner],
        ThreadState::Runnable,
        "the releasing worker must yield before another acquisition and sleep can starve its waiter"
    );
    machine.run_one_thread(1).unwrap();
    assert_eq!(
        machine.scheduler.thread_states[&waiter],
        ThreadState::Terminated
    );
    machine.run_one_thread(1).unwrap();
    assert_eq!(
        machine.scheduler.thread_states[&owner],
        ThreadState::Terminated
    );
    assert!(machine.scheduler.monitors.is_empty());
    assert!(machine.scheduler.monitor_entry_waiters.is_empty());
}

#[test]
#[ignore = "manual release measurement of synchronized Java calls"]
fn synchronized_monitor_call_throughput() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_instructions: 3_000_000,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let receiver = machine
        .heap
        .managed
        .allocate_object("Monitor", HashMap::new())
        .unwrap();
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = worker.to_raw();
    machine.scheduler.quantum_remaining = u64::MAX;
    let mut locked = runtime_method(
        "Monitor",
        "locked",
        "()I",
        &[0x08, 0xac],
        1,
        1,
        Vec::new(),
        false,
    );
    locked.is_synchronized = true;
    for repeat in 0..5 {
        let started = std::time::Instant::now();
        for _ in 0..200_000 {
            assert!(matches!(
                machine
                    .call(&locked, [Value::Reference(Some(receiver))], 1)
                    .unwrap(),
                CallOutcome::Return(Some(Value::Int(5)))
            ));
        }
        eprintln!(
            "synchronized-monitor repeat={repeat} calls=200000 elapsed={:?}",
            started.elapsed()
        );
    }
    assert!(machine.scheduler.monitors.is_empty());
}
