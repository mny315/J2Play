use super::*;

fn track(runtime: &mut Runtime, property: i32, components: usize) -> Handle {
    let sequence = runtime
        .create(
            None,
            ObjectKind::KeyframeSequence(
                KeyframeSequenceState::new(1, components, crate::Interpolation::Step).unwrap(),
            ),
        )
        .unwrap();
    runtime
        .create(
            None,
            ObjectKind::AnimationTrack {
                sequence,
                controller: None,
                property,
            },
        )
        .unwrap()
}

#[test]
fn animation_track_creation_validates_keyframe_shape_before_allocating() {
    for (property, accepted) in [
        (256, &[1][..]),
        (258, &[3]),
        (259, &[2, 4]),
        (266, &[1, 2, 3, 4, 5]),
        (268, &[4]),
        (270, &[1, 3]),
        (275, &[3]),
        (255, &[]),
        (277, &[]),
    ] {
        for components in 1..=5 {
            let mut runtime = Runtime::default();
            let sequence = runtime
                .create(
                    None,
                    ObjectKind::KeyframeSequence(
                        KeyframeSequenceState::new(1, components, crate::Interpolation::Step)
                            .unwrap(),
                    ),
                )
                .unwrap();
            let before = runtime.counters();
            let created = runtime.create(
                Some(77),
                ObjectKind::AnimationTrack {
                    sequence,
                    controller: None,
                    property,
                },
            );
            assert_eq!(
                created.is_ok(),
                accepted.contains(&components),
                "property={property}, components={components}"
            );
            if created.is_err() {
                assert_eq!(runtime.counters(), before);
                assert!(runtime.resolve_guest(77).is_err());
            }
        }
    }
}

#[test]
fn animation_track_creation_rejects_wrong_reference_types() {
    for invalid_sequence in [false, true] {
        let mut runtime = Runtime::default();
        let object = runtime.create(None, ObjectKind::Object).unwrap();
        let sequence = runtime
            .create(
                None,
                ObjectKind::KeyframeSequence(
                    KeyframeSequenceState::new(1, 1, crate::Interpolation::Step).unwrap(),
                ),
            )
            .unwrap();
        let before = runtime.counters();
        assert!(
            runtime
                .create(
                    None,
                    ObjectKind::AnimationTrack {
                        sequence: if invalid_sequence { object } else { sequence },
                        controller: (!invalid_sequence).then_some(object),
                        property: 256,
                    }
                )
                .is_err()
        );
        assert_eq!(runtime.counters(), before);
    }
}

#[test]
fn incompatible_tracks_leave_the_target_and_arena_unchanged() {
    let mut runtime = Runtime::default();
    let node = runtime
        .create(None, ObjectKind::Node(NodeState::default()))
        .unwrap();
    let scalar = track(&mut runtime, 270, 1);
    let vector = track(&mut runtime, 270, 3);
    let color = track(&mut runtime, 258, 3);
    let other_scalar = track(&mut runtime, 270, 1);
    let not_a_track = runtime.create(None, ObjectKind::Object).unwrap();
    runtime.add_animation_track(node, scalar).unwrap();
    for invalid in [scalar, vector, color, not_a_track] {
        let before = runtime.counters();
        assert!(runtime.add_animation_track(node, invalid).is_err());
        assert_eq!(runtime.animation_tracks(node).unwrap(), [scalar]);
        assert_eq!(runtime.counters(), before);
    }
    runtime.add_animation_track(node, other_scalar).unwrap();
    assert_eq!(
        runtime.animation_tracks(node).unwrap(),
        [scalar, other_scalar]
    );
    runtime.remove_animation_track(node, scalar).unwrap();
    runtime.remove_animation_track(node, other_scalar).unwrap();
    runtime.add_animation_track(node, vector).unwrap();
    assert_eq!(runtime.animation_tracks(node).unwrap(), [vector]);
}

#[test]
fn checkpoint_rejects_invalid_animation_tracks_and_bindings() {
    let limits = ArenaLimits::default();
    for corruption in 0..6 {
        let mut runtime = Runtime::new_with_graph_depth(limits, 32);
        let node = runtime
            .create(None, ObjectKind::Node(NodeState::default()))
            .unwrap();
        let scalar = track(&mut runtime, 270, 1);
        let other_scalar = track(&mut runtime, 270, 1);
        let vector = track(&mut runtime, 270, 3);
        let color = track(&mut runtime, 258, 3);
        runtime.add_animation_track(node, scalar).unwrap();
        runtime.add_animation_track(node, other_scalar).unwrap();
        runtime
            .validate_checkpoint(limits, 32, 1024, |_| true)
            .unwrap();
        // Corrupt the already-accounted state without changing any allocation.
        let ObjectKind::AnimationTrack {
            sequence,
            controller,
            property,
        } = runtime.kind_mut(scalar).unwrap()
        else {
            unreachable!();
        };
        match corruption {
            0 => *sequence = node,
            1 => *controller = Some(node),
            2 => *property = 258,
            3 => runtime.objects.get_mut(node).unwrap().base.animation_tracks[0] = color,
            4 => runtime.objects.get_mut(node).unwrap().base.animation_tracks[1] = vector,
            _ => runtime.objects.get_mut(node).unwrap().base.animation_tracks[1] = scalar,
        }
        let mut restored: Runtime =
            save_state::decode(&save_state::encode(&runtime).unwrap()).unwrap();
        assert!(
            restored
                .validate_checkpoint(limits, 32, 1024, |_| true)
                .is_err()
        );
    }
}
