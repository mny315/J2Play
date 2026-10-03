use super::*;
use std::{cell::Cell, hint::black_box, rc::Rc, time::Instant};

fn lookup_table(machine: &mut Machine<'_, '_>, handles: &[Handle]) -> Handle {
    let count = i32::try_from(handles.len()).unwrap();
    let keys = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Reference("java/lang/Object".into()), count)
        .unwrap();
    for (index, handle) in handles.iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(
                keys,
                i32::try_from(index).unwrap(),
                HeapValue::Reference(Some(*handle)),
            )
            .unwrap();
    }
    machine
        .heap
        .managed
        .allocate_object(
            "java/util/Hashtable",
            HashMap::from([
                (
                    "java/util/Hashtable.keys:[Ljava/lang/Object;".into(),
                    Value::Reference(Some(keys)),
                ),
                ("java/util/Hashtable.count:I".into(), Value::Int(count)),
            ]),
        )
        .unwrap()
}

fn lookup_key(machine: &mut Machine<'_, '_>, class: &str, number: i32) -> Handle {
    let descriptor = match class {
        "java/lang/Byte" => Some("java/lang/Byte.value:B"),
        "java/lang/Character" => Some("java/lang/Character.value:C"),
        "java/lang/Integer" => Some("java/lang/Integer.value:I"),
        "java/lang/Short" => Some("java/lang/Short.value:S"),
        _ => None,
    };
    let fields = descriptor.map_or_else(HashMap::new, |descriptor| {
        HashMap::from([(descriptor.into(), Value::Int(number))])
    });
    let key = machine.heap.managed.allocate_object(class, fields).unwrap();
    if class == "java/lang/String" {
        machine
            .heap
            .string_values
            .insert(key, format!("key-{number:08}").encode_utf16().collect());
    }
    key
}

#[test]
fn hashtable_lookup_keeps_key_classes_and_fallback_order() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for class in [
        "java/lang/Byte",
        "java/lang/Character",
        "java/lang/Integer",
        "java/lang/Short",
        "java/lang/String",
    ] {
        let different_class = lookup_key(&mut machine, "test/Key", 0);
        let first = lookup_key(&mut machine, class, 3);
        let last = lookup_key(&mut machine, class, 7);
        let query = lookup_key(&mut machine, class, 7);
        let absent = lookup_key(&mut machine, class, 9);
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 0)
            .unwrap();
        let table = lookup_table(&mut machine, &[different_class, array, first, last]);
        for (key, expected) in [
            (last, Some(3)),
            (query, Some(3)),
            (absent, Some(-1)),
            (different_class, Some(0)),
            (array, None),
        ] {
            assert_eq!(
                machine
                    .hashtable_find(&[Value::Reference(Some(table)), Value::Reference(Some(key))])
                    .unwrap(),
                expected,
                "{class}"
            );
        }
        let other_guest = lookup_key(&mut machine, "test/Key", 0);
        assert_eq!(
            machine
                .hashtable_find(&[
                    Value::Reference(Some(table)),
                    Value::Reference(Some(other_guest))
                ])
                .unwrap(),
            None
        );
        let array_table = lookup_table(&mut machine, &[array]);
        assert_eq!(
            machine
                .hashtable_find(&[
                    Value::Reference(Some(array_table)),
                    Value::Reference(Some(array)),
                ])
                .unwrap(),
            Some(0)
        );
    }
    let first = lookup_key(&mut machine, "test/Key", 1);
    let last = lookup_key(&mut machine, "test/Key", 2);
    let table = lookup_table(&mut machine, &[first, last]);
    assert_eq!(
        machine
            .hashtable_find(&[Value::Reference(Some(table)), Value::Reference(Some(last))])
            .unwrap(),
        None
    );
    let empty = lookup_table(&mut machine, &[]);
    assert_eq!(
        machine
            .hashtable_find(&[Value::Reference(Some(empty)), Value::Reference(Some(last))])
            .unwrap(),
        Some(-1)
    );
}

