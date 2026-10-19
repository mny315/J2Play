use super::*;

fn compare_store(code: &[u8], left: i32, right: i32, locals: usize, instruction_limit: u64) {
    let method = runtime_method(
        "LocalStores",
        "apply",
        "()I",
        code,
        2,
        locals,
        vec![
            None,
            Some(Constant::Integer(right)),
            Some(Constant::Integer(left)),
        ],
        true,
    );
    let mut program = Program::new();
    program.methods.insert(method.key.clone(), method);
    for stack_limit in [1, 2] {
        let limits = Limits {
            max_instructions: instruction_limit,
            max_stack_slots: stack_limit,
            ..Limits::default()
        };
        let expected = program.execute("LocalStores", "apply", "()I", limits.clone(), true);
        let actual = program.execute("LocalStores", "apply", "()I", limits, false);
        match (actual, expected) {
            (Ok(actual), Ok(expected)) => {
                assert_eq!(actual.value, expected.value);
                assert_eq!(actual.instructions, expected.instructions);
            }
            (Err(actual), Err(expected)) => assert_eq!(actual.to_string(), expected.to_string()),
            (actual, expected) => panic!(
                "local store mismatch: code={code:?} values={left},{right} limit={instruction_limit} slots={stack_limit}: {actual:?} / {expected:?}"
            ),
        }
    }
}

#[test]
fn local_integer_stores_preserve_arithmetic_aliasing_and_limits() {
    for opcode in [0x60, 0x64, 0x68, 0x78, 0x7a, 0x7c, 0x7e, 0x80, 0x82] {
        for right in [-1, 0, 33, i32::MIN] {
            for left in [i32::MIN, i32::MAX, -17, 0x5555_5555] {
                // Both compact and indexed stores, including replacement of
                // the source local and a destination beyond max_locals.
                for (store, load, locals) in [
                    (vec![0x3c], vec![0x1b], 3),
                    (vec![0x36, 2], vec![0x15, 2], 3),
                    (vec![0x3d], vec![0x1c], 2),
                ] {
                    for left_load in [vec![0x12, 2], vec![0x12, 2, 0x3b, 0x1a]] {
                        let mut code = vec![0x12, 1, 0x3c];
                        code.extend(left_load);
                        code.extend([0x1b, opcode]);
                        code.extend_from_slice(&store);
                        code.extend_from_slice(&load);
                        code.push(0xac);
                        for limit in [4, 5, 6, 7, 8, 9, 10, 11, 1024] {
                            compare_store(&code, left, right, locals, limit);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn local_integer_stores_preserve_poll_boundaries_and_operand_errors() {
    for padding in [1016, 1017, 1018, 1019, 1020, 1021, 1022, 1023] {
        for prefix in [0x04, 0x01, 0x0c, 0x0a] {
            let mut code = vec![0x00; padding];
            code.extend([0x12, 1, 0x3c, prefix, 0x1b, 0x60, 0x3d, 0x1c, 0xac]);
            for limit in [1022, 1023, 1024, 1025, 1026, 1032] {
                compare_store(&code, 7, 11, 3, limit);
            }
            let mut code = vec![0x00; padding];
            code.extend([
                0x12, 1, 0x3c, prefix, 0x3b, 0x1a, 0x1b, 0x60, 0x3d, 0x1c, 0xac,
            ]);
            for limit in [1022, 1023, 1024, 1025, 1026, 1032] {
                compare_store(&code, 7, 11, 3, limit);
            }
        }
    }
}

#[test]
fn local_integer_stores_keep_direct_entries_and_wide_local_invalidation() {
    let method = runtime_method(
        "LocalStores",
        "apply",
        "()I",
        &[0x1a, 0x60, 0x3d, 0x1c, 0xac],
        5,
        3,
        vec![None],
        true,
    );
    for (pc, operands, budget, next_pc, count) in [
        (0, vec![Value::Int(5)], 3, 3, 3),
        (1, vec![Value::Int(5), Value::Int(7)], 2, 3, 2),
        (2, vec![Value::Int(12)], 1, 3, 1),
    ] {
        for wide_prefix in [false, true] {
            let mut stack = if wide_prefix {
                vec![Value::Long(19)]
            } else {
                Vec::new()
            };
            stack.extend(operands.iter().copied());
            let mut locals = [Some(Value::Int(7)), Some(Value::Long(31)), None];
            let mut heap = Heap::new(4096);
            let mut classes = ClassState::default();
            let slots = slot_count(&stack);
            let result = crate::machine::interpreter_batch::execute(
                &method,
                &mut locals,
                &mut stack,
                pc,
                slots,
                5,
                budget,
                &mut heap,
                &mut classes,
            );
            assert_eq!(
                (result.pc, result.last_pc, result.instructions),
                (next_pc, 2, count)
            );
            assert_eq!(locals, [Some(Value::Int(7)), None, Some(Value::Int(12))]);
            assert_eq!(
                stack,
                if wide_prefix {
                    vec![Value::Long(19)]
                } else {
                    Vec::new()
                }
            );
            assert_eq!(result.slots, slot_count(&stack));
        }
    }
}

#[test]
#[cfg(target_arch = "x86_64")]
fn local_integer_updates_preserve_stack_prefixes_and_wide_local_invalidation() {
    let method = runtime_method(
        "LocalStores",
        "apply",
        "()I",
        &[0x1a, 0x1b, 0x60, 0x3e, 0x1d, 0xac],
        5,
        4,
        vec![None],
        true,
    );
    for wide_prefix in [false, true] {
        for budget in 0..=4 {
            let mut stack = if wide_prefix {
                vec![Value::Long(19)]
            } else {
                Vec::new()
            };
            let before = stack.clone();
            let initial_locals = [
                Some(Value::Int(5)),
                Some(Value::Int(7)),
                Some(Value::Long(31)),
                None,
            ];
            let mut locals = initial_locals;
            let mut heap = Heap::new(4096);
            let mut classes = ClassState::default();
            let slots = slot_count(&stack);
            let result = crate::machine::interpreter_batch::execute(
                &method,
                &mut locals,
                &mut stack,
                0,
                slots,
                5,
                budget,
                &mut heap,
                &mut classes,
            );
            assert_eq!(stack, before);
            if budget == 4 {
                assert_eq!((result.pc, result.last_pc, result.instructions), (4, 3, 4));
                assert_eq!(
                    locals,
                    [
                        Some(Value::Int(5)),
                        Some(Value::Int(7)),
                        None,
                        Some(Value::Int(12))
                    ]
                );
            } else {
                assert_eq!(result.instructions, 0);
                assert_eq!(locals, initial_locals);
            }
            assert_eq!(result.slots, slots);
        }
    }
}
