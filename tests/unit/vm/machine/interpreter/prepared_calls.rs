use super::*;

fn prepared_class(with_child: bool, padding: usize) -> ClassFile {
    use classfile::{CodeAttribute, Member};
    let code = |bytes, max_stack, max_locals| {
        Attribute::Code(CodeAttribute {
            name_index: 5,
            max_stack,
            max_locals,
            code: bytes,
            exception_table: vec![],
            attributes: vec![],
        })
    };
    let mut callee = vec![0x00; padding];
    callee.extend([0x2a, 0x03, 0x5c, 0x2e, 0x04, 0x60, 0x4f]);
    if with_child {
        callee.extend([0xb8, 0, 12]);
    }
    callee.extend([0x2a, 0x03, 0x2e, 0xac]);
    ClassFile {
        minor_version: 0,
        major_version: 45,
        constant_pool: vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("PreparedCalls".into())),
            Some(Constant::Utf8("entry".into())),
            Some(Constant::Utf8("([I)I".into())),
            Some(Constant::Utf8("Code".into())),
            Some(Constant::Utf8("callee".into())),
            Some(Constant::NameAndType {
                name_index: 6,
                descriptor_index: 4,
            }),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 7,
            }),
            Some(Constant::Utf8("touch".into())),
            Some(Constant::Utf8("()V".into())),
            Some(Constant::NameAndType {
                name_index: 9,
                descriptor_index: 10,
            }),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 11,
            }),
        ],
        access_flags: 0x21,
        this_class: 1,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        attributes: vec![],
        methods: vec![
            Member {
                access_flags: 9,
                name_index: 3,
                descriptor_index: 4,
                attributes: vec![code(vec![0x2a, 0xb8, 0, 8, 0xac], 1, 1)],
            },
            Member {
                access_flags: 9,
                name_index: 6,
                descriptor_index: 4,
                attributes: vec![code(callee, 4, 1)],
            },
            Member {
                access_flags: 9,
                name_index: 9,
                descriptor_index: 10,
                attributes: vec![code(vec![0x04, 0xbc, 10, 0x57, 0xb1], 1, 0)],
            },
        ],
    }
}

fn prepared_program(with_child: bool, padding: usize) -> Program {
    let class = prepared_class(with_child, padding);
    let mut program = Program::new();
    program.add_class(&class, &Limits::default()).unwrap();
    program
}

#[test]
fn prepared_locals_keep_the_only_remaining_reference_during_native_collection() {
    use classfile::CodeAttribute;
    let mut class = prepared_class(false, 0);
    class.constant_pool[4] = Some(Constant::Utf8("([Ljava/lang/Object;)I".into()));
    class.methods[1].attributes = vec![Attribute::Code(CodeAttribute {
        name_index: 5,
        max_stack: 3,
        max_locals: 2,
        code: vec![
            0x2a, 0x03, 0x32, 0x4c, // local 1 = container[0]
            0x2a, 0x03, 0x01, 0x53, // container[0] = null
            0xb8, 0, 12, // collect after transferring the prefix
            0x2b, 0xbe, 0xac,
        ],
        exception_table: vec![],
        attributes: vec![],
    })];
    class.methods[2].access_flags = 0x0109;
    class.methods[2].attributes.clear();
    let mut program = Program::new();
    program.add_class(&class, &Limits::default()).unwrap();
    program
        .native_registry_mut()
        .register(
            NativeSignature::new("PreparedCalls", "touch", "()V"),
            |context, args| {
                assert!(args.is_empty());
                context.allocate_java_int_array(&[0; 180])?;
                Ok(None)
            },
        )
        .unwrap();
    for tracing in [true, false] {
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                max_heap_bytes: 1024,
                ..Limits::default()
            },
            tracing,
            &mut host,
        );
        let garbage = (0..6)
            .map(|_| {
                machine
                    .heap
                    .managed
                    .allocate_array(ArrayKind::Int, 8)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let child = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 7)
            .unwrap();
        let container = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Reference("Ljava/lang/Object;".into()), 1)
            .unwrap();
        machine
            .heap
            .managed
            .array_set_typed(
                container,
                0,
                heap::ArrayAccessKind::Reference,
                Value::Reference(Some(child)),
            )
            .unwrap();
        let method = program
            .methods
            .get(&MethodKey {
                class: "PreparedCalls".into(),
                name: "entry".into(),
                descriptor: "([Ljava/lang/Object;)I".into(),
            })
            .unwrap();
        let result = machine
            .call(method, [Value::Reference(Some(container))], 1)
            .unwrap();
        assert!(matches!(result, CallOutcome::Return(Some(Value::Int(7)))));
        assert!(machine.heap.managed.get(child).is_ok());
        assert!(
            garbage
                .iter()
                .all(|handle| machine.heap.managed.get(*handle).is_err())
        );
        assert_eq!(
            machine
                .heap
                .managed
                .array_get_typed(container, 0, heap::ArrayAccessKind::Reference)
                .unwrap(),
            Value::Reference(None)
        );
        assert_eq!(machine.execution.frame_slots, 0);
        assert_eq!(machine.execution.stack_slots, 0);
        assert!(machine.execution.call_stack.is_empty());
    }
}

