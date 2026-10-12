use super::*;

fn join_method() -> Method {
    let mut method = runtime_method(
        "java/lang/Thread",
        "join",
        "()V",
        &[],
        0,
        1,
        vec![None],
        false,
    );
    method.is_native = true;
    method
}

#[test]
fn joining_an_idle_timer_waits_until_cancellation_or_collection() {
    let program = Program::new();
    let join = join_method();
    for collect_timer in [false, true] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let timer = machine
            .heap
            .managed
            .allocate_object("java/util/Timer", HashMap::new())
            .unwrap();
        let target = machine
            .heap
            .managed
            .allocate_object("java/lang/Thread", HashMap::new())
            .unwrap();
        let waiter = machine
            .heap
            .managed
            .allocate_object("java/lang/Thread", HashMap::new())
            .unwrap();
        machine.scheduler.timer_threads.insert(timer, target);
        machine.scheduler.current_thread = waiter.to_raw();
        machine
            .scheduler
            .thread_states
            .insert(waiter, ThreadState::Running);
        let CallOutcome::Suspend(continuation) = machine
            .call(&join, [Value::Reference(Some(target))], 1)
            .unwrap()
        else {
            panic!("join returned while the Timer thread was alive");
        };
        assert_eq!(
            machine.scheduler.thread_states[&waiter],
            ThreadState::Sleeping
        );

        machine
            .scheduler
            .thread_states
            .insert(waiter, ThreadState::Running);
        let CallOutcome::Suspend(continuation) =
            machine.resume_suspended_call(continuation, 1).unwrap()
        else {
            panic!("a spurious wake-up completed join while the Timer thread was alive");
        };
        if collect_timer {
            let roots = machine.roots(&[Some(Value::Reference(Some(target)))], &[]);
            machine.collect_heap(roots);
        } else {
            machine.scheduler.cancel_timer(timer);
        }
        assert_eq!(
            machine.scheduler.thread_states[&waiter],
            ThreadState::Runnable
        );
        assert_eq!(machine.scheduler.runnable_threads.pop_front(), Some(waiter));
        assert!(matches!(
            machine.resume_suspended_call(continuation, 1).unwrap(),
            CallOutcome::Return(None)
        ));
        assert!(machine.scheduler.join_waiters.is_empty());
    }
}

#[test]
fn main_thread_join_runs_due_timer_callbacks_before_returning() {
    let program = super::timer_lifecycle::cancelling_callback_program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let timer = machine
        .heap
        .managed
        .allocate_object("java/util/Timer", HashMap::new())
        .unwrap();
    let target = machine.timer_thread(timer).unwrap();
    let task = machine
        .heap
        .managed
        .allocate_object(
            "Callback",
            HashMap::from([
                ("java/util/TimerTask.cancelled:Z".into(), HeapValue::Int(0)),
                (
                    "java/util/TimerTask.executionTime:J".into(),
                    HeapValue::Long(0),
                ),
                (
                    "Callback.timer:Ljava/util/Timer;".into(),
                    HeapValue::Reference(Some(timer)),
                ),
            ]),
        )
        .unwrap();
    machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
        timer,
        task,
        deadline: 10,
        period: 40,
        fixed_rate: false,
    });

    assert!(matches!(
        machine
            .call(&join_method(), [Value::Reference(Some(target))], 1)
            .unwrap(),
        CallOutcome::Return(None)
    ));

    assert_eq!(machine.scheduler.virtual_wall_millis, 10);
    assert!(machine.scheduler.scheduled_tasks.is_empty());
    assert!(machine.scheduler.timer_threads.is_empty());
}

