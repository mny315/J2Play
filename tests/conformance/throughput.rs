//! Bounded performance controls using the public loader and the normal VM dependency.
use crate::support::guest::{MethodCode, Pool, code_method};
use classfile::{ClassFile, Constant};
use vm::{Limits, Program, Value};

const CLASS: &str = "fixtures/ArrayLoop";

#[derive(Clone, Copy)]
enum NumberKind {
    Int,
    Float,
    Double,
}

impl NumberKind {
    fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::Double => "double",
        }
    }

    fn instructions(self) -> ([u8; 7], &'static str, Constant) {
        match self {
            Self::Int => (
                [10, 0x2e, 0x4f, 0x68, 0x60, 0x04, 0xac],
                "()I",
                Constant::Integer(2),
            ),
            Self::Float => (
                [6, 0x30, 0x51, 0x6a, 0x62, 0x0c, 0xae],
                "()F",
                Constant::Float(0.5_f32.to_bits()),
            ),
            Self::Double => (
                [7, 0x31, 0x52, 0x6b, 0x63, 0x0f, 0xaf],
                "()D",
                Constant::Double(0.5_f64.to_bits()),
            ),
        }
    }
}

fn program(mut pool: Pool, code: Vec<u8>, descriptor: &str, stack: u16, locals: u16) -> Program {
    let this_class = pool.class(CLASS);
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let method = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 9,
            name: "run",
            descriptor,
            max_stack: stack,
            max_locals: locals,
            code,
        },
    );
    let class = ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x21,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![method],
        attributes: Vec::new(),
    };
    let mut program = Program::new();
    let limits = Limits::default();
    for bootstrap in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&bootstrap, &limits).unwrap();
    }
    program.add_class(&class, &limits).unwrap();
    program
}

fn branch(code: &mut [u8], at: usize, target: usize) {
    let offset = i16::try_from(target).unwrap() - i16::try_from(at).unwrap();
    code[at + 1..at + 3].copy_from_slice(&offset.to_be_bytes());
}

fn integer_array_program(iterations: i32) -> Program {
    assert!((1..=1_000_000).contains(&iterations));
    let mut pool = Pool::new();
    assert_eq!(pool.push(Constant::Integer(iterations)), 1);
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
    // Include the updated array value as well as the checksum of loop indices.
    code.extend([0x1b, 0x2a, 0x03, 0x2e, 0x60, 0xac]);
    branch(&mut code, exit_branch, exit_pc);
    branch(&mut code, back_branch, loop_pc);
    program(pool, code, "()I", 5, 3)
}

fn integer_branch_program(iterations: i32) -> Program {
    assert!((2..=2_000_000).contains(&iterations) && iterations % 2 == 0);
    let mut pool = Pool::new();
    assert_eq!(pool.push(Constant::Integer(iterations)), 1);
    assert_eq!(pool.push(Constant::Integer(iterations / 2)), 2);
    let mut code = vec![0x03, 0x3b, 0x12, 1, 0x3c, 0x03, 0x3d, 0x12, 2, 0x3e];
    let start = code.len();
    code.extend([0x1a, 0x1b]);
    let exit = code.len();
    code.extend([0xa2, 0, 0, 0x1a, 0x1d]);
    let skip = code.len();
    code.extend([0xa1, 0, 0, 0x1c, 0x1a, 0x60, 0x3d]);
    let increment = code.len();
    code.extend([0x84, 0, 1]);
    let back = code.len();
    code.extend([0xa7, 0, 0]);
    let end = code.len();
    code.extend([0x1c, 0xac]);
    branch(&mut code, exit, end);
    branch(&mut code, skip, increment);
    branch(&mut code, back, start);
    program(pool, code, "()I", 2, 4)
}

