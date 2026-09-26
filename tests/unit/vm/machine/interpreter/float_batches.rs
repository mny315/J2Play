use super::*;

#[test]
#[ignore = "manual release throughput measurement"]
fn numeric_array_batch_throughput() {
    for (kind, descriptor, array_type, load, store, multiply, add, one, ret, factor) in [
        (
            "int",
            "()I",
            10,
            0x2e,
            0x4f,
            0x68,
            0x60,
            0x04,
            0xac,
            Constant::Integer(2),
        ),
        (
            "float",
            "()F",
            6,
            0x30,
            0x51,
            0x6a,
            0x62,
            0x0c,
            0xae,
            Constant::Float(0.5_f32.to_bits()),
        ),
        (
            "double",
            "()D",
            7,
            0x31,
            0x52,
            0x6b,
            0x63,
            0x0f,
            0xaf,
            Constant::Double(0.5_f64.to_bits()),
        ),
    ] {
        for conversions in [false, true] {
            if kind == "int" && conversions {
                continue;
            }
            let mut code = vec![0x10, 64, 0xbc, array_type, 0x4b, 0x03, 0x3c];
            let loop_pc = code.len();
            code.extend([0x1b, 0x12, 1]);
            let exit_branch = code.len();
            code.extend([0xa2, 0, 0]);
            code.extend([0x2a, 0x1b, 0x10, 63, 0x7e, 0x5c, load]);
            if kind == "double" {
                code.extend([0x14, 0, 2]);
            } else {
                code.extend([0x12, 2]);
            }
            code.extend([multiply, one, add]);
            if conversions {
                code.extend(if kind == "float" {
                    [0x8b, 0x86]
                } else {
                    [0x8e, 0x87]
                });
            }
            code.extend([store, 0x84, 1, 1]);
            let back_branch = code.len();
            code.extend([0xa7, 0, 0]);
            let exit_pc = code.len();
            code.extend([0x2a, 0x03, load, ret]);
            code[exit_branch + 1..exit_branch + 3]
                .copy_from_slice(&((exit_pc - exit_branch) as i16).to_be_bytes());
            code[back_branch + 1..back_branch + 3]
                .copy_from_slice(&(loop_pc as i16 - back_branch as i16).to_be_bytes());
            let m = runtime_method(
                "NumericArrays",
                "main",
                descriptor,
                &code,
                6,
                2,
                vec![None, Some(Constant::Integer(262_144)), Some(factor.clone())],
                true,
            );
            let mut program = Program::new();
            program.methods.insert(m.key.clone(), m);
            let started = std::time::Instant::now();
            let result = program
                .execute(
                    "NumericArrays",
                    "main",
                    descriptor,
                    Limits {
                        max_instructions: 10_000_000,
                        ..Limits::default()
                    },
                    false,
                )
                .unwrap();
            eprintln!(
                "numeric-array kind={kind} conversions={conversions} elapsed={:?} instructions={} value={:?}",
                started.elapsed(),
                result.instructions,
                result.value
            );
        }
    }
}

fn assert_same_value(actual: Option<Value>, expected: Option<Value>) {
    match (actual, expected) {
        (Some(Value::Float(a)), Some(Value::Float(b))) => {
            assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()));
        }
        (Some(Value::Double(a)), Some(Value::Double(b))) => {
            assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()));
        }
        _ => assert_eq!(actual, expected),
    }
}

fn operation_program(
    opcode: u8,
    inputs: &[Value],
    padding: usize,
    limit: usize,
) -> (Program, &'static str) {
    let (descriptor, ret) = match opcode {
        0x62 | 0x66 | 0x6a | 0x6e | 0x72 | 0x76 | 0x86 | 0x89 | 0x90 => ("()F", 0xae),
        0x63 | 0x67 | 0x6b | 0x6f | 0x73 | 0x77 | 0x87 | 0x8a | 0x8d => ("()D", 0xaf),
        0x8c | 0x8f => ("()J", 0xad),
        _ => ("()I", 0xac),
    };
    let mut code = vec![0x00; padding];
    let mut constants = vec![None];
    for &input in inputs {
        let constant = match input {
            Value::Int(value) => Constant::Integer(value),
            Value::Long(value) => Constant::Long(value),
            Value::Float(value) => Constant::Float(value.to_bits()),
            Value::Double(value) => Constant::Double(value.to_bits()),
            Value::Reference(_) => {
                code.push(0x01);
                continue;
            }
        };
        let index = constants.len() as u8;
        constants.push(Some(constant));
        if input.slots() == 2 {
            code.extend([0x14, 0, index]);
        } else {
            code.extend([0x12, index]);
        }
    }
    code.extend([opcode, ret]);
    let m = runtime_method(
        "FloatBatches",
        "main",
        descriptor,
        &code,
        limit,
        0,
        constants,
        true,
    );
    let mut program = Program::new();
    program.methods.insert(m.key.clone(), m);
    (program, descriptor)
}

fn compare_operation(
    opcode: u8,
    inputs: &[Value],
    padding: usize,
    stack_limit: usize,
    instruction_limits: &[u64],
) {
    let (program, descriptor) = operation_program(opcode, inputs, padding, stack_limit);
    for &max_instructions in instruction_limits {
        let limits = Limits {
            max_instructions,
            ..Limits::default()
        };
        let ordinary = program.execute("FloatBatches", "main", descriptor, limits.clone(), true);
        let batched = program.execute("FloatBatches", "main", descriptor, limits, false);
        match (ordinary, batched) {
            (Ok(expected), Ok(actual)) => {
                assert_same_value(actual.value, expected.value);
                assert_eq!(actual.instructions, expected.instructions);
            }
            (Err(expected), Err(actual)) => assert_eq!(actual.to_string(), expected.to_string()),
            (expected, actual) => {
                panic!("opcode={opcode:x} inputs={inputs:?}: {actual:?} vs {expected:?}")
            }
        }
    }
}

