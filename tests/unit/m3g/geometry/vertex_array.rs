use super::*;

#[test]
fn checkpoint_rejects_vertex_dimensions_and_components_that_could_escape_storage() {
    let valid = VertexArrayState::new(1, 3, VertexComponent::Byte).unwrap();
    let restored: VertexArrayState =
        save_state::decode(&save_state::encode(&valid).unwrap()).unwrap();
    restored.validate_checkpoint().unwrap();
    for corruption in 0..5 {
        let mut array = valid.clone();
        match corruption {
            0 => array.vertex_count = 0,
            1 => array.vertex_count = usize::MAX,
            2 => array.component_count = 1,
            3 => array.values = Arc::from([0_i16; 2]),
            _ => array.values = Arc::from([128_i16; 3]),
        }
        let limits = crate::ArenaLimits::default();
        let mut runtime = crate::Runtime::new_with_graph_depth(limits, 32);
        runtime
            .create(None, crate::ObjectKind::VertexArray(array))
            .unwrap();
        assert!(
            runtime
                .validate_checkpoint(limits, 32, 1024, |_| true)
                .is_err()
        );
    }
}

#[test]
fn normal_decoding_preserves_signed_endpoints_and_source_components() {
    for component_type in [VertexComponent::Byte, VertexComponent::Short] {
        let mut array = VertexArrayState::new(1, 3, component_type).unwrap();
        let expected = match component_type {
            VertexComponent::Byte => {
                array.set_bytes(0, 1, &[i8::MIN, 0, i8::MAX]).unwrap();
                1.0 / 255.0
            }
            VertexComponent::Short => {
                array.set_shorts(0, 1, &[i16::MIN, 0, i16::MAX]).unwrap();
                1.0 / 65_535.0
            }
        };
        assert_eq!(
            array.normal(0).unwrap(),
            Vec4::new(-1.0, expected, 1.0, 0.0)
        );
        assert_eq!(array.component(0, 1).unwrap(), 0);
    }
}

#[test]
fn normal_decoding_rejects_short_attributes_and_invalid_vertex_indices() {
    for component_type in [VertexComponent::Byte, VertexComponent::Short] {
        let short = VertexArrayState::new(2, 2, component_type).unwrap();
        assert_eq!(short.normal(0).unwrap_err().code(), "vertex-bounds");
        let normals = VertexArrayState::new(2, 3, component_type).unwrap();
        for index in [2, usize::MAX] {
            assert_eq!(normals.normal(index).unwrap_err().code(), "vertex-bounds");
        }
    }
}

#[test]
fn vertex_arrays_bound_their_count_and_allow_empty_updates() {
    assert!(VertexArrayState::new(65_536, 3, VertexComponent::Short).is_err());
    let mut array = VertexArrayState::new(1, 3, VertexComponent::Short).unwrap();
    let shared = array.clone();
    array.set_shorts(1, 0, &[]).unwrap();
    assert!(Arc::ptr_eq(&array.values, &shared.values));
    assert!(array.set_shorts(2, 0, &[]).is_err());
}

#[test]
fn cloned_vertex_arrays_share_storage_until_mutated() {
    let mut original = VertexArrayState::new(1, 3, VertexComponent::Short).unwrap();
    original.set_shorts(0, 1, &[1, 2, 3]).unwrap();
    let mut clone = original.clone();
    assert!(Arc::ptr_eq(&original.values, &clone.values));

    clone.set_shorts(0, 1, &[4, 5, 6]).unwrap();
    assert!(!Arc::ptr_eq(&original.values, &clone.values));
    assert_eq!(original.component(0, 0).unwrap(), 1);
    assert_eq!(clone.component(0, 0).unwrap(), 4);
}
