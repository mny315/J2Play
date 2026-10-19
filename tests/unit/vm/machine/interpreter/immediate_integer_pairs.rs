use super::*;

fn immediate(value: i8, compact: bool) -> Vec<u8> {
    if compact && (-1..=5).contains(&value) {
        vec![(i32::from(value) + 3) as u8]
    } else {
        vec![0x10, value as u8]
    }
}

#[test]
fn immediate_integer_pairs_preserve_values_and_intermediate_limits() {
    for opcode in [0x60, 0x64, 0x68, 0x78, 0x7a, 0x7c, 0x7e, 0x80, 0x82] {
        for right in [-128, -1, 0, 1, 5, 127] {
            for compact in [false, true] {
                for left in [i32::MIN, i32::MAX, -17, 0, 0x5555_5555] {
                    let mut code = vec![0x12, 1];
                    code.extend(immediate(right, compact));
                    code.extend([opcode, 0xac]);
                    let method = runtime_method(
                        "ImmediatePairs",
                        "apply",
                        "()I",
                        &code,
                        2,
                        0,
                        vec![None, Some(Constant::Integer(left))],
                        true,
                    );
                    let mut program = Program::new();
                    program.methods.insert(method.key.clone(), method);
                    for instructions in [1, 2, 3, 4, 1024] {
                        for stack_limit in [1, 2] {
                            let limits = Limits {
                                max_instructions: instructions,
                                max_stack_slots: stack_limit,
                                ..Limits::default()
                            };
                            let reference = program.execute(
                                "ImmediatePairs",
                                "apply",
                                "()I",
                                limits.clone(),
                                true,
                            );
                            let actual =
                                program.execute("ImmediatePairs", "apply", "()I", limits, false);
                            match (actual, reference) {
                                (Ok(actual), Ok(reference)) => {
                                    assert_eq!(actual.value, reference.value);
                                    assert_eq!(actual.instructions, reference.instructions);
                                }
                                (Err(actual), Err(reference)) => {
                                    assert_eq!(actual.to_string(), reference.to_string())
                                }
                                (actual, reference) => panic!(
                                    "pair mismatch: opcode={opcode:02x} left={left} right={right} compact={compact} instructions={instructions} slots={stack_limit}: {actual:?} / {reference:?}"
                                ),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn immediate_integer_pairs_keep_branch_entry_and_wide_stack_prefixes() {
    for compact in [false, true] {
        let mut code = immediate(5, compact);
        let binary_pc = code.len();
        code.extend([0x64, 0xac]);
        let method = runtime_method(
            "ImmediatePairs",
            "apply",
            "()I",
            &code,
            4,
            0,
            vec![None],
            true,
        );
        for prefix in [vec![], vec![Value::Long(7)]] {
            let mut heap = Heap::new(4096);
            let mut classes = ClassState::default();
            let mut stack = prefix.clone();
            stack.extend([Value::Int(11), Value::Int(5)]);
            let slots = slot_count(&stack);
            let result = crate::machine::interpreter_batch::execute(
                &method,
                &mut [],
                &mut stack,
                binary_pc,
                slots,
                slots,
                1,
                &mut heap,
                &mut classes,
            );
            assert_eq!(
                (result.pc, result.last_pc, result.instructions, result.slots),
                (binary_pc + 1, binary_pc, 1, slots - 1)
            );
            assert_eq!(stack.last(), Some(&Value::Int(6)));
            assert_eq!(&stack[..prefix.len()], prefix);

            let mut stack = prefix.clone();
            stack.push(Value::Int(11));
            let slots = slot_count(&stack);
            let result = crate::machine::interpreter_batch::execute(
                &method,
                &mut [],
                &mut stack,
                0,
                slots,
                slots + 1,
                2,
                &mut heap,
                &mut classes,
            );
            assert_eq!(
                (result.pc, result.last_pc, result.instructions, result.slots),
                (binary_pc + 1, binary_pc, 2, slots)
            );
            assert_eq!(stack.last(), Some(&Value::Int(6)));
            assert_eq!(&stack[..prefix.len()], prefix);
        }
    }
}

#[test]
fn immediate_integer_pairs_preserve_poll_boundaries_and_operand_errors() {
    for padding in [0, 1021, 1022, 1023, 1024] {
        for prefix in [vec![0x01], vec![0x0c], vec![0x0a], vec![0x04]] {
            let mut code = vec![0x00; padding];
            code.extend(prefix);
            code.extend([0x10, 7, 0x60, 0xac]);
            let method = runtime_method(
                "ImmediatePairs",
                "apply",
                "()I",
                &code,
                4,
                0,
                vec![None],
                true,
            );
            let mut program = Program::new();
            program.methods.insert(method.key.clone(), method);
            for instruction_limit in [1022, 1023, 1024, 1025, 1026, 2000] {
                let limits = Limits {
                    max_instructions: instruction_limit,
                    ..Limits::default()
                };
                let reference =
                    program.execute("ImmediatePairs", "apply", "()I", limits.clone(), true);
                let actual = program.execute("ImmediatePairs", "apply", "()I", limits, false);
                match (actual, reference) {
                    (Ok(actual), Ok(reference)) => {
                        assert_eq!(actual.value, reference.value);
                        assert_eq!(actual.instructions, reference.instructions);
                    }
                    (Err(actual), Err(reference)) => {
                        assert_eq!(actual.to_string(), reference.to_string())
                    }
                    (actual, reference) => panic!(
                        "poll/type mismatch: padding={padding} limit={instruction_limit}: {actual:?} / {reference:?}"
                    ),
                }
            }
        }
    }
}
