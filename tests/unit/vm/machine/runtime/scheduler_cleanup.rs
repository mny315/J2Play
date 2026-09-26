use super::*;

#[test]
fn display_dispatch_clears_its_active_flag_after_host_errors_and_cancellation() {
    const DISPLAY: &str = "javax/microedition/lcdui/Display";
    for code in ["execution-cancelled", "fixture-failure"] {
        for inline in [false, true] {
            let mut method = runtime_method(DISPLAY, "__hostIdle", "()V", &[], 0, 0, vec![], true);
            method.is_native = true;
            let mut program = Program::new();
            program.methods.insert(method.key.clone(), method.clone());
            program
                .native_registry_mut()
                .register(
                    NativeSignature::new(DISPLAY, "__hostIdle", "()V"),
                    move |_, _| Err(EmuError::new(Category::Vm, code, "fixture")),
                )
                .unwrap();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            machine.classes.initialized.insert(DISPLAY.into());
            let outcome = if inline {
                machine.run_display_event_turn(1)
            } else {
                machine.call(&method, [], 1)
            };
            assert_eq!(outcome.err().unwrap().code(), code);
            assert!(!machine.scheduler.dispatching_display);
            assert!(!machine.scheduler.dispatching_canvas_paint);
            assert!(machine.execution.call_stack.is_empty());
            assert_eq!(machine.execution.frame_slots, 0);
            assert_eq!(machine.execution.stack_slots, 0);
        }
    }
}

#[test]
fn timer_dispatch_restores_the_interrupted_worker_after_host_errors_and_cancellation() {
    for code in ["execution-cancelled", "fixture-failure"] {
        let mut callback = runtime_method("Callback", "run", "()V", &[], 0, 1, vec![], false);
        callback.is_native = true;
        let mut program = Program::new();
        program
            .classes
            .insert("Callback".into(), test_class_definition(None));
        program.methods.insert(callback.key.clone(), callback);
        program
            .native_registry_mut()
            .register(
                NativeSignature::new("Callback", "run", "()V"),
                move |_, _| Err(EmuError::new(Category::Vm, code, "fixture")),
            )
            .unwrap();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let worker = machine
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
        machine.scheduler.current_thread = worker.to_raw();
        machine.scheduler.quantum_remaining = 17;
        machine.scheduler.suspend_requested = true;
        machine
            .scheduler
            .thread_states
            .insert(worker, ThreadState::Running);
        machine.scheduler.timer_threads.insert(timer, timer_thread);
        machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
            timer,
            task,
            deadline: 0,
            period: 0,
            fixed_rate: false,
        });
        assert_eq!(
            machine.run_due_timer_tasks(1, &[], &[]).unwrap_err().code(),
            code
        );
        assert_eq!(machine.scheduler.current_thread, worker.to_raw());
        assert_eq!(machine.scheduler.quantum_remaining, 17);
        assert!(machine.scheduler.suspend_requested);
        assert!(!machine.scheduler.dispatching_timer);
        assert!(!machine.scheduler.thread_states.contains_key(&timer_thread));
        assert!(machine.heap.frame_roots.at_depth(1).is_empty());
        assert!(machine.execution.call_stack.is_empty());
    }
}
