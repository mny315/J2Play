use super::*;

#[test]
pub(crate) fn unverified_glyph_fingerprint_preserves_bytecode_and_execution_limits() {
    for (code, expected_error) in [
        (&[0xb1][..], None),
        (&[0xa7, 0, 0][..], Some("instruction-limit")),
    ] {
        let mut method = runtime_method(
            "FixtureRenderer",
            "draw",
            "(IIC)V",
            code,
            0,
            4,
            Vec::new(),
            false,
        );
        // Exercise a matching legacy hash with project-owned bytecode. The
        // hash alone proves neither the linked fields nor the called helpers.
        method.code_fingerprint = 0xb602_0544_88a9_9d0b;
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                max_instructions: 5,
                ..Limits::default()
            },
            false,
            &mut context,
        );
        let renderer = machine
            .heap
            .managed
            .allocate_object("FixtureRenderer", HashMap::new())
            .unwrap();
        let outcome = machine.call(
            &method,
            [
                Value::Reference(Some(renderer)),
                Value::Int(0),
                Value::Int(0),
                Value::Int(65),
            ],
            1,
        );
        if let Some(expected_error) = expected_error {
            let Err(error) = outcome else {
                panic!("guest loop bypassed its execution limit");
            };
            assert_eq!(error.code(), expected_error);
        } else {
            assert!(matches!(outcome, Ok(CallOutcome::Return(None))));
            assert_eq!(machine.execution.instructions, 1);
        }
    }
}

#[test]
pub(crate) fn project_graphics_scalar_intrinsics_match_bootstrap_boundaries() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let invoke = |machine: &mut Machine<'_, '_>, name, descriptor, arguments: &[Value]| {
        let method = runtime_method(
            "javax/microedition/lcdui/Graphics",
            name,
            descriptor,
            &[0x03, 0xac],
            4,
            arguments.len(),
            Vec::new(),
            true,
        );
        let outcome = machine
            .invoke_compatibility_intrinsic(&method, arguments, 1)
            .unwrap();
        let Some(CallOutcome::Return(Some(Value::Int(value)))) = outcome else {
            panic!("Graphics scalar intrinsic did not return an int");
        };
        value
    };

    assert_eq!(
        invoke(
            &mut machine,
            "sat",
            "(II)I",
            &[Value::Int(i32::MAX), Value::Int(1)],
        ),
        i32::MAX
    );
    assert_eq!(
        invoke(
            &mut machine,
            "sat",
            "(II)I",
            &[Value::Int(i32::MIN), Value::Int(-1)],
        ),
        i32::MIN
    );
    for (value, expected) in [(-5, 0), (5, 5), (15, 10)] {
        assert_eq!(
            invoke(
                &mut machine,
                "clamp",
                "(III)I",
                &[Value::Int(value), Value::Int(0), Value::Int(10)],
            ),
            expected
        );
    }
    for (anchor, horizontal, expected) in [(8, 1, 6), (1, 1, 8), (32, 0, 6), (2, 0, 8)] {
        assert_eq!(
            invoke(
                &mut machine,
                "origin",
                "(IIIZ)I",
                &[
                    Value::Int(10),
                    Value::Int(4),
                    Value::Int(anchor),
                    Value::Int(horizontal),
                ],
            ),
            expected
        );
    }

    let invalid = runtime_method(
        "javax/microedition/lcdui/Graphics",
        "origin",
        "(IIIZ)I",
        &[0x03, 0xac],
        4,
        4,
        Vec::new(),
        true,
    );
    let invalid_result = machine.invoke_compatibility_intrinsic(
        &invalid,
        &[Value::Int(10), Value::Int(4), Value::Int(9), Value::Int(1)],
        1,
    );
    let Err(error) = invalid_result else {
        panic!("invalid Graphics origin anchor was accepted");
    };
    assert_eq!(error.code(), "illegal-argument");
    assert_eq!(machine.execution.instructions, 0);
}