#[test]
fn main_driver_join_parks_and_resumes_when_the_timer_is_cancelled() {
    let mut program = Program::new();
    program
        .classes
        .insert("java/lang/Thread".into(), thread_class());
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let timer = machine
        .heap
        .managed
        .allocate_object("java/util/Timer", HashMap::new())
        .unwrap();
    let target = machine.timer_thread(timer).unwrap();
    machine.scheduler.host_driver_active = true;
    // A future timer must not keep the lifecycle driver inside join until
    // its deadline. Host events need their next turn immediately.
    let task = machine
        .heap
        .managed
        .allocate_object("java/util/TimerTask", HashMap::new())
        .unwrap();
    machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
        timer,
        task,
        deadline: 100,
        period: 0,
        fixed_rate: false,
    });
    let CallOutcome::Suspend(continuation) = machine
        .call(&join_method(), [Value::Reference(Some(target))], 1)
        .unwrap()
    else {
        panic!("main driver join did not wait for the live Timer");
    };
    assert_eq!(machine.scheduler.virtual_wall_millis, 0);
    assert!(!machine.scheduler.notified_threads.contains(&MAIN_THREAD_ID));
    machine.scheduler.cancel_timer(timer);
    assert!(machine.scheduler.notified_threads.contains(&MAIN_THREAD_ID));
    assert!(matches!(
        machine.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Return(None)
    ));
    assert!(!machine.scheduler.notified_threads.contains(&MAIN_THREAD_ID));
    assert!(machine.scheduler.join_waiters.is_empty());
}

#[test]
fn main_thread_creation_during_join_can_collect_the_finished_timer() {
    let mut program = Program::new();
    program
        .classes
        .insert("java/lang/Thread".into(), thread_class());
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 512,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let timer = machine
        .heap
        .managed
        .allocate_object("java/util/Timer", HashMap::new())
        .unwrap();
    let target = machine.timer_thread(timer).unwrap();
    machine.scheduler.host_driver_active = true;
    let garbage = machine
        .heap
        .managed
        .allocate_array(
            ArrayKind::Byte,
            i32::try_from(512 - machine.heap.managed.bytes() - 24).unwrap(),
        )
        .unwrap();

    assert!(matches!(
        machine
            .call(&join_method(), [Value::Reference(Some(target))], 1)
            .unwrap(),
        CallOutcome::Return(None)
    ));

    assert!(machine.heap.managed.get(garbage).is_err());
    assert!(machine.heap.managed.get(timer).is_err());
    assert!(machine.heap.managed.get(target).is_ok());
    assert!(machine.scheduler.join_waiters.is_empty());
}

#[test]
fn interrupting_a_parked_join_removes_only_that_waiter() {
    let program = program_with_interrupted_exception();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let target = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let waiter = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let other_waiter = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine
        .scheduler
        .thread_states
        .insert(target, ThreadState::Running);
    machine
        .scheduler
        .thread_states
        .insert(waiter, ThreadState::Running);
    machine
        .scheduler
        .thread_states
        .insert(other_waiter, ThreadState::Sleeping);
    machine.scheduler.current_thread = waiter.to_raw();
    let mut join = runtime_method(
        "java/lang/Thread",
        "join",
        "()V",
        &[0xb1],
        0,
        1,
        Vec::new(),
        false,
    );
    join.is_native = true;
    let Some(CallOutcome::Suspend(continuation)) = machine
        .invoke_vm_native(&join, &[Value::Reference(Some(target))], 1)
        .unwrap()
    else {
        panic!("joining a running outer worker must suspend");
    };
    machine
        .scheduler
        .join_waiters
        .get_mut(&target)
        .unwrap()
        .push(other_waiter);

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
    machine.scheduler.current_thread = target.to_raw();
    assert!(matches!(
        machine
            .invoke_vm_native(&interrupt, &[Value::Reference(Some(waiter))], 1)
            .unwrap(),
        Some(CallOutcome::Return(None))
    ));
    assert_eq!(machine.scheduler.runnable_threads.pop_front(), Some(waiter));
    machine.scheduler.current_thread = waiter.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(waiter, ThreadState::Running);
    assert!(matches!(
        machine.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Throw(_)
    ));
    assert_eq!(
        machine.scheduler.join_waiters.get(&target),
        Some(&vec![other_waiter])
    );
    assert!(
        !machine
            .scheduler
            .interrupted_threads
            .contains(&waiter.to_raw())
    );

    machine
        .scheduler
        .thread_states
        .insert(target, ThreadState::Terminated);
    machine.scheduler.wake_join_waiters(target);
    assert_eq!(
        machine.scheduler.runnable_threads.pop_front(),
        Some(other_waiter)
    );
    assert!(!machine.scheduler.runnable_threads.contains(&waiter));
    assert!(!machine.scheduler.join_waiters.contains_key(&target));
}

