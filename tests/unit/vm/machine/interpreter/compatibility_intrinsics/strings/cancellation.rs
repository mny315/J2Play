use super::*;
use std::cell::Cell;
use std::rc::Rc;

fn check_string_search_cancellation(long_needle: bool) {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut host = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let (text, needle, expected) = if long_needle {
        let needle = ("a".repeat(8_191) + "b").repeat(2);
        (needle.clone(), needle, 0)
    } else {
        ("a".repeat(32_768) + "b", "ab".to_owned(), 32_767)
    };
    let value = machine.allocate_dynamic_string(&text, &[], &[]).unwrap();
    let needle = machine
        .allocate_dynamic_string(&needle, &[], &[Value::Reference(Some(value))])
        .unwrap();
    checks.set(0);
    assert_eq!(machine.string_index_of(value, needle, 0).unwrap(), expected);
    let total_checks = checks.get();
    assert!(
        total_checks > 1,
        "String.indexOf did not poll during the search"
    );
    for point in 1..=total_checks {
        checks.set(0);
        cancel_at.set(point);
        assert_eq!(
            machine
                .string_index_of(value, needle, 0)
                .unwrap_err()
                .code(),
            "execution-cancelled"
        );
        assert_eq!(checks.get(), point);
        checks.set(0);
        cancel_at.set(usize::MAX);
        assert_eq!(machine.string_index_of(value, needle, 0).unwrap(), expected);
    }
}

#[test]
fn string_index_of_can_cancel_while_preparing_a_long_needle() {
    check_string_search_cancellation(true);
}

#[test]
fn string_index_of_can_cancel_while_scanning_a_long_value() {
    check_string_search_cancellation(false);
}

#[test]
fn string_scalar_operations_can_cancel_during_a_long_scan() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut host = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let mut strings = Vec::new();
    for last in [1, 2] {
        let string = machine
            .heap
            .managed
            .allocate_object("java/lang/String", HashMap::new())
            .unwrap();
        let mut units = vec![0; 16_385];
        units[16_384] = last;
        machine.heap.string_values.insert(string, units);
        strings.push(Value::Reference(Some(string)));
    }
    for (name, descriptor, args, expected) in [
        ("equals", "(Ljava/lang/Object;)Z", strings.clone(), 0),
        ("startsWith", "(Ljava/lang/String;)Z", strings.clone(), 0),
        ("compareTo", "(Ljava/lang/String;)I", strings.clone(), -1),
        ("hashCode", "()I", vec![strings[0]], 1),
        ("indexOf", "(I)I", vec![strings[0], Value::Int(1)], 16_384),
    ] {
        let method = runtime_method(
            "java/lang/String",
            name,
            descriptor,
            &[0x03, 0xac],
            1,
            2,
            vec![None],
            false,
        );
        let invoke = |machine: &mut Machine<'_, '_>| -> Result<i32, EmuError> {
            let Some(CallOutcome::Return(Some(Value::Int(value)))) =
                machine.invoke_compatibility_intrinsic(&method, &args, 1)?
            else {
                panic!("String.{name} did not return an integer");
            };
            Ok(value)
        };
        checks.set(0);
        assert_eq!(invoke(&mut machine).unwrap(), expected);
        let total_checks = checks.get();
        assert!(
            total_checks > 1,
            "String.{name} did not poll during its scan"
        );
        for point in 1..=total_checks {
            checks.set(0);
            cancel_at.set(point);
            assert_eq!(
                invoke(&mut machine).unwrap_err().code(),
                "execution-cancelled"
            );
            assert_eq!(checks.get(), point);
            checks.set(0);
            cancel_at.set(usize::MAX);
            assert_eq!(invoke(&mut machine).unwrap(), expected);
        }
    }
}

#[test]
fn integer_parse_int_can_cancel_while_reading_leading_zeroes() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut host = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let string = machine
        .allocate_dynamic_string(&("0".repeat(16_384) + "42"), &[], &[])
        .unwrap();
    checks.set(0);
    assert_eq!(machine.integer_parse_int(Some(string), 10).unwrap(), 42);
    let total_checks = checks.get();
    assert!(
        total_checks > 1,
        "Integer.parseInt did not poll while reading digits"
    );
    for point in 1..=total_checks {
        checks.set(0);
        cancel_at.set(point);
        assert_eq!(
            machine
                .integer_parse_int(Some(string), 10)
                .unwrap_err()
                .code(),
            "execution-cancelled"
        );
        assert_eq!(checks.get(), point);
        checks.set(0);
        cancel_at.set(usize::MAX);
        assert_eq!(machine.integer_parse_int(Some(string), 10).unwrap(), 42);
    }
}

