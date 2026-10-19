use super::*;

#[test]
fn observation_flags_survive_full_filter_capacity_and_reset() {
    let mut heap = Heap::new(4096);
    let array = heap.allocate_array(heap::ArrayKind::Int, 128).unwrap();
    let mut observation = Observation::default();
    observation.mark_unsupported();
    for index in 0..64 {
        observation.record(Read::Array(
            array,
            index,
            ArrayAccessKind::Int,
            Value::Int(0),
        ));
    }
    assert_eq!(observation.reads.len(), MAX_READS);
    assert!(!observation.overflowed());
    assert!(observation.unsupported());
    observation.record(Read::Array(array, 64, ArrayAccessKind::Int, Value::Int(0)));
    assert!(observation.overflowed());
    assert!(observation.unsupported());
    observation.reset();
    assert!(!observation.overflowed());
    assert!(!observation.unsupported());
    assert!(observation.reads.is_empty());
    for _ in 0..3 {
        for index in 60..64 {
            observation.record(Read::Array(
                array,
                index,
                ArrayAccessKind::Int,
                Value::Int(0),
            ));
        }
    }
    assert_eq!(observation.reads.len(), 4);
    assert!(!observation.overflowed());
    assert!(!observation.unsupported());
}

#[test]
fn cached_array_reads_validate_every_value_kind_bound_and_generation() {
    let mut method = crate::machine::tests::runtime_method(
        "Probe",
        "identity",
        "(I)I",
        &[0x1a, 0xac],
        1,
        1,
        vec![None],
        true,
    );
    method.stack_key_id = Some(0);
    let classes = ClassState::default();
    for (kind, access, expected, changed) in [
        (
            heap::ArrayKind::Boolean,
            ArrayAccessKind::ByteOrBoolean,
            Value::Int(0),
            Value::Int(1),
        ),
        (
            heap::ArrayKind::Byte,
            ArrayAccessKind::ByteOrBoolean,
            Value::Int(-7),
            Value::Int(8),
        ),
        (
            heap::ArrayKind::Char,
            ArrayAccessKind::Char,
            Value::Int(0x1234),
            Value::Int(0x2345),
        ),
        (
            heap::ArrayKind::Short,
            ArrayAccessKind::Short,
            Value::Int(-1234),
            Value::Int(1234),
        ),
        (
            heap::ArrayKind::Int,
            ArrayAccessKind::Int,
            Value::Int(i32::MIN),
            Value::Int(i32::MAX),
        ),
        (
            heap::ArrayKind::Long,
            ArrayAccessKind::Long,
            Value::Long(i64::MIN),
            Value::Long(i64::MAX),
        ),
        (
            heap::ArrayKind::Float,
            ArrayAccessKind::Float,
            Value::Float(-0.0),
            Value::Float(0.0),
        ),
        (
            heap::ArrayKind::Double,
            ArrayAccessKind::Double,
            Value::Double(f64::from_bits(0x7ff8_0000_0000_0001)),
            Value::Double(f64::from_bits(0x7ff8_0000_0000_0002)),
        ),
        (
            heap::ArrayKind::Reference("Ljava/lang/Object;".into()),
            ArrayAccessKind::Reference,
            Value::Reference(None),
            Value::Reference(None),
        ),
    ] {
        let mut heap = Heap::new(4096);
        let changed = if access == ArrayAccessKind::Reference {
            Value::Reference(Some(
                heap.allocate_object("Value", std::collections::HashMap::new())
                    .unwrap(),
            ))
        } else {
            changed
        };
        let first = heap.allocate_array(kind.clone(), 3).unwrap();
        let second = heap.allocate_array(kind, 3).unwrap();
        for array in [first, second] {
            for index in 0..3 {
                heap.array_set_typed(array, index, access, expected)
                    .unwrap();
            }
        }
        let observe = || {
            let mut observation = Observation::default();
            for (array, index) in [
                (first, 0),
                (first, 1),
                (second, 0),
                (first, 2),
                (second, 1),
                (second, 2),
            ] {
                observation.record(Read::Array(array, index, access, expected));
                if array == second && index == 1 {
                    observation.record(Read::Length(second, 3));
                }
            }
            observation
        };
        let mut observation = observe();
        let mut entry = Entry::default();
        entry.finish(&method, &[Value::Int(7)], [0; 4], 42, 2, &mut observation);
        assert_eq!(
            entry.lookup(&method, &[Value::Int(7)], [0; 4], &heap, &classes),
            Some((42, 2))
        );
        for (array, index) in [(first, 1), (second, 2)] {
            heap.array_set_typed(array, index, access, changed).unwrap();
            assert_eq!(
                entry.lookup(&method, &[Value::Int(7)], [0; 4], &heap, &classes),
                None
            );
            heap.array_set_typed(array, index, access, expected)
                .unwrap();
            assert_eq!(
                entry.lookup(&method, &[Value::Int(7)], [0; 4], &heap, &classes),
                Some((42, 2))
            );
        }
        let wrong = if access == ArrayAccessKind::Int {
            ArrayAccessKind::Long
        } else {
            ArrayAccessKind::Int
        };
        for (index, next_access) in [(-1, access), (3, access), (1, wrong)] {
            observation.reset();
            observation.record(Read::Array(first, 0, access, expected));
            observation.record(Read::Array(first, index, next_access, expected));
            entry.finish(&method, &[Value::Int(7)], [0; 4], 42, 2, &mut observation);
            assert_eq!(
                entry.lookup(&method, &[Value::Int(7)], [0; 4], &heap, &classes),
                None
            );
        }
        observation = observe();
        entry.finish(&method, &[Value::Int(7)], [0; 4], 42, 2, &mut observation);
        heap.collect([]);
        heap.allocate_array(heap::ArrayKind::Int, 3).unwrap();
        assert_eq!(
            entry.lookup(&method, &[Value::Int(7)], [0; 4], &heap, &classes),
            None
        );
    }
}

