use super::*;

#[test]
fn exception_handler_entry_enforces_the_callers_operand_stack_limit() {
    let constants = vec![
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
        Some(Constant::Utf8("test/ThrowingCall".into())),
        Some(Constant::Utf8("fail".into())),
        Some(Constant::Utf8("()V".into())),
    ];
    for tracing in [false, true] {
        for max_stack in [0, 1] {
            let mut caller = runtime_method(
                "test/ThrowingCall",
                "run",
                "()V",
                &[0xb8, 0, 1, 0xb1, 0x57, 0xb1],
                max_stack,
                0,
                constants.clone(),
                true,
            );
            Arc::make_mut(&mut caller.exception_table).push(ExceptionHandler {
                start_pc: 0,
                end_pc: 3,
                handler_pc: 4,
                catch_type: 0,
            });
            let callee = runtime_method(
                "test/ThrowingCall",
                "fail",
                "()V",
                &[0x01, 0xbf],
                1,
                0,
                vec![None],
                true,
            );
            let mut program = program_with_exception("java/lang/NullPointerException");
            program.methods.insert(caller.key.clone(), caller);
            program.methods.insert(callee.key.clone(), callee);
            let result = program.execute(
                "test/ThrowingCall",
                "run",
                "()V",
                Limits::default(),
                tracing,
            );
            if max_stack == 0 {
                assert_eq!(result.unwrap_err().code(), "operand-stack-overflow");
            } else {
                assert_eq!(result.unwrap().value, None);
            }
        }
    }
}

#[test]
fn resumed_return_values_enforce_the_callers_operand_stack_limit() {
    for (descriptor, code, slots) in [("()I", [0x04, 0xac], 1), ("()J", [0x0a, 0xad], 2)] {
        let constants = vec![
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
            Some(Constant::Utf8("test/ResumedCall".into())),
            Some(Constant::Utf8("value".into())),
            Some(Constant::Utf8(descriptor.into())),
        ];
        for tracing in [false, true] {
            for max_stack in [slots - 1, slots] {
                let discard = if slots == 1 { 0x57 } else { 0x58 };
                let caller = runtime_method(
                    "test/ResumedCall",
                    "run",
                    "()V",
                    &[0xb8, 0, 1, discard, 0xb1],
                    max_stack,
                    0,
                    constants.clone(),
                    true,
                );
                let callee = runtime_method(
                    "test/ResumedCall",
                    "value",
                    descriptor,
                    &code,
                    slots,
                    0,
                    vec![None],
                    true,
                );
                let mut program = Program::new();
                program.methods.insert(callee.key.clone(), callee);
                let mut host = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), tracing, &mut host);
                let worker = machine
                    .heap
                    .managed
                    .allocate_object("java/lang/Thread", HashMap::new())
                    .unwrap();
                machine.scheduler.current_thread = worker.to_raw();
                machine.scheduler.quantum_remaining = 1;
                let CallOutcome::Suspend(continuation) = machine.call(&caller, [], 1).unwrap()
                else {
                    panic!("the callee must suspend before returning its value");
                };
                assert!(continuation.pending.is_some());
                machine.scheduler.quantum_remaining = 64;
                let outcome = machine.resume_suspended_call(continuation, 1);
                if max_stack < slots {
                    assert_eq!(outcome.err().unwrap().code(), "operand-stack-overflow");
                } else {
                    assert!(matches!(outcome.unwrap(), CallOutcome::Return(None)));
                }
                assert_eq!(machine.execution.frame_slots, 0);
                assert_eq!(machine.execution.stack_slots, 0);
                assert!(machine.execution.call_stack.is_empty());
                assert!(machine.heap.frame_roots.values().all(Vec::is_empty));
            }
        }
    }
}

#[test]
fn resumed_native_allocation_errors_become_java_exceptions_and_restore_accounting() {
    let mut program = program_with_exception("java/lang/OutOfMemoryError");
    let read = runtime_method(
        "test/DataInput",
        "readUnsignedByte",
        "()I",
        &[0x10, 65, 0xac],
        1,
        1,
        vec![None],
        false,
    );
    program.methods.insert(read.key.clone(), read);
    let mut read_utf = runtime_method(
        "java/io/DataInputStream",
        "readUTF",
        "()Ljava/lang/String;",
        &[],
        0,
        1,
        vec![None],
        false,
    );
    read_utf.is_native = true;
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 4096,
            ..Limits::default()
        },
        false,
        &mut host,
    );
    let input = machine
        .heap
        .managed
        .allocate_object("test/DataInput", HashMap::new())
        .unwrap();
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = worker.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Running);
    machine.scheduler.quantum_remaining = 0;
    machine.execution.frame_slots = 23;
    machine.execution.stack_slots = 7;
    let CallOutcome::Suspend(continuation) = machine
        .continue_data_input_read_utf(&read_utf, input, Some(8192), vec![b'A'; 8191], 1)
        .unwrap()
    else {
        panic!("final byte read must suspend");
    };
    assert_eq!(machine.execution.frame_slots, 23);
    assert_eq!(machine.execution.stack_slots, 7);
    machine.scheduler.quantum_remaining = 64;
    let CallOutcome::Throw(exception) = machine.resume_suspended_call(continuation, 1).unwrap()
    else {
        panic!("oversized decoded string must throw OutOfMemoryError after resume");
    };
    assert_eq!(
        machine.object_class(exception).unwrap(),
        "java/lang/OutOfMemoryError"
    );
    assert_eq!(machine.execution.frame_slots, 23);
    assert_eq!(machine.execution.stack_slots, 7);
    assert!(machine.execution.call_stack.is_empty());
    assert!(machine.heap.frame_roots.values().all(Vec::is_empty));
    assert_eq!(
        machine.heap.throwable_traces[&exception][0].key.name,
        "readUTF"
    );
}

