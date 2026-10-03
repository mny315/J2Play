use super::*;

fn collection_program(intrinsics: bool) -> Program {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory() {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    for method in program.methods.values_mut() {
        method.compatibility_candidate = Some(intrinsics);
    }
    program
}

#[test]
fn cldc_collections_park_contended_calls_before_reading_or_mutating_state() {
    for intrinsics in [false, true] {
        let program = collection_program(intrinsics);
        for (class, name, descriptor) in [
            ("java/util/Vector", "addElement", "(Ljava/lang/Object;)V"),
            ("java/util/Vector", "elementAt", "(I)Ljava/lang/Object;"),
            (
                "java/util/Hashtable",
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            ),
            (
                "java/util/Hashtable",
                "get",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
            ),
        ] {
            let method = |name: &str, descriptor: &str| {
                &program.methods[&MethodKey {
                    class: class.into(),
                    name: name.into(),
                    descriptor: descriptor.into(),
                }]
            };
            let mut host = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut host);
            let receiver = machine.allocate_native_instance(class, &[]).unwrap();
            let receiver_arg = Value::Reference(Some(receiver));
            assert!(matches!(
                machine
                    .call(method("<init>", "()V"), vec![receiver_arg], 1)
                    .unwrap(),
                CallOutcome::Return(None)
            ));
            let key = machine.intern_string("key", &[], &[receiver_arg]).unwrap();
            let key_arg = Value::Reference(Some(key));
            let value = machine
                .intern_string("value", &[], &[receiver_arg, key_arg])
                .unwrap();
            let value_arg = Value::Reference(Some(value));
            let (setup_name, setup_descriptor, setup_args, count_field) =
                if class.ends_with("Vector") {
                    (
                        "addElement",
                        "(Ljava/lang/Object;)V",
                        vec![receiver_arg, key_arg],
                        "java/util/Vector.elementCount:I",
                    )
                } else {
                    (
                        "put",
                        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                        vec![receiver_arg, key_arg, key_arg],
                        "java/util/Hashtable.count:I",
                    )
                };
            assert!(matches!(
                machine
                    .call(method(setup_name, setup_descriptor), setup_args, 1)
                    .unwrap(),
                CallOutcome::Return(_)
            ));
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
            machine.scheduler.quantum_remaining = 10_000;
            let args = match name {
                "addElement" => vec![receiver_arg, value_arg],
                "elementAt" => vec![receiver_arg, Value::Int(0)],
                "put" => vec![receiver_arg, key_arg, value_arg],
                _ => vec![receiver_arg, key_arg],
            };
            let CallOutcome::Suspend(continuation) =
                machine.call(method(name, descriptor), args, 1).unwrap()
            else {
                panic!(
                    "{class}.{name} ran while another thread owned its monitor; intrinsics={intrinsics}"
                );
            };
            assert_eq!(
                continuation
                    .monitor_entry
                    .as_ref()
                    .map(|entry| entry.object),
                Some(receiver)
            );
            assert_eq!(
                machine.graphics_int_field(receiver, count_field).unwrap(),
                1
            );
            assert_eq!(
                machine.scheduler.thread_states[&waiter],
                ThreadState::Sleeping
            );
            assert_eq!(
                machine.scheduler.monitor_entry_waiters[&receiver],
                VecDeque::from([waiter])
            );
            machine.scheduler.current_thread = owner.to_raw();
            machine.exit_monitor(receiver).unwrap();
            assert_eq!(machine.scheduler.runnable_threads.pop_front(), Some(waiter));
            machine.scheduler.current_thread = waiter.to_raw();
            machine
                .scheduler
                .thread_states
                .insert(waiter, ThreadState::Running);
            let expected = if name == "addElement" {
                None
            } else {
                Some(key_arg)
            };
            let CallOutcome::Return(actual) =
                machine.resume_suspended_call(continuation, 1).unwrap()
            else {
                panic!("{class}.{name} did not finish after acquiring the monitor");
            };
            assert_eq!(actual, expected);
            assert_eq!(
                machine.graphics_int_field(receiver, count_field).unwrap(),
                if name == "addElement" { 2 } else { 1 }
            );
            assert!(!machine.scheduler.monitors.contains_key(&receiver));
            assert!(
                !machine
                    .scheduler
                    .monitor_entry_waiters
                    .contains_key(&receiver)
            );
            let (read_name, read_descriptor, read_argument) = if class.ends_with("Vector") {
                (
                    "elementAt",
                    "(I)Ljava/lang/Object;",
                    Value::Int(i32::from(name == "addElement")),
                )
            } else {
                ("get", "(Ljava/lang/Object;)Ljava/lang/Object;", key_arg)
            };
            let CallOutcome::Return(actual) = machine
                .call(
                    method(read_name, read_descriptor),
                    vec![receiver_arg, read_argument],
                    1,
                )
                .unwrap()
            else {
                panic!("collection value was unavailable after the call");
            };
            assert_eq!(
                actual,
                Some(if matches!(name, "addElement" | "put") {
                    value_arg
                } else {
                    key_arg
                })
            );
        }
    }
}
