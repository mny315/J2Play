use super::*;

fn reference_index_result(
    tracing: bool,
    budget: u64,
    stack_limit: usize,
    padding: usize,
    layout: (bool, bool),
    index: i32,
    null: bool,
) -> (Result<Option<Value>, String>, u64) {
    let (wide, explicit) = layout;
    let mut code = vec![0; padding];
    if wide {
        code.push(0x0a);
    }
    if explicit {
        code.extend([0x19, 0, 0x15, 1]);
    } else {
        code.extend([0x2a, 0x1b]);
    }
    code.push(0x2e);
    if wide {
        code.extend([0x57, 0xad]);
    } else {
        code.push(0xac);
    }
    let method = runtime_method(
        "ReferenceIndex",
        "read",
        if wide { "([II)J" } else { "([II)I" },
        &code,
        if wide { 4 } else { 2 },
        2,
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
    machine
        .heap
        .managed
        .array_set(array, 0, Value::Int(13))
        .unwrap();
    let args = [
        Value::Reference((!null).then_some(array)),
        Value::Int(index),
    ];
    let result = match machine.call(&method, args, 1) {
        Ok(CallOutcome::Return(value)) => Ok(value),
        Err(error) => Err(error.to_string()),
        _ => panic!("unexpected throw/suspension in bounded reference/index fixture"),
    };
    assert!(machine.execution.call_stack.is_empty());
    assert_eq!(machine.execution.frame_slots, 0);
    assert_eq!(machine.execution.stack_slots, 0);
    (result, machine.execution.instructions)
}

#[test]
fn reference_index_loads_preserve_values_errors_and_poll_boundaries() {
    for padding in [0, 1021, 1023] {
        for (wide, explicit) in [(false, false), (false, true), (true, false), (true, true)] {
            for index in [-1, 0, 1] {
                for null in [false, true] {
                    for stack_limit in [1, 2, 3, 4] {
                        for budget in (0..=8).chain(1022..=1030) {
                            let expected = reference_index_result(
                                true,
                                budget,
                                stack_limit,
                                padding,
                                (wide, explicit),
                                index,
                                null,
                            );
                            let actual = reference_index_result(
                                false,
                                budget,
                                stack_limit,
                                padding,
                                (wide, explicit),
                                index,
                                null,
                            );
                            assert_eq!(
                                actual, expected,
                                "padding={padding} wide={wide} explicit={explicit} index={index} null={null} slots={stack_limit} budget={budget}"
                            );
                            if let Ok(value) = actual.0 {
                                assert_eq!(
                                    value,
                                    Some(if wide { Value::Long(1) } else { Value::Int(13) })
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn reference_index_loads_keep_individual_targets_and_invalid_locals() {
    let cases: &[&[u8]] = &[
        // The branch enters iload directly, with a reference already stacked.
        &[0x2a, 0xa7, 0, 4, 0x2a, 0x1b, 0x2e, 0xac],
        &[0x2b, 0x1b, 0x2e, 0xac],
        &[0x2a, 0x1a, 0x2e, 0xac],
        &[0x2d, 0x1b, 0x2e, 0xac],
        &[0x2a, 0x1d, 0x2e, 0xac],
        &[0x19, 255, 0x1b, 0x2e, 0xac],
        &[0x2a, 0x15, 255, 0x2e, 0xac],
    ];
    for (case, suffix) in cases.iter().enumerate() {
        for padding in [0, 1022] {
            let mut code = vec![0; padding];
            code.extend_from_slice(suffix);
            let method = runtime_method(
                "ReferenceIndexTargets",
                "read",
                "([II)I",
                &code,
                2,
                4,
                vec![None],
                true,
            );
            for budget in (0..=10).chain(1022..=1033) {
                let execute = |tracing| {
                    let program = Program::new();
                    let mut host = DefaultNativeContext;
                    let mut machine = program.machine(
                        Limits {
                            max_instructions: budget,
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
                    machine
                        .heap
                        .managed
                        .array_set(array, 0, Value::Int(13))
                        .unwrap();
                    let result = match machine.call(
                        &method,
                        [Value::Reference(Some(array)), Value::Int(0)],
                        1,
                    ) {
                        Ok(CallOutcome::Return(value)) => Ok(value),
                        Err(error) => Err(error.to_string()),
                        _ => panic!("unexpected throw/suspension in bounded local fixture"),
                    };
                    assert!(machine.execution.call_stack.is_empty());
                    assert_eq!(machine.execution.stack_slots, 0);
                    assert_eq!(machine.execution.frame_slots, 0);
                    (result, machine.execution.instructions)
                };
                let expected = execute(true);
                let actual = execute(false);
                assert_eq!(
                    actual, expected,
                    "case={case} padding={padding} budget={budget}"
                );
                if case == 0 && budget >= padding as u64 + 5 {
                    assert_eq!(actual, (Ok(Some(Value::Int(13))), padding as u64 + 5));
                }
            }
        }
    }
}