#[test]
fn colliding_observations_keep_distinct_values_and_exact_capacity() {
    eprintln!(
        "leaf-cache layout observation={} entry={}",
        std::mem::size_of::<Observation>(),
        std::mem::size_of::<Entry>()
    );
    let mut heap = Heap::new(32_768);
    let array = heap.allocate_array(heap::ArrayKind::Int, 4097).unwrap();
    let mut observation = Observation::default();
    for _ in 0..3 {
        for index in 0..MAX_READS {
            observation.record(Read::Array(
                array,
                (index * 64) as i32,
                ArrayAccessKind::Int,
                Value::Int(0),
            ));
        }
    }
    assert_eq!(observation.reads.len(), MAX_READS);
    assert!(!observation.overflowed());
    observation.record(Read::Array(
        array,
        4096,
        ArrayAccessKind::Int,
        Value::Int(0),
    ));
    assert_eq!(observation.reads.len(), MAX_READS);
    assert!(observation.overflowed());

    observation.reset();
    for value in [0, 1, 0, 1] {
        observation.record(Read::Array(
            array,
            0,
            ArrayAccessKind::Int,
            Value::Int(value),
        ));
    }
    assert_eq!(observation.reads.len(), 2);
    assert!(!observation.overflowed());
    let classes = ClassState::default();
    assert!(observation.reads[0].unchanged(&heap, &classes));
    assert!(!observation.reads[1].unchanged(&heap, &classes));
}

#[test]
fn oversized_observation_preserves_a_valid_entry_until_a_complete_replacement() {
    let mut method = crate::machine::tests::runtime_method(
        "Probe",
        "identity",
        "(I)I",
        &[0x1a, 0xac],
        1,
        1,
        vec![None],
        true,
    );
    method.stack_key_id = Some(0);
    let mut heap = Heap::new(4096);
    let array = heap.allocate_array(heap::ArrayKind::Int, 128).unwrap();
    let classes = ClassState::default();
    let mut entry = Entry::default();
    let mut observation = Observation::default();
    observation.record(Read::Length(array, 128));
    entry.finish(&method, &[Value::Int(7)], [0; 4], 7, 2, &mut observation);

    for index in 0..=MAX_READS as i32 {
        observation.record(Read::Array(
            array,
            index,
            ArrayAccessKind::Int,
            Value::Int(0),
        ));
    }
    assert!(observation.overflowed());
    entry.finish(&method, &[Value::Int(9)], [0; 4], 9, 2, &mut observation);
    assert_eq!(
        entry.lookup(&method, &[Value::Int(7)], [0; 4], &heap, &classes),
        Some((7, 2))
    );
    assert_eq!(
        entry.lookup(&method, &[Value::Int(9)], [0; 4], &heap, &classes),
        None
    );

    observation.reset();
    observation.record(Read::Length(array, 128));
    entry.finish(&method, &[Value::Int(9)], [0; 4], 9, 2, &mut observation);
    assert_eq!(
        entry.lookup(&method, &[Value::Int(9)], [0; 4], &heap, &classes),
        Some((9, 2))
    );
    heap.collect([]);
    assert_eq!(
        entry.lookup(&method, &[Value::Int(9)], [0; 4], &heap, &classes),
        None
    );
}

