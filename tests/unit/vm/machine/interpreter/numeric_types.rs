use super::*;

fn check_integer_casts(value: Value, integer: i32, long: i64) {
    let (to_int, to_long) = match value {
        Value::Float(_) => (0x8b, 0x8c),
        Value::Double(_) => (0x8e, 0x8f),
        _ => panic!("expected a floating-point input"),
    };
    assert_eq!(convert(to_int, value).unwrap(), Value::Int(integer));
    assert_eq!(convert(to_long, value).unwrap(), Value::Long(long));
}

#[test]
fn integer_conversions_truncate_and_saturate_special_values() {
    for (value, integer, long) in [
        (0.0_f64, 0, 0),
        (-0.0, 0, 0),
        (0.75, 0, 0),
        (-0.75, 0, 0),
        (1.75, 1, 1),
        (-1.75, -1, -1),
        (f64::NAN, 0, 0),
        (-f64::NAN, 0, 0),
        (f64::INFINITY, i32::MAX, i64::MAX),
        (f64::NEG_INFINITY, i32::MIN, i64::MIN),
        (1.0e20, i32::MAX, i64::MAX),
        (-1.0e20, i32::MIN, i64::MIN),
    ] {
        check_integer_casts(Value::Double(value), integer, long);
        check_integer_casts(Value::Float(value as f32), integer, long);
    }
}

#[test]
fn integer_conversions_preserve_precision_at_range_edges() {
    for (bits, integer, long) in [
        (0x4eff_ffff, 2_147_483_520, 2_147_483_520),
        (0xceff_ffff, -2_147_483_520, -2_147_483_520),
        (0x4f00_0000, i32::MAX, 2_147_483_648),
        (0xcf00_0000, i32::MIN, -2_147_483_648),
        (0x5eff_ffff, i32::MAX, 9_223_371_487_098_961_920),
        (0xdeff_ffff, i32::MIN, -9_223_371_487_098_961_920),
        (0x5f00_0000, i32::MAX, i64::MAX),
        (0xdf00_0000, i32::MIN, i64::MIN),
    ] {
        check_integer_casts(Value::Float(f32::from_bits(bits)), integer, long);
    }
    for (value, integer, long) in [
        (16_777_217.0_f64, 16_777_217, 16_777_217),
        (2_147_483_647.75, i32::MAX, 2_147_483_647),
        (-2_147_483_647.75, -2_147_483_647, -2_147_483_647),
        (2_147_483_648.0, i32::MAX, 2_147_483_648),
        (-2_147_483_648.0, i32::MIN, -2_147_483_648),
        (
            9_223_372_036_854_774_784.0,
            i32::MAX,
            9_223_372_036_854_774_784,
        ),
        (
            -9_223_372_036_854_774_784.0,
            i32::MIN,
            -9_223_372_036_854_774_784,
        ),
        (9_223_372_036_854_775_808.0, i32::MAX, i64::MAX),
        (-9_223_372_036_854_775_808.0, i32::MIN, i64::MIN),
    ] {
        check_integer_casts(Value::Double(value), integer, long);
    }
}

#[test]
fn integer_division_preserves_overflow_and_reports_zero_divisors() {
    for (divide, remainder, left, right, quotient, residual, zero) in [
        (
            0x6c,
            0x70,
            Value::Int(i32::MIN),
            Value::Int(-1),
            Value::Int(i32::MIN),
            Value::Int(0),
            Value::Int(0),
        ),
        (
            0x6d,
            0x71,
            Value::Long(i64::MIN),
            Value::Long(-1),
            Value::Long(i64::MIN),
            Value::Long(0),
            Value::Long(0),
        ),
        (
            0x6c,
            0x70,
            Value::Int(-7),
            Value::Int(3),
            Value::Int(-2),
            Value::Int(-1),
            Value::Int(0),
        ),
        (
            0x6d,
            0x71,
            Value::Long(-7),
            Value::Long(3),
            Value::Long(-2),
            Value::Long(-1),
            Value::Long(0),
        ),
    ] {
        assert_eq!(binary(divide, left, right).unwrap(), quotient);
        assert_eq!(binary(remainder, left, right).unwrap(), residual);
        for opcode in [divide, remainder] {
            assert_eq!(
                binary(opcode, left, zero).unwrap_err().code(),
                "arithmetic-exception"
            );
        }
    }
}

#[test]
fn numeric_negation_requires_the_opcode_operand_type() {
    for (opcode, expected) in [(0x74, 0), (0x75, 1), (0x76, 2), (0x77, 3)] {
        for (kind, load, descriptor, return_op, negated) in [
            (0, 0x04, "()I", 0xac, Value::Int(-1)),
            (1, 0x0a, "()J", 0xad, Value::Long(-1)),
            (2, 0x0c, "()F", 0xae, Value::Float(-1.0)),
            (3, 0x0f, "()D", 0xaf, Value::Double(-1.0)),
        ] {
            let method = runtime_method(
                "Numeric",
                "negate",
                descriptor,
                &[load, opcode, return_op],
                2,
                0,
                vec![None],
                true,
            );
            let mut program = Program::new();
            program.methods.insert(method.key.clone(), method);
            for tracing in [false, true] {
                let result =
                    program.execute("Numeric", "negate", descriptor, Limits::default(), tracing);
                if kind == expected {
                    assert_eq!(result.unwrap().value, Some(negated), "opcode {opcode:x}");
                } else {
                    assert_eq!(
                        result.unwrap_err().code(),
                        "type-mismatch",
                        "opcode {opcode:x}, kind {kind}"
                    );
                }
            }
        }
    }
}