fn run_prepared(
    program: &Program,
    tracing: bool,
    max_instructions: u64,
    max_stack_slots: usize,
    max_frames: usize,
    extra_locals: usize,
) -> (Result<Option<Value>, String>, Value, u64) {
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_instructions,
            max_stack_slots,
            max_frames,
            max_runtime_bytes: program.runtime_bytes
                + extra_locals * std::mem::size_of::<Option<Value>>(),
            ..Limits::default()
        },
        tracing,
        &mut host,
    );
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    let method = program
        .methods
        .get(&MethodKey {
            class: "PreparedCalls".into(),
            name: "entry".into(),
            descriptor: "([I)I".into(),
        })
        .unwrap();
    let result = match machine.call(method, [Value::Reference(Some(array))], 1) {
        Ok(CallOutcome::Return(value)) => Ok(value),
        Err(error) => Err(error.to_string()),
        _ => panic!("unexpected throw/suspension in prepared-call fixture"),
    };
    let stored = machine
        .heap
        .managed
        .array_get_typed(array, 0, heap::ArrayAccessKind::Int)
        .unwrap();
    assert_eq!(machine.execution.frame_slots, 0);
    assert_eq!(machine.execution.stack_slots, 0);
    assert!(machine.execution.call_stack.is_empty());
    (result, stored, machine.execution.instructions)
}