fn integer_call_program(iterations: i32, writes: bool) -> Program {
    const CALLEE: &str = "fixtures/IntegerCalls";
    let mut pool = Pool::new();
    assert_eq!(pool.push(Constant::Integer(iterations)), 1);
    let call = pool.method(CALLEE, "step", "(I)I");
    let mut code = vec![0x03, 0x3b, 0x12, 1, 0x3c, 0x03, 0x3d];
    let start = code.len();
    code.extend([0x1a, 0x1b]);
    let exit = code.len();
    code.extend([0xa2, 0, 0, 0x1c, 0x1a, 0xb8]);
    code.extend(call.to_be_bytes());
    code.extend([0x60, 0x3d, 0x84, 0, 1]);
    let back = code.len();
    code.extend([0xa7, 0, 0]);
    let end = code.len();
    code.extend([0x1c, 0xac]);
    branch(&mut code, exit, end);
    branch(&mut code, back, start);
    let mut program = program(pool, code, "()I", 2, 3);

    let mut pool = Pool::new();
    let this_class = pool.class(CALLEE);
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let name_index = pool.utf8("last");
    let descriptor_index = pool.utf8("I");
    let field = pool.field(CALLEE, "last", "I");
    let mut code = vec![0x1a, 0x04, 0x60];
    if writes {
        code.extend([0x59, 0xb3]);
        code.extend(field.to_be_bytes());
    }
    code.push(0xac);
    let method = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 9,
            name: "step",
            descriptor: "(I)I",
            max_stack: 2,
            max_locals: 1,
            code,
        },
    );
    program
        .add_class(
            &ClassFile {
                minor_version: 0,
                major_version: 48,
                constant_pool: pool.entries,
                access_flags: 0x21,
                this_class,
                super_class,
                interfaces: Vec::new(),
                fields: vec![classfile::Member {
                    access_flags: 9,
                    name_index,
                    descriptor_index,
                    attributes: Vec::new(),
                }],
                methods: vec![method],
                attributes: Vec::new(),
            },
            &Limits::default(),
        )
        .unwrap();
    program
}

// Keep the paired caller/callee bytecodes together in this synthetic fixture.
#[allow(clippy::too_many_lines)]
fn readonly_array_call_program(
    iterations: i32,
    hits: bool,
    reads: u8,
    interleaved: bool,
) -> Program {
    const CALLEE: &str = "fixtures/ReadonlyCalls";
    assert!((1..=65).contains(&reads));
    let mut pool = Pool::new();
    assert_eq!(pool.push(Constant::Integer(iterations)), 1);
    let descriptor = if interleaved { "([I[II)I" } else { "([II)I" };
    let call = pool.method(CALLEE, "sum", descriptor);
    let mut code = vec![0x10, reads, 0xbc, 10, 0x4b, 0x03, 0x3c];
    let init = code.len();
    code.extend([0x1b, 0x10, reads]);
    let init_exit = code.len();
    code.extend([0xa2, 0, 0, 0x2a, 0x1b, 0x1b, 0x4f, 0x84, 1, 1]);
    let init_back = code.len();
    code.extend([0xa7, 0, 0]);
    let init_end = code.len();
    branch(&mut code, init_exit, init_end);
    branch(&mut code, init_back, init);
    if interleaved {
        code.extend([0x10, reads, 0xbc, 10, 0x3a, 4]);
    }
    code.extend([0x03, 0x3c, 0x03, 0x3d, 0x12, 1, 0x3e]);
    let start = code.len();
    code.extend([0x1b, 0x1d]);
    let exit = code.len();
    code.extend([0xa2, 0, 0, 0x1c, 0x2a]);
    if interleaved {
        code.extend([0x19, 4]);
    }
    code.extend([if hits { 0x03 } else { 0x1b }, 0xb8]);
    code.extend(call.to_be_bytes());
    code.extend([0x60, 0x3d, 0x84, 1, 1]);
    let back = code.len();
    code.extend([0xa7, 0, 0]);
    let end = code.len();
    code.extend([0x1c, 0xac]);
    branch(&mut code, exit, end);
    branch(&mut code, back, start);
    let mut program = program(
        pool,
        code,
        "()I",
        if interleaved { 4 } else { 3 },
        if interleaved { 5 } else { 4 },
    );

    let mut pool = Pool::new();
    let this_class = pool.class(CALLEE);
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let mut code = if interleaved {
        vec![0x03, 0x3e, 0x1c, 0x36, 4]
    } else {
        vec![0x03, 0x3d, 0x1b, 0x3e]
    };
    let start = code.len();
    code.extend([if interleaved { 0x1d } else { 0x1c }, 0x10, reads]);
    let exit = code.len();
    code.extend([0xa2, 0, 0]);
    if interleaved {
        code.extend([
            0x15, 4, 0x2a, 0x1d, 0x2e, 0x60, 0x2b, 0x1d, 0x2e, 0x60, 0x36, 4, 0x84, 3, 1,
        ]);
    } else {
        code.extend([0x1d, 0x2a, 0x1c, 0x2e, 0x60, 0x3e, 0x84, 2, 1]);
    }
    let back = code.len();
    code.extend([0xa7, 0, 0]);
    let end = code.len();
    if interleaved {
        code.extend([0x15, 4, 0xac]);
    } else {
        code.extend([0x1d, 0xac]);
    }
    branch(&mut code, exit, end);
    branch(&mut code, back, start);
    let method = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 9,
            name: "sum",
            descriptor,
            max_stack: 3,
            max_locals: if interleaved { 5 } else { 4 },
            code,
        },
    );
    program
        .add_class(
            &ClassFile {
                minor_version: 0,
                major_version: 48,
                constant_pool: pool.entries,
                access_flags: 0x21,
                this_class,
                super_class,
                interfaces: Vec::new(),
                fields: Vec::new(),
                methods: vec![method],
                attributes: Vec::new(),
            },
            &Limits::default(),
        )
        .unwrap();
    program
}

