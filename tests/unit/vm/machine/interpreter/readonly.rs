use super::*;

fn readonly_leaf_program(iterations: i32, enabled: bool, index: u8) -> Program {
    readonly_fixture_program(iterations, enabled, index, false)
}

fn readonly_fixture_program(iterations: i32, enabled: bool, index: u8, with_call: bool) -> Program {
    use classfile::{CodeAttribute, Member};
    let mut caller = vec![0x04, 0xbc, 10, 0x4b, 0x03, 0x3c, 0x03, 0x3d];
    let start = caller.len();
    caller.extend([0x1b, 0x12, 10]);
    let exit = caller.len();
    caller.extend([
        0xa2,
        0,
        0,
        0x2a,
        0x03,
        0x1b,
        0x4f,
        0x1c,
        0x2a,
        index,
        0xb8,
        0,
        if with_call { 13 } else { 9 },
        0x60,
        0x3d,
        0x84,
        1,
        1,
    ]);
    let back = caller.len();
    caller.extend([0xa7, 0, 0]);
    let finish = caller.len();
    caller.extend([0x1c, 0xac]);
    caller[exit + 1..exit + 3]
        .copy_from_slice(&i16::try_from(finish - exit).unwrap().to_be_bytes());
    caller[back + 1..back + 3]
        .copy_from_slice(&(-i16::try_from(back - start).unwrap()).to_be_bytes());
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
    let mut class = ClassFile {
        minor_version: 0,
        major_version: 45,
        constant_pool: vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("ReadProbe".into())),
            Some(Constant::Utf8("main".into())),
            Some(Constant::Utf8("()I".into())),
            Some(Constant::Utf8("Code".into())),
            Some(Constant::Utf8("read".into())),
            Some(Constant::Utf8("([II)I".into())),
            Some(Constant::NameAndType {
                name_index: 6,
                descriptor_index: 7,
            }),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 8,
            }),
            Some(Constant::Integer(iterations)),
            Some(Constant::Utf8("wrapper".into())),
            Some(Constant::NameAndType {
                name_index: 11,
                descriptor_index: 7,
            }),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 12,
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
                attributes: vec![code(caller, 3, 3)],
            },
            Member {
                access_flags: 9,
                name_index: 6,
                descriptor_index: 7,
                attributes: vec![code(vec![0x2a, 0x1b, 0x2e, 0x04, 0x60, 0xac], 2, 2)],
            },
        ],
    };
    if with_call {
        let mut wrapper = vec![0x00; 16];
        wrapper.extend([0x2a, 0x1b, 0xb8, 0, 9, 0xac]);
        class.methods.push(Member {
            access_flags: 9,
            name_index: 11,
            descriptor_index: 7,
            attributes: vec![code(wrapper, 2, 2)],
        });
    }
    let mut program = Program::new();
    program.add_class(&class, &Limits::default()).unwrap();
    assert!(program.methods.values().any(|method| method.readonly_leaf));
    if !enabled {
        for method in program.methods.values_mut() {
            method.readonly_leaf = false;
            method.readonly_call_tree = false;
        }
    }
    program
}