#[test]
fn scheduler_worker_join_parks_for_every_live_target_state() {
    for target_state in [
        ThreadState::Runnable,
        ThreadState::Running,
        ThreadState::Sleeping,
    ] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let target = machine
            .heap
            .managed
            .allocate_object("java/lang/Thread", HashMap::new())
            .unwrap();
        let waiter = machine
            .heap
            .managed
            .allocate_object("java/lang/Thread", HashMap::new())
            .unwrap();
        machine.scheduler.thread_states.insert(target, target_state);
        machine
            .scheduler
            .thread_states
            .insert(waiter, ThreadState::Running);
        if target_state == ThreadState::Runnable {
            machine.scheduler.runnable_threads.push_back(target);
        }
        machine.scheduler.current_thread = waiter.to_raw();
        let mut join = runtime_method(
            "java/lang/Thread",
            "join",
            "()V",
            &[0xb1],
            0,
            1,
            Vec::new(),
            false,
        );
        join.is_native = true;

        let Some(CallOutcome::Suspend(_)) = machine
            .invoke_vm_native(&join, &[Value::Reference(Some(target))], 1)
            .unwrap()
        else {
            panic!("joining a {target_state:?} worker must suspend");
        };
        assert_eq!(
            machine.scheduler.thread_states.get(&waiter),
            Some(&ThreadState::Sleeping)
        );
        assert_eq!(
            machine.scheduler.thread_states.get(&target),
            Some(&target_state)
        );
        assert_eq!(
            machine.scheduler.join_waiters.get(&target),
            Some(&vec![waiter])
        );
        if target_state == ThreadState::Runnable {
            assert_eq!(machine.scheduler.runnable_threads.front(), Some(&target));
        }
    }
}

#[test]
fn scheduler_worker_self_join_waits_until_interrupted() {
    let program = program_with_interrupted_exception();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let interrupter = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = worker.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Running);
    let mut join = runtime_method(
        "java/lang/Thread",
        "join",
        "()V",
        &[0xb1],
        0,
        1,
        Vec::new(),
        false,
    );
    join.is_native = true;
    let Some(CallOutcome::Suspend(continuation)) = machine
        .invoke_vm_native(&join, &[Value::Reference(Some(worker))], 1)
        .unwrap()
    else {
        panic!("a scheduler worker joining itself must wait");
    };
    assert_eq!(
        machine.scheduler.thread_states.get(&worker),
        Some(&ThreadState::Sleeping)
    );
    assert_eq!(
        machine.scheduler.join_waiters.get(&worker),
        Some(&vec![worker])
    );

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
            .invoke_vm_native(&interrupt, &[Value::Reference(Some(worker))], 1)
            .unwrap(),
        Some(CallOutcome::Return(None))
    ));
    assert_eq!(machine.scheduler.runnable_threads.pop_front(), Some(worker));

    machine.scheduler.current_thread = worker.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Running);
    let CallOutcome::Throw(exception) = machine.resume_suspended_call(continuation, 1).unwrap()
    else {
        panic!("interrupting a self-join must throw InterruptedException");
    };
    assert_eq!(
        machine.object_class(exception).unwrap(),
        "java/lang/InterruptedException"
    );
    assert!(!machine.scheduler.join_waiters.contains_key(&worker));
    assert!(
        !machine
            .scheduler
            .interrupted_threads
            .contains(&worker.to_raw())
    );
}
