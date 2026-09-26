use super::*;

#[test]
fn realtime_pacing_waits_without_advancing_virtual_time() {
    let program = Program::new();
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
        let deadline = machine.pacing_monotonic_millis() + 25;
        machine.pace_or_advance_time(25).unwrap();
        assert_eq!(machine.scheduler.virtual_monotonic_millis, 0);
        assert_eq!(machine.pacing_monotonic_millis(), deadline);
    }
    assert_eq!(context.paced_millis, 25);
}

#[test]
fn realtime_instruction_pacing_adds_compute_time_before_guest_sleep() {
    let program = Program::new();
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: Some(1_000),
        yield_25_millis: false,
        random_seed: None,
    };
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        machine.execution.instructions = 25;
        machine.pace_pending_instructions().unwrap();
        assert_eq!(machine.pacing_monotonic_millis(), 125);

        machine.pace_or_advance_time(20).unwrap();
        assert_eq!(machine.pacing_monotonic_millis(), 145);
        assert_eq!(machine.scheduler.virtual_monotonic_millis, 0);
    }
    assert_eq!(context.paced_millis, 45);
}

#[test]
fn realtime_instruction_pacing_does_not_spend_sleep_as_compute_credit() {
    let program = Program::new();
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: Some(1_000),
        yield_25_millis: false,
        random_seed: None,
    };
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let thread = machine
            .heap
            .managed
            .allocate_object("java/lang/Thread", HashMap::new())
            .unwrap();
        machine.scheduler.current_thread = thread.to_raw();
        machine
            .scheduler
            .thread_states
            .insert(thread, ThreadState::Running);
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

        machine.execution.instructions = 25;
        let Some(CallOutcome::Suspend(continuation)) = machine
            .invoke_vm_native(&sleep, &[Value::Long(20)], 1)
            .unwrap()
        else {
            panic!("a realtime worker sleep must suspend its native call");
        };
        assert_eq!(machine.pacing_monotonic_millis(), 125);
        assert_eq!(machine.scheduler.sleeping_threads.get(&thread), Some(&145));

        machine.pace_or_advance_time(20).unwrap();
        machine.wake_sleeping_threads();
        assert_eq!(machine.scheduler.runnable_threads.pop_front(), Some(thread));
        machine.scheduler.current_thread = thread.to_raw();
        machine
            .scheduler
            .thread_states
            .insert(thread, ThreadState::Running);
        assert!(matches!(
            machine.resume_suspended_call(continuation, 1).unwrap(),
            CallOutcome::Return(None)
        ));
        machine.execution.instructions = 50;
        machine.pace_pending_instructions().unwrap();
        assert_eq!(machine.pacing_monotonic_millis(), 170);
        assert_eq!(machine.scheduler.virtual_monotonic_millis, 0);
    }
    assert_eq!(context.paced_millis, 70);
}

#[test]
fn virtual_execution_ignores_profile_instruction_rate() {
    let program = Program::new();
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: false,
        instructions_per_second: Some(1_000),
        yield_25_millis: false,
        random_seed: None,
    };
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        machine.execution.instructions = 25;
        machine.pace_pending_instructions().unwrap();
        assert_eq!(machine.pacing_monotonic_millis(), 0);
    }
    assert_eq!(context.paced_millis, 0);
    assert_eq!(context.now_millis, 100);
}

#[test]
fn virtual_pacing_keeps_headless_waits_deterministic() {
    let program = Program::new();
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: false,
        instructions_per_second: None,
        yield_25_millis: false,
        random_seed: None,
    };
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        machine.pace_or_advance_time(25).unwrap();
        assert_eq!(machine.scheduler.virtual_monotonic_millis, 25);
        assert_eq!(machine.pacing_monotonic_millis(), 25);
    }
    assert_eq!(context.paced_millis, 0);
    assert_eq!(context.now_millis, 100);
}