#[test]
fn hashtable_lookup_defers_payload_errors_until_comparison() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for class in ["java/lang/String", "java/lang/Integer"] {
        let malformed = machine
            .heap
            .managed
            .allocate_object(class, HashMap::new())
            .unwrap();
        let table = lookup_table(&mut machine, &[malformed]);
        assert_eq!(
            machine
                .hashtable_find(&[
                    Value::Reference(Some(table)),
                    Value::Reference(Some(malformed))
                ])
                .unwrap(),
            Some(0)
        );
        let empty = lookup_table(&mut machine, &[]);
        assert_eq!(
            machine
                .hashtable_find(&[
                    Value::Reference(Some(empty)),
                    Value::Reference(Some(malformed))
                ])
                .unwrap(),
            Some(-1)
        );
        let valid = lookup_key(&mut machine, class, 3);
        let comparison = lookup_table(&mut machine, &[valid]);
        assert!(
            machine
                .hashtable_find(&[
                    Value::Reference(Some(comparison)),
                    Value::Reference(Some(malformed))
                ])
                .is_err()
        );
        if class == "java/lang/Integer" {
            let different = lookup_key(&mut machine, "test/Key", 0);
            let table = lookup_table(&mut machine, &[different, malformed]);
            assert_eq!(
                machine
                    .hashtable_find(&[
                        Value::Reference(Some(table)),
                        Value::Reference(Some(malformed))
                    ])
                    .unwrap(),
                Some(1)
            );
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement for Hashtable lookups"]
fn hashtable_lookup_throughput() {
    let program = Program::new();
    for class in ["java/lang/String", "java/lang/Integer"] {
        for count in [1, 32, 512, 4096] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let keys: Vec<_> = (0..count)
                .map(|index| lookup_key(&mut machine, class, index))
                .collect();
            let table = lookup_table(&mut machine, &keys);
            let equal = lookup_key(&mut machine, class, count - 1);
            let missing = lookup_key(&mut machine, class, count);
            for (case, key, expected) in [
                ("same", keys[usize::try_from(count - 1).unwrap()], count - 1),
                ("equal", equal, count - 1),
                ("missing", missing, -1),
            ] {
                let args = [Value::Reference(Some(table)), Value::Reference(Some(key))];
                let iterations = 131_072 / count;
                let started = Instant::now();
                let mut sum = 0_i64;
                for _ in 0..iterations {
                    sum += i64::from(machine.hashtable_find(black_box(&args)).unwrap().unwrap());
                }
                let elapsed = started.elapsed();
                assert_eq!(sum, i64::from(expected) * i64::from(iterations));
                let kind = class.rsplit('/').next().unwrap();
                eprintln!("hashtable-{kind}-{count}-{case} elapsed={elapsed:?} sum={sum}");
            }
        }
    }
}

#[test]
fn hashtable_find_fastpath_preserves_standard_key_equality() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let first = machine
        .heap
        .managed
        .allocate_object(
            "java/lang/Character",
            HashMap::from([(
                "java/lang/Character.value:C".to_owned(),
                HeapValue::Int('a' as i32),
            )]),
        )
        .unwrap();
    let second = machine
        .heap
        .managed
        .allocate_object(
            "java/lang/Character",
            HashMap::from([(
                "java/lang/Character.value:C".to_owned(),
                HeapValue::Int('b' as i32),
            )]),
        )
        .unwrap();
    let equal_but_distinct = machine
        .heap
        .managed
        .allocate_object(
            "java/lang/Character",
            HashMap::from([(
                "java/lang/Character.value:C".to_owned(),
                HeapValue::Int('b' as i32),
            )]),
        )
        .unwrap();
    let keys = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Reference("java/lang/Object".into()), 3)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(keys, 0, HeapValue::Reference(Some(first)))
        .unwrap();
    machine
        .heap
        .managed
        .array_set(keys, 1, HeapValue::Reference(Some(second)))
        .unwrap();
    let table = machine
        .heap
        .managed
        .allocate_object(
            "java/util/Hashtable",
            HashMap::from([
                (
                    "java/util/Hashtable.keys:[Ljava/lang/Object;".to_owned(),
                    HeapValue::Reference(Some(keys)),
                ),
                ("java/util/Hashtable.count:I".to_owned(), HeapValue::Int(2)),
            ]),
        )
        .unwrap();

    assert_eq!(
        machine
            .hashtable_find(&[
                Value::Reference(Some(table)),
                Value::Reference(Some(equal_but_distinct)),
            ])
            .unwrap(),
        Some(1)
    );
    assert_eq!(
        machine
            .hashtable_find(&[Value::Reference(Some(table)), Value::Reference(None)])
            .unwrap(),
        None
    );
}