#[test]
fn ordinary_call_boundaries_restore_parent_slots_on_returns_and_errors() {
    let program = Program::new();
    for (code, successful) in [
        (&[0x04, 0xac][..], true),
        (&[0x04, 0x03, 0x6c, 0xac], false),
    ] {
        for tracing in [false, true] {
            let method = runtime_method("test/Call", "run", "()I", code, 2, 4, vec![None], true);
            let mut host = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), tracing, &mut host);
            machine.execution.frame_slots = 23;
            machine.execution.stack_slots = 7;
            let outcome = machine.call(&method, [], 1);
            if successful {
                assert!(matches!(
                    outcome.unwrap(),
                    CallOutcome::Return(Some(Value::Int(1)))
                ));
            } else {
                assert_eq!(outcome.err().unwrap().code(), "arithmetic-exception");
            }
            assert_eq!(machine.execution.frame_slots, 23);
            assert_eq!(machine.execution.stack_slots, 7);
            assert!(machine.execution.call_stack.is_empty());
            assert!(machine.heap.frame_roots.values().all(Vec::is_empty));
        }
    }
}

#[test]
fn stack_rearrangement_respects_method_and_caller_slot_limits() {
    let program = Program::new();
    for (max_stack, max_stack_slots, succeeds) in [(3, 11, false), (4, 10, false), (4, 11, true)] {
        for tracing in [false, true] {
            let method = runtime_method(
                "test/Call",
                "run",
                "()I",
                &[0x0a, 0x5c, 0x58, 0x88, 0xac],
                max_stack,
                0,
                vec![None],
                true,
            );
            let mut host = DefaultNativeContext;
            let mut machine = program.machine(
                Limits {
                    max_stack_slots,
                    ..Limits::default()
                },
                tracing,
                &mut host,
            );
            machine.execution.stack_slots = 7;
            let outcome = machine.call(&method, [], 1);
            if succeeds {
                assert!(matches!(
                    outcome,
                    Ok(CallOutcome::Return(Some(Value::Int(1))))
                ));
            } else {
                assert_eq!(outcome.err().unwrap().code(), "operand-stack-overflow");
            }
            assert_eq!(machine.execution.stack_slots, 7);
            assert!(machine.execution.call_stack.is_empty());
        }
    }
}

#[test]
fn class_new_instance_resumes_a_suspended_worker_constructor() {
    let mut new_instance = runtime_method(
        "java/lang/Class",
        "newInstance",
        "()Ljava/lang/Object;",
        &[],
        0,
        1,
        Vec::new(),
        false,
    );
    new_instance.is_native = true;
    let constructor = runtime_method(
        "ReflectiveTarget",
        "<init>",
        "()V",
        &[0xb1],
        0,
        1,
        Vec::new(),
        false,
    );
    let mut program = Program::new();
    for name in ["java/lang/Class", "ReflectiveTarget"] {
        program.classes.insert(
            name.to_owned(),
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
    }
    program
        .methods
        .insert(new_instance.key.clone(), new_instance.clone());
    program.methods.insert(constructor.key.clone(), constructor);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    machine.scheduler.current_thread = worker.to_raw();
    let class = machine.intern_java_class("ReflectiveTarget").unwrap();

    let CallOutcome::Suspend(first) = machine
        .call(&new_instance, vec![Value::Reference(Some(class))], 1)
        .unwrap()
    else {
        panic!("worker constructor must suspend when its quantum is exhausted");
    };
    let CallOutcome::Suspend(second) = machine.resume_suspended_call(first, 1).unwrap() else {
        panic!("Class.newInstance must preserve a repeatedly suspended constructor");
    };
    machine.scheduler.quantum_remaining = WORKER_QUANTUM;
    let CallOutcome::Return(Some(Value::Reference(Some(instance)))) =
        machine.resume_suspended_call(second, 1).unwrap()
    else {
        panic!("Class.newInstance must return its instance after constructor resume");
    };
    assert_eq!(machine.object_class(instance).unwrap(), "ReflectiveTarget");
}