#[test]
fn floating_batches_preserve_arithmetic_comparisons_and_special_values() {
    let values = [
        0.0,
        -0.0,
        1.0,
        -2.5,
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        f64::from(f32::from_bits(1)),
        f64::from(f32::MAX),
        f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::from_bits(0x7ff8_0000_0000_1234),
    ];
    for opcode in [
        0x62, 0x63, 0x66, 0x67, 0x6a, 0x6b, 0x6e, 0x6f, 0x72, 0x73, 0x95, 0x96, 0x97, 0x98,
    ] {
        for left in values {
            for right in values {
                let inputs = if matches!(opcode, 0x63 | 0x67 | 0x6b | 0x6f | 0x73 | 0x97 | 0x98) {
                    [Value::Double(left), Value::Double(right)]
                } else {
                    [Value::Float(left as f32), Value::Float(right as f32)]
                };
                compare_operation(opcode, &inputs, 0, 4, &[2, 3, 4, 8]);
            }
        }
    }
}

#[test]
fn floating_batches_preserve_conversions_negation_and_stack_bounds() {
    let values = [
        Value::Int(i32::MIN),
        Value::Int(i32::MAX),
        Value::Long(i64::MIN),
        Value::Long(i64::MAX),
        Value::Float(-0.0),
        Value::Float(f32::NAN),
        Value::Float(f32::INFINITY),
        Value::Float(-1.75),
        Value::Double(-0.0),
        Value::Double(f64::NAN),
        Value::Double(f64::NEG_INFINITY),
        Value::Double(1.0e30),
        Value::Reference(None),
    ];
    for opcode in [
        0x76, 0x77, 0x86, 0x87, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f, 0x90,
    ] {
        for value in values {
            for limit in [0, 1, 2, 4] {
                compare_operation(opcode, &[value], 0, limit, &[1, 2, 3, 8]);
            }
        }
    }
}

#[test]
fn floating_batches_preserve_malformed_operands_and_poll_boundaries() {
    for (opcode, valid) in [
        (0x62, Value::Float(1.0)),
        (0x63, Value::Double(1.0)),
        (0x95, Value::Float(f32::NAN)),
        (0x98, Value::Double(f64::NAN)),
    ] {
        for inputs in [
            vec![],
            vec![valid],
            vec![valid, Value::Int(3)],
            vec![Value::Reference(None), valid],
            vec![valid, valid],
        ] {
            for padding in [0, 1021, 1022, 1023, 1024] {
                compare_operation(opcode, &inputs, padding, 4, &[1023, 1024, 1025, 1030]);
            }
        }
    }
}

#[test]
fn floating_batches_commit_one_instruction_or_leave_the_stack_untouched() {
    for (opcode, input, expected) in [
        (
            0x62,
            vec![Value::Float(1.0), Value::Float(2.0)],
            Value::Float(3.0),
        ),
        (
            0x63,
            vec![Value::Double(1.0), Value::Double(2.0)],
            Value::Double(3.0),
        ),
        (
            0x95,
            vec![Value::Float(f32::NAN), Value::Float(2.0)],
            Value::Int(-1),
        ),
        (
            0x98,
            vec![Value::Double(f64::NAN), Value::Double(2.0)],
            Value::Int(1),
        ),
        (0x76, vec![Value::Float(0.0)], Value::Float(-0.0)),
        (0x77, vec![Value::Double(0.0)], Value::Double(-0.0)),
        (0x86, vec![Value::Int(3)], Value::Float(3.0)),
        (0x87, vec![Value::Int(3)], Value::Double(3.0)),
        (0x8e, vec![Value::Double(1.0e30)], Value::Int(i32::MAX)),
    ] {
        let m = method(&[opcode, 0xb1], 8, 0);
        let mut heap = Heap::new(4096);
        let mut classes = ClassState::default();
        // A wide prefix enters the general slot-counting batch and stays rooted.
        for prefix in [vec![], vec![Value::Long(7)]] {
            let mut original = prefix.clone();
            original.extend_from_slice(&input);
            let slots = slot_count(&original);
            for budget in [0, 1] {
                let mut stack = original.clone();
                let batch = crate::machine::interpreter_batch::execute(
                    &m,
                    &mut [],
                    &mut stack,
                    0,
                    slots,
                    8,
                    budget,
                    &mut heap,
                    &mut classes,
                );
                // The single-slot specialization deliberately yields before an
                // instruction introduces its first category-2 stack value.
                let processed = budget != 0 && (slots != original.len() || expected.slots() == 1);
                assert_eq!(
                    (batch.pc, batch.instructions),
                    if processed { (1, 1) } else { (0, 0) }
                );
                let expected_stack = if processed {
                    let mut values = prefix.clone();
                    values.push(expected);
                    values
                } else {
                    original.clone()
                };
                assert_eq!(batch.slots, slot_count(&expected_stack));
                assert_eq!(stack.len(), expected_stack.len());
                for (actual, expected) in stack.into_iter().zip(expected_stack) {
                    assert_same_value(Some(actual), Some(expected));
                }
            }
        }
    }
}
