use super::*;
use crate::machine::{CallOutcome, DefaultNativeContext, Limits, Program, tests::runtime_method};

#[test]
#[ignore = "release class reference resolution throughput measurement"]
fn class_reference_resolution_throughput() {
    for name_bytes in [16, 4096] {
        let constants = vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8(format!("Fixture{}", "X".repeat(name_bytes)))),
        ];
        let started = std::time::Instant::now();
        let mut checksum = 0_usize;
        for _ in 0..262_144 {
            let name = resolve_class(std::hint::black_box(&constants), 1).unwrap();
            checksum += std::hint::black_box(name.as_bytes()).len();
        }
        eprintln!(
            "class-reference bytes={name_bytes} path=resolve ns={} checksum={checksum}",
            started.elapsed().as_nanos()
        );

        let mut code = [0x01, 0xc0, 0, 1, 0x57].repeat(512);
        code.push(0xb1);
        let method = runtime_method("Fixture", "casts", "()V", &code, 1, 0, constants, true);
        let program = Program::new();
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let started = std::time::Instant::now();
        for _ in 0..128 {
            assert!(matches!(
                machine.call(&method, [], 1).unwrap(),
                CallOutcome::Return(None)
            ));
        }
        let instructions = machine.execution.instructions;
        assert_eq!(instructions, 128 * (512 * 3 + 1));
        eprintln!(
            "class-reference bytes={name_bytes} path=bytecode ns={} checksum={instructions}",
            started.elapsed().as_nanos()
        );
    }
}

#[test]
fn method_parameter_slots_are_bounded_before_allocating_an_oversized_argument_list() {
    for (component, width) in [("I", 1), ("J", 2), ("D", 2), ("[J", 1), ("[[D", 1)] {
        for count in [127, 128, 255, 256, 60_000] {
            let descriptor = format!("({})D", component.repeat(count));
            let result = parse_method_descriptor(&descriptor);
            assert_eq!(
                result.is_ok(),
                count * width <= 255,
                "{component} * {count}"
            );
            if let Err(error) = result {
                assert_eq!(error.code(), "invalid-descriptor");
            }
        }
    }
}

#[test]
fn method_descriptors_preserve_scalar_and_array_value_kinds() {
    for (component, scalar) in [
        ("B", ValueKind::Int),
        ("C", ValueKind::Int),
        ("I", ValueKind::Int),
        ("S", ValueKind::Int),
        ("Z", ValueKind::Int),
        ("J", ValueKind::Long),
        ("F", ValueKind::Float),
        ("D", ValueKind::Double),
        ("Ljava/lang/Object;", ValueKind::Reference),
    ] {
        for dimensions in [0, 1, 2, 255] {
            let field = format!("{}{component}", "[".repeat(dimensions));
            let descriptor = parse_method_descriptor(&format!("({field}{field}){field}")).unwrap();
            let expected = if dimensions == 0 {
                scalar
            } else {
                ValueKind::Reference
            };
            assert_eq!(descriptor.parameters, [expected, expected]);
            assert_eq!(descriptor.returns, Some(expected));
        }
    }
    let descriptor = parse_method_descriptor("()V").unwrap();
    assert!(descriptor.parameters.is_empty());
    assert_eq!(descriptor.returns, None);
}

#[test]
fn malformed_or_excessively_nested_descriptors_return_errors() {
    for dimensions in [256, 65_535] {
        for descriptor in [
            format!("({}I)V", "[".repeat(dimensions)),
            format!("(){}Ljava/lang/Object;", "[".repeat(dimensions)),
        ] {
            assert_eq!(
                parse_method_descriptor(&descriptor).unwrap_err().code(),
                "invalid-descriptor"
            );
        }
    }
    for descriptor in [
        "", "()", "V", "([)V", "(V)V", "()[V", "(L;)I", "()L;", "(Lname)V", "()Iextra",
    ] {
        assert_eq!(
            parse_method_descriptor(descriptor).unwrap_err().code(),
            "invalid-descriptor",
            "{descriptor}"
        );
    }
}
