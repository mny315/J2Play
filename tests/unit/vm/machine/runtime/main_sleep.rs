use super::*;

fn thread_method(name: &str, descriptor: &str, is_static: bool) -> Method {
    let mut method = runtime_method(
        "java/lang/Thread",
        name,
        descriptor,
        &[],
        0,
        2,
        vec![None],
        is_static,
    );
    method.is_native = true;
    method
}

fn thread_constants(name: &str, descriptor: &str) -> Vec<Option<Constant>> {
    vec![
        None,
        Some(Constant::Methodref {
            class_index: 2,
            name_and_type_index: 3,
        }),
        Some(Constant::Class { name_index: 4 }),
        Some(Constant::NameAndType {
            name_index: 5,
            descriptor_index: 6,
        }),
        Some(Constant::Utf8("java/lang/Thread".into())),
        Some(Constant::Utf8(name.into())),
        Some(Constant::Utf8(descriptor.into())),
    ]
}

fn pacing_context() -> PacingContext {
    PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: None,
        yield_25_millis: false,
        random_seed: None,
    }
}

#[test]
fn main_sleep_allows_multiple_loader_quanta_and_an_earlier_worker_wakeup() {
    for worker_sleeps in [false, true] {
        let sleep = thread_method("sleep", "(J)V", true);
        // More than one scheduling quantum of independently owned guest work.
        // The second case first parks for 5 ms, well before main's 20 ms wake.
        let mut code = if worker_sleeps {
            vec![0x08, 0x85, 0xb8, 0, 1]
        } else {
            vec![]
        };
        code.extend(std::iter::repeat_n(0x00, 40_000));
        code.push(0xb1);
        let worker_run = runtime_method(
            "Loader",
            "run",
            "()V",
            &code,
            2,
            1,
            thread_constants("sleep", "(J)V"),
            false,
        );
        let mut program = Program::new();
        program
            .classes
            .insert("java/lang/Thread".into(), test_class_definition(None));
        program
            .classes
            .insert("Loader".into(), test_class_definition(None));
        program.methods.insert(sleep.key.clone(), sleep.clone());
        program.methods.insert(worker_run.key.clone(), worker_run);
        let mut context = pacing_context();
        {
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let worker = machine
                .heap
                .managed
                .allocate_object("Loader", HashMap::new())
                .unwrap();
            machine
                .scheduler
                .thread_states
                .insert(worker, ThreadState::Runnable);
            machine.scheduler.runnable_threads.push_back(worker);
            assert!(matches!(
                machine
                    .invoke_vm_native(&sleep, &[Value::Long(20)], 1)
                    .unwrap(),
                Some(CallOutcome::Return(None))
            ));
            assert_eq!(
                machine.scheduler.thread_states.get(&worker),
                Some(&ThreadState::Terminated)
            );
            assert!(machine.scheduler.sleeping_threads.is_empty());
            assert!(machine.execution.instructions >= 40_000);
        }
        assert_eq!(context.now_millis, 120);
        assert_eq!(context.paced_millis, 20);
    }
}

#[test]
fn main_sleep_keeps_worker_cpu_pacing() {
    let sleep = thread_method("sleep", "(J)V", true);
    let mut code = vec![0x00; 40_000];
    code.push(0xb1);
    let worker_run = runtime_method("Loader", "run", "()V", &code, 0, 1, vec![None], false);
    let mut program = Program::new();
    program
        .classes
        .insert("Loader".into(), test_class_definition(None));
    program.methods.insert(worker_run.key.clone(), worker_run);
    let mut context = pacing_context();
    context.instructions_per_second = Some(1_000_000);
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let worker = machine
            .heap
            .managed
            .allocate_object("Loader", HashMap::new())
            .unwrap();
        machine
            .scheduler
            .thread_states
            .insert(worker, ThreadState::Runnable);
        machine.scheduler.runnable_threads.push_back(worker);
        machine
            .invoke_vm_native(&sleep, &[Value::Long(20)], 1)
            .unwrap();
        assert_eq!(
            machine.scheduler.thread_states.get(&worker),
            Some(&ThreadState::Runnable)
        );
        assert!(machine.execution.instructions < 40_000);
    }
    assert!(context.paced_millis >= 32);
}

#[test]
fn interrupt_from_a_worker_ends_main_sleep_without_waiting_out_the_timeout() {
    let sleep = thread_method("sleep", "(J)V", true);
    let interrupt = thread_method("interrupt", "()V", false);
    let mut constants = thread_constants("interrupt", "()V");
    constants.extend([
        Some(Constant::Fieldref {
            class_index: 8,
            name_and_type_index: 9,
        }),
        Some(Constant::Class { name_index: 10 }),
        Some(Constant::NameAndType {
            name_index: 11,
            descriptor_index: 12,
        }),
        Some(Constant::Utf8("Interrupter".into())),
        Some(Constant::Utf8("target".into())),
        Some(Constant::Utf8("Ljava/lang/Thread;".into())),
    ]);
    let worker_run = runtime_method(
        "Interrupter",
        "run",
        "()V",
        &[0xb2, 0, 7, 0xb6, 0, 1, 0xb1],
        1,
        1,
        constants,
        false,
    );
    let mut program = program_with_interrupted_exception();
    program
        .classes
        .insert("java/lang/Thread".into(), test_class_definition(None));
    let mut worker_class = test_class_definition(None);
    worker_class.fields.push(Field {
        key: "Interrupter.target:Ljava/lang/Thread;".into(),
        declaring_class: "Interrupter".into(),
        kind: ValueKind::Reference,
        is_static: true,
        field_token: FieldToken::new(),
        instance_slot: None,
        initial: Value::Reference(None),
        constant_string: None,
    });
    program.classes.insert("Interrupter".into(), worker_class);
    program.methods.insert(interrupt.key.clone(), interrupt);
    program.methods.insert(worker_run.key.clone(), worker_run);
    let mut context = pacing_context();
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let main = machine
            .heap
            .managed
            .allocate_object("java/lang/Thread", HashMap::new())
            .unwrap();
        machine.scheduler.main_thread = Some(main);
        machine.classes.static_fields.insert(
            "Interrupter.target:Ljava/lang/Thread;".into(),
            Value::Reference(Some(main)),
        );
        let worker = machine
            .heap
            .managed
            .allocate_object("Interrupter", HashMap::new())
            .unwrap();
        machine
            .scheduler
            .thread_states
            .insert(worker, ThreadState::Runnable);
        machine.scheduler.runnable_threads.push_back(worker);
        let Some(CallOutcome::Throw(exception)) = machine
            .invoke_vm_native(&sleep, &[Value::Long(1000)], 1)
            .unwrap()
        else {
            panic!("interrupted main sleep must throw");
        };
        assert_eq!(
            machine.object_class(exception).unwrap(),
            "java/lang/InterruptedException"
        );
        assert!(
            !machine
                .scheduler
                .interrupted_threads
                .contains(&MAIN_THREAD_ID)
        );
    }
    assert_eq!(context.paced_millis, 0);
}