#[test]
fn prepared_calls_preserve_writes_pc_and_bounds_when_entering_nested_calls() {
    for with_child in [false, true] {
        for padding in [0, 1016, 1020] {
            let program = prepared_program(with_child, padding);
            for max_frames in [1, 2, 3] {
                for stack_slots in [3, 4, 5] {
                    for locals in [1, 2] {
                        for budget in [
                            1, 2, 7, 8, 9, 10, 11, 15, 16, 19, 1022, 1023, 1024, 1025, 1040,
                        ] {
                            let expected = run_prepared(
                                &program,
                                true,
                                budget,
                                stack_slots,
                                max_frames,
                                locals,
                            );
                            let actual = run_prepared(
                                &program,
                                false,
                                budget,
                                stack_slots,
                                max_frames,
                                locals,
                            );
                            assert_eq!(
                                actual, expected,
                                "child={with_child} padding={padding} budget={budget} slots={stack_slots} frames={max_frames} locals={locals}"
                            );
                            if let Ok(value) = actual.0 {
                                assert_eq!(value, Some(Value::Int(1)));
                                assert_eq!(actual.1, Value::Int(1));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn run_worker_prepared(
    program: &Program,
    tracing: bool,
    quantum: u64,
) -> (Vec<String>, Option<Value>, Value, u64) {
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), tracing, &mut host);
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
    machine.scheduler.quantum_remaining = quantum;
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    let method = program
        .methods
        .get(&MethodKey {
            class: "PreparedCalls".into(),
            name: "entry".into(),
            descriptor: "([I)I".into(),
        })
        .unwrap();
    let mut outcome = machine
        .call(method, [Value::Reference(Some(array))], 1)
        .unwrap();
    let mut checkpoints = Vec::new();
    while let CallOutcome::Suspend(continuation) = outcome {
        assert!(checkpoints.len() < 2048, "worker failed to make progress");
        assert_eq!(machine.execution.frame_slots, 0);
        assert_eq!(machine.execution.stack_slots, 0);
        assert!(machine.execution.call_stack.is_empty());
        let stored = machine
            .heap
            .managed
            .array_get_typed(array, 0, heap::ArrayAccessKind::Int)
            .unwrap();
        let frames = continuation
            .frames()
            .map(|frame| {
                (
                    &frame.method.key,
                    frame.pc,
                    &frame.locals,
                    &frame.stack,
                    frame.pending.as_ref().map(|pending| pending.next_pc),
                )
            })
            .collect::<Vec<_>>();
        checkpoints.push(format!(
            "{} {stored:?} {frames:?}",
            machine.execution.instructions
        ));
        let mut roots = machine.roots(&[], &[]);
        continuation.roots(&mut roots);
        machine.collect_heap(roots);
        assert!(machine.heap.managed.get(array).is_ok());
        machine.scheduler.quantum_remaining = quantum.max(1);
        outcome = machine.resume_suspended_call(continuation, 1).unwrap();
    }
    let CallOutcome::Return(value) = outcome else {
        panic!("unexpected exception in prepared worker fixture");
    };
    let stored = machine
        .heap
        .managed
        .array_get_typed(array, 0, heap::ArrayAccessKind::Int)
        .unwrap();
    assert_eq!(machine.execution.frame_slots, 0);
    assert_eq!(machine.execution.stack_slots, 0);
    assert!(machine.execution.call_stack.is_empty());
    (checkpoints, value, stored, machine.execution.instructions)
}

#[test]
fn prepared_worker_calls_preserve_every_yield_and_resume_without_replaying_writes() {
    for with_child in [false, true] {
        for padding in [0, 1016] {
            let program = prepared_program(with_child, padding);
            for quantum in [0, 1, 2, 3, 7, 9, 10, 11, 12, 17, 1021] {
                let expected = run_worker_prepared(&program, true, quantum);
                let actual = run_worker_prepared(&program, false, quantum);
                assert_eq!(
                    actual, expected,
                    "child={with_child} padding={padding} quantum={quantum}"
                );
                assert_eq!(actual.1, Some(Value::Int(1)));
                assert_eq!(actual.2, Value::Int(1));
            }
        }
    }
}

fn wide_call_program(is_static: bool) -> Program {
    use classfile::CodeAttribute;
    let mut class = prepared_class(false, 0);
    let entry_descriptor = u16::try_from(class.constant_pool.len()).unwrap();
    class
        .constant_pool
        .push(Some(Constant::Utf8("(LPreparedCalls;)I".into())));
    let callee_descriptor = u16::try_from(class.constant_pool.len()).unwrap();
    class
        .constant_pool
        .push(Some(Constant::Utf8("(JDI)J".into())));
    class.constant_pool[7] = Some(Constant::NameAndType {
        name_index: 6,
        descriptor_index: callee_descriptor,
    });
    class.methods.truncate(2);
    class.methods[0].descriptor_index = entry_descriptor;
    let mut entry = vec![0x06]; // Keep 3 below the invocation arguments.
    if !is_static {
        entry.push(0x2a);
    }
    entry.extend([0x0a, 0x0f, 0x05, if is_static { 0xb8 } else { 0xb6 }, 0, 8]);
    // After the call, fill the caller's exact eleven-slot limit while the
    // returned long and the unrelated integer remain on its operand stack.
    entry.extend([0x03; 8]);
    entry.extend([0x57; 8]);
    entry.extend([0x88, 0x60, 0xac]);
    class.methods[0].attributes = vec![Attribute::Code(CodeAttribute {
        name_index: 5,
        max_stack: 11,
        max_locals: 1,
        code: entry,
        exception_table: vec![],
        attributes: vec![],
    })];
    class.methods[1].access_flags = if is_static { 9 } else { 1 };
    class.methods[1].descriptor_index = callee_descriptor;
    class.methods[1].attributes = vec![Attribute::Code(CodeAttribute {
        name_index: 5,
        max_stack: 4,
        max_locals: if is_static { 5 } else { 6 },
        code: vec![
            if is_static { 0x1e } else { 0x1f },
            if is_static { 0x28 } else { 0x29 },
            0x8f,
            0x61,
            0x15,
            if is_static { 4 } else { 5 },
            0x85,
            0x61,
            0xad,
        ],
        exception_table: vec![],
        attributes: vec![],
    })];
    let mut program = Program::new();
    program.add_class(&class, &Limits::default()).unwrap();
    program
}

fn run_wide_call(
    program: &Program,
    tracing: bool,
    max_stack_slots: usize,
    max_instructions: u64,
) -> (Result<Option<Value>, String>, u64) {
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_instructions,
            max_stack_slots,
            ..Limits::default()
        },
        tracing,
        &mut host,
    );
    let receiver = machine
        .heap
        .managed
        .allocate_object("PreparedCalls", HashMap::new())
        .unwrap();
    let method = program
        .methods
        .get(&MethodKey {
            class: "PreparedCalls".into(),
            name: "entry".into(),
            descriptor: "(LPreparedCalls;)I".into(),
        })
        .unwrap();
    let result = match machine.call(method, [Value::Reference(Some(receiver))], 1) {
        Ok(CallOutcome::Return(value)) => Ok(value),
        Err(error) => Err(error.to_string()),
        _ => panic!("unexpected throw/suspension in wide-call fixture"),
    };
    assert_eq!(machine.execution.stack_slots, 0);
    assert_eq!(machine.execution.frame_slots, 0);
    assert!(machine.execution.call_stack.is_empty());
    (result, machine.execution.instructions)
}

#[test]
fn wide_call_arguments_preserve_caller_slots_at_limits() {
    for is_static in [false, true] {
        let program = wide_call_program(is_static);
        let instructions = 32 + u64::from(!is_static);
        for slots in [10, 11] {
            for budget in [1, 4, 8, 12, 13, 14, 31, 32, 33, 40] {
                let expected = run_wide_call(&program, true, slots, budget);
                let actual = run_wide_call(&program, false, slots, budget);
                assert_eq!(
                    actual, expected,
                    "static={is_static} slots={slots} budget={budget}"
                );
                if slots == 11 && budget >= instructions {
                    assert_eq!(actual, (Ok(Some(Value::Int(7))), instructions));
                } else {
                    assert!(actual.0.is_err());
                }
            }
        }
    }
}

#[test]
fn shorter_readonly_path_still_finishes_when_cached_path_exceeds_remaining_budget() {
    use classfile::CodeAttribute;
    let mut class = prepared_class(false, 0);
    let mut code = vec![0x2a, 0x03, 0x2e, 0x99, 0, 0];
    code.extend([0x00; 200]);
    code.extend([0x10, 7, 0xac]);
    let short = code.len();
    code[4..6].copy_from_slice(&i16::try_from(short - 3).unwrap().to_be_bytes());
    code.extend([0x08, 0xac]);
    class.methods[1].attributes = vec![Attribute::Code(CodeAttribute {
        name_index: 5,
        max_stack: 2,
        max_locals: 1,
        code,
        exception_table: vec![],
        attributes: vec![],
    })];
    let mut program = Program::new();
    program.add_class(&class, &Limits::default()).unwrap();
    for tracing in [true, false] {
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), tracing, &mut host);
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 1)
            .unwrap();
        machine
            .heap
            .managed
            .array_set_typed(array, 0, heap::ArrayAccessKind::Int, Value::Int(1))
            .unwrap();
        let method = program
            .methods
            .get(&MethodKey {
                class: "PreparedCalls".into(),
                name: "entry".into(),
                descriptor: "([I)I".into(),
            })
            .unwrap();
        for _ in 0..2 {
            assert!(matches!(
                machine
                    .call(method, [Value::Reference(Some(array))], 1)
                    .unwrap(),
                CallOutcome::Return(Some(Value::Int(7)))
            ));
        }
        assert_eq!(machine.execution.instructions, 418);
        if !tracing {
            assert!(machine.execution.leaf_cache.hits > 0);
        }
        machine
            .heap
            .managed
            .array_set_typed(array, 0, heap::ArrayAccessKind::Int, Value::Int(0))
            .unwrap();
        machine.execution.instructions = 1000;
        machine.limits.max_instructions = 1010;
        assert!(matches!(
            machine
                .call(method, [Value::Reference(Some(array))], 1)
                .unwrap(),
            CallOutcome::Return(Some(Value::Int(5)))
        ));
        assert_eq!(machine.execution.instructions, 1009);
        assert_eq!(machine.execution.stack_slots, 0);
        assert_eq!(machine.execution.frame_slots, 0);
        assert!(machine.execution.call_stack.is_empty());
    }
}
