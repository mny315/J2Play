use super::*;

#[test]
fn bounded_bytecode_batches_match_traced_execution_and_limits() {
    assert_eq!(std::mem::size_of::<RuntimeInstructionMeta>(), 16);
    let fixtures: &[(&[u8], usize, usize)] = &[
        // Count across several cancellation polling boundaries.
        (
            &[
                0x11, 0x08, 0x01, 0x3b, 0x03, 0x3c, 0x84, 0x01, 0x03, 0x84, 0x00, 0xff, 0x1a, 0x9d,
                0xff, 0xf9, 0x1b, 0xac,
            ],
            2,
            2,
        ),
        (
            &[0x02, 0x10, 0x21, 0x78, 0x10, 0x07, 0x82, 0x91, 0xac],
            2,
            0,
        ),
        (&[0x09, 0x3f, 0x04, 0x3c, 0x1e, 0x88, 0xac], 2, 2),
        (&[0x04, 0x03, 0x6c, 0xac], 2, 0),
        (&[0x04, 0x59, 0x60, 0xac], 1, 0),
        (&[0x04, 0x05, 0x5c, 0x60, 0x60, 0x60, 0xac], 4, 0),
        (&[0x04, 0x05, 0x5c, 0xac], 3, 0),
        (&[0x0a, 0x5c, 0x58, 0x88, 0xac], 4, 0),
        (&[0x04, 0x05, 0x5a, 0x64, 0x64, 0xac], 3, 0),
        (&[0x04, 0x05, 0x5f, 0x64, 0xac], 2, 0),
        (&[0x0a, 0x04, 0x5c, 0xac], 5, 0),
        (&[0x04, 0x05, 0x58, 0x06, 0xac], 2, 0),
        (&[0x02, 0x85, 0x75, 0x88, 0xac], 2, 0),
        (&[0x02, 0x85, 0x88, 0xac], 1, 0),
        (&[0x01, 0xc6, 0x00, 0x05, 0x03, 0xac, 0x04, 0xac], 1, 0),
        (
            &[
                0x05, 0xbc, 0x0a, 0x59, 0x03, 0x10, 0x63, 0x4f, 0x03, 0x2e, 0xac,
            ],
            4,
            0,
        ),
        (
            &[
                0x05, 0xbc, 0x08, 0x59, 0x04, 0x11, 0x01, 0xff, 0x54, 0x04, 0x33, 0xac,
            ],
            4,
            0,
        ),
        (
            &[
                0x05, 0xbc, 0x0b, 0x59, 0x03, 0x0a, 0x50, 0x03, 0x2f, 0x88, 0xac,
            ],
            5,
            0,
        ),
        (&[0x05, 0xbc, 0x0a, 0xbe, 0xac], 1, 0),
        (&[0x05, 0xbc, 0x0a, 0x4b, 0x2a, 0x03, 0x2e, 0xac], 2, 1),
        (&[0x05, 0xbc, 0x0a, 0x4b, 0x2a, 0x03, 0x2e, 0xac], 1, 1),
        (&[0x05, 0xbc, 0x0a, 0x4b, 0x2a, 0x05, 0x2e, 0xac], 2, 1),
        (&[0x05, 0xbc, 0x08, 0x4b, 0x2a, 0x03, 0x2e, 0xac], 2, 1),
        (&[0x05, 0xbc, 0x08, 0x4b, 0x2a, 0x03, 0x33, 0xac], 2, 1),
        (
            &[0x05, 0xbc, 0x0b, 0x4b, 0x2a, 0x03, 0x2f, 0x88, 0xac],
            2,
            1,
        ),
        (&[0x01, 0x4b, 0x2a, 0x03, 0x2e, 0xac], 2, 1),
        (
            &[0x05, 0xbc, 0x0a, 0x4b, 0x04, 0x2a, 0x03, 0x2e, 0x60, 0xac],
            3,
            1,
        ),
        (&[0x05, 0xbc, 0x0a, 0x02, 0x2e, 0xac], 2, 0),
        (&[0x05, 0xbc, 0x0a, 0x03, 0x0a, 0x50, 0x03, 0xac], 4, 0),
        (&[0x01, 0x03, 0x2e, 0xac], 2, 0),
        (&[0x1a, 0xac], 1, 0),
        (&[0xa7, 0x00, 0x01], 1, 0),
        // Invalid branch targets are observable only when the branch is taken.
        (&[0x03, 0x9a, 0xff, 0xf0, 0x04, 0xac], 1, 0),
        (&[0x04, 0x9a, 0xff, 0xf0, 0x04, 0xac], 1, 0),
        (&[0x04, 0x9a, 0x00, 0x01, 0x04, 0xac], 1, 0),
        (&[0xa7, 0x00, 0x00], 0, 0),
        (&[0xa7, 0x00, 0x03, 0x04, 0xac], 1, 0),
    ];
    for &(code, stack, locals) in fixtures {
        let m = method(code, stack, locals);
        let mut program = Program::new();
        program.methods.insert(m.key.clone(), m);
        for instruction_limit in [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 1_022, 1_023, 1_024, 1_025, 50_000,
        ] {
            let limits = Limits {
                max_instructions: instruction_limit,
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
                (a, b) => {
                    panic!("batch mismatch for {code:?}, limit={instruction_limit}: {a:?} / {b:?}")
                }
            }
        }
    }
}