#[test]
fn observations_deduplicate_exact_reads_and_reject_stale_handles() {
    let mut heap = Heap::new(4096);
    let array = heap.allocate_array(heap::ArrayKind::Int, 128).unwrap();
    let mut observation = Observation::default();
    for _ in 0..100 {
        observation.record(Read::Array(array, 0, ArrayAccessKind::Int, Value::Int(0)));
    }
    assert_eq!(observation.reads.len(), 1);
    assert!(!observation.overflowed());
    let classes = ClassState::default();
    assert!(observation.reads[0].unchanged(&heap, &classes));
    heap.array_set_typed(array, 0, ArrayAccessKind::Int, heap::HeapValue::Int(1))
        .unwrap();
    assert!(!observation.reads[0].unchanged(&heap, &classes));
    for index in 1..=64 {
        observation.record(Read::Array(
            array,
            index,
            ArrayAccessKind::Int,
            Value::Int(0),
        ));
    }
    assert_eq!(observation.reads.len(), MAX_READS);
    assert!(observation.overflowed());
    heap.collect([]);
    let replacement = heap.allocate_array(heap::ArrayKind::Int, 128).unwrap();
    assert_ne!(replacement, array);
    assert!(!observation.reads[1].unchanged(&heap, &classes));
    assert!(!same(Value::Float(0.0), Value::Float(-0.0)));
    assert!(same(
        Value::Float(f32::from_bits(0x7fc0_0001)),
        Value::Float(f32::from_bits(0x7fc0_0001))
    ));
}
#[test]
fn observed_batch_records_instance_static_and_length_dependencies() {
    let mut method = crate::machine::tests::runtime_method(
        "Probe",
        "read",
        "(LProbe;[I)I",
        &[0x2a, 0xb4, 0, 1, 0xb2, 0, 2, 0x60, 0x2b, 0xbe, 0x60, 0xac],
        2,
        2,
        vec![None],
        true,
    );
    method.constant_pool_id = Some(0);
    let mut heap = Heap::new(4096);
    let field = Rc::new(Field {
        key: "Probe.value:I".into(),
        declaring_class: "Probe".into(),
        kind: super::super::ValueKind::Int,
        is_static: false,
        field_token: heap::FieldToken::new(),
        instance_slot: Some(0),
        initial: Value::Int(0),
        constant_string: None,
    });
    let object = heap
        .allocate_object_linked(
            "Probe",
            &mut vec![(
                field.key.clone(),
                field.field_token.clone(),
                heap::HeapValue::Int(7),
            )],
        )
        .unwrap();
    let array = heap.allocate_array(heap::ArrayKind::Int, 3).unwrap();
    let mut classes = ClassState::default();
    classes.initialized.linked.push(true);
    classes.static_fields.linked.push(Some(Value::Int(11)));
    classes.field_inline_cache.insert_resolved(
        0,
        1,
        field.clone(),
        super::super::FieldRuntimeSlots::default(),
    );
    let mut static_field = (*field).clone();
    static_field.is_static = true;
    static_field.instance_slot = None;
    classes.field_inline_cache.insert_resolved(
        0,
        2,
        Rc::new(static_field),
        super::super::FieldRuntimeSlots {
            static_field: Some(0),
            declaring_class: Some(0),
        },
    );
    let mut locals = [
        Some(Value::Reference(Some(object))),
        Some(Value::Reference(Some(array))),
    ];
    let mut stack = Vec::new();
    let mut observation = Observation::default();
    let result = super::super::interpreter_batch::execute_inner::<true, true>(
        &method,
        &mut locals,
        &mut stack,
        0,
        0,
        2,
        100,
        &mut heap,
        &mut classes,
        &mut observation,
        None,
    );
    assert!(result.returned());
    assert_eq!(stack, [Value::Int(21)]);
    assert_eq!(observation.reads.len(), 3);
    assert!(
        observation
            .reads
            .iter()
            .all(|read| read.unchanged(&heap, &classes))
    );
    heap.set_field_at(
        object,
        0,
        &field.field_token,
        &field.key,
        heap::HeapValue::Int(8),
    )
    .unwrap();
    assert!(!observation.reads[0].unchanged(&heap, &classes));
    classes.static_fields.linked[0] = Some(Value::Int(12));
    assert!(!observation.reads[1].unchanged(&heap, &classes));
    classes.static_fields.linked[0] = Some(Value::Int(11));
    classes.initialized.linked[0] = false;
    assert!(!observation.reads[1].unchanged(&heap, &classes));
    heap.collect([object]);
    assert!(!observation.reads[2].unchanged(&heap, &classes));
}
