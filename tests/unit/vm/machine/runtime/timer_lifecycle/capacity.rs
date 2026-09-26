use super::*;

#[test]
fn timer_callback_cannot_fill_the_slot_needed_to_reschedule_itself() {
    let mut callback = runtime_method("Callback", "run", "()V", &[], 0, 1, vec![None], false);
    callback.is_native = true;
    let key = callback.key.clone();
    let mut program = Program::new();
    program
        .classes
        .insert("Callback".into(), test_class_definition(None));
    program
        .classes
        .insert("java/lang/Thread".into(), thread_class());
    program.methods.insert(key.clone(), callback);
    program
        .native_registry_mut()
        .register(
            NativeSignature::new("Callback", "run", "()V"),
            |context, args| {
                let [NativeValue::Reference(Some(task))] = args else {
                    panic!("expected callback receiver");
                };
                let timer = context
                    .read_reference_field(*task, "Callback.timer:Ljava/util/Timer;")?
                    .unwrap();
                let mut pending =
                    context.read_reference_field(*task, "Callback.pending:LPendingTask;")?;
                while let Some(task) = pending {
                    if let Err(error) = context.schedule_timer_task(timer, task, 1_000, 0, false) {
                        assert_eq!(error.code(), "timer-limit");
                        break;
                    }
                    pending =
                        context.read_reference_field(task, "PendingTask.next:LPendingTask;")?;
                }
                Ok(None)
            },
        )
        .unwrap();
    let mut host = SuspendedHost;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let timer = machine
        .heap
        .managed
        .allocate_object("java/util/Timer", HashMap::new())
        .unwrap();
    let mut pending = None;
    for _ in 0..1_024 {
        pending = Some(
            machine
                .heap
                .managed
                .allocate_object(
                    "PendingTask",
                    HashMap::from([(
                        "PendingTask.next:LPendingTask;".into(),
                        HeapValue::Reference(pending),
                    )]),
                )
                .unwrap(),
        );
    }
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
                (
                    "Callback.pending:LPendingTask;".into(),
                    HeapValue::Reference(pending),
                ),
            ]),
        )
        .unwrap();
    machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
        timer,
        task,
        deadline: 0,
        period: 40,
        fixed_rate: false,
    });

    machine.run_due_timer_tasks(1, &[], &[]).unwrap();

    assert_eq!(machine.scheduler.scheduled_tasks.len(), 1_024);
    assert_eq!(
        machine
            .scheduler
            .scheduled_tasks
            .iter()
            .filter(|entry| entry.task == task)
            .count(),
        1
    );
    assert_eq!(
        machine.scheduler.scheduled_tasks.last().unwrap().deadline,
        40
    );
    assert!(!machine.scheduler.dispatching_timer);

    // Outside a callback all 1024 slots are available. A second fill attempt
    // must leave the queue at capacity rather than bypassing an equality guard.
    machine.scheduler.cancel_timer(timer);
    for _ in 0..2 {
        assert!(matches!(
            machine
                .call(&program.methods[&key], [Value::Reference(Some(task))], 1)
                .unwrap(),
            CallOutcome::Return(None)
        ));
        assert_eq!(machine.scheduler.scheduled_tasks.len(), 1_024);
    }
}