fn check_hashtable_search_cancellation(long_string: bool) {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut host = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let count = if long_string { 1 } else { 4096 };
    let keys = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Reference("java/lang/Object".into()), count)
        .unwrap();
    let mut query = None;
    // The query equals the final key but is a distinct object, so a long
    // String must be compared even when the table has only one entry.
    for index in 0..=count {
        let handle = if long_string {
            let handle = machine
                .heap
                .managed
                .allocate_object("java/lang/String", HashMap::new())
                .unwrap();
            machine
                .heap
                .string_values
                .insert(handle, vec![u16::from(b'a'); 16_384]);
            handle
        } else {
            machine
                .heap
                .managed
                .allocate_object(
                    "java/lang/Integer",
                    HashMap::from([(
                        "java/lang/Integer.value:I".into(),
                        Value::Int(index.min(count - 1)),
                    )]),
                )
                .unwrap()
        };
        if index == count {
            query = Some(handle);
        } else {
            machine
                .heap
                .managed
                .array_set(keys, index, Value::Reference(Some(handle)))
                .unwrap();
        }
    }
    let table = machine
        .heap
        .managed
        .allocate_object(
            "java/util/Hashtable",
            HashMap::from([
                (
                    "java/util/Hashtable.keys:[Ljava/lang/Object;".into(),
                    Value::Reference(Some(keys)),
                ),
                ("java/util/Hashtable.count:I".into(), Value::Int(count)),
            ]),
        )
        .unwrap();
    let args = [Value::Reference(Some(table)), Value::Reference(query)];
    assert_eq!(machine.hashtable_find(&args).unwrap(), Some(count - 1));
    let total_checks = checks.get();
    assert!(
        total_checks > 1,
        "Hashtable search did not poll during its work"
    );
    for point in 1..=total_checks {
        checks.set(0);
        cancel_at.set(point);
        assert_eq!(
            machine.hashtable_find(&args).unwrap_err().code(),
            "execution-cancelled"
        );
        assert_eq!(checks.get(), point);
        checks.set(0);
        cancel_at.set(usize::MAX);
        assert_eq!(machine.hashtable_find(&args).unwrap(), Some(count - 1));
    }
    if long_string {
        let query = query.unwrap();
        // A difference after a long shared prefix, and a strict prefix of
        // the stored key, must both remain misses after the chunked scan.
        let mut different = vec![u16::from(b'a'); 16_384];
        *different.last_mut().unwrap() = 0xd800;
        machine.heap.string_values.insert(query, different);
        assert_eq!(machine.hashtable_find(&args).unwrap(), Some(-1));
        machine
            .heap
            .string_values
            .insert(query, vec![u16::from(b'a'); 16_383]);
        assert_eq!(machine.hashtable_find(&args).unwrap(), Some(-1));
    }
}

#[test]
fn hashtable_search_can_cancel_during_a_large_key_scan() {
    check_hashtable_search_cancellation(false);
}

#[test]
fn hashtable_search_can_cancel_inside_one_long_string_comparison() {
    check_hashtable_search_cancellation(true);
}
