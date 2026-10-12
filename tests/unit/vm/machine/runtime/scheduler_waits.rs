use super::*;

fn wait_method() -> Method {
    let mut method = runtime_method(
        "java/lang/Object",
        "wait",
        "(J)V",
        &[0xb1],
        0,
        3,
        Vec::new(),
        false,
    );
    method.is_native = true;
    method
}

#[test]
fn notify_preserves_waiter_order_and_does_not_requeue_runnable_threads() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
    let [owner, first, runnable, last] = std::array::from_fn(|_| {
        machine
            .heap
            .managed
            .allocate_object("java/lang/Thread", HashMap::new())
            .unwrap()
    });
    machine.scheduler.current_thread = owner.to_raw();
    assert!(machine.enter_monitor(monitor, 1).unwrap());
    machine.scheduler.thread_states.extend([
        (owner, ThreadState::Running),
        (first, ThreadState::Sleeping),
        (runnable, ThreadState::Runnable),
        (last, ThreadState::Sleeping),
    ]);
    machine.scheduler.runnable_threads.push_back(runnable);
    machine
        .scheduler
        .sleeping_threads
        .extend([(first, 10), (last, 20)]);
    machine.scheduler.monitor_waiters.insert(
        monitor,
        VecDeque::from([
            first.to_raw(),
            MAIN_THREAD_ID,
            runnable.to_raw(),
            last.to_raw(),
        ]),
    );

    machine.notify_monitor(monitor, false).unwrap();
    assert_eq!(
        machine.scheduler.monitor_waiters[&monitor],
        VecDeque::from([MAIN_THREAD_ID, runnable.to_raw(), last.to_raw()])
    );
    assert_eq!(
        machine.scheduler.notified_threads,
        HashSet::from([first.to_raw()])
    );
    assert_eq!(
        machine.scheduler.thread_states[&first],
        ThreadState::Runnable
    );
    assert_eq!(
        machine.scheduler.sleeping_threads,
        HashMap::from([(last, 20)])
    );

    machine.notify_monitor(monitor, true).unwrap();
    machine.notify_monitor(monitor, true).unwrap();
    assert!(!machine.scheduler.monitor_waiters.contains_key(&monitor));
    assert!(machine.scheduler.sleeping_threads.is_empty());
    assert_eq!(
        machine.scheduler.thread_states[&last],
        ThreadState::Runnable
    );
    assert_eq!(
        machine.scheduler.runnable_threads,
        VecDeque::from([runnable, first, last])
    );
    assert_eq!(
        machine.scheduler.notified_threads,
        HashSet::from([
            first.to_raw(),
            MAIN_THREAD_ID,
            runnable.to_raw(),
            last.to_raw()
        ])
    );
    assert_eq!(machine.scheduler.monitors[&monitor].owner, owner.to_raw());
}

