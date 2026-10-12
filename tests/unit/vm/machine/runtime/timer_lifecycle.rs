use super::*;

mod callbacks;
mod cancellation;
mod capacity;
mod timing;

struct SuspendedHost;

impl HostServices for SuspendedHost {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn execution_suspended(&self) -> bool {
        true
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}

#[test]
fn lifecycle_suspend_does_not_park_inline_timer_callbacks() {
    let mut code = vec![0x00; 2048];
    code.push(0xb1);
    let callback = runtime_method("Callback", "run", "()V", &code, 0, 1, vec![None], false);
    let mut program = Program::new();
    program
        .classes
        .insert("Callback".into(), test_class_definition(None));
    program.methods.insert(callback.key.clone(), callback);
    let mut context = SuspendedHost;
    let mut machine = program.machine(Limits::default(), false, &mut context);
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
    machine.scheduler.timer_threads.insert(timer, timer_thread);
    machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
        timer,
        task,
        deadline: 0,
        period: 0,
        fixed_rate: false,
    });
    machine.run_due_timer_tasks(1, &[], &[]).unwrap();
    assert!(machine.scheduler.scheduled_tasks.is_empty());
    assert_eq!(
        machine
            .heap
            .managed
            .field(task, "java/util/TimerTask.cancelled:Z")
            .unwrap(),
        HeapValue::Int(1)
    );
    assert!(!machine.scheduler.dispatching_timer);
    assert!(!machine.scheduler.suspend_requested);
    assert!(machine.execution.instructions >= 2048);
}

#[test]
fn timer_poll_accepts_exactly_the_firing_limit_and_rejects_more_due_work() {
    let callback = runtime_method("Callback", "run", "()V", &[0xb1], 0, 1, vec![None], false);
    let mut program = Program::new();
    program
        .classes
        .insert("Callback".into(), test_class_definition(None));
    program
        .classes
        .insert("java/lang/Thread".into(), thread_class());
    program.methods.insert(callback.key.clone(), callback);
    for (first_deadline, expected_deadline) in [(-1023, 1), (-1024, 0)] {
        let mut context = SuspendedHost;
        let mut machine = program.machine(Limits::default(), false, &mut context);
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
            deadline: first_deadline,
            period: 1,
            fixed_rate: true,
        });

        let result = machine.run_due_timer_tasks(1, &[], &[]);
        if expected_deadline == 1 {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err().code(), "timer-limit");
        }
        assert_eq!(
            machine.scheduler.scheduled_tasks[0].deadline,
            expected_deadline
        );
        assert_eq!(machine.execution.instructions, 1024);
        assert!(!machine.scheduler.dispatching_timer);
    }
}

#[test]
fn timer_task_survives_collection_while_its_first_thread_is_created() {
    let callback = runtime_method("Callback", "run", "()V", &[0xb1], 0, 1, vec![None], false);
    let mut program = Program::new();
    program
        .classes
        .insert("Callback".into(), test_class_definition(None));
    program
        .classes
        .insert("java/lang/Thread".into(), thread_class());
    program.methods.insert(callback.key.clone(), callback);
    let mut context = SuspendedHost;
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
        period: 0,
        fixed_rate: false,
    });
    let garbage = machine
        .heap
        .managed
        .allocate_array(
            ArrayKind::Byte,
            i32::try_from(512 - machine.heap.managed.bytes() - 24).unwrap(),
        )
        .unwrap();
    assert_eq!(machine.heap.managed.bytes(), 512);

    let error = machine
        .run_due_timer_tasks(1, &[Some(Value::Reference(Some(garbage)))], &[])
        .unwrap_err();
    assert_eq!(error.code(), MANAGED_HEAP_LIMIT_CODE);
    assert!(!machine.scheduler.dispatching_timer);
    assert!(machine.heap.frame_roots.at_depth(1).is_empty());
    assert_eq!(machine.scheduler.scheduled_tasks.len(), 1);

    machine.run_due_timer_tasks(1, &[], &[]).unwrap();

    assert!(machine.heap.managed.get(garbage).is_err());
    assert!(machine.heap.managed.get(timer).is_ok());
    assert_eq!(
        machine
            .heap
            .managed
            .field(task, "java/util/TimerTask.cancelled:Z")
            .unwrap(),
        HeapValue::Int(1)
    );
    assert!(machine.scheduler.scheduled_tasks.is_empty());
    assert!(!machine.scheduler.dispatching_timer);
    assert!(machine.heap.frame_roots.at_depth(1).is_empty());
}

pub(super) fn cancelling_callback_program() -> Program {
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
            |context, args| {
                let [NativeValue::Reference(Some(task))] = args else {
                    panic!("expected timer callback receiver");
                };
                let timer = context
                    .read_reference_field(*task, "Callback.timer:Ljava/util/Timer;")?
                    .unwrap();
                context.cancel_timer(timer);
                Ok(None)
            },
        )
        .unwrap();
    program
}

#[test]
fn cancelling_a_timer_from_its_callback_does_not_reschedule_the_current_task() {
    let program = cancelling_callback_program();
    let mut context = SuspendedHost;
    let mut machine = program.machine(Limits::default(), false, &mut context);
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
        deadline: 0,
        period: 40,
        fixed_rate: false,
    });

    machine.run_due_timer_tasks(1, &[], &[]).unwrap();

    assert!(machine.scheduler.scheduled_tasks.is_empty());
    assert!(machine.scheduler.timer_threads.is_empty());
    assert!(!machine.scheduler.dispatching_timer);
}

#[test]
fn timer_threads_follow_live_timer_ownership_and_pending_tasks() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let timer = machine
        .heap
        .managed
        .allocate_object("java/util/Timer", HashMap::new())
        .unwrap();
    let thread = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let owner = machine
        .heap
        .managed
        .allocate_object(
            "Owner",
            HashMap::from([(
                "Owner.timer:Ljava/util/Timer;".into(),
                HeapValue::Reference(Some(timer)),
            )]),
        )
        .unwrap();
    machine.scheduler.timer_threads.insert(timer, thread);

    let roots = machine.roots(&[Some(Value::Reference(Some(owner)))], &[]);
    machine.collect_heap(roots);
    assert!(machine.heap.managed.get(timer).is_ok());
    assert!(machine.heap.managed.get(thread).is_ok());

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
    let roots = machine.roots(&[], &[]);
    machine.collect_heap(roots);
    assert!(machine.heap.managed.get(owner).is_err());
    assert!(machine.heap.managed.get(timer).is_ok());
    assert!(machine.heap.managed.get(thread).is_ok());
    assert!(machine.heap.managed.get(task).is_ok());

    machine.scheduler.scheduled_tasks.clear();
    let roots = machine.roots(&[Some(Value::Reference(Some(thread)))], &[]);
    machine.collect_heap(roots);
    assert!(machine.heap.managed.get(timer).is_err());
    assert!(machine.heap.managed.get(task).is_err());
    assert!(machine.heap.managed.get(thread).is_ok());
    assert!(machine.scheduler.timer_threads.is_empty());

    let roots = machine.roots(&[], &[]);
    machine.collect_heap(roots);
    assert!(machine.heap.managed.is_empty());
}
