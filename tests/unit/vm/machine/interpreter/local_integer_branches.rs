use super::*;

fn taken(opcode: u8, left: i32, right: i32) -> bool {
    match opcode {
        0x9f => left == right,
        0xa0 => left != right,
        0xa1 => left < right,
        0xa2 => left >= right,
        0xa3 => left > right,
        0xa4 => left <= right,
        _ => unreachable!(),
    }
}

#[test]
fn local_integer_branches_preserve_operands_targets_and_partial_budgets() {
    for load in [&[0x1b][..], &[0x15, 1][..]] {
        for opcode in 0x9f..=0xa4 {
            let mut code = load.to_vec();
            let branch_pc = code.len();
            code.extend([opcode, 0, 4, 0x00, 0xb1]);
            let method = runtime_method("Compare", "run", "()V", &code, 5, 2, vec![None], true);
            for left in [i32::MIN, -1, 0, 1, i32::MAX] {
                for right in [i32::MIN, -1, 0, 1, i32::MAX] {
                    for wide in [false, true] {
                        let prefix = if wide { vec![Value::Long(17)] } else { vec![] };
                        let prefix_slots = if wide { 2 } else { 0 };
                        let mut heap = Heap::new(4096);
                        let mut classes = ClassState::default();
                        let mut locals = [None, Some(Value::Int(right))];
                        let mut stack = prefix.clone();
                        stack.push(Value::Int(left));
                        let result = crate::machine::interpreter_batch::execute(
                            &method,
                            &mut locals,
                            &mut stack,
                            0,
                            prefix_slots + 1,
                            prefix_slots + 2,
                            2,
                            &mut heap,
                            &mut classes,
                        );
                        let target = branch_pc + if taken(opcode, left, right) { 4 } else { 3 };
                        assert_eq!(
                            (result.pc, result.last_pc, result.instructions, result.slots),
                            (target, branch_pc, 2, prefix_slots)
                        );
                        assert_eq!(stack, prefix);

                        // A target can enter the original comparison directly.
                        stack.extend([Value::Int(left), Value::Int(right)]);
                        let result = crate::machine::interpreter_batch::execute(
                            &method,
                            &mut locals,
                            &mut stack,
                            branch_pc,
                            prefix_slots + 2,
                            prefix_slots + 2,
                            1,
                            &mut heap,
                            &mut classes,
                        );
                        assert_eq!(
                            (result.pc, result.last_pc, result.instructions, result.slots),
                            (target, branch_pc, 1, prefix_slots)
                        );
                        assert_eq!(stack, prefix);

                        // A short batch may stop before the pair or after its
                        // first opcode. Both must retain the original state.
                        for budget in [0, 1] {
                            stack = prefix.clone();
                            stack.push(Value::Int(left));
                            let result = crate::machine::interpreter_batch::execute(
                                &method,
                                &mut locals,
                                &mut stack,
                                0,
                                prefix_slots + 1,
                                prefix_slots + 2,
                                budget,
                                &mut heap,
                                &mut classes,
                            );
                            assert!(result.instructions <= budget);
                            assert_eq!(
                                result.slots,
                                prefix_slots + 1 + result.instructions as usize
                            );
                            assert_eq!(
                                result.pc,
                                if result.instructions == 0 {
                                    0
                                } else {
                                    branch_pc
                                }
                            );
                            if result.instructions == 1 {
                                assert_eq!(stack.pop(), Some(Value::Int(right)));
                            }
                            assert_eq!(stack.pop(), Some(Value::Int(left)));
                            assert_eq!(stack, prefix);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn local_integer_branches_keep_checked_errors_and_poll_boundaries() {
    for opcode in 0x9f..=0xa4 {
        for padding in [0, 1_020, 1_021, 1_022, 1_023, 1_024] {
            for suffix in [
                vec![0x04, 0x3c, 0x03, 0x1b, opcode, 0, 5, 0x04, 0xac, 0x05, 0xac],
                vec![0x0b, 0x44, 0x03, 0x1b, opcode, 0, 5, 0x04, 0xac, 0x05, 0xac],
                vec![0x03, 0x1b, opcode, 0, 5, 0x04, 0xac, 0x05, 0xac],
                vec![0x04, 0x3c, 0x01, 0x1b, opcode, 0, 5, 0x04, 0xac, 0x05, 0xac],
                // Taken malformed branch; an untaken branch still falls through.
                vec![0x04, 0x3c, 0x03, 0x1b, opcode, 0, 1, 0x04, 0xac],
                // Backward comparisons keep looping under the same budget.
                vec![0x04, 0x3c, 0x03, 0x1b, opcode, 0xff, 0xfe, 0x04, 0xac],
            ] {
                let mut code = vec![0; padding];
                code.extend(suffix);
                let method = method(&code, 2, 2);
                let mut program = Program::new();
                program.methods.insert(method.key.clone(), method);
                for limit in [1, 1_023, 1_024, 1_025, 1_030, 2_000] {
                    for stack_limit in [1, 2] {
                        let limits = Limits {
                            max_instructions: limit,
                            max_stack_slots: stack_limit,
                            ..Limits::default()
                        };
                        let ordinary = program.execute("T", "main", "()I", limits.clone(), true);
                        let batched = program.execute("T", "main", "()I", limits, false);
                        match (ordinary, batched) {
                            (Ok(a), Ok(b)) => {
                                assert_eq!(a.value, b.value);
                                assert_eq!(a.instructions, b.instructions);
                            }
                            (Err(a), Err(b)) => {
                                assert_eq!(a.code(), b.code());
                                assert_eq!(a.message(), b.message());
                            }
                            (a, b) => panic!("branch mismatch: {a:?} / {b:?}"),
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn short_branches_above_classfile_pc_range_keep_checked_execution() {
    // Public Rust fixtures can construct a method larger than a parsed JVM
    // Code attribute. Compact batch targets must retain its ordinary path.
    for opcode in [0x99, 0x9a] {
        let mut code = [0x10, 0, 0x57].repeat(22_000);
        code.extend([0x03, opcode, 0, 5, 0x04, 0xac, 0x05, 0xac]);
        let method = method(&code, 1, 0);
        let mut program = Program::new();
        program.methods.insert(method.key.clone(), method);
        for limit in [44_000, 44_001, 44_002, 44_003, 44_004, 44_005] {
            let execute = |profile_methods| {
                program.execute(
                    "T",
                    "main",
                    "()I",
                    Limits {
                        max_instructions: limit,
                        profile_methods,
                        ..Limits::default()
                    },
                    false,
                )
            };
            match (execute(true), execute(false)) {
                (Ok(a), Ok(b)) => {
                    let expected = Some(Value::Int(if opcode == 0x99 { 2 } else { 1 }));
                    assert_eq!((a.value, a.instructions), (expected, 44_004));
                    assert_eq!((b.value, b.instructions), (expected, 44_004));
                }
                (Err(a), Err(b)) => {
                    assert_eq!(a.code(), b.code());
                    assert_eq!(a.message(), b.message());
                }
                (a, b) => panic!("large-PC branch mismatch: {a:?} / {b:?}"),
            }
        }
    }
}