#[test]
fn wait_release_wakes_a_parked_monitor_entrant() {
    let program = Program::new();
    let wait = wait_method();
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
        .allocate_object("owner", HashMap::new())
        .unwrap();
    let entrant = machine
        .heap
        .managed
        .allocate_object("entrant", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = owner.to_raw();
    assert!(machine.enter_monitor(monitor, 1).unwrap());
    assert!(machine.enter_monitor(monitor, 1).unwrap());
    machine.scheduler.current_thread = entrant.to_raw();
    assert!(!machine.enter_monitor(monitor, 1).unwrap());
    machine.scheduler.current_thread = owner.to_raw();
    assert!(matches!(
        machine.wait_on_monitor(&wait, monitor, 0, 1).unwrap(),
        CallOutcome::Suspend(_)
    ));
    assert_eq!(
        machine.scheduler.thread_states.get(&entrant),
        Some(&ThreadState::Runnable)
    );
    assert_eq!(
        machine.scheduler.runnable_threads.pop_front(),
        Some(entrant)
    );
    machine.scheduler.current_thread = entrant.to_raw();
    assert!(machine.enter_monitor(monitor, 1).unwrap());
    assert!(matches!(
        machine.notify_monitor(monitor, false).unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(
        machine.scheduler.thread_states.get(&owner),
        Some(&ThreadState::Runnable)
    );
}

#[test]
fn already_interrupted_worker_wait_throws_without_parking() {
    let program = program_with_interrupted_exception();
    let wait = wait_method();
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
        .allocate_object("owner", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = owner.to_raw();
    assert!(machine.enter_monitor(monitor, 1).unwrap());
    assert!(machine.enter_monitor(monitor, 1).unwrap());
    machine.scheduler.interrupted_threads.insert(owner.to_raw());
    let CallOutcome::Throw(exception) = machine.wait_on_monitor(&wait, monitor, 0, 1).unwrap()
    else {
        panic!("an existing interrupt must not enter an indefinite wait");
    };
    assert_eq!(
        machine.object_class(exception).unwrap(),
        "java/lang/InterruptedException"
    );
    assert!(
        !machine
            .scheduler
            .interrupted_threads
            .contains(&owner.to_raw())
    );
    assert_eq!(machine.scheduler.monitors[&monitor].depth, 2);
    assert_eq!(machine.scheduler.monitors[&monitor].owner, owner.to_raw());
    assert!(machine.scheduler.monitor_waiters.is_empty());
}

#[test]
fn timed_wait_expires_while_an_unrelated_worker_remains_live() {
    let program = Program::new();
    let wait = wait_method();
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: None,
        yield_25_millis: false,
        random_seed: None,
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
    let waiting = machine
        .heap
        .managed
        .allocate_object("waiting", HashMap::new())
        .unwrap();
    let unrelated = machine
        .heap
        .managed
        .allocate_object("worker", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = waiting.to_raw();
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: waiting.to_raw(),
            depth: 2,
        },
    );
    machine
        .scheduler
        .thread_states
        .insert(unrelated, ThreadState::Sleeping);
    machine.scheduler.sleeping_threads.insert(unrelated, 1_000);

    let outcome = machine.wait_on_monitor(&wait, monitor, 5, 1).unwrap();

    let CallOutcome::Suspend(continuation) = outcome else {
        panic!("a timed worker wait must suspend its native call");
    };
    assert_eq!(machine.pacing_monotonic_millis(), 100);
    assert!(!machine.scheduler.monitors.contains_key(&monitor));
    assert_eq!(machine.scheduler.sleeping_threads.get(&waiting), Some(&105));
    assert!(
        machine
            .scheduler
            .monitor_waiters
            .get(&monitor)
            .is_some_and(|waiters| waiters.contains(&waiting.to_raw()))
    );
    assert_eq!(
        machine.scheduler.thread_states.get(&unrelated),
        Some(&ThreadState::Sleeping)
    );

    machine.pace_or_advance_time(5).unwrap();
    machine.wake_sleeping_threads();
    assert_eq!(
        machine.scheduler.runnable_threads.pop_front(),
        Some(waiting)
    );
    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    assert!(matches!(
        machine.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(machine.scheduler.monitors.get(&monitor).unwrap().depth, 2);
    assert!(!machine.scheduler.sleeping_threads.contains_key(&waiting));
    assert!(!machine.scheduler.monitor_waiters.contains_key(&monitor));
}

#[test]
fn scheduler_worker_timed_wait_does_not_run_a_sibling_recursively() {
    let program = Program::new();
    let wait = wait_method();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
    let waiting = machine
        .heap
        .managed
        .allocate_object("waiting", HashMap::new())
        .unwrap();
    let sibling = machine
        .heap
        .managed
        .allocate_object("sibling", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    machine
        .scheduler
        .thread_states
        .insert(sibling, ThreadState::Runnable);
    machine.scheduler.runnable_threads.push_back(sibling);
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: waiting.to_raw(),
            depth: 1,
        },
    );

    let outcome = machine.wait_on_monitor(&wait, monitor, 40, 1).unwrap();

    assert!(matches!(outcome, CallOutcome::Suspend(_)));
    assert_eq!(machine.scheduler.virtual_monotonic_millis, 0);
    assert_eq!(machine.scheduler.runnable_threads.front(), Some(&sibling));
    assert_eq!(
        machine.scheduler.thread_states.get(&sibling),
        Some(&ThreadState::Runnable)
    );
    assert!(!machine.scheduler.monitors.contains_key(&monitor));
    assert_eq!(machine.scheduler.sleeping_threads.get(&waiting), Some(&40));
    assert!(
        machine
            .scheduler
            .monitor_waiters
            .get(&monitor)
            .is_some_and(|waiters| waiters.contains(&waiting.to_raw()))
    );
    assert!(!machine.scheduler.suspend_requested);
}

#[test]
fn deterministic_timed_wait_resumes_at_the_scheduler_deadline() {
    let program = Program::new();
    let wait = wait_method();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
    let waiting = machine
        .heap
        .managed
        .allocate_object("waiting", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: waiting.to_raw(),
            depth: 2,
        },
    );
    let CallOutcome::Suspend(continuation) =
        machine.wait_on_monitor(&wait, monitor, 40, 1).unwrap()
    else {
        panic!("a timed worker wait must suspend");
    };
    machine
        .scheduler
        .thread_continuations
        .insert(waiting, continuation);
    machine.scheduler.current_thread = MAIN_THREAD_ID;

    assert!(machine.run_one_thread(1).unwrap());

    assert_eq!(machine.scheduler.virtual_monotonic_millis, 40);
    assert_eq!(
        machine.scheduler.thread_states.get(&waiting),
        Some(&ThreadState::Terminated)
    );
    assert!(!machine.scheduler.sleeping_threads.contains_key(&waiting));
    assert!(!machine.scheduler.monitor_waiters.contains_key(&monitor));
}

#[test]
fn deterministic_timed_wait_expires_with_a_permanently_runnable_sibling() {
    let wait = wait_method();
    let busy_run = runtime_method("Busy", "run", "()V", &[0xa7, 0, 0], 0, 1, Vec::new(), false);
    let mut program = Program::new();
    program.classes.insert(
        "Busy".to_owned(),
        Class {
            super_name: Some("java/lang/Object".to_owned()),
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    program.methods.insert(busy_run.key.clone(), busy_run);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
    let waiting = machine
        .heap
        .managed
        .allocate_object("waiting", HashMap::new())
        .unwrap();
    let busy = machine
        .heap
        .managed
        .allocate_object("Busy", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: waiting.to_raw(),
            depth: 1,
        },
    );
    let CallOutcome::Suspend(continuation) = machine.wait_on_monitor(&wait, monitor, 1, 1).unwrap()
    else {
        panic!("a timed worker wait must suspend");
    };
    machine
        .scheduler
        .thread_continuations
        .insert(waiting, continuation);
    machine
        .scheduler
        .thread_states
        .insert(busy, ThreadState::Runnable);
    machine.scheduler.runnable_threads.push_back(busy);
    machine.scheduler.current_thread = MAIN_THREAD_ID;

    assert!(machine.run_one_thread(1).unwrap());

    assert_eq!(machine.scheduler.virtual_monotonic_millis, 1);
    assert_eq!(
        machine.scheduler.thread_states.get(&waiting),
        Some(&ThreadState::Runnable)
    );
    assert!(!machine.scheduler.sleeping_threads.contains_key(&waiting));
    assert!(machine.scheduler.runnable_threads.contains(&waiting));
}

#[test]
fn timed_wait_can_be_notified_before_its_deadline() {
    let program = Program::new();
    let wait = wait_method();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
    let waiting = machine
        .heap
        .managed
        .allocate_object("waiting", HashMap::new())
        .unwrap();
    let notifier = machine
        .heap
        .managed
        .allocate_object("notifier", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: waiting.to_raw(),
            depth: 2,
        },
    );
    let CallOutcome::Suspend(continuation) =
        machine.wait_on_monitor(&wait, monitor, 40, 1).unwrap()
    else {
        panic!("a timed worker wait must suspend");
    };

    machine.scheduler.current_thread = notifier.to_raw();
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: notifier.to_raw(),
            depth: 1,
        },
    );
    assert!(matches!(
        machine.notify_monitor(monitor, false).unwrap(),
        CallOutcome::Return(None)
    ));
    machine.exit_monitor(monitor).unwrap();
    assert_eq!(machine.scheduler.virtual_monotonic_millis, 0);
    assert!(!machine.scheduler.sleeping_threads.contains_key(&waiting));
    assert_eq!(
        machine.scheduler.runnable_threads.pop_front(),
        Some(waiting)
    );

    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    assert!(matches!(
        machine.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Return(None)
    ));
    let reacquired = machine.scheduler.monitors.get(&monitor).unwrap();
    assert_eq!(reacquired.owner, waiting.to_raw());
    assert_eq!(reacquired.depth, 2);
    assert!(!machine.scheduler.monitor_waiters.contains_key(&monitor));
    assert!(
        !machine
            .scheduler
            .notified_threads
            .contains(&waiting.to_raw())
    );
}

#[test]
fn notified_scheduler_worker_parks_until_monitor_is_released() {
    let program = Program::new();
    let wait = wait_method();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
    let waiting = machine
        .heap
        .managed
        .allocate_object("waiting", HashMap::new())
        .unwrap();
    let notifier = machine
        .heap
        .managed
        .allocate_object("notifier", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: waiting.to_raw(),
            depth: 2,
        },
    );
    let CallOutcome::Suspend(first_continuation) =
        machine.wait_on_monitor(&wait, monitor, 0, 1).unwrap()
    else {
        panic!("indefinite worker wait must suspend");
    };

    machine.scheduler.current_thread = notifier.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(notifier, ThreadState::Running);
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: notifier.to_raw(),
            depth: 1,
        },
    );
    assert!(matches!(
        machine.notify_monitor(monitor, false).unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(
        machine.scheduler.runnable_threads.pop_front(),
        Some(waiting)
    );

    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    let CallOutcome::Suspend(second_continuation) = machine
        .resume_suspended_call(first_continuation, 1)
        .unwrap()
    else {
        panic!("a notified worker must suspend until the notifier releases the monitor");
    };
    assert_eq!(
        machine.scheduler.thread_states.get(&waiting),
        Some(&ThreadState::Sleeping)
    );
    assert_eq!(
        machine.scheduler.monitor_entry_waiters.get(&monitor),
        Some(&VecDeque::from([waiting]))
    );
    assert_eq!(
        machine.scheduler.monitors.get(&monitor).unwrap().owner,
        notifier.to_raw()
    );

    machine.scheduler.current_thread = notifier.to_raw();
    machine.exit_monitor(monitor).unwrap();
    assert_eq!(
        machine.scheduler.runnable_threads.pop_front(),
        Some(waiting)
    );
    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    assert!(matches!(
        machine
            .resume_suspended_call(second_continuation, 1)
            .unwrap(),
        CallOutcome::Return(None)
    ));
    let reacquired = machine.scheduler.monitors.get(&monitor).unwrap();
    assert_eq!(reacquired.owner, waiting.to_raw());
    assert_eq!(reacquired.depth, 2);
    assert!(
        !machine
            .scheduler
            .monitor_entry_waiters
            .contains_key(&monitor)
    );
}

#[test]
fn scheduler_worker_indefinite_wait_yields_for_future_lifecycle_notifier() {
    let program = Program::new();
    let wait = wait_method();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
    let waiting = machine
        .heap
        .managed
        .allocate_object("waiting", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: waiting.to_raw(),
            depth: 2,
        },
    );

    let outcome = machine.wait_on_monitor(&wait, monitor, 0, 1).unwrap();

    let CallOutcome::Suspend(continuation) = outcome else {
        panic!("indefinite worker wait must suspend its native call");
    };
    assert!(!machine.scheduler.monitors.contains_key(&monitor));
    assert_eq!(
        machine.scheduler.thread_states.get(&waiting),
        Some(&ThreadState::Sleeping)
    );
    assert!(
        machine
            .scheduler
            .monitor_waiters
            .get(&monitor)
            .is_some_and(|waiters| waiters.contains(&waiting.to_raw()))
    );

    let notifier = machine
        .heap
        .managed
        .allocate_object("notifier", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = notifier.to_raw();
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: notifier.to_raw(),
            depth: 1,
        },
    );
    assert!(matches!(
        machine.notify_monitor(monitor, false).unwrap(),
        CallOutcome::Return(None)
    ));
    machine.exit_monitor(monitor).unwrap();
    assert_eq!(
        machine.scheduler.thread_states.get(&waiting),
        Some(&ThreadState::Runnable)
    );

    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    assert!(matches!(
        machine.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Return(None)
    ));
    let reacquired = machine.scheduler.monitors.get(&monitor).unwrap();
    assert_eq!(reacquired.owner, waiting.to_raw());
    assert_eq!(reacquired.depth, 2);
}

