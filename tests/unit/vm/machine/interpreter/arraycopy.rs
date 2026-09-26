use super::*;
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn reference_copy_accepts_inheritance_interfaces_and_nested_arrays() {
    let mut program = Program::new();
    let mut parent = test_class_definition(Some("java/lang/Object"));
    parent.interfaces.push("Iface".into());
    program.classes.insert("Parent".into(), parent);
    program
        .classes
        .insert("Child".into(), test_class_definition(Some("Parent")));
    program.classes.insert(
        "Iface".into(),
        Class {
            is_interface: true,
            ..test_class_definition(None)
        },
    );
    for (source_type, destination_type, actual_type) in [
        ("java/lang/String", "java/lang/Object", "java/lang/String"),
        ("Child", "Parent", "Child"),
        ("Child", "Iface", "Child"),
        ("Parent", "Child", "Child"),
        ("[LChild;", "[LParent;", "[LChild;"),
        ("[LChild;", "[LIface;", "[LChild;"),
        ("[I", "java/lang/Object", "[I"),
        ("[[I", "[Ljava/lang/Object;", "[[I"),
    ] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let source = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Reference(source_type.into()), 6)
            .unwrap();
        let destination = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Reference(destination_type.into()), 7)
            .unwrap();
        let element = if let Some(component) = actual_type.strip_prefix('[') {
            machine
                .heap
                .managed
                .allocate_array(array_kind_from_descriptor(component).unwrap(), 0)
        } else {
            machine
                .heap
                .managed
                .allocate_object(actual_type, HashMap::new())
        }
        .unwrap();
        let value = Value::Reference(Some(element));
        let null = Value::Reference(None);
        let source_values = [value, value, null, value, value, value];
        for (index, value) in source_values.into_iter().enumerate() {
            machine
                .heap
                .managed
                .array_set(source, index as i32, value)
                .unwrap();
        }
        machine
            .system_arraycopy(&[
                Value::Reference(Some(source)),
                Value::Int(1),
                Value::Reference(Some(destination)),
                Value::Int(2),
                Value::Int(4),
            ])
            .unwrap();
        for (index, expected) in [null, null, value, null, value, value, null]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                machine
                    .heap
                    .managed
                    .array_get(destination, index as i32)
                    .unwrap(),
                expected,
                "{source_type}[] -> {destination_type}[], index={index}",
            );
        }
    }
}

#[test]
fn reference_copy_can_cancel_during_validation_and_be_retried() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let source = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Reference("java/lang/Object".into()), 8192)
        .unwrap();
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Reference("java/lang/String".into()), 8192)
        .unwrap();
    let text = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    for index in 0..8192 {
        machine
            .heap
            .managed
            .array_set(source, index, HeapValue::Reference(Some(text)))
            .unwrap();
    }
    let args = [
        Value::Reference(Some(source)),
        Value::Int(0),
        Value::Reference(Some(destination)),
        Value::Int(0),
        Value::Int(8192),
    ];
    machine.system_arraycopy(&args).unwrap();
    let total_checks = checks.get();
    assert!(
        total_checks > 1,
        "reference validation did not poll for cancellation"
    );
    for point in 1..=total_checks {
        for index in 0..8192 {
            machine
                .heap
                .managed
                .array_set(destination, index, HeapValue::Reference(None))
                .unwrap();
        }
        checks.set(0);
        cancel_at.set(point);
        assert_eq!(
            machine.system_arraycopy(&args).unwrap_err().code(),
            "execution-cancelled"
        );
        for index in 0..8192 {
            assert_eq!(
                machine.heap.managed.array_get(destination, index).unwrap(),
                HeapValue::Reference(None)
            );
            assert_eq!(
                machine.heap.managed.array_get(source, index).unwrap(),
                HeapValue::Reference(Some(text))
            );
        }
    }
    checks.set(0);
    cancel_at.set(usize::MAX);
    machine.system_arraycopy(&args).unwrap();
    for index in 0..8192 {
        assert_eq!(
            machine.heap.managed.array_get(destination, index).unwrap(),
            HeapValue::Reference(Some(text))
        );
    }
}

#[test]
fn reference_copy_preserves_offsets_nulls_and_every_failure_prefix() {
    for compatible_count in 0..=4 {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let source = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Reference("java/lang/Object".into()), 6)
            .unwrap();
        let destination = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Reference("java/lang/String".into()), 6)
            .unwrap();
        let text = machine
            .heap
            .managed
            .allocate_object("java/lang/String", HashMap::new())
            .unwrap();
        let sentinel = machine
            .heap
            .managed
            .allocate_object("java/lang/String", HashMap::new())
            .unwrap();
        let incompatible = machine
            .heap
            .managed
            .allocate_object("java/lang/Object", HashMap::new())
            .unwrap();
        let mut source_values = [HeapValue::Reference(Some(incompatible)); 6];
        for (index, value) in source_values[1..5].iter_mut().enumerate() {
            *value = if index == compatible_count {
                HeapValue::Reference(Some(incompatible))
            } else if index % 2 == 0 {
                HeapValue::Reference(None)
            } else {
                HeapValue::Reference(Some(text))
            };
        }
        let mut expected = [HeapValue::Reference(Some(sentinel)); 6];
        for index in 0..6 {
            machine
                .heap
                .managed
                .array_set(source, index as i32, source_values[index])
                .unwrap();
            machine
                .heap
                .managed
                .array_set(destination, index as i32, expected[index])
                .unwrap();
        }
        let copy = |source_position, destination_position, length| {
            [
                Value::Reference(Some(source)),
                Value::Int(source_position),
                Value::Reference(Some(destination)),
                Value::Int(destination_position),
                Value::Int(length),
            ]
        };
        // Range errors take precedence over checking source references and
        // leave the entire destination untouched.
        for args in [copy(1, 2, 5), copy(3, 2, 4), copy(-1, 2, 4)] {
            assert_eq!(
                machine.system_arraycopy(&args).unwrap_err().code(),
                "array-index-out-of-bounds-exception"
            );
            for (index, value) in expected.iter().enumerate() {
                assert_eq!(
                    machine
                        .heap
                        .managed
                        .array_get(destination, index as i32)
                        .unwrap(),
                    *value
                );
            }
        }
        machine.system_arraycopy(&copy(6, 6, 0)).unwrap();
        let result = machine.system_arraycopy(&copy(1, 2, 4));
        if compatible_count == 4 {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err().code(), "array-store-exception");
        }
        expected[2..2 + compatible_count].copy_from_slice(&source_values[1..][..compatible_count]);
        for index in 0..6 {
            assert_eq!(
                machine
                    .heap
                    .managed
                    .array_get(destination, index as i32)
                    .unwrap(),
                expected[index]
            );
            assert_eq!(
                machine
                    .heap
                    .managed
                    .array_get(source, index as i32)
                    .unwrap(),
                source_values[index]
            );
        }
    }
}
