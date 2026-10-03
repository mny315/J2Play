use super::*;

#[test]
fn removed_handle_never_aliases_reused_slot() {
    let mut arena = Arena::new(ArenaLimits {
        objects: 2,
        bytes: 16,
    });
    let first = arena.insert(7_u32, 4).unwrap();
    assert_eq!(arena.remove(first).unwrap(), 7);
    let second = arena.insert(9, 4).unwrap();
    assert_eq!(first.index(), second.index());
    assert_ne!(first.generation(), second.generation());
    assert_eq!(arena.get(first).unwrap_err().code(), "stale-handle");
    assert_eq!(*arena.get(second).unwrap(), 9);
}

#[test]
fn exhausted_generations_remain_retired_after_teardown() {
    for clear_live in [false, true] {
        let mut arena = Arena::new(ArenaLimits {
            objects: 1,
            bytes: 4,
        });
        let first = arena.insert(7_u32, 4).unwrap();
        arena.slots[0].generation = u32::MAX;
        let last = Handle {
            index: 0,
            generation: u32::MAX,
        };
        if !clear_live {
            arena.remove(last).unwrap();
        }
        arena.clear();
        assert_eq!(arena.insert(9, 4).unwrap_err().code(), "resource-limit");
        assert_eq!(arena.get(first).unwrap_err().code(), "stale-handle");
        assert_eq!(arena.get(last).unwrap_err().code(), "stale-handle");
        assert_eq!(arena.live_objects(), 0);
    }
}

#[test]
fn enforces_object_and_byte_limits_before_mutation() {
    let mut arena = Arena::new(ArenaLimits {
        objects: 1,
        bytes: 4,
    });
    let handle = arena.insert(1_u8, 4).unwrap();
    assert_eq!(arena.insert(2, 0).unwrap_err().code(), "resource-limit");
    assert_eq!(arena.live_objects(), 1);
    assert_eq!(arena.live_bytes(), 4);
    arena.remove(handle).unwrap();
    assert_eq!(arena.insert(3, 5).unwrap_err().code(), "resource-limit");
    assert_eq!(arena.live_objects(), 0);
}

#[test]
fn teardown_invalidates_handles_and_retains_peaks() {
    let mut arena = Arena::new(ArenaLimits::default());
    let handle = arena.insert(vec![1_u8; 32], 32).unwrap();
    arena.clear();
    assert_eq!(arena.get(handle).unwrap_err().code(), "stale-handle");
    assert_eq!(arena.live_objects(), 0);
    assert_eq!(arena.live_bytes(), 0);
    assert_eq!(arena.peak_objects(), 1);
    assert_eq!(arena.peak_bytes(), 32);
}

#[test]
fn failed_replacement_preserves_value_and_accounting() {
    let mut arena = Arena::new(ArenaLimits {
        objects: 1,
        bytes: 8,
    });
    let handle = arena.insert(vec![1_u8; 4], 4).unwrap();

    assert_eq!(
        arena.replace(handle, vec![2_u8; 9], 9).unwrap_err().code(),
        "resource-limit"
    );
    assert_eq!(arena.get(handle).unwrap(), &[1, 1, 1, 1]);
    assert_eq!(arena.live_bytes(), 4);
    assert_eq!(arena.peak_bytes(), 4);

    arena.replace(handle, vec![3_u8; 8], 8).unwrap();
    assert_eq!(arena.get(handle).unwrap(), &[3; 8]);
    assert_eq!(arena.live_bytes(), 8);
    assert_eq!(arena.peak_bytes(), 8);
}

#[test]
fn checkpoint_preserves_live_reusable_and_retired_slots() {
    let limits = ArenaLimits {
        objects: 3,
        bytes: 12,
    };
    let mut arena = Arena::new(limits);
    let live = arena.insert(1_u32, 4).unwrap();
    let reusable = arena.insert(2, 4).unwrap();
    let retired = arena.insert(3, 4).unwrap();
    arena.remove(reusable).unwrap();
    arena.slots[retired.index() as usize].generation = u32::MAX;
    arena
        .remove(Handle {
            generation: u32::MAX,
            ..retired
        })
        .unwrap();

    let bytes = save_state::encode(&arena).unwrap();
    let mut restored: Arena<u32> = save_state::decode(&bytes).unwrap();
    restored.validate_checkpoint(limits, |_| 4).unwrap();
    assert_eq!(*restored.get(live).unwrap(), 1);
    assert!(restored.get(reusable).is_err());
    assert!(restored.get(retired).is_err());
    let next = restored.insert(4, 4).unwrap();
    assert_eq!(next.index(), reusable.index());
    assert_ne!(next.generation(), reusable.generation());
    assert!(restored.insert(5, 4).is_err());
    restored.clear();
    restored.validate_checkpoint(limits, |_| 4).unwrap();
}

#[test]
fn checkpoint_rejects_incomplete_duplicate_and_invalid_free_lists() {
    let limits = ArenaLimits {
        objects: 2,
        bytes: 8,
    };
    let mut arena = Arena::new(limits);
    let live = arena.insert(1_u32, 4).unwrap();
    let removed = arena.insert(2, 4).unwrap();
    arena.remove(removed).unwrap();
    for free in [
        vec![],
        vec![removed.index(), removed.index()],
        vec![live.index()],
        vec![u32::MAX],
    ] {
        arena.free = free;
        assert!(arena.validate_checkpoint(limits, |_| 4).is_err());
    }
    arena.free = vec![removed.index()];
    arena.slots[removed.index() as usize].generation = 0;
    assert!(arena.validate_checkpoint(limits, |_| 4).is_err());
}

#[test]
fn checkpoint_rejects_live_retired_slot_and_inconsistent_counters() {
    let limits = ArenaLimits {
        objects: 2,
        bytes: 8,
    };
    let mut arena = Arena::new(limits);
    let live = arena.insert(1_u32, 4).unwrap();
    arena.slots[live.index() as usize].generation = 0;
    assert!(arena.validate_checkpoint(limits, |_| 4).is_err());
    arena.slots[live.index() as usize].generation = live.generation();

    for (objects, bytes, peak_objects, peak_bytes) in [
        (0, 4, 1, 4),
        (1, 3, 1, 4),
        (1, 4, 0, 4),
        (1, 4, 1, 3),
        (1, 4, 3, 4),
        (1, 4, 1, 9),
    ] {
        arena.live_objects = objects;
        arena.live_bytes = bytes;
        arena.peak_objects = peak_objects;
        arena.peak_bytes = peak_bytes;
        assert!(arena.validate_checkpoint(limits, |_| 4).is_err());
    }
}