#[test]
fn string_comparisons_can_cancel_before_a_late_difference() {
    for (name, ignore_case) in [
        ("equalsIgnoreCase", true),
        ("regionMatches", true),
        ("regionMatches", false),
    ] {
        let program = Program::new();
        let checks = Rc::new(Cell::new(0));
        let cancel_at = Rc::new(Cell::new(usize::MAX));
        let mut host = CancellationContext {
            checks: Rc::clone(&checks),
            cancel_at: Rc::clone(&cancel_at),
        };
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let left = machine
            .allocate_dynamic_string(&"Σ".repeat(8_192), &[], &[])
            .unwrap();
        let right = machine
            .allocate_dynamic_string(
                &if ignore_case { "ς" } else { "Σ" }.repeat(8_192),
                &[],
                &[Value::Reference(Some(left))],
            )
            .unwrap();
        let args = if name == "equalsIgnoreCase" {
            vec![Value::Reference(Some(left)), Value::Reference(Some(right))]
        } else {
            vec![
                Value::Reference(Some(left)),
                Value::Int(i32::from(ignore_case)),
                Value::Int(0),
                Value::Reference(Some(right)),
                Value::Int(0),
                Value::Int(8_192),
            ]
        };
        for expected in [true, false] {
            if !expected {
                let mut units = machine.heap.string_values.get(&right).unwrap().clone();
                units[8_191] = 0xd800;
                machine.heap.string_values.insert(right, units);
            }
            let invoke = |machine: &mut Machine<'_, '_>| {
                if name == "equalsIgnoreCase" {
                    machine.string_equals_ignore_case(left, Some(right))
                } else {
                    machine.string_region_matches(&args)
                }
            };
            checks.set(0);
            assert_eq!(invoke(&mut machine).unwrap(), expected, "{name}");
            let total_checks = checks.get();
            assert!(total_checks > 1, "{name} did not poll during comparison");
            for point in 1..=total_checks {
                checks.set(0);
                cancel_at.set(point);
                assert_eq!(
                    invoke(&mut machine).unwrap_err().code(),
                    "execution-cancelled"
                );
                assert_eq!(checks.get(), point);
                checks.set(0);
                cancel_at.set(usize::MAX);
                assert_eq!(invoke(&mut machine).unwrap(), expected, "{name}");
            }
        }
    }
}

#[test]
fn string_case_conversion_can_cancel_during_scan_and_conversion() {
    for uppercase in [false, true] {
        for first_change in [None, Some(0), Some(8_191)] {
            let program = Program::new();
            let checks = Rc::new(Cell::new(0));
            let cancel_at = Rc::new(Cell::new(usize::MAX));
            let mut host = CancellationContext {
                checks: Rc::clone(&checks),
                cancel_at: Rc::clone(&cancel_at),
            };
            let mut machine = program.machine(Limits::default(), false, &mut host);
            let (unchanged, changed) = if uppercase {
                ('Σ', 'σ')
            } else {
                ('σ', 'Σ')
            };
            let mut source = vec![unchanged as u16; 8_192];
            if let Some(first_change) = first_change {
                source[first_change..].fill(changed as u16);
            }
            source.extend([0, 0xd800, 0xdf]);
            let mut expected = vec![unchanged as u16; 8_192];
            expected.extend([0, 0xd800, 0xdf]);
            let string = machine
                .heap
                .managed
                .allocate_object("java/lang/String", HashMap::new())
                .unwrap();
            machine.heap.string_values.insert(string, source.clone());
            let invoke = |machine: &mut Machine<'_, '_>| {
                let roots = [Value::Reference(Some(string))];
                if uppercase {
                    machine.string_to_upper_case(string, &roots)
                } else {
                    machine.string_to_lower_case(string, &roots)
                }
            };
            checks.set(0);
            let converted = invoke(&mut machine).unwrap();
            assert_eq!(machine.heap.string_values.get(&converted), Some(&expected));
            assert_eq!(converted == string, first_change.is_none());
            let total_checks = checks.get();
            assert!(
                total_checks > 1,
                "case conversion did not poll during its work"
            );
            for point in 1..=total_checks {
                checks.set(0);
                cancel_at.set(point);
                let heap_bytes = machine.heap.managed.bytes();
                assert_eq!(
                    invoke(&mut machine).unwrap_err().code(),
                    "execution-cancelled"
                );
                assert_eq!(checks.get(), point);
                assert_eq!(machine.heap.managed.bytes(), heap_bytes);
                assert_eq!(machine.heap.string_values.get(&string), Some(&source));
                checks.set(0);
                cancel_at.set(usize::MAX);
                let converted = invoke(&mut machine).unwrap();
                assert_eq!(machine.heap.string_values.get(&converted), Some(&expected));
                assert_eq!(converted == string, first_change.is_none());
            }
        }
    }
}
