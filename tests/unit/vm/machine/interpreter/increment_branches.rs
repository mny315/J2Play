use super::*;

#[test]
fn increment_backedges_keep_every_budget_stack_prefix_and_last_pc() {
    for local in [0_u8, 3, 255] {
        for amount in [i8::MIN, -1, 0, 1, i8::MAX] {
            let method = runtime_method(
                "IncrementBranches",
                "run",
                "()V",
                &[0x84, local, amount.cast_unsigned(), 0xa7, 0xff, 0xfd],
                4,
                usize::from(local) + 1,
                vec![None],
                true,
            );
            for initial in [i32::MIN, -7, 0, i32::MAX] {
                for start in [0, 3] {
                    for budget in 0..10 {
                        for prefix in [vec![], vec![Value::Long(7), Value::Int(11)]] {
                            let mut heap = Heap::new(4096);
                            let mut classes = ClassState::default();
                            let mut locals = vec![Some(Value::Int(123)); usize::from(local) + 1];
                            locals[usize::from(local)] = Some(Value::Int(initial));
                            let mut stack = prefix.clone();
                            let slots = slot_count(&stack);
                            let result = crate::machine::interpreter_batch::execute(
                                &method,
                                &mut locals,
                                &mut stack,
                                start,
                                slots,
                                slots,
                                budget,
                                &mut heap,
                                &mut classes,
                            );
                            let mut pc = start;
                            let mut last_pc = start;
                            let mut expected = initial;
                            for _ in 0..budget {
                                last_pc = pc;
                                if pc == 0 {
                                    expected = expected.wrapping_add(i32::from(amount));
                                    pc = 3;
                                } else {
                                    pc = 0;
                                }
                            }
                            assert_eq!(
                                (result.pc, result.last_pc, result.instructions, result.slots),
                                (pc, last_pc, budget, slots)
                            );
                            assert_eq!(locals[usize::from(local)], Some(Value::Int(expected)));
                            for (index, value) in locals.iter().enumerate() {
                                if index != usize::from(local) {
                                    assert_eq!(*value, Some(Value::Int(123)));
                                }
                            }
                            assert_eq!(stack, prefix);
                        }
                    }
                }
            }
        }
    }
}

fn compare_increment_execution(program: &Program, limit: u64) {
    let limits = Limits {
        max_instructions: limit,
        ..Limits::default()
    };
    let actual = program.execute("IncrementBranches", "run", "()I", limits.clone(), false);
    let expected = program.execute("IncrementBranches", "run", "()I", limits, true);
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => {
            assert_eq!(actual.value, expected.value);
            assert_eq!(actual.instructions, expected.instructions);
        }
        (Err(actual), Err(expected)) => assert_eq!(actual.to_string(), expected.to_string()),
        (actual, expected) => panic!("increment limit={limit}: {actual:?} / {expected:?}"),
    }
}

#[test]
fn increment_forward_branches_preserve_errors_and_poll_boundaries() {
    for padding in [0, 1019, 1020, 1021, 1022, 1023] {
        for amount in [i8::MIN, 0, i8::MAX] {
            for initial in [i32::MIN, i32::MAX] {
                for prefix in [
                    vec![0x12, 1, 0x3b],
                    vec![],
                    vec![0x0b, 0x43],
                    vec![0x09, 0x3f],
                    vec![0x01, 0x4b],
                ] {
                    let mut code = prefix;
                    code.extend(std::iter::repeat_n(0x00, padding));
                    // The forward branch skips a null throw and resumes at iload_0.
                    code.extend([
                        0x84,
                        0,
                        amount.cast_unsigned(),
                        0xa7,
                        0,
                        5,
                        0x01,
                        0xbf,
                        0x1a,
                        0xac,
                    ]);
                    let method = runtime_method(
                        "IncrementBranches",
                        "run",
                        "()I",
                        &code,
                        2,
                        2,
                        vec![None, Some(Constant::Integer(initial))],
                        true,
                    );
                    let mut program = Program::new();
                    program.methods.insert(method.key.clone(), method);
                    for limit in 1..=9 {
                        compare_increment_execution(&program, padding as u64 + limit);
                    }
                }
            }
        }
    }
}

#[test]
fn increment_backedges_preserve_host_cancellation_location() {
    use std::cell::Cell;
    use std::rc::Rc;
    for padding in 0..4 {
        let mut code = vec![0x03, 0x3b];
        code.extend(std::iter::repeat_n(0, padding));
        code.extend([0x84, 0, 1, 0xa7, 0xff, 0xfd]);
        let method = runtime_method(
            "IncrementBranches",
            "run",
            "()I",
            &code,
            1,
            1,
            vec![None],
            true,
        );
        let mut program = Program::new();
        program.methods.insert(method.key.clone(), method);
        for point in [1, 2, 3, 5] {
            let run = |trace| {
                let checks = Rc::new(Cell::new(0));
                let mut context = CancellationContext {
                    checks: Rc::clone(&checks),
                    cancel_at: Rc::new(Cell::new(point)),
                };
                let error = program
                    .execute_with_context(
                        "IncrementBranches",
                        "run",
                        "()I",
                        Limits {
                            max_instructions: 10_000,
                            ..Limits::default()
                        },
                        trace,
                        &mut context,
                    )
                    .unwrap_err();
                assert_eq!(error.code(), "execution-cancelled");
                (error.to_string(), checks.get())
            };
            assert_eq!(run(false), run(true));
        }
    }
}

#[test]
fn increment_without_goto_or_with_invalid_local_keeps_the_ordinary_path() {
    let method = runtime_method(
        "IncrementBranches",
        "run",
        "()V",
        &[0x84, 0, 1, 0, 0xb1],
        0,
        1,
        vec![None],
        true,
    );
    for value in [
        None,
        Some(Value::Long(7)),
        Some(Value::Float(1.0)),
        Some(Value::Reference(None)),
        Some(Value::Int(i32::MAX)),
    ] {
        let mut heap = Heap::new(4096);
        let mut classes = ClassState::default();
        let mut locals = [value];
        let mut stack = vec![];
        let result = crate::machine::interpreter_batch::execute(
            &method,
            &mut locals,
            &mut stack,
            0,
            0,
            0,
            2,
            &mut heap,
            &mut classes,
        );
        if matches!(value, Some(Value::Int(_))) {
            assert_eq!((result.pc, result.last_pc, result.instructions), (4, 3, 2));
            assert_eq!(locals, [Some(Value::Int(i32::MIN))]);
        } else {
            assert_eq!((result.pc, result.last_pc, result.instructions), (0, 0, 0));
            assert_eq!(locals, [value]);
        }
        assert!(stack.is_empty());
    }
}