#[test]
fn loaded_readonly_calls_preserve_read_dependencies_values_and_budgets() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "loaded_readonly_calls_preserve_read_dependencies_values_and_budgets"
    )) {
        return;
    }
    for (interleaved, reads, instructions) in [
        (false, 32, 7_747),
        (false, 64, 15_075),
        (false, 65, 15_304),
        (true, 32, 10_330),
        (true, 64, 20_218),
        (true, 65, 20_527),
    ] {
        for hits in [false, true] {
            let program = readonly_array_call_program(20, hits, reads, interleaved);
            for profile in [false, true] {
                let result = execute(&program, "()I", profile);
                assert_eq!(
                    result.value,
                    Some(Value::Int(
                        20 * i32::from(reads) * (i32::from(reads) - 1) / 2
                            + if hits { 0 } else { 190 }
                    ))
                );
                assert_eq!(result.instructions, instructions);
                for limit in [1, 1_023, 1_024, instructions - 1] {
                    let error = program
                        .execute(
                            CLASS,
                            "run",
                            "()I",
                            Limits {
                                max_instructions: limit,
                                profile_methods: profile,
                                ..Limits::default()
                            },
                            false,
                        )
                        .unwrap_err();
                    assert_eq!(error.code(), "instruction-limit");
                }
            }
        }
    }
}

#[test]
#[ignore = "manual release measurement of readonly call observation and reuse"]
fn linked_readonly_call_throughput() {
    measure_readonly_calls(32, false, "linked-readonly");
}

#[test]
#[ignore = "manual release measurement at the readonly observation limit"]
fn linked_readonly_wide_observation_throughput() {
    for reads in [64, 65] {
        measure_readonly_calls(reads, false, &format!("linked-readonly-wide reads={reads}"));
    }
}

#[test]
#[ignore = "manual release measurement of alternating array observations"]
fn linked_readonly_mixed_observation_throughput() {
    measure_readonly_calls(32, true, "linked-readonly-mixed");
}

fn measure_readonly_calls(reads: u8, interleaved: bool, label: &str) {
    let iterations = 30_000;
    for hits in [false, true] {
        let program = readonly_array_call_program(iterations, hits, reads, interleaved);
        let expected = iterations * i32::from(reads) * (i32::from(reads) - 1) / 2
            + if hits {
                0
            } else {
                iterations * (iterations - 1) / 2
            };
        for (repeat, profile) in [true, false, true, false].into_iter().enumerate() {
            let started = std::time::Instant::now();
            let result = execute(&program, "()I", profile);
            let elapsed = started.elapsed();
            assert_eq!(result.value, Some(Value::Int(expected)));
            assert_eq!(
                result.instructions,
                (if interleaved {
                    21 + 15 * u64::from(reads)
                } else {
                    20 + 11 * u64::from(reads)
                }) * u64::try_from(iterations).unwrap()
                    + 9 * u64::from(reads)
                    + if interleaved { 22 } else { 19 }
            );
            eprintln!(
                "{label} hits={hits} batched={} repeat={repeat} elapsed={elapsed:?} instructions={} checksum={:08x}",
                !profile,
                result.instructions,
                expected.cast_unsigned()
            );
        }
    }
}

#[test]
fn loaded_integer_calls_preserve_values_counts_and_limits() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "loaded_integer_calls_preserve_values_counts_and_limits"
    )) {
        return;
    }
    for writes in [false, true] {
        let program = integer_call_program(100, writes);
        let instructions = (14 + u64::from(writes) * 2) * 100 + 11;
        for profile in [false, true] {
            let result = execute(&program, "()I", profile);
            assert_eq!(result.value, Some(Value::Int(5_050)));
            assert_eq!(result.instructions, instructions);
            for limit in [1, 1_023, 1_024, instructions - 1] {
                let error = program
                    .execute(
                        CLASS,
                        "run",
                        "()I",
                        Limits {
                            max_instructions: limit,
                            profile_methods: profile,
                            ..Limits::default()
                        },
                        false,
                    )
                    .unwrap_err();
                assert_eq!(error.code(), "instruction-limit");
            }
        }
    }
}

