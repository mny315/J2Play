use super::*;
use std::rc::Rc;
use vm::acceleration::{CompiledIntegerMethod, IntegerMethodCompiler, IntegerMethodSpec};

mod support;

#[path = "../../support/storage.rs"]
mod test_storage;
pub(crate) use test_storage::Scratch;

fn spec(code: &[u8], parameters: u8) -> IntegerMethodSpec {
    IntegerMethodSpec {
        static_integer_parameters: vec![],
        code: code.to_vec(),
        integer_constants: vec![],
        parameter_slots: (0..parameters).collect(),
        max_locals: 4,
        max_stack: 8,
    }
}

fn compiled(spec: IntegerMethodSpec) -> Rc<dyn CompiledIntegerMethod> {
    let mut compiler = Compiler::new();
    compiler
        .compile(&[spec], &|| false)
        .into_iter()
        .next()
        .flatten()
        .expect("numeric code compiled")
}

#[test]
fn static_integer_inputs_are_dynamic_and_reserved_from_guest_locals() {
    let mut input = spec(&[0xb2, 0, 1, 0x04, 0x60, 0xac], 1);
    input.static_integer_parameters = vec![(1, 0)];
    let method = compiled(input.clone());
    for value in [i32::MIN, -1, 0, 41, i32::MAX] {
        let result = method.execute(&[value], 4).unwrap();
        assert_eq!(result.value, value.wrapping_add(1));
        assert_eq!(result.instructions, 4);
        assert!(method.execute(&[value], 3).is_none());
    }
    for code in [
        vec![0x1a, 0xac],
        vec![0x03, 0x3b, 0xb2, 0, 1, 0xac],
        vec![0x84, 0, 1, 0xb2, 0, 1, 0xac],
    ] {
        input.code = code;
        assert!(Compiler::new().compile(&[input.clone()], &|| false)[0].is_none());
    }
}

#[test]
fn vm_compiled_static_reads_observe_changes_and_cold_resolution() {
    use classfile::{Attribute, Constant, Member};
    let mut class = support::fixture(30);
    class.constant_pool.extend([
        Some(Constant::Utf8("scale".into())),
        Some(Constant::Utf8("I".into())),
        Some(Constant::NameAndType {
            name_index: 11,
            descriptor_index: 12,
        }),
        Some(Constant::Fieldref {
            class_index: 1,
            name_and_type_index: 13,
        }),
    ]);
    class.fields.push(Member {
        access_flags: 8,
        name_index: 11,
        descriptor_index: 12,
        attributes: vec![],
    });
    let Attribute::Code(kernel) = &mut class.methods[0].attributes[0] else {
        unreachable!()
    };
    kernel.code = vec![0x1a, 0xb2, 0, 14, 0x04, 0x60, 0x68, 0x04, 0x60, 0xac];
    let Attribute::Code(run) = &mut class.methods[1].attributes[0] else {
        unreachable!()
    };
    // The first call precedes any field access and must resolve through the VM.
    // Later calls read a different mutable static value on every iteration.
    run.code = vec![
        0x03, 0xb8, 0, 5, 0x57, 0x12, 10, 0x3b, 0x03, 0x3c, 0x1a, 0x9e, 0, 20, 0x1a, 0xb3, 0, 14,
        0x05, 0xb8, 0, 5, 0x1b, 0x60, 0x3c, 0x84, 0, 0xff, 0xa7, 0xff, 0xee, 0x1b, 0xac,
    ];
    let limits = vm::Limits::default();
    let mut baseline = vm::Program::new();
    baseline.add_class(&class, &limits).unwrap();
    let mut accelerated = vm::Program::new();
    accelerated.add_class(&class, &limits).unwrap();
    let mut compiler = Compiler::new();
    assert_eq!(
        accelerated.prepare_integer_methods(&mut compiler, &|| false),
        1
    );
    let cases = [1, 8, 20, 40, 100, 1023, 1024, 1025, 4000]
        .into_iter()
        .map(|max_instructions| vm::Limits {
            max_instructions,
            ..limits.clone()
        })
        .chain([1, 2].into_iter().map(|max_frames| vm::Limits {
            max_frames,
            ..limits.clone()
        }))
        .chain([1, 2, 3].into_iter().map(|max_stack_slots| vm::Limits {
            max_stack_slots,
            ..limits.clone()
        }));
    for limits in cases {
        let expected = baseline.execute("NumericProbe", "run", "()I", limits.clone(), false);
        let actual = accelerated.execute("NumericProbe", "run", "()I", limits, false);
        match (actual, expected) {
            (Ok(actual), Ok(expected)) => {
                assert_eq!(actual.value, Some(vm::Value::Int(1020)));
                assert_eq!(
                    (actual.value, actual.instructions),
                    (expected.value, expected.instructions)
                );
            }
            (Err(actual), Err(expected)) => assert_eq!(actual.to_string(), expected.to_string()),
            (actual, expected) => panic!("static integer mismatch: {actual:?} vs {expected:?}"),
        }
    }
    assert!(compiler.run_stats().calls > 0);
}

