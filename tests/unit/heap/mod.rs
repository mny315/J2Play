use super::*;

mod metadata;

#[test]
fn collection_preserves_transitive_graph_and_rejects_stale_handle() {
    let mut heap = Heap::new(4096);
    let leaf = heap.allocate_object("Leaf", HashMap::new()).unwrap();
    let root = heap
        .allocate_object(
            "Root",
            HashMap::from([("child".into(), HeapValue::Reference(Some(leaf)))]),
        )
        .unwrap();
    let garbage = heap.allocate_object("Garbage", HashMap::new()).unwrap();
    assert_eq!(heap.len(), 3);
    assert_eq!(heap.collect([root]).objects, 1);
    assert_eq!(heap.len(), 2);
    assert!(heap.get(root).is_ok());
    assert!(heap.get(leaf).is_ok());
    assert_eq!(heap.get(garbage).unwrap_err(), HeapError::InvalidHandle);
    let replacement = heap.allocate_object("Replacement", HashMap::new()).unwrap();
    assert_eq!(heap.len(), 3);
    assert!(heap.get(replacement).is_ok());
}

#[test]
fn incremental_reachability_returns_only_the_new_frontier() {
    let mut heap = Heap::new(4096);
    let shared = heap.allocate_object("Shared", HashMap::new()).unwrap();
    let leaf = heap.allocate_object("Leaf", HashMap::new()).unwrap();
    let root = heap
        .allocate_object(
            "Root",
            HashMap::from([("shared".into(), HeapValue::Reference(Some(shared)))]),
        )
        .unwrap();
    let native_root = heap
        .allocate_object(
            "NativeRoot",
            HashMap::from([
                ("shared".into(), HeapValue::Reference(Some(shared))),
                ("leaf".into(), HeapValue::Reference(Some(leaf))),
            ]),
        )
        .unwrap();

    let mut marked = HashSet::new();
    let initial = heap
        .extend_reachable_handles(&mut marked, [root])
        .into_iter()
        .collect::<HashSet<_>>();
    assert_eq!(initial, HashSet::from([root, shared]));

    let frontier = heap
        .extend_reachable_handles(&mut marked, [native_root])
        .into_iter()
        .collect::<HashSet<_>>();
    assert_eq!(frontier, HashSet::from([native_root, leaf]));
    assert_eq!(marked, HashSet::from([root, shared, native_root, leaf]));
}

#[test]
fn collection_keeps_primitive_arrays_reached_through_nested_reference_arrays() {
    let mut heap = Heap::new(64 * 1024);
    let kinds = [
        ArrayKind::Boolean,
        ArrayKind::Byte,
        ArrayKind::Char,
        ArrayKind::Short,
        ArrayKind::Int,
        ArrayKind::Long,
        ArrayKind::Float,
        ArrayKind::Double,
    ];
    let arrays = kinds.map(|kind| heap.allocate_array(kind, 128).unwrap());
    let children = heap
        .allocate_array(ArrayKind::Reference("java/lang/Object".into()), 8)
        .unwrap();
    for (index, array) in arrays.iter().copied().enumerate() {
        heap.array_set(
            children,
            i32::try_from(index).unwrap(),
            HeapValue::Reference(Some(array)),
        )
        .unwrap();
    }
    let root = heap
        .allocate_array(ArrayKind::Reference("[Ljava/lang/Object;".into()), 1)
        .unwrap();
    heap.array_set(root, 0, HeapValue::Reference(Some(children)))
        .unwrap();
    let garbage = heap.allocate_array(ArrayKind::Byte, 128).unwrap();

    assert_eq!(heap.collect([root]).objects, 1);
    assert_eq!(heap.len(), 10);
    assert_eq!(heap.get(garbage).unwrap_err(), HeapError::InvalidHandle);
    for array in arrays {
        assert_eq!(heap.array_length(array).unwrap(), 128);
    }
    assert_eq!(heap.collect([]).objects, 10);
    assert!(heap.is_empty());
}