#[test]
#[ignore = "manual release measurement of interpreted method calls"]
fn linked_integer_call_throughput() {
    let iterations = 300_000;
    let expected = (1..=iterations).fold(0_i32, i32::wrapping_add);
    for writes in [false, true] {
        let program = integer_call_program(iterations, writes);
        for (repeat, profile) in [true, false, true, false].into_iter().enumerate() {
            let started = std::time::Instant::now();
            let result = execute(&program, "()I", profile);
            let elapsed = started.elapsed();
            assert_eq!(result.value, Some(Value::Int(expected)));
            assert_eq!(
                result.instructions,
                (14 + u64::from(writes) * 2) * u64::try_from(iterations).unwrap() + 11
            );
            eprintln!(
                "linked-calls writes={writes} batched={} repeat={repeat} elapsed={elapsed:?} instructions={} checksum={:08x}",
                !profile,
                result.instructions,
                expected.cast_unsigned()
            );
        }
    }
}

#[test]
fn loaded_integer_branches_preserve_values_counts_and_limits() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "loaded_integer_branches_preserve_values_counts_and_limits"
    )) {
        return;
    }
    let program = integer_branch_program(128);
    for profile in [false, true] {
        let result = execute(&program, "()I", profile);
        assert_eq!(result.value, Some(Value::Int((64..128).sum())));
        assert_eq!(result.instructions, 1_293);
        for limit in [1, 1_022, 1_023, 1_024, 1_025, 1_292] {
            let error = program
                .execute(
                    CLASS,
                    "run",
                    "()I",
                    Limits {
                        max_instructions: limit,
                        profile_methods: profile,
                        ..Limits::default()
                    },
                    false,
                )
                .unwrap_err();
            assert_eq!(error.code(), "instruction-limit");
        }
    }
}

#[test]
#[ignore = "manual release measurement of integer branch dispatch"]
fn linked_integer_branch_throughput() {
    let program = integer_branch_program(2_000_000);
    let expected = (1_000_000_i32..2_000_000).fold(0_i32, i32::wrapping_add);
    for (repeat, profile) in [true, false, true, false].into_iter().enumerate() {
        let started = std::time::Instant::now();
        let result = execute(&program, "()I", profile);
        let elapsed = started.elapsed();
        assert_eq!(result.value, Some(Value::Int(expected)));
        assert_eq!(result.instructions, 20_000_013);
        eprintln!(
            "linked-branches batched={} repeat={repeat} elapsed={elapsed:?} instructions={} checksum={:08x}",
            !profile,
            result.instructions,
            expected.cast_unsigned()
        );
    }
}

fn numeric_array_program(
    kind: NumberKind,
    conversions: bool,
    iterations: i32,
) -> (Program, &'static str) {
    assert!((1..=262_144).contains(&iterations));
    let ([array_type, load, store, multiply, add, one, ret], descriptor, factor) =
        kind.instructions();
    let mut pool = Pool::new();
    assert_eq!(pool.push(Constant::Integer(iterations)), 1);
    assert_eq!(pool.push(factor), 2);
    if matches!(kind, NumberKind::Double) {
        pool.entries.push(None);
    }
    let mut code = vec![0x10, 64, 0xbc, array_type, 0x4b, 0x03, 0x3c];
    let loop_pc = code.len();
    code.extend([0x1b, 0x12, 1]);
    let exit_branch = code.len();
    code.extend([0xa2, 0, 0]);
    code.extend([0x2a, 0x1b, 0x10, 63, 0x7e, 0x5c, load]);
    if matches!(kind, NumberKind::Double) {
        code.extend([0x14, 0, 2]);
    } else {
        code.extend([0x12, 2]);
    }
    code.extend([multiply, one, add]);
    if conversions {
        code.extend(match kind {
            NumberKind::Float => [0x8b, 0x86],
            NumberKind::Double => [0x8e, 0x87],
            NumberKind::Int => panic!("integer fixture has no floating conversion"),
        });
    }
    code.extend([store, 0x84, 1, 1]);
    let back_branch = code.len();
    code.extend([0xa7, 0, 0]);
    let exit_pc = code.len();
    code.extend([0x2a, 0x03, load, ret]);
    branch(&mut code, exit_branch, exit_pc);
    branch(&mut code, back_branch, loop_pc);
    (program(pool, code, descriptor, 6, 2), descriptor)
}

