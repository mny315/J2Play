use super::*;
use std::cell::Cell;
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

struct TimerClock(Arc<AtomicI64>);

impl HostServices for TimerClock {
    fn monotonic_millis(&self) -> i64 {
        self.0.load(Ordering::Relaxed)
    }

    fn wall_clock_millis(&self) -> i64 {
        self.monotonic_millis()
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}

#[test]
fn repeating_timers_use_their_start_time_without_adding_callback_duration() {
    for (fixed_rate, first_deadlines, second_deadlines, due_count) in [
        (false, [147, 157], [157, 187], 1),
        (true, [130, 130], [170, 170], 2),
    ] {
        let clock = Arc::new(AtomicI64::new(100));
        let callback_clock = Arc::clone(&clock);
        let mut callback = runtime_method("Callback", "run", "()V", &[], 0, 1, vec![None], false);
        callback.is_native = true;
        let mut program = Program::new();
        program
            .classes
            .insert("Callback".into(), test_class_definition(None));
        program
            .classes
            .insert("java/lang/Thread".into(), thread_class());
        program.methods.insert(callback.key.clone(), callback);
        program
            .native_registry_mut()
            .register(
                NativeSignature::new("Callback", "run", "()V"),
                move |_, _| {
                    callback_clock.fetch_add(10, Ordering::Relaxed);
                    Ok(None)
                },
            )
            .unwrap();
        let mut host = TimerClock(Arc::clone(&clock));
        let mut machine = program.machine(Limits::default(), false, &mut host);
        machine.scheduler.virtual_wall_millis = 7;
        let timer = machine
            .heap
            .managed
            .allocate_object("java/util/Timer", HashMap::new())
            .unwrap();
        for _ in 0..2 {
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
                    ]),
                )
                .unwrap();
            machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
                timer,
                task,
                deadline: 90,
                period: 40,
                fixed_rate,
            });
        }

        machine.run_due_timer_tasks(1, &[], &[]).unwrap();
        assert_eq!(clock.load(Ordering::Relaxed), 120);
        let deadlines = |machine: &Machine<'_, '_>| {
            let mut result: Vec<_> = machine
                .scheduler
                .scheduled_tasks
                .iter()
                .map(|task| task.deadline)
                .collect();
            result.sort_unstable();
            result
        };
        assert_eq!(
            deadlines(&machine),
            first_deadlines,
            "fixed_rate={fixed_rate}"
        );

        let next_host_time = first_deadlines[0] - 7;
        clock.store(next_host_time - 1, Ordering::Relaxed);
        machine.run_due_timer_tasks(1, &[], &[]).unwrap();
        assert_eq!(clock.load(Ordering::Relaxed), next_host_time - 1);

        clock.store(next_host_time, Ordering::Relaxed);
        machine.run_due_timer_tasks(1, &[], &[]).unwrap();
        assert_eq!(
            clock.load(Ordering::Relaxed),
            next_host_time + 10 * due_count
        );
        assert_eq!(deadlines(&machine), second_deadlines);
    }
}

