use super::*;

#[test]
fn checkpoint_validates_sparse_payloads_and_rejects_hidden_or_misplaced_entries() {
    let dense_handle = Handle::from_raw(2 << 32);
    let mut values = StringValues::from([
        (dense_handle, vec![65]),
        (Handle::from_raw(40_000 << 32), vec![0xd800, 0xdc00]),
    ]);
    let bytes = save_state::encode(&values).unwrap();
    let restored: StringValues = save_state::decode(&bytes).unwrap();
    let mut visited = HashMap::new();
    assert!(restored.validate_checkpoint(|handle, units| {
        visited.insert(handle, units.to_vec());
        true
    }));
    assert_eq!(visited.len(), 2);
    assert_eq!(visited[&dense_handle], [65]);
    assert_eq!(visited[&Handle::from_raw(40_000 << 32)], [0xd800, 0xdc00]);
    assert!(!restored.validate_checkpoint(|handle, _| handle != dense_handle));

    values.dense.swap(1, 2);
    assert!(!values.validate_checkpoint(|_, _| true));
    values.dense.swap(1, 2);
    values.overflow.insert(dense_handle, vec![66]);
    assert!(!values.validate_checkpoint(|_, _| true));
    values.overflow.remove(&dense_handle);
    values.dense.resize(DENSE_SLOTS + 1, None);
    assert!(!values.validate_checkpoint(|_, _| true));
}

#[test]
fn sparse_insertions_do_not_grow_dense_capacity_past_its_ceiling() {
    let mut strings = StringValues::new();
    for slot in [20_000_u64, 32_767, 32_768] {
        let handle = Handle::from_raw(slot << 32);
        strings.insert(handle, vec![65]);
        assert_eq!(strings.get(&handle), Some(&vec![65]));
        assert!(strings.dense.capacity() <= DENSE_SLOTS);
    }
}

#[test]
fn payload_slots_match_hash_map_across_gc_reuse_and_sparse_handles() {
    let mut actual = StringValues::new();
    let mut expected = HashMap::new();
    let handles = [
        0,
        1 << 32,
        (1 << 32) | 1,
        32_767 << 32,
        32_768 << 32,
        u64::MAX,
    ]
    .map(Handle::from_raw);
    for round in 0..8 {
        for (index, handle) in handles.into_iter().enumerate() {
            let units = vec![round, index as u16];
            assert_eq!(
                actual.insert(handle, units.clone()),
                expected.insert(handle, units)
            );
        }
        assert_eq!(actual.len(), expected.len());
        for handle in handles {
            assert_eq!(actual.get(&handle), expected.get(&handle));
        }
        actual.retain(|handle, units| {
            units.push(42);
            handle.to_raw() & 1 == u64::from(round & 1)
        });
        expected.retain(|handle, units| {
            units.push(42);
            handle.to_raw() & 1 == u64::from(round & 1)
        });
        assert_eq!(actual.len(), expected.len());
        for handle in handles {
            assert_eq!(actual.get(&handle), expected.get(&handle));
        }
    }
    assert!(actual.dense.len() <= DENSE_SLOTS);
    actual.retain(|_, _| false);
    assert!(actual.is_empty());
    for handle in handles {
        assert_eq!(actual.get(&handle), None);
    }
}