fn execute(program: &Program, descriptor: &str, profile_methods: bool) -> vm::Execution {
    program
        .execute(
            CLASS,
            "run",
            descriptor,
            Limits {
                max_instructions: 30_000_000,
                profile_methods,
                ..Limits::default()
            },
            false,
        )
        .unwrap()
}

#[test]
fn loaded_array_loops_preserve_values_counts_and_instruction_limits() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "loaded_array_loops_preserve_values_counts_and_instruction_limits"
    )) {
        return;
    }
    let integer = integer_array_program(128);
    for profile in [false, true] {
        let result = execute(&integer, "()I", profile);
        assert_eq!(result.value, Some(Value::Int(2)));
        assert_eq!(result.instructions, 18 * 128 + 16);
    }
    for kind in [NumberKind::Int, NumberKind::Float, NumberKind::Double] {
        for conversions in [false, true] {
            if conversions && matches!(kind, NumberKind::Int) {
                continue;
            }
            let (program, descriptor) = numeric_array_program(kind, conversions, 128);
            let expected = match kind {
                NumberKind::Int => Value::Int(3),
                NumberKind::Float => Value::Float(if conversions { 1.0 } else { 1.5 }),
                NumberKind::Double => Value::Double(if conversions { 1.0 } else { 1.5 }),
            };
            for profile in [false, true] {
                let result = execute(&program, descriptor, profile);
                assert_eq!(result.value, Some(expected));
                assert_eq!(
                    result.instructions,
                    (16 + u64::from(conversions) * 2) * 128 + 12
                );
                let error = program
                    .execute(
                        CLASS,
                        "run",
                        descriptor,
                        Limits {
                            max_instructions: 1,
                            profile_methods: profile,
                            ..Limits::default()
                        },
                        false,
                    )
                    .unwrap_err();
                assert_eq!(error.code(), "instruction-limit");
            }
        }
    }
}

#[test]
#[ignore = "manual release measurement of the normal VM dependency"]
fn linked_bytecode_array_throughput() {
    let program = integer_array_program(1_000_000);
    for (repeat, profile) in [true, false, true, false].into_iter().enumerate() {
        let started = std::time::Instant::now();
        let result = execute(&program, "()I", profile);
        let elapsed = started.elapsed();
        assert_eq!(result.value, Some(Value::Int(15_625)));
        assert_eq!(result.instructions, 18_000_016);
        eprintln!(
            "linked-bytecode batched={} repeat={repeat} elapsed={elapsed:?} instructions={} checksum=00003d09",
            !profile, result.instructions
        );
    }
}

#[test]
#[ignore = "manual release measurement of the normal VM dependency"]
fn linked_numeric_array_throughput() {
    for kind in [NumberKind::Int, NumberKind::Float, NumberKind::Double] {
        for conversions in [false, true] {
            if conversions && matches!(kind, NumberKind::Int) {
                continue;
            }
            let (program, descriptor) = numeric_array_program(kind, conversions, 262_144);
            let reference = execute(&program, descriptor, true);
            let started = std::time::Instant::now();
            let result = execute(&program, descriptor, false);
            let elapsed = started.elapsed();
            assert_eq!(result.value, reference.value);
            assert_eq!(result.instructions, reference.instructions);
            assert_eq!(
                result.instructions,
                (16 + u64::from(conversions) * 2) * 262_144 + 12,
            );
            let expected = match kind {
                NumberKind::Int => Value::Int(-1),
                NumberKind::Float => Value::Float(if conversions { 1.0 } else { 2.0 }),
                NumberKind::Double => Value::Double(if conversions { 1.0 } else { 2.0 }),
            };
            assert_eq!(result.value, Some(expected));
            let checksum = match result.value.unwrap() {
                Value::Int(value) => u64::from(value.cast_unsigned()),
                Value::Float(value) => u64::from(value.to_bits()),
                Value::Double(value) => value.to_bits(),
                _ => panic!("primitive numeric result required"),
            };
            eprintln!(
                "linked-numeric kind={} conversions={conversions} elapsed={elapsed:?} instructions={} checksum={checksum:016x}",
                kind.name(),
                result.instructions
            );
        }
    }
}
