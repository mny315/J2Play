use super::*;

#[test]
fn repeated_heap_metadata_shares_storage_across_objects_and_arrays() {
    let mut heap = Heap::new(4096);
    let class = format!("fixture/{}", "LongName".repeat(4096));
    let field = format!("{class}.value:I");
    let linked_field: Arc<str> = field.clone().into();
    let mut objects = Vec::new();
    let mut arrays = Vec::new();
    for linked in [false, true] {
        for _ in 0..8 {
            let object = if linked {
                heap.allocate_object_linked(
                    &*class,
                    &mut vec![(linked_field.clone(), FieldToken::new(), HeapValue::Int(7))],
                )
            } else {
                heap.allocate_object(&*class, HashMap::from([(field.clone(), HeapValue::Int(7))]))
            }
            .unwrap();
            objects.push(object);
            arrays.push(
                heap.allocate_array(ArrayKind::Reference(class.clone().into()), 0)
                    .unwrap(),
            );
        }
    }
    let Allocation::Object {
        class: first_class,
        fields: first_fields,
    } = heap.get(objects[0]).unwrap()
    else {
        panic!("expected object");
    };
    for (index, object) in objects.iter().enumerate() {
        let Allocation::Object { class, fields } = heap.get(*object).unwrap() else {
            panic!("expected object");
        };
        assert_eq!(class.as_ptr(), first_class.as_ptr());
        let expected_field = if index < 8 {
            &first_fields.entries[0].0
        } else {
            &linked_field
        };
        assert!(Arc::ptr_eq(&fields.entries[0].0, expected_field));
        assert_eq!(fields.entries[0].2, HeapValue::Int(7));
    }
    for array in &arrays {
        let ArrayKind::Reference(component) = heap.array_kind(*array).unwrap() else {
            panic!("expected reference array");
        };
        assert_eq!(component.as_ptr(), first_class.as_ptr());
    }
    assert_eq!(heap.bytes(), 16 * (12 + 24));
    heap.collect([objects[0], arrays[0]]);
    assert_eq!(heap.len(), 2);
    assert_eq!(heap.field(objects[0], &field), Ok(HeapValue::Int(7)));
}

#[test]
fn checkpoint_restores_shared_metadata_without_changing_encoded_values() {
    let mut heap = Heap::new(4096);
    let objects: Vec<_> = (0..8)
        .map(|_| {
            heap.allocate_object(
                "Owner",
                HashMap::from([("Owner.value:I".into(), HeapValue::Int(7))]),
            )
            .unwrap()
        })
        .collect();
    let array = heap
        .allocate_array(ArrayKind::Reference("Owner".into()), 0)
        .unwrap();
    let bytes = save_state::encode(&heap).unwrap();
    let mut restored: Heap = save_state::decode(&bytes).unwrap();
    restored
        .validate_checkpoint(4096, |_, _, _| Some(FieldToken::new()))
        .unwrap();
    assert_eq!(save_state::encode(&restored).unwrap(), bytes);
    let Allocation::Object {
        class: first_class,
        fields: first_fields,
    } = restored.get(objects[0]).unwrap()
    else {
        panic!("expected object");
    };
    for handle in &objects {
        let Allocation::Object { class, fields } = restored.get(*handle).unwrap() else {
            panic!("expected object");
        };
        assert!(Arc::ptr_eq(class, first_class));
        assert!(Arc::ptr_eq(
            &fields.entries[0].0,
            &first_fields.entries[0].0
        ));
    }
    let ArrayKind::Reference(component) = restored.array_kind(array).unwrap() else {
        panic!("expected reference array");
    };
    assert!(Arc::ptr_eq(component, first_class));
}
