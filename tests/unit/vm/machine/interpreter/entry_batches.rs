use super::*;

fn entry_result(
    tracing: bool,
    budget: u64,
    stack_limit: usize,
    padding: usize,
) -> (Result<Option<Value>, String>, Value, u64) {
    let mut code = vec![0x00; padding];
    // Increment the argument array, then force an allocation outside the
    // non-allocating batch. Resuming at the wrong PC repeats the visible write.
    code.extend([
        0x2a, 0x03, 0x5c, 0x2e, 0x04, 0x60, 0x4f, 0x04, 0xbc, 10, 0x57, 0x2a, 0x03, 0x2e, 0xac,
    ]);
    let method = runtime_method(
        "EntryBatches",
        "apply",
        "([I)I",
        &code,
        4,
        1,
        vec![None],
        true,
    );
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_instructions: budget,
            max_stack_slots: stack_limit,
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
    let result = match machine.call(&method, [Value::Reference(Some(array))], 1) {
        Ok(CallOutcome::Return(value)) => Ok(value),
        Err(error) => Err(error.to_string()),
        _ => panic!("unexpected throw/suspension in bounded entry fixture"),
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
fn fresh_entry_preserves_writes_when_continuing_at_allocations_and_limits() {
    for padding in [0, 1016, 1017, 1018, 1019, 1020, 1021, 1022, 1023] {
        for stack_limit in [1, 3, 4, 16] {
            for budget in (1..=18).chain([1022, 1023, 1024, 1025, 1026, 1032, 1040]) {
                let expected = entry_result(true, budget, stack_limit, padding);
                let actual = entry_result(false, budget, stack_limit, padding);
                assert_eq!(
                    actual, expected,
                    "padding={padding} slots={stack_limit} budget={budget}"
                );
                if let Ok(value) = actual.0 {
                    assert_eq!(value, Some(Value::Int(1)));
                    assert_eq!(actual.1, Value::Int(1));
                }
            }
        }
    }
}

#[test]
fn wide_and_reference_stacks_keep_values_at_growth_and_batch_boundaries() {
    for wide in [false, true] {
        for padding in [0, 990, 1023] {
            let mut code = vec![0x00; padding];
            if wide {
                code.extend([0x0a; 80]);
                code.extend([0x58; 79]);
                code.extend([0x88, 0xac]);
            } else {
                code.extend([0x04, 0xbc, 10]);
                code.extend([0x59; 79]);
                code.extend([0x04, 0xbc, 10, 0x57]);
                code.extend([0x57; 79]);
                code.extend([0xbe, 0xac]);
            }
            let method = runtime_method(
                "StackBoundaries",
                "run",
                "()I",
                &code,
                160,
                0,
                vec![None],
                true,
            );
            let mut program = Program::new();
            program.methods.insert(method.key.clone(), method);
            for max_stack_slots in [31, 32, 63, 64, 80, 81, 159, 160] {
                for max_instructions in [
                    30, 32, 33, 64, 65, 159, 160, 161, 165, 1023, 1024, 1025, 1200,
                ] {
                    let limits = Limits {
                        max_instructions,
                        max_stack_slots,
                        ..Limits::default()
                    };
                    let execute = |tracing| {
                        program
                            .execute("StackBoundaries", "run", "()I", limits.clone(), tracing)
                            .map(|result| (result.value, result.instructions))
                            .map_err(|error| error.to_string())
                    };
                    let expected = execute(true);
                    let actual = execute(false);
                    assert_eq!(
                        actual, expected,
                        "wide={wide} padding={padding} slots={max_stack_slots} budget={max_instructions}"
                    );
                    if let Ok((value, _)) = actual {
                        assert_eq!(value, Some(Value::Int(1)));
                    }
                }
            }
        }
    }
}