#[test]
fn numeric_comparison_requires_the_opcode_operand_type() {
    for (opcode, expected) in [
        (0x94, 0x0a),
        (0x95, 0x0c),
        (0x96, 0x0c),
        (0x97, 0x0f),
        (0x98, 0x0f),
    ] {
        for load in [0x0a, 0x0c, 0x0f] {
            let method = runtime_method(
                "Numeric",
                "compare",
                "()I",
                &[load, load, opcode, 0xac],
                4,
                0,
                vec![None],
                true,
            );
            let mut program = Program::new();
            program.methods.insert(method.key.clone(), method);
            for tracing in [false, true] {
                let result =
                    program.execute("Numeric", "compare", "()I", Limits::default(), tracing);
                if load == expected {
                    assert_eq!(result.unwrap().value, Some(Value::Int(0)));
                } else {
                    assert_eq!(
                        result.unwrap_err().code(),
                        "type-mismatch",
                        "opcode {opcode:x}, load {load:x}"
                    );
                }
            }
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn primitive_opcode_matrix_has_expected_results() {
    let binary_cases = [
        (0x60, Value::Int(7), Value::Int(3), Value::Int(10)),
        (0x64, Value::Int(7), Value::Int(3), Value::Int(4)),
        (0x68, Value::Int(7), Value::Int(3), Value::Int(21)),
        (0x6c, Value::Int(7), Value::Int(3), Value::Int(2)),
        (0x70, Value::Int(7), Value::Int(3), Value::Int(1)),
        (0x61, Value::Long(7), Value::Long(3), Value::Long(10)),
        (0x65, Value::Long(7), Value::Long(3), Value::Long(4)),
        (0x69, Value::Long(7), Value::Long(3), Value::Long(21)),
        (0x6d, Value::Long(7), Value::Long(3), Value::Long(2)),
        (0x71, Value::Long(7), Value::Long(3), Value::Long(1)),
        (
            0x62,
            Value::Float(7.0),
            Value::Float(2.0),
            Value::Float(9.0),
        ),
        (
            0x66,
            Value::Float(7.0),
            Value::Float(2.0),
            Value::Float(5.0),
        ),
        (
            0x6a,
            Value::Float(7.0),
            Value::Float(2.0),
            Value::Float(14.0),
        ),
        (
            0x6e,
            Value::Float(7.0),
            Value::Float(2.0),
            Value::Float(3.5),
        ),
        (
            0x72,
            Value::Float(7.0),
            Value::Float(2.0),
            Value::Float(1.0),
        ),
        (
            0x63,
            Value::Double(7.0),
            Value::Double(2.0),
            Value::Double(9.0),
        ),
        (
            0x67,
            Value::Double(7.0),
            Value::Double(2.0),
            Value::Double(5.0),
        ),
        (
            0x6b,
            Value::Double(7.0),
            Value::Double(2.0),
            Value::Double(14.0),
        ),
        (
            0x6f,
            Value::Double(7.0),
            Value::Double(2.0),
            Value::Double(3.5),
        ),
        (
            0x73,
            Value::Double(7.0),
            Value::Double(2.0),
            Value::Double(1.0),
        ),
    ];
    for (opcode, left, right, expected) in binary_cases {
        assert_eq!(
            binary(opcode, left, right).unwrap(),
            expected,
            "opcode {opcode:#x}"
        );
    }
    for (opcode, input, expected) in [
        (0x85, Value::Int(65), Value::Long(65)),
        (0x86, Value::Int(65), Value::Float(65.0)),
        (0x87, Value::Int(65), Value::Double(65.0)),
        (0x88, Value::Long(65), Value::Int(65)),
        (0x89, Value::Long(65), Value::Float(65.0)),
        (0x8a, Value::Long(65), Value::Double(65.0)),
        (0x8b, Value::Float(65.0), Value::Int(65)),
        (0x8c, Value::Float(65.0), Value::Long(65)),
        (0x8d, Value::Float(65.0), Value::Double(65.0)),
        (0x8e, Value::Double(65.0), Value::Int(65)),
        (0x8f, Value::Double(65.0), Value::Long(65)),
        (0x90, Value::Double(65.0), Value::Float(65.0)),
        (0x91, Value::Int(65), Value::Int(65)),
        (0x92, Value::Int(65), Value::Int(65)),
        (0x93, Value::Int(65), Value::Int(65)),
    ] {
        assert_eq!(
            convert(opcode, input).unwrap(),
            expected,
            "opcode {opcode:#x}"
        );
    }
    for (zero_opcode, pair_opcode, expected) in [
        (0x99, 0x9f, [false, true, false]),
        (0x9a, 0xa0, [true, false, true]),
        (0x9b, 0xa1, [true, false, false]),
        (0x9c, 0xa2, [false, true, true]),
        (0x9d, 0xa3, [false, false, true]),
        (0x9e, 0xa4, [true, true, false]),
    ] {
        for (value, expected) in [-1, 0, 1].into_iter().zip(expected) {
            assert_eq!(test_zero(zero_opcode, value), expected);
            assert_eq!(test_pair(pair_opcode, value + 2, 2), expected);
        }
    }
}

#[test]
fn integer_to_long_arithmetic_and_narrowing_execute() {
    assert_eq!(
        run(&[0x05, 0x85, 0x0a, 0x61, 0x88, 0xac], 4, 0).unwrap(),
        Some(Value::Int(3))
    )
}