#[test]
fn collection_follows_external_edges_to_a_fixed_point_once_per_live_handle() {
    let mut heap = Heap::new(4096);
    let stale = heap.allocate_object("Stale", HashMap::new()).unwrap();
    heap.collect([]);
    let replacement = heap.allocate_object("Replacement", HashMap::new()).unwrap();
    let wrapper = heap.allocate_object("Wrapper", HashMap::new()).unwrap();
    let root = heap
        .allocate_object(
            "Root",
            HashMap::from([("wrapper".into(), HeapValue::Reference(Some(wrapper)))]),
        )
        .unwrap();
    let leaf = heap.allocate_array(ArrayKind::Byte, 128).unwrap();
    let bridge = heap
        .allocate_array(ArrayKind::Reference("[B".into()), 1)
        .unwrap();
    heap.array_set(bridge, 0, HeapValue::Reference(Some(leaf)))
        .unwrap();
    let unreachable = heap.allocate_object("Unreachable", HashMap::new()).unwrap();
    let orphan = heap.allocate_array(ArrayKind::Int, 16).unwrap();
    let external = HashMap::from([
        (wrapper, vec![bridge, bridge, stale]),
        (leaf, vec![root, wrapper]),
        (unreachable, vec![orphan]),
    ]);
    let mut visited = HashSet::new();
    let collected = heap.collect_with_external_edges([root, root], |frontier| {
        for handle in frontier {
            assert!(
                visited.insert(*handle),
                "external frontier was processed twice"
            );
        }
        frontier
            .iter()
            .filter_map(|handle| external.get(handle))
            .flatten()
            .copied()
            .collect::<Vec<_>>()
    });

    assert_eq!(collected.objects, 3);
    assert_eq!(visited, HashSet::from([root, wrapper, bridge, leaf]));
    assert_eq!(heap.len(), 4);
    for dead in [stale, replacement, unreachable, orphan] {
        assert_eq!(heap.get(dead).unwrap_err(), HeapError::InvalidHandle);
    }
    assert_eq!(heap.collect_with_external_edges([], |_| [root]).objects, 4);
    assert!(heap.is_empty());
}

#[test]
fn arrays_are_typed_bounded_and_counted() {
    let mut heap = Heap::new(4096);
    let array = heap.allocate_array(ArrayKind::Int, 3).unwrap();
    heap.array_set(array, 2, HeapValue::Int(9)).unwrap();
    assert_eq!(heap.array_get(array, 2).unwrap(), HeapValue::Int(9));
    assert_eq!(heap.array_get(array, 3).unwrap_err(), HeapError::Bounds);
    assert_eq!(
        heap.array_set(array, 0, HeapValue::Long(1)).unwrap_err(),
        HeapError::TypeMismatch
    );
}

#[test]
fn narrow_primitive_arrays_apply_jvm_store_conversion() {
    let mut heap = Heap::new(4096);
    for (kind, input, expected) in [
        (ArrayKind::Boolean, 2, 0),
        (ArrayKind::Byte, 255, -1),
        (ArrayKind::Char, -1, 65_535),
        (ArrayKind::Short, 65_535, -1),
    ] {
        let array = heap.allocate_array(kind, 1).unwrap();
        heap.array_set(array, 0, HeapValue::Int(input)).unwrap();
        assert_eq!(heap.array_get(array, 0).unwrap(), HeapValue::Int(expected));
    }
}