#[test]
fn bounded_bytecode_batches_preserve_integer_arithmetic() {
    let mut seed = 123_u32;
    for opcode in [
        0x60, 0x64, 0x68, 0x6c, 0x70, 0x78, 0x7a, 0x7c, 0x7e, 0x80, 0x82,
    ] {
        for _ in 0..64 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let a = seed as i32;
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let b = seed as i32;
            let mut m = method(&[0x12, 0x01, 0x12, 0x02, opcode, 0xac], 2, 0);
            m.constants = Arc::new(vec![
                None,
                Some(Constant::Integer(a)),
                Some(Constant::Integer(b)),
            ]);
            let mut program = Program::new();
            program.methods.insert(m.key.clone(), m);
            let ordinary = program
                .execute("T", "main", "()I", Limits::default(), true)
                .unwrap();
            let batched = program
                .execute("T", "main", "()I", Limits::default(), false)
                .unwrap();
            assert_eq!(ordinary.value, batched.value);
            assert_eq!(ordinary.instructions, batched.instructions);
        }
    }
}

#[test]
fn bounded_bytecode_batches_reject_reserved_instruction_indices() {
    // The PC map reserves u16::MAX for invalid instruction boundaries, even
    // when a hand-built method has enough instructions to index that slot.
    let mut code = vec![0x00; usize::from(u16::MAX) + 3];
    code[..3].copy_from_slice(&[0xa7, 0x00, 0x01]);
    let m = method(&code, 0, 0);
    let batch = crate::machine::interpreter_batch::execute(
        &m,
        &mut [],
        &mut Vec::new(),
        1,
        0,
        0,
        100,
        &mut Heap::new(1024),
        &mut ClassState::default(),
    );
    assert_eq!(batch.instructions, 0);
    let mut program = Program::new();
    program.methods.insert(m.key.clone(), m);
    for tracing in [false, true] {
        let limits = Limits {
            max_instructions: 100,
            ..Limits::default()
        };
        let error = program
            .execute("T", "main", "()I", limits, tracing)
            .unwrap_err();
        assert_eq!(error.code(), "invalid-pc");
    }
}

