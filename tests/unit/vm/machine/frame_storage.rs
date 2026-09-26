use super::*;

#[test]
fn frame_storage_pool_reuses_cleared_vectors_with_hard_bounds() {
    let mut pool = FrameStoragePool::default();
    let mut locals = Vec::with_capacity(16);
    locals.resize(8, Some(Value::Int(7)));
    let allocation = locals.as_ptr();
    pool.put_locals(locals);

    let reused = pool.take_locals(6);
    assert_eq!(reused.as_ptr(), allocation);
    assert_eq!(reused, vec![None; 6]);
    assert_eq!(pool.retained_slots, 0);

    pool.put_operand_stack(Vec::with_capacity(2));
    let larger = pool.take_operand_stack(4);
    assert!(larger.capacity() >= 4);
    assert!(larger.is_empty());

    pool.put_operand_stack(Vec::with_capacity(
        FRAME_STORAGE_POOL_MAX_SLOTS.saturating_add(1),
    ));
    assert!(pool.operand_stacks.is_empty());
    assert_eq!(pool.retained_slots, 0);

    pool.put_locals(Vec::new());
    pool.put_operand_stack(Vec::new());
    assert!(pool.locals.is_empty());
    assert!(pool.operand_stacks.is_empty());

    for _ in 0..=FRAME_STORAGE_POOL_MAX_VECTORS {
        pool.put_locals(Vec::with_capacity(1));
    }
    assert_eq!(pool.locals.len(), FRAME_STORAGE_POOL_MAX_VECTORS);
}