#[test]
fn native_arithmetic_matches_java_boundaries() {
    let inputs: [i32; 10] = [i32::MIN, i32::MAX, -65_537, -33, -1, 0, 1, 31, 32, 65_535];
    for opcode in [
        0x60, 0x64, 0x68, 0x6c, 0x70, 0x78, 0x7a, 0x7c, 0x7e, 0x80, 0x82,
    ] {
        let method = compiled(spec(&[0x1a, 0x1b, opcode, 0xac], 2));
        for left in inputs {
            for right in inputs {
                if matches!(opcode, 0x6c | 0x70) && right == 0 {
                    assert!(method.execute(&[left, right], 1_023).is_none());
                    continue;
                }
                let expected = match opcode {
                    0x60 => left.wrapping_add(right),
                    0x64 => left.wrapping_sub(right),
                    0x68 => left.wrapping_mul(right),
                    0x6c => left.wrapping_div(right),
                    0x70 => left.wrapping_rem(right),
                    0x78 => left.wrapping_shl(right.cast_unsigned()),
                    0x7a => left.wrapping_shr(right.cast_unsigned()),
                    0x7c => left
                        .cast_unsigned()
                        .wrapping_shr(right.cast_unsigned())
                        .cast_signed(),
                    0x7e => left & right,
                    0x80 => left | right,
                    _ => left ^ right,
                };
                let result = method.execute(&[left, right], 1_023).unwrap();
                assert_eq!(result.value, expected, "opcode {opcode:x}: {left}, {right}");
                assert_eq!(result.instructions, 4);
                assert!(method.execute(&[left, right], 3).is_none());
            }
        }
    }
}

#[test]
fn native_branches_loops_and_fuel_are_exact() {
    // sum = 0; while (n > 0) sum += n--; return sum;
    let code = [
        0x03, 0x3c, 0x1a, 0x9e, 0, 13, 0x1b, 0x1a, 0x60, 0x3c, 0x84, 0, 0xff, 0xa7, 0xff, 0xf5,
        0x1b, 0xac,
    ];
    let method = compiled(spec(&code, 1));
    for n in [-1, 0, 1, 2, 10, 100] {
        let result = method.execute(&[n], 1_023).unwrap();
        assert_eq!(result.value, if n <= 0 { 0 } else { n * (n + 1) / 2 });
        assert_eq!(result.instructions, 6 + 8 * n.max(0).cast_unsigned());
        assert!(method.execute(&[n], result.instructions - 1).is_none());
        assert_eq!(method.execute(&[n], result.instructions), Some(result));
    }
    assert!(method.execute(&[i32::MAX], 1_023).is_none());
    assert!(method.execute(&[], 1_023).is_none());
}

