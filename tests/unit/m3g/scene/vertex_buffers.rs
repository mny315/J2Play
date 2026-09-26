use super::*;

#[test]
fn transactional_vertex_buffer_setter_preserves_state_on_limit() {
    let positions = VertexArrayState::new(4, 3, crate::VertexComponent::Short).unwrap();
    let positions_kind = ObjectKind::VertexArray(positions.clone());
    let buffer_kind = ObjectKind::VertexBuffer {
        state: VertexBufferState::default(),
        arrays: [None; 5],
    };
    let budget = estimated_bytes(&positions_kind) + estimated_bytes(&buffer_kind);
    let mut runtime = Runtime::new(ArenaLimits {
        objects: 2,
        bytes: budget,
    });
    let positions_handle = runtime.create(None, positions_kind).unwrap();
    let buffer = runtime.create(None, buffer_kind).unwrap();
    let before = runtime.counters();

    let error = runtime
        .set_vertex_buffer_positions(
            buffer,
            Some(positions),
            Some(positions_handle),
            1.0,
            [0.0; 3],
        )
        .unwrap_err();
    assert_eq!(error.code(), "resource-limit");
    let ObjectKind::VertexBuffer { state, arrays } = runtime.kind(buffer).unwrap() else {
        unreachable!();
    };
    assert!(state.positions().is_none());
    assert_eq!(arrays, &[None; 5]);
    assert_eq!(runtime.counters(), before);
}

#[test]
fn vertex_buffer_updates_retain_object_metadata_and_track_storage() {
    let mut runtime = Runtime::default();
    let buffer = runtime
        .create(
            Some(11),
            ObjectKind::VertexBuffer {
                state: VertexBufferState::default(),
                arrays: [None; 5],
            },
        )
        .unwrap();
    let sequence = runtime
        .create(
            None,
            ObjectKind::KeyframeSequence(
                KeyframeSequenceState::new(1, 1, crate::Interpolation::Step).unwrap(),
            ),
        )
        .unwrap();
    let track = runtime
        .create(
            None,
            ObjectKind::AnimationTrack {
                sequence,
                controller: None,
                property: 256,
            },
        )
        .unwrap();
    runtime.add_animation_track(buffer, track).unwrap();
    runtime.set_user_id(buffer, 42).unwrap();
    runtime.set_user_object(buffer, Some(22)).unwrap();
    let tracks = runtime.animation_tracks(buffer).unwrap().as_ptr();
    let bytes = runtime.counters().2;
    let positions = VertexArrayState::new(4, 3, crate::VertexComponent::Short).unwrap();
    let position_bytes = positions.allocated_bytes();

    runtime
        .set_vertex_buffer_positions(buffer, Some(positions), None, 1.0, [0.0; 3])
        .unwrap();
    assert_eq!(runtime.counters().2, bytes + position_bytes);
    runtime
        .set_vertex_buffer_positions(buffer, None, None, 1.0, [0.0; 3])
        .unwrap();
    assert_eq!(runtime.counters().2, bytes);
    assert_eq!(runtime.guest_reference(buffer), Some(11));
    assert_eq!(runtime.user_id(buffer).unwrap(), 42);
    assert_eq!(runtime.user_object(buffer).unwrap(), Some(22));
    assert_eq!(runtime.animation_tracks(buffer).unwrap(), &[track]);
    assert_eq!(runtime.animation_tracks(buffer).unwrap().as_ptr(), tracks);
}

#[test]
fn texture_coordinate_unit_bounds_preserve_existing_slots() {
    let mut runtime = Runtime::default();
    let coordinates = VertexArrayState::new(1, 2, crate::VertexComponent::Short).unwrap();
    let source = runtime
        .create(None, ObjectKind::VertexArray(coordinates.clone()))
        .unwrap();
    let buffer = runtime
        .create(
            None,
            ObjectKind::VertexBuffer {
                state: VertexBufferState::default(),
                arrays: [None; 5],
            },
        )
        .unwrap();
    for unit in 0..2 {
        runtime
            .set_vertex_buffer_texture_coordinates(
                buffer,
                unit,
                Some(coordinates.clone()),
                Some(source),
                2.0,
                [3.0; 3],
            )
            .unwrap();
    }
    let before = runtime.resolved_vertex_buffer(buffer).unwrap();
    let counters = runtime.counters();
    for unit in [2, usize::MAX - 2, usize::MAX - 1, usize::MAX] {
        let error = runtime
            .set_vertex_buffer_texture_coordinates(buffer, unit, None, None, 1.0, [0.0; 3])
            .unwrap_err();
        assert_eq!(error.code(), "invalid-texture-unit");
        assert_eq!(runtime.resolved_vertex_buffer(buffer).unwrap(), before);
        let ObjectKind::VertexBuffer { arrays, .. } = runtime.kind(buffer).unwrap() else {
            unreachable!();
        };
        assert_eq!(arrays, &[None, None, None, Some(source), Some(source)]);
        assert_eq!(runtime.counters(), counters);
    }
}

#[test]
fn vertex_buffer_resolves_mutated_source_array() {
    let mut runtime = Runtime::default();
    let mut positions = VertexArrayState::new(1, 3, crate::VertexComponent::Short).unwrap();
    positions.set_shorts(0, 1, &[1, 2, 3]).unwrap();
    let source = runtime
        .create(None, ObjectKind::VertexArray(positions.clone()))
        .unwrap();
    let mut state = VertexBufferState::default();
    state
        .set_positions(Some(positions), 2.0, [4.0, 5.0, 6.0])
        .unwrap();
    let buffer = runtime
        .create(
            None,
            ObjectKind::VertexBuffer {
                state,
                arrays: [Some(source), None, None, None, None],
            },
        )
        .unwrap();

    let ObjectKind::VertexArray(positions) = runtime.kind_mut(source).unwrap() else {
        unreachable!();
    };
    positions.set_shorts(0, 1, &[7, 8, 9]).unwrap();

    let resolved = runtime.resolved_vertex_buffer(buffer).unwrap();
    let (positions, scale, bias) = resolved.positions().unwrap();
    assert_eq!(positions.component(0, 0).unwrap(), 7);
    assert_eq!(positions.component(0, 1).unwrap(), 8);
    assert_eq!(positions.component(0, 2).unwrap(), 9);
    assert_eq!(scale, 2.0);
    assert_eq!(bias, [4.0, 5.0, 6.0]);
}