#[test]
fn typed_array_access_checks_family_without_changing_narrowing() {
    let mut heap = Heap::new(4096);
    let bytes = heap.allocate_array(ArrayKind::Byte, 1).unwrap();
    heap.array_set_typed(
        bytes,
        0,
        ArrayAccessKind::ByteOrBoolean,
        HeapValue::Int(255),
    )
    .unwrap();
    assert_eq!(
        heap.array_get_typed(bytes, 0, ArrayAccessKind::ByteOrBoolean),
        Ok(HeapValue::Int(-1))
    );
    assert_eq!(
        heap.array_get_typed(bytes, 0, ArrayAccessKind::Int),
        Err(HeapError::TypeMismatch)
    );
    assert_eq!(
        heap.array_set_typed(bytes, 1, ArrayAccessKind::ByteOrBoolean, HeapValue::Int(1)),
        Err(HeapError::Bounds)
    );

    heap.array_fill_typed(
        bytes,
        0,
        1,
        ArrayAccessKind::ByteOrBoolean,
        HeapValue::Int(128),
    )
    .unwrap();
    assert_eq!(heap.array_get(bytes, 0), Ok(HeapValue::Int(-128)));
    assert_eq!(
        heap.array_fill_typed(
            bytes,
            0,
            2,
            ArrayAccessKind::ByteOrBoolean,
            HeapValue::Int(1),
        ),
        Err(HeapError::Bounds)
    );
    assert_eq!(heap.array_get(bytes, 0), Ok(HeapValue::Int(-128)));
}

#[test]
fn oversized_array_is_rejected_before_host_allocation() {
    let mut heap = Heap::new(64);
    assert_eq!(
        heap.allocate_array(ArrayKind::Int, i32::MAX).unwrap_err(),
        HeapError::LimitExceeded
    );
}

#[test]
fn primitive_array_accounting_matches_cldc_element_widths() {
    let mut bytes = Heap::new(4096);
    bytes.allocate_array(ArrayKind::Byte, 100).unwrap();
    assert_eq!(bytes.bytes(), 124);

    let mut ints = Heap::new(4096);
    ints.allocate_array(ArrayKind::Int, 100).unwrap();
    assert_eq!(ints.bytes(), 424);
}

#[test]
fn allocation_limit_is_enforced() {
    let mut heap = Heap::new(4);
    assert_eq!(
        heap.allocate_object("X", HashMap::new()).unwrap_err(),
        HeapError::LimitExceeded
    );
}

#[test]
fn external_payload_is_bounded_replaced_and_collected() {
    let mut heap = Heap::new(40);
    let object = heap.allocate_object("X", HashMap::new()).unwrap();
    heap.set_external_bytes(object, 32).unwrap();
    assert_eq!(heap.bytes(), 40);
    assert_eq!(
        heap.set_external_bytes(object, 33).unwrap_err(),
        HeapError::LimitExceeded
    );
    assert_eq!(heap.bytes(), 40);
    heap.set_external_bytes(object, 4).unwrap();
    assert_eq!(heap.bytes(), 12);
    assert_eq!(heap.collect([]).bytes, 12);
    assert_eq!(heap.bytes(), 0);
}

#[test]
fn object_accounting_uses_32_bit_cldc_field_widths() {
    let mut heap = Heap::new(4096);
    heap.allocate_object(
        "Fields",
        HashMap::from([
            ("count:I".into(), HeapValue::Int(0)),
            ("peer:Ljava/lang/Object;".into(), HeapValue::Reference(None)),
            ("wide:J".into(), HeapValue::Long(0)),
        ]),
    )
    .unwrap();
    assert_eq!(heap.bytes(), 24);
}

#[test]
fn ordered_field_slots_keep_exact_name_fallback() {
    let mut heap = Heap::new(4096);
    let parent = FieldToken::new();
    let child = FieldToken::new();
    let object = heap
        .allocate_object_linked(
            "Child",
            &mut vec![
                ("Parent.value:I".into(), parent.clone(), HeapValue::Int(7)),
                ("Child.value:I".into(), child.clone(), HeapValue::Int(9)),
            ],
        )
        .unwrap();

    assert_eq!(
        heap.field_at(object, 0, &parent, "Parent.value:I").unwrap(),
        HeapValue::Int(7)
    );
    // A stale or defensive slot must not alias another legal field.
    assert_eq!(
        heap.field_at(object, 0, &child, "Child.value:I").unwrap(),
        HeapValue::Int(9)
    );
    heap.set_field_at(object, 0, &child, "Child.value:I", HeapValue::Int(11))
        .unwrap();
    assert_eq!(
        heap.field(object, "Parent.value:I").unwrap(),
        HeapValue::Int(7)
    );
    assert_eq!(
        heap.field(object, "Child.value:I").unwrap(),
        HeapValue::Int(11)
    );
}

