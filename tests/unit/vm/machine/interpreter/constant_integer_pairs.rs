use super::*;

fn constant_forms(value: i32) -> Vec<(&'static str, Vec<u8>)> {
    let mut forms = vec![("pool", vec![0x12, 1]), ("wide-pool", vec![0x13, 1, 2])];
    if let Ok(value) = i16::try_from(value) {
        let [high, low] = value.to_be_bytes();
        forms.push(("short", vec![0x11, high, low]));
    }
    if let Ok(value) = i8::try_from(value) {
        forms.push(("byte", vec![0x10, value.cast_unsigned()]));
    }
    if (-1..=5).contains(&value) {
        forms.push(("small", vec![(value + 3) as u8]));
    }
    forms
}

fn pair_constants(left: i32, right: i32) -> Vec<Option<Constant>> {
    let mut constants = vec![None; 259];
    constants[1] = Some(Constant::Integer(right));
    constants[2] = Some(Constant::Integer(left));
    constants[3] = Some(Constant::Integer(262_144));
    constants[258] = Some(Constant::Integer(right));
    constants
}

#[test]
fn all_integer_constant_forms_preserve_values_slots_and_instruction_limits() {
    for right in [
        i32::MIN,
        -32769,
        -32768,
        -129,
        -1,
        3,
        128,
        32767,
        65535,
        i32::MAX,
    ] {
        for (form, load) in constant_forms(right) {
            for opcode in [0x60, 0x64, 0x68, 0x78, 0x7a, 0x7c, 0x7e, 0x80, 0x82] {
                for left in [i32::MIN, i32::MAX, 0x1234_5678] {
                    let mut code = vec![0x12, 2];
                    code.extend_from_slice(&load);
                    code.extend([opcode, 0xac]);
                    let method = runtime_method(
                        "ConstantPairs",
                        "apply",
                        "()I",
                        &code,
                        2,
                        0,
                        pair_constants(left, right),
                        true,
                    );
                    let mut program = Program::new();
                    program.methods.insert(method.key.clone(), method);
                    for instruction_limit in [1, 2, 3, 4, 1024] {
                        for stack_limit in [1, 2] {
                            let limits = Limits {
                                max_instructions: instruction_limit,
                                max_stack_slots: stack_limit,
                                ..Limits::default()
                            };
                            let reference = program.execute(
                                "ConstantPairs",
                                "apply",
                                "()I",
                                limits.clone(),
                                true,
                            );
                            let actual =
                                program.execute("ConstantPairs", "apply", "()I", limits, false);
                            match (actual, reference) {
                                (Ok(actual), Ok(reference)) => {
                                    assert_eq!(actual.value, reference.value);
                                    assert_eq!(actual.instructions, reference.instructions);
                                }
                                (Err(actual), Err(reference)) => {
                                    assert_eq!(actual.to_string(), reference.to_string())
                                }
                                (actual, reference) => panic!(
                                    "constant mismatch: {form} {left} {right} op={opcode:02x} limit={instruction_limit} slots={stack_limit}: {actual:?} / {reference:?}"
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
fn constant_pairs_preserve_malformed_pool_errors_and_poll_boundaries() {
    for padding in [0, 1021, 1022, 1023, 1024] {
        for load in [
            [0x12, 1].as_slice(),
            [0x13, 1, 2].as_slice(),
            [0x14, 0, 1].as_slice(),
            [0x13, 0xff, 0xff].as_slice(),
        ] {
            for constant in [
                None,
                Some(Constant::Integer(17)),
                Some(Constant::Float(1.0_f32.to_bits())),
                Some(Constant::Long(7)),
            ] {
                let mut constants = pair_constants(31, 17);
                constants[1] = constant.clone();
                constants[258] = constant;
                let mut code = vec![0x00; padding];
                code.push(0x04);
                code.extend_from_slice(load);
                code.extend([0x60, 0xac]);
                let method = runtime_method(
                    "ConstantPairs",
                    "apply",
                    "()I",
                    &code,
                    4,
                    0,
                    constants,
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
                        program.execute("ConstantPairs", "apply", "()I", limits.clone(), true);
                    let actual = program.execute("ConstantPairs", "apply", "()I", limits, false);
                    match (actual, reference) {
                        (Ok(actual), Ok(reference)) => {
                            assert_eq!(actual.value, reference.value);
                            assert_eq!(actual.instructions, reference.instructions);
                        }
                        (Err(actual), Err(reference)) => {
                            assert_eq!(actual.to_string(), reference.to_string())
                        }
                        (actual, reference) => panic!(
                            "pool mismatch: padding={padding} load={load:?} limit={instruction_limit}: {actual:?} / {reference:?}"
                        ),
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "manual integer constant form throughput measurement"]
fn constant_integer_pair_throughput() {
    for right in [3, 127, 32767, 65535] {
        for (form, load) in constant_forms(right) {
            for opcode in [0x60, 0x7e] {
                let mut code = vec![0x03, 0x3b, 0x12, 2, 0x3c];
                let loop_pc = code.len();
                code.extend([0x1a, 0x12, 3]);
                let exit_branch = code.len();
                code.extend([0xa2, 0, 0]);
                code.push(0x1b);
                code.extend_from_slice(&load);
                code.extend([opcode, 0x3c, 0x84, 0, 1]);
                let back_branch = code.len();
                code.extend([0xa7, 0, 0]);
                let exit_pc = code.len();
                code.extend([0x1b, 0xac]);
                code[exit_branch + 1..exit_branch + 3]
                    .copy_from_slice(&((exit_pc - exit_branch) as i16).to_be_bytes());
                code[back_branch + 1..back_branch + 3]
                    .copy_from_slice(&(loop_pc as i16 - back_branch as i16).to_be_bytes());
                let method = runtime_method(
                    "ConstantPairs",
                    "apply",
                    "()I",
                    &code,
                    2,
                    2,
                    pair_constants(0x1234_5678, right),
                    true,
                );
                let mut program = Program::new();
                program.methods.insert(method.key.clone(), method);
                let limits = Limits {
                    max_instructions: 10_000_000,
                    ..Limits::default()
                };
                let reference = program
                    .execute(
                        "ConstantPairs",
                        "apply",
                        "()I",
                        Limits {
                            profile_methods: true,
                            ..limits.clone()
                        },
                        false,
                    )
                    .unwrap();
                let started = std::time::Instant::now();
                let actual = program
                    .execute("ConstantPairs", "apply", "()I", limits, false)
                    .unwrap();
                let elapsed = started.elapsed();
                assert_eq!(actual.value, reference.value);
                assert_eq!(actual.instructions, reference.instructions);
                let Some(Value::Int(value)) = actual.value else {
                    panic!("integer loop result");
                };
                eprintln!(
                    "constant form={form} right={right} binary={opcode:02x} elapsed={elapsed:?} instructions={} checksum={:08x}",
                    actual.instructions,
                    value.cast_unsigned()
                );
            }
        }
    }
}
