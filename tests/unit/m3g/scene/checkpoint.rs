use super::*;

#[test]
fn checkpoint_checks_actual_storage_instead_of_a_consistent_but_stale_charge() {
    for extra_budget in [64, 4096] {
        let limits = ArenaLimits {
            objects: 1,
            bytes: size_of::<Object>() + extra_budget,
        };
        let mut runtime = Runtime::new_with_graph_depth(limits, 32);
        let handle = runtime.create(None, ObjectKind::Object).unwrap();
        // Model a decoded payload with consistent counters left from a smaller object.
        *runtime.kind_mut(handle).unwrap() =
            ObjectKind::Image2D(Image2DState::mutable(crate::ImageFormat::Rgba, 32, 32).unwrap());
        let mut restored: Runtime =
            save_state::decode(&save_state::encode(&runtime).unwrap()).unwrap();
        let result = restored.validate_checkpoint(limits, 32, 1024, |_| true);
        if extra_budget == 64 {
            assert_eq!(result.unwrap_err().code(), "resource-limit");
        } else {
            result.unwrap();
            assert_eq!(restored.counters().2, limits.bytes);
            restored.rollback_created(&[handle]);
            assert_eq!(restored.counters().2, 0);
        }
    }
}

#[test]
fn checkpoint_compacts_large_keyframe_vectors_to_fit_the_original_exact_budget() {
    let kind = ObjectKind::KeyframeSequence(
        KeyframeSequenceState::new(150_000, 2, crate::Interpolation::Linear).unwrap(),
    );
    let limits = ArenaLimits {
        objects: 1,
        bytes: estimated_bytes(&kind),
    };
    let mut runtime = Runtime::new_with_graph_depth(limits, 32);
    let handle = runtime.create(None, kind).unwrap();
    let mut restored: Runtime = save_state::decode(&save_state::encode(&runtime).unwrap()).unwrap();
    restored
        .validate_checkpoint(limits, 32, 1024, |_| true)
        .unwrap();
    assert_eq!(restored.counters().2, limits.bytes);
    let ObjectKind::KeyframeSequence(sequence) = restored.kind(handle).unwrap() else {
        panic!("keyframe sequence was not retained");
    };
    let ObjectKind::KeyframeSequence(original) = runtime.kind(handle).unwrap() else {
        unreachable!();
    };
    assert_eq!(sequence, original);
    restored.rollback_created(&[handle]);
    assert_eq!(restored.counters().2, 0);
}

#[test]
fn checkpoint_requires_complete_and_unique_guest_bindings() {
    let limits = ArenaLimits::default();
    for corruption in 0..4 {
        let mut runtime = Runtime::new_with_graph_depth(limits, 32);
        let first = runtime.create(Some(11), ObjectKind::Object).unwrap();
        let second = runtime.create(Some(22), ObjectKind::Object).unwrap();
        runtime.create(None, ObjectKind::Object).unwrap();
        runtime
            .validate_checkpoint(limits, 32, 1024, |_| true)
            .unwrap();
        match corruption {
            0 => {
                runtime.guest_handles.remove(&11);
            }
            1 => {
                runtime.guest_handles.insert(33, first);
            }
            2 => {
                runtime.guest_handles.insert(11, second);
            }
            _ => {
                runtime.objects.get_mut(second).unwrap().guest_reference = Some(11);
            }
        }
        let mut restored: Runtime =
            save_state::decode(&save_state::encode(&runtime).unwrap()).unwrap();
        assert_eq!(
            restored
                .validate_checkpoint(limits, 32, 1024, |_| true)
                .unwrap_err()
                .code(),
            "checkpoint-arena"
        );
    }
}

#[test]
fn checkpoint_and_gc_follow_alignment_links_hidden_from_public_references() {
    let limits = ArenaLimits::default();
    let mut runtime = Runtime::new_with_graph_depth(limits, 32);
    let node = runtime
        .create(Some(11), ObjectKind::Node(NodeState::default()))
        .unwrap();
    let target = runtime
        .create(Some(22), ObjectKind::Node(NodeState::default()))
        .unwrap();
    runtime
        .set_alignments(node, Some(target), 148, None, 144)
        .unwrap();
    assert!(runtime.references(node).unwrap().is_empty());
    assert_eq!(runtime.guest_closure([11]), [11, 22]);
    let mut restored: Runtime = save_state::decode(&save_state::encode(&runtime).unwrap()).unwrap();
    restored
        .validate_checkpoint(limits, 32, 1024, |_| true)
        .unwrap();
    assert_eq!(restored.guest_closure([11]), [11, 22]);

    restored.rollback_created(&[target]);
    assert!(
        restored
            .validate_checkpoint(limits, 32, 1024, |_| true)
            .is_err()
    );
}