#[test]
fn realtime_sleeping_worker_wakes_at_deadline() {
    let program = Program::new();
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: None,
        yield_25_millis: false,
        random_seed: None,
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let thread = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine
        .scheduler
        .thread_states
        .insert(thread, ThreadState::Sleeping);
    machine.scheduler.sleeping_threads.insert(thread, 105);

    // Stale deadlines must neither revive terminated threads nor enqueue an
    // already runnable thread twice. Future sleepers remain parked.
    let mut others = Vec::new();
    for (state, deadline) in [
        (ThreadState::Terminated, 100),
        (ThreadState::Runnable, 100),
        (ThreadState::Sleeping, 110),
    ] {
        let other = machine
            .heap
            .managed
            .allocate_object("java/lang/Thread", HashMap::new())
            .unwrap();
        machine.scheduler.thread_states.insert(other, state);
        machine.scheduler.sleeping_threads.insert(other, deadline);
        others.push(other);
    }

    machine.wake_sleeping_threads();
    assert_eq!(
        machine.scheduler.thread_states.get(&thread),
        Some(&ThreadState::Sleeping)
    );
    assert!(machine.scheduler.runnable_threads.is_empty());
    assert!(!machine.scheduler.sleeping_threads.contains_key(&others[0]));
    assert!(!machine.scheduler.sleeping_threads.contains_key(&others[1]));
    assert_eq!(
        machine.scheduler.sleeping_threads.get(&others[2]),
        Some(&110)
    );
    assert_eq!(
        machine.scheduler.thread_states[&others[0]],
        ThreadState::Terminated
    );
    assert_eq!(
        machine.scheduler.thread_states[&others[1]],
        ThreadState::Runnable
    );

    machine.pace_or_advance_time(5).unwrap();
    machine.wake_sleeping_threads();
    assert_eq!(
        machine.scheduler.thread_states.get(&thread),
        Some(&ThreadState::Runnable)
    );
    assert_eq!(machine.scheduler.runnable_threads.pop_front(), Some(thread));
    assert!(!machine.scheduler.sleeping_threads.contains_key(&thread));
    machine.wake_sleeping_threads();
    assert!(machine.scheduler.runnable_threads.is_empty());
    assert_eq!(machine.scheduler.sleeping_threads.len(), 1);
    machine.pace_or_advance_time(5).unwrap();
    machine.wake_sleeping_threads();
    assert_eq!(
        machine.scheduler.runnable_threads.pop_front(),
        Some(others[2])
    );
    assert!(machine.scheduler.runnable_threads.is_empty());
    assert!(machine.scheduler.sleeping_threads.is_empty());
}

#[test]
fn realtime_sleep_override_keeps_worker_yield_without_host_wait() {
    let program = Program::new();
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: None,
        yield_25_millis: true,
        random_seed: None,
    };
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let thread = machine
            .heap
            .managed
            .allocate_object("java/lang/Thread", HashMap::new())
            .unwrap();
        machine.scheduler.current_thread = thread.to_raw();
        machine
            .scheduler
            .thread_states
            .insert(thread, ThreadState::Running);
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

        let Some(CallOutcome::Suspend(continuation)) = outcome else {
            panic!("a realtime worker sleep must suspend its native call");
        };
        assert_eq!(
            machine.scheduler.thread_states.get(&thread),
            Some(&ThreadState::Sleeping)
        );
        assert_eq!(machine.scheduler.sleeping_threads.get(&thread), Some(&100));
        assert!(!machine.scheduler.suspend_requested);

        machine.wake_sleeping_threads();
        assert_eq!(machine.scheduler.runnable_threads.pop_front(), Some(thread));
        machine
            .scheduler
            .thread_states
            .insert(thread, ThreadState::Running);
        assert!(matches!(
            machine.resume_suspended_call(continuation, 1).unwrap(),
            CallOutcome::Return(None)
        ));
    }
    assert_eq!(context.paced_millis, 0);
    assert_eq!(context.now_millis, 100);
}
