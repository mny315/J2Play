use super::*;

#[test]
fn checkpoint_rejects_integers_outside_the_array_component_width() {
    for (kind, invalid) in [
        (ArrayKind::Boolean, [-1, 2]),
        (ArrayKind::Byte, [-129, 128]),
        (ArrayKind::Char, [-1, 65_536]),
        (ArrayKind::Short, [-32_769, 32_768]),
    ] {
        for value in invalid {
            let mut heap = Heap::new(4096);
            let handle = heap.allocate_array(kind.clone(), 1).unwrap();
            let Allocation::Array { elements, .. } = heap.get_mut(handle).unwrap() else {
                panic!("allocated an array");
            };
            elements[0] = HeapValue::Int(value);
            let bytes = save_state::encode(&heap).unwrap();
            let mut restored: Heap = save_state::decode(&bytes).unwrap();
            assert_eq!(
                restored.validate_checkpoint(4096, |_, _, _| None),
                Err(HeapError::TypeMismatch),
                "{kind:?}: {value}"
            );
        }
    }
}

#[test]
fn checkpoint_preserves_valid_array_values_including_float_bit_patterns() {
    for (kind, values) in [
        (
            ArrayKind::Boolean,
            vec![HeapValue::Int(0), HeapValue::Int(1)],
        ),
        (
            ArrayKind::Byte,
            vec![HeapValue::Int(-128), HeapValue::Int(127)],
        ),
        (
            ArrayKind::Char,
            vec![HeapValue::Int(0), HeapValue::Int(65_535)],
        ),
        (
            ArrayKind::Short,
            vec![HeapValue::Int(-32_768), HeapValue::Int(32_767)],
        ),
        (
            ArrayKind::Int,
            vec![HeapValue::Int(i32::MIN), HeapValue::Int(i32::MAX)],
        ),
        (
            ArrayKind::Long,
            vec![HeapValue::Long(i64::MIN), HeapValue::Long(i64::MAX)],
        ),
        (
            ArrayKind::Float,
            vec![
                HeapValue::Float(f32::from_bits(0x7fc0_1234)),
                HeapValue::Float(-0.0),
                HeapValue::Float(f32::INFINITY),
            ],
        ),
        (
            ArrayKind::Double,
            vec![
                HeapValue::Double(f64::from_bits(0x7ff8_0000_0000_1234)),
                HeapValue::Double(-0.0),
                HeapValue::Double(f64::NEG_INFINITY),
            ],
        ),
        (
            ArrayKind::Reference("java/lang/Object".into()),
            vec![HeapValue::Reference(None)],
        ),
    ] {
        let mut heap = Heap::new(4096);
        let handle = heap
            .allocate_array(kind, i32::try_from(values.len()).unwrap())
            .unwrap();
        for (index, value) in values.into_iter().enumerate() {
            heap.array_set(handle, i32::try_from(index).unwrap(), value)
                .unwrap();
        }
        let bytes = save_state::encode(&heap).unwrap();
        let mut restored: Heap = save_state::decode(&bytes).unwrap();
        restored.validate_checkpoint(4096, |_, _, _| None).unwrap();
        assert_eq!(save_state::encode(&restored).unwrap(), bytes);
    }
}

#[test]
fn checkpoint_preserves_handles_free_slots_and_external_charges() {
    let mut heap = Heap::new(4096);
    let retained = heap.allocate_array(ArrayKind::Byte, 10).unwrap();
    let discarded = heap.allocate_array(ArrayKind::Int, 2).unwrap();
    heap.collect([retained]);
    heap.set_external_bytes(retained, 20).unwrap();
    let bytes = save_state::encode(&heap).unwrap();
    let mut restored: Heap = save_state::decode(&bytes).unwrap();
    restored.validate_checkpoint(4096, |_, _, _| None).unwrap();
    assert!(restored.get(retained).is_ok());
    assert!(restored.get(discarded).is_err());
    let next = restored.allocate_array(ArrayKind::Int, 2).unwrap();
    assert_ne!(next, discarded);
    assert_eq!(restored.bytes(), heap.bytes() + 32);
}

#[test]
fn checkpoint_rejects_forged_free_list_and_profile_limit() {
    let mut heap = Heap::new(4096);
    let retained = heap.allocate_array(ArrayKind::Byte, 1).unwrap();
    let discarded = heap.allocate_array(ArrayKind::Byte, 1).unwrap();
    heap.collect([retained]);
    assert!(heap.validate_checkpoint(2048, |_, _, _| None).is_err());
    for free in [
        vec![],
        vec![retained.slot],
        vec![discarded.slot, discarded.slot],
        vec![u32::MAX],
    ] {
        heap.free = free;
        assert!(heap.validate_checkpoint(4096, |_, _, _| None).is_err());
    }
    heap.free = vec![discarded.slot];
    heap.slots[discarded.slot as usize].generation = 0;
    assert!(heap.validate_checkpoint(4096, |_, _, _| None).is_err());
}

#[test]
fn checkpoint_distinguishes_last_reusable_generation_from_retired_slots() {
    for generation in [u32::MAX - 1, u32::MAX] {
        let mut heap = Heap::new(4096);
        let discarded = heap.allocate_array(ArrayKind::Byte, 1).unwrap();
        heap.slots[discarded.slot as usize].generation = generation;
        heap.collect([]);
        let bytes = save_state::encode(&heap).unwrap();
        let mut restored: Heap = save_state::decode(&bytes).unwrap();
        restored.validate_checkpoint(4096, |_, _, _| None).unwrap();
        let next = restored.allocate_array(ArrayKind::Byte, 1).unwrap();
        assert_eq!(next.slot == discarded.slot, generation < u32::MAX);
        assert!(restored.get(discarded).is_err());
    }
}