#[test]
fn failed_linked_allocation_preserves_fields_for_collection_and_retry() {
    let mut heap = Heap::new(24);
    let child = heap.allocate_object("Child", HashMap::new()).unwrap();
    let discarded = heap.allocate_object("Discarded", HashMap::new()).unwrap();
    let mut fields = vec![
        (
            "Owner.child:Ljava/lang/Object;".into(),
            FieldToken::new(),
            HeapValue::Reference(Some(child)),
        ),
        ("Owner.count:I".into(), FieldToken::new(), HeapValue::Int(7)),
    ];

    assert_eq!(
        heap.allocate_object_linked("Owner", &mut fields),
        Err(HeapError::LimitExceeded)
    );
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].2, HeapValue::Reference(Some(child)));
    assert_eq!((heap.bytes(), heap.peak_bytes(), heap.len()), (16, 16, 2));

    assert_eq!(heap.collect([child]).objects, 1);
    assert!(heap.get(discarded).is_err());
    let owner = heap.allocate_object_linked("Owner", &mut fields).unwrap();
    assert!(fields.is_empty());
    assert_eq!(heap.collect([owner]).objects, 0);
    assert_eq!(heap.field(owner, "Owner.count:I"), Ok(HeapValue::Int(7)));
    assert_eq!(heap.bytes(), 24);
}

#[test]
fn allocation_summary_orders_equal_sizes_by_label() {
    let mut heap = Heap::new(4096);
    heap.allocate_object("Zulu", HashMap::new()).unwrap();
    heap.allocate_object("Alpha", HashMap::new()).unwrap();

    assert_eq!(
        heap.allocation_summary(),
        vec![("Alpha".into(), 1, 8), ("Zulu".into(), 1, 8)]
    );
}

#[test]
fn repeated_collection_preserves_a_cycle_without_growth() {
    let mut heap = Heap::new(4096);
    let left = heap
        .allocate_object(
            "Node",
            HashMap::from([("next".into(), HeapValue::Reference(None))]),
        )
        .unwrap();
    let right = heap
        .allocate_object(
            "Node",
            HashMap::from([("next".into(), HeapValue::Reference(Some(left)))]),
        )
        .unwrap();
    heap.set_field(left, "next", HeapValue::Reference(Some(right)))
        .unwrap();
    let stable_bytes = heap.bytes();
    for _ in 0..100 {
        let garbage = heap.allocate_object("Garbage", HashMap::new()).unwrap();
        assert!(heap.get(garbage).is_ok());
        assert_eq!(heap.collect([left]).objects, 1);
        assert_eq!(heap.bytes(), stable_bytes);
        assert!(heap.get(right).is_ok());
    }
}
#[test]
fn pair_mut_borrows_distinct_arrays_and_rejects_invalid_handles() {
    let mut heap = Heap::new(4096);
    let source = heap.allocate_array(ArrayKind::Int, 2).unwrap();
    let target = heap.allocate_array(ArrayKind::Int, 2).unwrap();
    {
        let (source_allocation, target_allocation) = heap.get_pair_mut(source, target).unwrap();
        let Allocation::Array {
            elements: source_elements,
            ..
        } = source_allocation
        else {
            panic!("source must be an array");
        };
        let Allocation::Array {
            elements: target_elements,
            ..
        } = target_allocation
        else {
            panic!("target must be an array");
        };
        source_elements[0] = HeapValue::Int(7);
        target_elements[1] = HeapValue::Int(9);
    }
    assert_eq!(heap.array_get(source, 0).unwrap(), HeapValue::Int(7));
    assert_eq!(heap.array_get(target, 1).unwrap(), HeapValue::Int(9));
    assert_eq!(
        heap.get_pair_mut(source, source).unwrap_err(),
        HeapError::TypeMismatch
    );
    assert_eq!(
        heap.get_pair_mut(source, Handle::from_raw(u64::MAX))
            .unwrap_err(),
        HeapError::InvalidHandle
    );
}
