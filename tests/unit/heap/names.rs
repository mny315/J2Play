use super::*;
use crate::{ArrayKind, FieldToken, Heap, HeapValue};

#[test]
fn metadata_limit_preserves_allocations_and_collection_reclaims_names() {
    let mut heap = Heap::new(4096);
    heap.names.limit = 2 * (NAME_OVERHEAD + 5);
    let first = heap
        .allocate_array(ArrayKind::Reference("First".into()), 0)
        .unwrap();
    let second = heap
        .allocate_array(ArrayKind::Reference("Other".into()), 0)
        .unwrap();
    assert_eq!(heap.names.bytes, heap.names.limit);
    let before = (heap.len(), heap.bytes(), heap.peak_bytes());
    assert_eq!(
        heap.allocate_array(ArrayKind::Reference("Third".into()), 0),
        Err(HeapError::MetadataLimitExceeded)
    );
    assert_eq!((heap.len(), heap.bytes(), heap.peak_bytes()), before);
    heap.collect([first]);
    assert!(heap.get(second).is_err());
    assert_eq!(heap.names.bytes, NAME_OVERHEAD + 5);
    heap.allocate_array(ArrayKind::Reference("Third".into()), 0)
        .unwrap();
    heap.collect([]);
    assert_eq!(heap.names.bytes, 0);
    assert!(heap.names.values.is_empty());
}

#[test]
fn metadata_limit_keeps_linked_fields_available_for_retry() {
    let mut heap = Heap::new(4096);
    heap.names.limit = NAME_OVERHEAD + 4;
    let mut fields = vec![("Owner.value:I".into(), FieldToken::new(), HeapValue::Int(7))];
    assert_eq!(
        heap.allocate_object_linked("Owner", &mut fields),
        Err(HeapError::MetadataLimitExceeded)
    );
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].2, HeapValue::Int(7));
    assert_eq!((heap.len(), heap.bytes(), heap.peak_bytes()), (0, 0, 0));
    heap.collect([]);
    assert_eq!(heap.names.bytes, 0);
    heap.names.limit = 1024;
    let owner = heap.allocate_object_linked("Owner", &mut fields).unwrap();
    assert!(fields.is_empty());
    assert_eq!(heap.field(owner, "Owner.value:I"), Ok(HeapValue::Int(7)));
}