#[test]
fn bounded_bytecode_batches_preserve_long_arithmetic() {
    for opcode in [
        0x61, 0x65, 0x69, 0x6d, 0x71, 0x7f, 0x81, 0x83, 0x94, 0x79, 0x7b, 0x7d,
    ] {
        for left in [i64::MIN, i64::MAX, -1, 0, 1, 0x1_0000_0001] {
            for right in [i64::MIN, i64::MAX, -1, 0, 1, 63, 64, 65] {
                let shift = matches!(opcode, 0x79 | 0x7b | 0x7d);
                let mut code = vec![0x14, 0, 1];
                if shift {
                    code.extend([0x12, 2]);
                } else {
                    code.extend([0x14, 0, 2]);
                }
                code.push(opcode);
                if opcode != 0x94 {
                    code.push(0x88);
                }
                code.push(0xac);
                let mut m = method(&code, 4, 0);
                m.constants = Arc::new(vec![
                    None,
                    Some(Constant::Long(left)),
                    Some(if shift {
                        Constant::Integer(right as i32)
                    } else {
                        Constant::Long(right)
                    }),
                ]);
                let mut program = Program::new();
                program.methods.insert(m.key.clone(), m);
                for max_instructions in [2, 3, 4, 100] {
                    let limits = Limits {
                        max_instructions,
                        ..Limits::default()
                    };
                    let expected = program.execute("T", "main", "()I", limits.clone(), true);
                    let actual = program.execute("T", "main", "()I", limits, false);
                    match (actual, expected) {
                        (Ok(actual), Ok(expected)) => {
                            assert_eq!(actual.value, expected.value);
                            assert_eq!(actual.instructions, expected.instructions);
                        }
                        (Err(actual), Err(expected)) => {
                            assert_eq!(actual.to_string(), expected.to_string())
                        }
                        (actual, expected) => {
                            panic!("long arithmetic mismatch: {actual:?} vs {expected:?}")
                        }
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn bytecode_batch_throughput() {
    // An integer-array update loop, with no native calls or wall-clock pacing.
    let mut code = vec![0x10, 64, 0xbc, 10, 0x4b, 0x03, 0x3c, 0x03, 0x3d];
    let loop_pc = code.len();
    code.extend([0x1c, 0x12, 1]);
    let exit_branch = code.len();
    code.extend([0xa2, 0, 0]);
    code.extend([
        0x2a, 0x1c, 0x10, 63, 0x7e, 0x5c, 0x2e, 0x04, 0x60, 0x4f, 0x1b, 0x1c, 0x82, 0x3c, 0x84, 2,
        1,
    ]);
    let back_branch = code.len();
    code.extend([0xa7, 0, 0]);
    let exit_pc = code.len();
    code.extend([0x1b, 0xac]);
    code[exit_branch + 1..exit_branch + 3]
        .copy_from_slice(&((exit_pc - exit_branch) as i16).to_be_bytes());
    code[back_branch + 1..back_branch + 3]
        .copy_from_slice(&((loop_pc as i16) - (back_branch as i16)).to_be_bytes());
    let mut m = method(&code, 5, 3);
    m.constants = Arc::new(vec![None, Some(Constant::Integer(1_000_000))]);
    let mut program = Program::new();
    program.methods.insert(m.key.clone(), m);
    let mut last = None;
    // Method sampling retains the ordinary dispatcher and incurs a small
    // extra sample every 1024 instructions. It is not full instruction tracing.
    for profile_methods in [true, false, true, false] {
        let started = std::time::Instant::now();
        let result = program
            .execute(
                "T",
                "main",
                "()I",
                Limits {
                    max_instructions: 30_000_000,
                    profile_methods,
                    ..Limits::default()
                },
                false,
            )
            .unwrap();
        let observed = (result.value, result.instructions);
        if let Some(last) = last {
            assert_eq!(last, observed);
        }
        last = Some(observed);
        eprintln!(
            "batched={} elapsed={:?} instructions={}",
            !profile_methods,
            started.elapsed(),
            result.instructions
        );
    }
}

#[test]
fn batch_returns_preserve_types_errors_and_poll_boundaries() {
    let fixtures: &[(&str, &[u8])] = &[
        ("()V", &[0xb1]),
        ("()I", &[0x04, 0xac]),
        ("()J", &[0x0a, 0xad]),
        ("()F", &[0x0c, 0xae]),
        ("()D", &[0x0f, 0xaf]),
        ("()Ljava/lang/Object;", &[0x01, 0xb0]),
        ("()[I", &[0x04, 0xbc, 0x0a, 0xb0]),
        ("()V", &[0x04, 0xac]),
        ("()I", &[0xb1]),
        ("()I", &[0xac]),
        ("()I", &[0x01, 0xac]),
        ("()J", &[0x04, 0xad]),
        ("()F", &[0x04, 0xae]),
        ("()D", &[0x0a, 0xaf]),
        ("()Ljava/lang/Object;", &[0x04, 0xb0]),
    ];
    for &(descriptor, suffix) in fixtures {
        for padding in [0, 1, 1_021, 1_022, 1_023, 1_024] {
            let mut code = vec![0x00; padding];
            code.extend_from_slice(suffix);
            // A completed return must never execute unreachable trailing code.
            code.extend_from_slice(&[0x03, 0x03, 0x6c, 0xac]);
            let m = runtime_method("R", "main", descriptor, &code, 4, 0, vec![None], true);
            let mut program = Program::new();
            program.methods.insert(m.key.clone(), m);
            for limit in [1, 2, 1_022, 1_023, 1_024, 1_025, 2_000] {
                let limits = Limits {
                    max_instructions: limit,
                    ..Limits::default()
                };
                let ordinary = program.execute("R", "main", descriptor, limits.clone(), true);
                let batched = program.execute("R", "main", descriptor, limits, false);
                match (ordinary, batched) {
                    (Ok(a), Ok(b)) => {
                        assert_eq!(
                            a.value, b.value,
                            "{descriptor}, padding={padding}, limit={limit}"
                        );
                        assert_eq!(a.instructions, b.instructions);
                    }
                    (Err(a), Err(b)) => {
                        assert_eq!(a.code(), b.code());
                        assert_eq!(a.message(), b.message());
                    }
                    (a, b) => panic!(
                        "return mismatch {descriptor}, padding={padding}, limit={limit}: {a:?} / {b:?}"
                    ),
                }
            }
        }
    }
}