#[test]
fn scheduler_worker_indefinite_wait_returns_before_a_sleeping_sibling() {
    let program = Program::new();
    let wait = wait_method();
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: None,
        yield_25_millis: false,
        random_seed: None,
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let monitor = machine
        .heap
        .managed
        .allocate_object("monitor", HashMap::new())
        .unwrap();
    let waiting = machine
        .heap
        .managed
        .allocate_object("waiting", HashMap::new())
        .unwrap();
    let sibling = machine
        .heap
        .managed
        .allocate_object("sibling", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = waiting.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiting, ThreadState::Running);
    machine
        .scheduler
        .thread_states
        .insert(sibling, ThreadState::Sleeping);
    machine.scheduler.sleeping_threads.insert(sibling, 1_000);
    machine.scheduler.monitors.insert(
        monitor,
        Monitor {
            owner: waiting.to_raw(),
            depth: 1,
        },
    );

    let outcome = machine.wait_on_monitor(&wait, monitor, 0, 1).unwrap();

    assert!(matches!(outcome, CallOutcome::Suspend(_)));
    assert_eq!(machine.pacing_monotonic_millis(), 100);
    assert_eq!(
        machine.scheduler.thread_states.get(&sibling),
        Some(&ThreadState::Sleeping)
    );
    assert_eq!(
        machine.scheduler.sleeping_threads.get(&sibling),
        Some(&1_000)
    );
    assert!(!machine.scheduler.monitors.contains_key(&monitor));
    assert_eq!(
        machine.scheduler.thread_states.get(&waiting),
        Some(&ThreadState::Sleeping)
    );
    assert!(
        machine
            .scheduler
            .monitor_waiters
            .get(&monitor)
            .is_some_and(|waiters| waiters.contains(&waiting.to_raw()))
    );
}
