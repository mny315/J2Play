use super::*;

#[test]
fn action_header_exposes_all_frame_counts() {
    let mut bytes = b"MT\x05\0\x02\0\x01\0".to_vec();
    bytes.extend_from_slice(&[0; 20]);
    bytes.extend_from_slice(&[6, 0, 1, 0, 0]);
    bytes.extend_from_slice(&[17, 0, 1, 0, 0]);
    bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");
    let table = ActionTableData::parse(&bytes, LoaderLimits::default()).unwrap();
    assert_eq!(table.frame_counts, [6, 17]);
}

#[test]
fn action_pose_linearly_interpolates_translation() {
    let mut bytes = b"MT\x05\0\x01\0\x01\0".to_vec();
    bytes.extend_from_slice(&[0; 20]);
    bytes.extend_from_slice(&10_u16.to_le_bytes());
    bytes.push(2);
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    for values in [[0_i16, 0, 0, 0], [10, 100, 0, 0]] {
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    for value in [0_i16, 4096, 4096, 4096] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    for value in [0_i16, 0, 0, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_i16.to_le_bytes());
    bytes.extend_from_slice(&0_i16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");

    let table = ActionTableData::parse(&bytes, LoaderLimits::default()).unwrap();
    let transform = table.actions[0].segments[0].sample_transform(5 << 16);
    assert_eq!(
        transform.transform(Vector3D::default()),
        Vector3D::new(50, 0, 0)
    );
}

#[test]
fn action_pattern_keys_replace_the_full_mask_at_fractional_frames() {
    // Project-owned MTRA: one stationary bone and four appearance states.
    let mut bytes = b"MT\x05\0\x01\0\x01\0".to_vec();
    bytes.extend_from_slice(&[0; 20]);
    bytes.extend_from_slice(&10_u16.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    for (frame, mask) in [(1_u16, 2_u32), (3, 5), (5, 0x8001_0000), (7, 0)] {
        bytes.extend_from_slice(&frame.to_le_bytes());
        bytes.extend_from_slice(&mask.to_le_bytes());
    }
    bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");
    let table = ActionTableData::parse(&bytes, LoaderLimits::default()).unwrap();
    let action = &table.actions[0];

    for (frame, mask) in [
        (-1, 0),
        (0, 0),
        ((1 << 16) - 1, 0),
        (1 << 16, 1),
        ((2 << 16) + 32_768, 1),
        ((3 << 16) - 1, 1),
        (3 << 16, 2),
        (5 << 16, 0x4000_8000),
        (7 << 16, 0),
        (i32::MAX, 0),
    ] {
        assert_eq!(action.sample_pattern(frame), mask, "frame={frame}");
    }
}

#[test]
fn action_keyframes_are_aggregate_bounded() {
    let mut bytes = b"MT\x05\0\x01\0\x01\0".to_vec();
    bytes.extend_from_slice(&[0; 20]);
    bytes.extend_from_slice(&10_u16.to_le_bytes());
    bytes.push(2);
    for _ in 0..3 {
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&[0; 8]);
    }
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");
    let limits = LoaderLimits {
        keyframes: 3,
        ..LoaderLimits::default()
    };

    assert_eq!(
        ActionTableData::parse(&bytes, limits).unwrap_err().code(),
        "action-keyframe-count"
    );
}

#[test]
fn compact_action_channels_only_allocate_encoded_keyframes() {
    for kind in 3..=6 {
        let mut bytes = b"MT\x05\0\x01\0\x01\0".to_vec();
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&10_u16.to_le_bytes());
        bytes.push(kind);
        if kind == 3 {
            for value in [10_i16, -20, 30] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        } else if kind == 6 {
            bytes.extend_from_slice(&0_u16.to_le_bytes());
        }
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        for key in [[0_i16, 0, 0, 4096], [10, 4096, 0, 0]] {
            for value in key {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        if kind == 3 {
            bytes.extend_from_slice(&256_i16.to_le_bytes());
        } else if kind != 5 {
            bytes.extend_from_slice(&0_u16.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");

        let base_bytes =
            size_of::<ActionData>() + size_of::<ActionSegmentData>() + size_of::<u16>();
        let decoded_bytes = base_bytes
            + 2 * size_of::<VectorKeyframe>()
            + if kind == 3 {
                size_of::<VectorKeyframe>() + size_of::<ScalarKeyframe>()
            } else {
                0
            };
        let limits = LoaderLimits {
            keyframes: if kind == 3 { 4 } else { 2 },
            decoded_bytes,
            ..LoaderLimits::default()
        };
        let table = ActionTableData::parse(&bytes, limits).unwrap();
        assert_eq!(table.frame_counts, [10], "kind={kind}");
        assert_eq!(table.allocated_bytes(), decoded_bytes, "kind={kind}");
        let expected = ActionSegmentData::Components {
            translation: vec![VectorKeyframe {
                frame: 0,
                value: if kind == 3 {
                    Vector3D::new(10, -20, 30)
                } else {
                    Vector3D::default()
                },
            }],
            scale: vec![VectorKeyframe {
                frame: 0,
                value: Vector3D::new(4096, 4096, 4096),
            }],
            rotation: vec![
                VectorKeyframe {
                    frame: 0,
                    value: Vector3D::new(0, 0, 4096),
                },
                VectorKeyframe {
                    frame: 10,
                    value: Vector3D::new(4096, 0, 0),
                },
            ],
            roll: vec![ScalarKeyframe {
                frame: 0,
                value: if kind == 3 { 256 } else { 0 },
            }],
        };
        for frame in [-1, 0, 5 << 15, 5 << 16, 10 << 16, i32::MAX] {
            assert_eq!(
                table.actions[0].segments[0].sample_transform(frame),
                expected.sample_transform(frame),
                "kind={kind}, frame={frame}"
            );
        }
    }
}

#[test]
fn action_segments_are_bounded_before_body_allocation() {
    let bytes = b"MT\x05\0\x02\0\x02\0";
    let limits = LoaderLimits {
        decoded_bytes: 1,
        ..LoaderLimits::default()
    };

    assert_eq!(
        ActionTableData::parse(bytes, limits).unwrap_err().code(),
        "resource-budget"
    );
}

#[test]
fn action_nested_buffers_are_bounded_before_allocation() {
    let base_bytes = size_of::<ActionData>() + size_of::<ActionSegmentData>() + size_of::<u16>();
    let limits = LoaderLimits {
        decoded_bytes: base_bytes,
        ..LoaderLimits::default()
    };

    let mut keyframes = b"MT\x05\0\x01\0\x01\0".to_vec();
    keyframes.extend_from_slice(&[0; 20]);
    keyframes.extend_from_slice(&10_u16.to_le_bytes());
    keyframes.push(2);
    keyframes.extend_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        ActionTableData::parse(&keyframes, limits)
            .unwrap_err()
            .code(),
        "resource-budget"
    );

    let mut pattern_keys = b"MT\x05\0\x01\0\x01\0".to_vec();
    pattern_keys.extend_from_slice(&[0; 20]);
    pattern_keys.extend_from_slice(&10_u16.to_le_bytes());
    pattern_keys.push(1);
    pattern_keys.extend_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        ActionTableData::parse(&pattern_keys, limits)
            .unwrap_err()
            .code(),
        "resource-budget"
    );

    pattern_keys.extend_from_slice(&[0; 6]);
    pattern_keys.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");
    let exact_bytes = base_bytes + size_of::<[u16; 3]>();
    let exact_limits = LoaderLimits {
        decoded_bytes: exact_bytes,
        ..LoaderLimits::default()
    };
    let table = ActionTableData::parse(&pattern_keys, exact_limits).unwrap();
    assert_eq!(table.allocated_bytes(), exact_bytes);
    assert_eq!(
        ActionTableData::parse(
            &pattern_keys,
            LoaderLimits {
                decoded_bytes: exact_bytes - 1,
                ..LoaderLimits::default()
            }
        )
        .unwrap_err()
        .code(),
        "resource-budget"
    );
}