#[test]
fn readonly_leaf_calls_preserve_array_changes_errors_and_limits() {
    for index in [0x02, 0x03, 0x04] {
        let ordinary = readonly_leaf_program(100, false, index);
        let accelerated = readonly_leaf_program(100, true, index);
        let cases = (1..60)
            .chain([1022, 1023, 1024, 1025, 5000])
            .map(|instruction_limit| Limits {
                max_instructions: instruction_limit,
                ..Limits::default()
            })
            .chain([
                Limits {
                    max_frames: 1,
                    ..Limits::default()
                },
                Limits {
                    max_frames: 2,
                    ..Limits::default()
                },
                Limits {
                    max_stack_slots: 4,
                    ..Limits::default()
                },
                Limits {
                    max_stack_slots: 5,
                    ..Limits::default()
                },
                Limits {
                    max_runtime_bytes: ordinary.runtime_bytes
                        + 3 * std::mem::size_of::<Option<Value>>(),
                    ..Limits::default()
                },
            ]);
        for limits in cases {
            let expected = ordinary.execute("ReadProbe", "main", "()I", limits.clone(), false);
            let actual = accelerated.execute("ReadProbe", "main", "()I", limits, false);
            match (actual, expected) {
                (Ok(actual), Ok(expected)) => {
                    assert_eq!(actual.value, Some(Value::Int(5050)));
                    assert_eq!(actual.value, expected.value);
                    assert_eq!(actual.instructions, expected.instructions);
                }
                (Err(actual), Err(expected)) => {
                    assert_eq!(actual.to_string(), expected.to_string())
                }
                (actual, expected) => panic!("leaf mismatch: {actual:?} vs {expected:?}"),
            }
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn readonly_leaf_throughput() {
    for enabled in [false, true, false, true] {
        let program = readonly_leaf_program(200_000, enabled, 0x03);
        let started = std::time::Instant::now();
        let result = program
            .execute(
                "ReadProbe",
                "main",
                "()I",
                Limits {
                    max_instructions: 10_000_000,
                    ..Limits::default()
                },
                false,
            )
            .unwrap();
        assert_eq!(result.value, Some(Value::Int(20_000_100_000_i64 as i32)));
        eprintln!(
            "leaf={enabled} elapsed={:?} instructions={}",
            started.elapsed(),
            result.instructions
        );
    }
}

#[test]
fn readonly_memo_checks_array_dependencies_arguments_and_poll_budget() {
    let mut code = vec![0x00; 16];
    code.extend([0x2a, 0x1b, 0xb8, 0, 1, 0xac]);
    let mut leaf = runtime_method("ReadProbe", "read", "([II)I", &code, 2, 2, vec![None], true);
    leaf.stack_key_id = Some(0);
    leaf.readonly_call_tree =
        crate::machine::interpreter_batch::is_readonly_call_tree(&leaf.instructions);
    leaf.constant_pool_id = Some(0);
    let mut child = runtime_method(
        "ReadProbe",
        "child",
        "([II)I",
        &[0x2a, 0x1b, 0x2e, 0x04, 0x60, 0xac],
        2,
        2,
        vec![None],
        true,
    );
    child.stack_key_id = Some(1);
    child.readonly_leaf = true;
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    machine.classes.initialized.linked.push(true);
    machine
        .classes
        .fixed_method_inline_cache
        .insert(0, 1, 0xb8, std::rc::Rc::new(child), Some(0));
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 2)
        .unwrap();
    let args = [Value::Reference(Some(array)), Value::Int(0)];
    for (stored, expected_hits) in [(0, 0), (0, 1), (7, 1), (7, 2), (0, 2)] {
        machine
            .heap
            .managed
            .array_set_typed(array, 0, heap::ArrayAccessKind::Int, HeapValue::Int(stored))
            .unwrap();
        let before = machine.execution.instructions;
        assert!(matches!(machine.try_readonly_leaf(&leaf, &args, 1),
            Some(CallOutcome::Return(Some(Value::Int(value)))) if value == stored + 1));
        assert_eq!(machine.execution.instructions - before, 26);
        assert_eq!(machine.execution.leaf_cache.hits, expected_hits);
    }
    assert!(
        machine
            .try_readonly_leaf(&leaf, &[args[0], Value::Int(-1)], 1)
            .is_none()
    );
    assert!(
        machine
            .try_readonly_leaf(&leaf, &[args[0], Value::Int(2)], 1)
            .is_none()
    );
    assert!(
        machine
            .try_readonly_leaf(&leaf, &[Value::Reference(None), args[1]], 1)
            .is_none()
    );
    let hits = machine.execution.leaf_cache.hits;
    machine.execution.instructions = 1020;
    assert!(machine.try_readonly_leaf(&leaf, &args, 1).is_none());
    machine.execution.instructions = 0;
    assert!(matches!(
        machine.try_readonly_leaf(&leaf, &args, 1),
        Some(CallOutcome::Return(Some(Value::Int(1))))
    ));
    assert_eq!(machine.execution.leaf_cache.hits, hits + 1);
    assert!(matches!(
        machine.try_readonly_leaf(&leaf, &args, 2),
        Some(CallOutcome::Return(Some(Value::Int(1))))
    ));
    let hits = machine.execution.leaf_cache.hits;
    assert!(matches!(
        machine.try_readonly_leaf(&leaf, &args, 1),
        Some(CallOutcome::Return(Some(Value::Int(1))))
    ));
    assert_eq!(machine.execution.leaf_cache.hits, hits + 1);
    machine.execution.instructions = 1023;
    assert!(machine.try_readonly_leaf(&leaf, &args, 1).is_none());
    machine.execution.instructions = 0;
    machine.execution.tracing = true;
    assert!(machine.try_readonly_leaf(&leaf, &args, 1).is_none());
    machine.execution.tracing = false;
    machine.limits.max_runtime_bytes =
        program.runtime_bytes + 2 * std::mem::size_of::<Option<Value>>();
    let hits = machine.execution.leaf_cache.hits;
    assert!(machine.try_readonly_leaf(&leaf, &args, 1).is_none());
    assert_eq!(machine.execution.leaf_cache.hits, hits);
}

#[test]
fn readonly_call_tree_matches_interpreter_at_limits_and_invalid_reads() {
    for index in [0x02, 0x03, 0x04] {
        let ordinary = readonly_fixture_program(100, false, index, true);
        let accelerated = readonly_fixture_program(100, true, index, true);
        for max_frames in [1, 2, 3, 4, 64] {
            for max_instructions in (1..100).chain([1022, 1023, 1024, 1025, 8000]) {
                let limits = Limits {
                    max_instructions,
                    max_frames,
                    ..Limits::default()
                };
                let expected = ordinary.execute("ReadProbe", "main", "()I", limits.clone(), false);
                let actual = accelerated.execute("ReadProbe", "main", "()I", limits, false);
                match (actual, expected) {
                    (Ok(actual), Ok(expected)) => {
                        assert_eq!(actual.value, Some(Value::Int(5050)));
                        assert_eq!(actual.value, expected.value);
                        assert_eq!(actual.instructions, expected.instructions);
                    }
                    (Err(actual), Err(expected)) => {
                        assert_eq!(actual.to_string(), expected.to_string())
                    }
                    (actual, expected) => panic!("tree mismatch: {actual:?} vs {expected:?}"),
                }
            }
        }
    }
}

#[test]
fn readonly_memo_bounds_long_loops_and_rechecks_observed_reads() {
    let mut code = vec![0x00; 16];
    code.extend([
        0x09, 0x41, 0x03, 0x36, 4, 0x15, 4, 0x1b, 0xa2, 0, 16, 0x20, 0x2a, 0x03, 0x2e, 0x85, 0x61,
        0x41, 0x84, 4, 1, 0xa7, 0xff, 0xf0, 0x20, 0x88, 0xac,
    ]);
    let mut leaf = runtime_method("LoopProbe", "read", "([II)I", &code, 4, 5, vec![None], true);
    leaf.stack_key_id = Some(0);
    leaf.readonly_call_tree =
        crate::machine::interpreter_batch::is_readonly_call_tree(&leaf.instructions);
    assert!(leaf.readonly_call_tree);
    assert!(!crate::machine::interpreter_batch::is_readonly_leaf(
        &leaf.instructions
    ));
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    let args = [Value::Reference(Some(array)), Value::Int(7)];
    for (stored, hits) in [(3, 0), (3, 1), (-4, 1), (-4, 2)] {
        machine
            .heap
            .managed
            .array_set_typed(array, 0, heap::ArrayAccessKind::Int, HeapValue::Int(stored))
            .unwrap();
        let before = machine.execution.instructions;
        assert!(matches!(machine.try_readonly_leaf(&leaf, &args, 1),
            Some(CallOutcome::Return(Some(Value::Int(value)))) if value == stored * 7));
        assert_eq!(machine.execution.instructions - before, 110);
        assert_eq!(machine.execution.leaf_cache.hits, hits);
    }
    let before = machine.execution.instructions;
    assert!(
        machine
            .try_readonly_leaf(&leaf, &[args[0], Value::Int(i32::MAX)], 1)
            .is_none()
    );
    assert_eq!(machine.execution.instructions, before);
    assert!(
        machine
            .try_readonly_leaf(&leaf, &[Value::Reference(None), args[1]], 1)
            .is_none()
    );
    assert_eq!(machine.execution.instructions, before);
    for budget in 0..110 {
        machine.execution.instructions = 1023 - budget;
        assert!(machine.try_readonly_leaf(&leaf, &args, 1).is_none());
        assert_eq!(machine.execution.instructions, 1023 - budget);
    }
    machine.execution.instructions = 0;
    assert!(matches!(
        machine.try_readonly_leaf(&leaf, &args, 1),
        Some(CallOutcome::Return(Some(Value::Int(-28))))
    ));
    assert_eq!(machine.execution.instructions, 110);
    assert_eq!(
        machine
            .heap
            .managed
            .array_get_typed(array, 0, heap::ArrayAccessKind::Int)
            .unwrap(),
        HeapValue::Int(-4)
    );
}

#[test]
fn readonly_loop_memo_keeps_prepared_integer_execution_first() {
    use crate::acceleration::{
        CompiledIntegerMethod, IntegerMethodCompiler, IntegerMethodResult, IntegerMethodSpec,
    };
    #[derive(Debug)]
    struct Compiled(std::rc::Rc<std::cell::Cell<u32>>);
    impl CompiledIntegerMethod for Compiled {
        fn execute(&self, arguments: &[i32], budget: u32) -> Option<IntegerMethodResult> {
            assert!(arguments.is_empty());
            self.0.set(self.0.get() + 1);
            (budget >= 30).then_some(IntegerMethodResult {
                value: 7,
                instructions: 30,
            })
        }
    }
    struct Compiler(std::rc::Rc<std::cell::Cell<u32>>);
    impl IntegerMethodCompiler for Compiler {
        fn compile(
            &mut self,
            methods: &[IntegerMethodSpec],
            _: &dyn Fn() -> bool,
        ) -> Vec<Option<std::rc::Rc<dyn CompiledIntegerMethod>>> {
            assert_eq!(methods.len(), 1);
            vec![Some(std::rc::Rc::new(Compiled(self.0.clone())))]
        }
    }
    let mut code = vec![0; 16];
    code.extend([
        0x05, 0x3b, 0x1a, 0x99, 0, 9, 0x84, 0, 0xff, 0xa7, 0xff, 0xf9, 0x10, 7, 0xac,
    ]);
    let mut leaf = method(&code, 1, 1);
    leaf.stack_key_id = Some(0);
    leaf.readonly_call_tree =
        crate::machine::interpreter_batch::is_readonly_call_tree(&leaf.instructions);
    assert!(leaf.readonly_call_tree);
    let mut program = Program::new();
    program.methods.insert(leaf.key.clone(), leaf.clone());
    let ordinary = program
        .execute("T", "main", "()I", Limits::default(), true)
        .unwrap();
    assert_eq!(
        (ordinary.value, ordinary.instructions),
        (Some(Value::Int(7)), 30)
    );
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    assert_eq!(
        program.prepare_integer_methods(&mut Compiler(calls.clone()), &|| false),
        1
    );
    leaf = program.methods.get(&leaf.key).unwrap().clone();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    for expected_calls in 1..=2 {
        assert!(matches!(
            machine.try_readonly_leaf(&leaf, &[], 1),
            Some(CallOutcome::Return(Some(Value::Int(7))))
        ));
        assert_eq!(calls.get(), expected_calls);
        assert_eq!(
            machine.execution.instructions,
            u64::from(expected_calls) * 30
        );
    }
    assert_eq!(machine.execution.leaf_cache.hits, 0);
}

#[test]
fn readonly_float_returns_and_wide_fallback_preserve_bits_and_stack_limits() {
    let program = Program::new();
    for bits in [
        0,
        1,
        0x8000_0000,
        0x7f80_0000,
        0x7fc0_0042,
        0x8000_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0x7ff8_0000_0000_0042,
        u64::MAX,
    ] {
        for (descriptor, code, argument) in [
            ("(J)J", [0x1e, 0xad], Value::Long(bits as i64)),
            ("(D)D", [0x26, 0xaf], Value::Double(f64::from_bits(bits))),
            (
                "(F)F",
                [0x22, 0xae],
                Value::Float(f32::from_bits(bits as u32)),
            ),
        ] {
            let mut method = runtime_method(
                "Leaf",
                "identity",
                descriptor,
                &code,
                argument.slots(),
                argument.slots(),
                vec![None],
                true,
            );
            method.readonly_leaf =
                crate::machine::interpreter_batch::is_readonly_leaf(&method.instructions);
            assert!(method.readonly_leaf);
            let value_bits = |value| match value {
                Value::Long(value) => value as u64,
                Value::Double(value) => value.to_bits(),
                Value::Float(value) => u64::from(value.to_bits()),
                _ => panic!("unexpected return category"),
            };
            for available in [0, 1, 2, 3] {
                let mut host = DefaultNativeContext;
                let mut machine = program.machine(
                    Limits {
                        max_stack_slots: 4,
                        ..Limits::default()
                    },
                    false,
                    &mut host,
                );
                machine.execution.stack_slots = 4 - available;
                let fast = machine.try_readonly_leaf(&method, &[argument], 1);
                if available < argument.slots() {
                    assert!(fast.is_none());
                    assert_eq!(machine.execution.instructions, 0);
                } else {
                    let outcome = if argument.slots() == 1 {
                        fast.expect("float identity must use the readonly path")
                    } else {
                        // A batch starting with an empty stack currently yields
                        // before pushing a two-slot value. This fallback must
                        // leave accounting intact and preserve the exact bits.
                        assert!(fast.is_none());
                        assert_eq!(machine.execution.instructions, 0);
                        machine.call(&method, [argument], 1).unwrap()
                    };
                    let CallOutcome::Return(Some(value)) = outcome else {
                        panic!("expected numeric return");
                    };
                    assert_eq!(value_bits(value), value_bits(argument));
                    assert_eq!(machine.execution.instructions, 2);
                }
                assert_eq!(machine.execution.stack_slots, 4 - available);
            }
        }
    }
}

#[test]
fn readonly_leaf_returns_references_floats_and_void_without_losing_limits() {
    for (descriptor, code, args, expected) in [
        (
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[0x2a, 0xb0][..],
            vec![Value::Reference(None)],
            Some(Value::Reference(None)),
        ),
        (
            "(F)F",
            &[0x22, 0xae][..],
            vec![Value::Float(1.25)],
            Some(Value::Float(1.25)),
        ),
        ("()V", &[0xb1][..], vec![], None),
    ] {
        let mut method = runtime_method(
            "Leaf",
            "read",
            descriptor,
            code,
            1,
            args.len(),
            vec![None],
            true,
        );
        method.readonly_leaf =
            crate::machine::interpreter_batch::is_readonly_leaf(&method.instructions);
        assert!(method.readonly_leaf);
        let program = Program::new();
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        assert!(matches!(machine.try_readonly_leaf(&method, &args, 1),
            Some(CallOutcome::Return(value)) if value == expected));
        assert_eq!(machine.execution.instructions, code.len() as u64);
        machine.execution.instructions = 1023;
        assert!(machine.try_readonly_leaf(&method, &args, 1).is_none());
        machine.execution.instructions = 0;
        machine.execution.tracing = true;
        assert!(machine.try_readonly_leaf(&method, &args, 1).is_none());
    }
}

#[test]
fn readonly_straight_leaf_memo_retains_live_reads_and_instruction_accounting() {
    let mut code = vec![0; 16];
    code.extend([0x2a, 0x03, 0x2e, 0xac]);
    let mut method = runtime_method("LinearRead", "read", "([I)I", &code, 2, 1, vec![None], true);
    method.readonly_leaf =
        crate::machine::interpreter_batch::is_readonly_leaf(&method.instructions);
    method.stack_key_id = Some(0);
    assert!(method.readonly_leaf);
    assert!(!method.readonly_call_tree);
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    for (value, hits) in [(0, 0), (0, 1), (7, 1), (7, 2)] {
        machine
            .heap
            .managed
            .array_set(array, 0, HeapValue::Int(value))
            .unwrap();
        let before = machine.execution.instructions;
        assert!(
            matches!(machine.try_readonly_leaf(&method, &[Value::Reference(Some(array))], 1),
            Some(CallOutcome::Return(Some(Value::Int(actual)))) if actual == value)
        );
        assert_eq!(machine.execution.instructions - before, 20);
        assert_eq!(machine.execution.leaf_cache.hits, hits);
    }
    machine.execution.instructions = 1020;
    assert!(
        machine
            .try_readonly_leaf(&method, &[Value::Reference(Some(array))], 1)
            .is_none()
    );
    assert_eq!(machine.execution.instructions, 1020);
    assert!(
        machine
            .try_readonly_leaf(&method, &[Value::Reference(None)], 1)
            .is_none()
    );
}