#[test]
fn busy_periodic_timer_leaves_cpu_time_for_guest_work() {
    for (fixed_rate, worker) in [(false, false), (true, false), (false, true), (true, true)] {
        let clock = Arc::new(AtomicI64::new(0));
        let callback_clock = Arc::clone(&clock);
        let mut callback = runtime_method("Callback", "run", "()V", &[], 0, 1, vec![None], false);
        callback.is_native = true;
        let mut program = Program::new();
        let mut code = vec![0x00; 60_000];
        code.push(0xb1);
        let work = runtime_method("Work", "run", "()V", &code, 0, 1, vec![], false);
        program
            .classes
            .insert("Work".into(), test_class_definition(None));
        program.methods.insert(work.key.clone(), work.clone());
        program
            .classes
            .insert("Callback".into(), test_class_definition(None));
        program
            .classes
            .insert("java/lang/Thread".into(), thread_class());
        program.methods.insert(callback.key.clone(), callback);
        program
            .native_registry_mut()
            .register(
                NativeSignature::new("Callback", "run", "()V"),
                move |_, _| {
                    // A callback that consumes its whole period, e.g. while
                    // waiting for presentation, must not starve interrupted work.
                    callback_clock.fetch_add(10, Ordering::Relaxed);
                    Ok(None)
                },
            )
            .unwrap();
        let mut host = TimerClock(Arc::clone(&clock));
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let timer = machine
            .heap
            .managed
            .allocate_object("java/util/Timer", HashMap::new())
            .unwrap();
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
                ]),
            )
            .unwrap();
        machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
            timer,
            task,
            deadline: 0,
            period: 10,
            fixed_rate,
        });
        let thread = machine
            .heap
            .managed
            .allocate_object("Work", HashMap::new())
            .unwrap();
        if worker {
            machine
                .scheduler
                .thread_states
                .insert(thread, ThreadState::Runnable);
            machine.scheduler.runnable_threads.push_back(thread);
            for _ in 0..4 {
                if machine.scheduler.thread_states.get(&thread) == Some(&ThreadState::Terminated) {
                    break;
                }
                assert!(machine.run_one_thread(1).unwrap());
            }
            assert_eq!(
                machine.scheduler.thread_states.get(&thread),
                Some(&ThreadState::Terminated)
            );
        } else {
            assert!(matches!(
                machine
                    .call(&work, [Value::Reference(Some(thread))], 1)
                    .unwrap(),
                CallOutcome::Return(None)
            ));
        }
        let elapsed = clock.load(Ordering::Relaxed);
        eprintln!(
            "busy timer fixed_rate={fixed_rate} worker={worker}: {elapsed} ms for 60000 guest instructions"
        );
        assert!(elapsed > 0, "the due callback must run");
        assert!(
            elapsed <= 100,
            "timer delayed bounded guest work by {elapsed} ms"
        );
        assert_eq!(machine.scheduler.scheduled_tasks.len(), 1);
        assert_eq!(machine.scheduler.scheduled_tasks[0].deadline, elapsed);

        // Once ordinary work finishes, short idle display turns must still
        // service the timer without waiting for a whole CPU slice of bytecode.
        let idle = runtime_method("Idle", "run", "()V", &[0xb1], 0, 0, vec![], true);
        for _ in 0..4 {
            assert!(!machine.run_one_thread(1).unwrap());
            clock.fetch_add(1, Ordering::Relaxed);
            assert!(matches!(
                machine.call(&idle, [], 1).unwrap(),
                CallOutcome::Return(None)
            ));
        }
        assert!(
            clock.load(Ordering::Relaxed) >= elapsed + 24,
            "idle turns delayed an overdue timer"
        );
    }
}

#[test]
fn future_timer_polls_wall_clock_at_bounded_instruction_intervals() {
    #[derive(Default)]
    struct CountingClock {
        wall_clock_calls: Cell<u64>,
    }

    impl HostServices for CountingClock {
        fn monotonic_millis(&self) -> i64 {
            0
        }

        fn wall_clock_millis(&self) -> i64 {
            self.wall_clock_calls
                .set(self.wall_clock_calls.get().saturating_add(1));
            0
        }

        fn system_property(&self, _: &str) -> Option<&str> {
            None
        }

        fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
            Ok(None)
        }
    }

    assert_eq!(timer_poll_instruction_interval(None), 256);
    assert_eq!(timer_poll_instruction_interval(Some(1)), 1);
    assert_eq!(timer_poll_instruction_interval(Some(1_000)), 1);
    assert_eq!(timer_poll_instruction_interval(Some(2_000)), 2);
    assert_eq!(timer_poll_instruction_interval(Some(6_250_000)), 256);

    let program = Program::new();
    let mut context = CountingClock::default();
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let timer = machine
        .heap
        .managed
        .allocate_object("java/util/Timer", HashMap::new())
        .unwrap();
    let task = machine
        .heap
        .managed
        .allocate_object("java/util/TimerTask", HashMap::new())
        .unwrap();
    machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
        timer,
        task,
        deadline: i64::MAX,
        period: 0,
        fixed_rate: false,
    });
    let mut code = vec![0x00; 1_024];
    code.push(0xb1);
    let method = runtime_method("T", "run", "()V", &code, 0, 0, Vec::new(), true);

    assert!(matches!(
        machine.call(&method, Vec::new(), 1).unwrap(),
        CallOutcome::Return(None)
    ));
    drop(machine);
    assert_eq!(context.wall_clock_calls.get(), 5);
}