#[test]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn all_integer_stack_permutations_and_conversions() {
    for opcode in 0x57..=0x5f {
        let mut code = vec![0x04, 0x05, 0x06, 0x07, opcode];
        let expected = match opcode {
            0x57 => vec![1, 2, 3],
            0x58 => vec![1, 2],
            0x59 => vec![1, 2, 3, 4, 4],
            0x5a => vec![1, 2, 4, 3, 4],
            0x5b => vec![1, 4, 2, 3, 4],
            0x5c => vec![1, 2, 3, 4, 3, 4],
            0x5d => vec![1, 3, 4, 2, 3, 4],
            0x5e => vec![3, 4, 1, 2, 3, 4],
            _ => vec![1, 2, 4, 3],
        };
        // Each stack position occupies a distinct decimal digit.
        for _ in 1..expected.len() {
            code.extend([0x10, 10, 0x68, 0x60]);
        }
        code.push(0xac);
        let expected = expected
            .iter()
            .rev()
            .fold(0, |number, digit| number * 10 + digit);
        assert_eq!(
            compiled(spec(&code, 0)).execute(&[], 1_023).unwrap().value,
            expected,
            "stack opcode {opcode:x}"
        );
    }
    for opcode in [0x74, 0x91, 0x92, 0x93] {
        let method = compiled(spec(&[0x1a, opcode, 0xac], 1));
        for value in [i32::MIN, i32::MAX, -65_537, -129, -1, 0, 127, 128, 65_535] {
            let expected = match opcode {
                0x74 => value.wrapping_neg(),
                0x91 => i32::from(value as i8),
                0x92 => i32::from(value as u16),
                _ => i32::from(value as i16),
            };
            assert_eq!(method.execute(&[value], 1_023).unwrap().value, expected);
        }
    }
}

#[test]
fn executable_cache_survives_compiler_and_module_teardown() {
    let root = Scratch::new();
    let input = spec(&[0x1a, 0x1b, 0x68, 0xac], 2);
    {
        let mut compiler = Compiler::with_cache(root.0.clone());
        let methods = compiler.compile(std::slice::from_ref(&input), &|| false);
        assert_eq!(compiler.preparation_stats().compiled, 1);
        assert_eq!(
            methods[0]
                .as_ref()
                .unwrap()
                .execute(&[6, 7], 1_023)
                .unwrap()
                .value,
            42
        );
    }
    let mut compiler = Compiler::with_cache(root.0.clone());
    let methods = compiler.compile(&[input], &|| false);
    assert_eq!(compiler.preparation_stats().compiled, 0);
    assert_eq!(compiler.preparation_stats().cache_hits, 1);
    assert_eq!(
        methods[0]
            .as_ref()
            .unwrap()
            .execute(&[7, 8], 1_023)
            .unwrap()
            .value,
        56
    );
}

#[test]
fn cancellation_leaves_candidates_interpreted() {
    let mut compiler = Compiler::new();
    assert!(compiler.compile(&[spec(&[0x1a, 0xac], 1)], &|| true)[0].is_none());
}

#[test]
fn vm_native_and_interpreter_have_identical_effects_and_instruction_limits() {
    let mut program = vm::Program::new();
    let limits = vm::Limits::default();
    program.add_class(&support::fixture(30), &limits).unwrap();
    let baseline = program
        .execute("NumericProbe", "run", "()I", limits.clone(), false)
        .unwrap();
    let mut compiler = Compiler::new();
    assert_eq!(program.prepare_integer_methods(&mut compiler, &|| false), 1);
    let native = program
        .execute("NumericProbe", "run", "()I", limits.clone(), false)
        .unwrap();
    assert_eq!(native.value, Some(vm::Value::Int(205_920)));
    assert_eq!(
        (native.value, native.instructions),
        (baseline.value, baseline.instructions)
    );
    assert!(compiler.run_stats().calls > 0);
    for budget in [1, 8, 20, 100, 1_023, 1_024, 1_025] {
        let mut limited = limits.clone();
        limited.max_instructions = budget;
        let error = program
            .execute("NumericProbe", "run", "()I", limited, false)
            .unwrap_err();
        assert_eq!(error.code(), "instruction-limit");
    }
}
