use super::*;

fn translation_matrix(value: Vector3D) -> AffineTrans {
    let mut matrix = AffineTrans::IDENTITY;
    matrix.values[3] = value.x;
    matrix.values[7] = value.y;
    matrix.values[11] = value.z;
    matrix
}

fn scale_matrix(value: Vector3D) -> AffineTrans {
    AffineTrans::new([value.x, 0, 0, 0, 0, value.y, 0, 0, 0, 0, value.z, 0])
}

#[test]
fn action_pose_composition_matches_dense_products_including_extreme_components() {
    let mut random = 0x1234_abcd_u32;
    let mut coordinate = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random.cast_signed()
    };
    for sample in 0..2048 {
        let translation = Vector3D::new(coordinate(), coordinate(), coordinate());
        let rotation = match sample % 5 {
            0 => Vector3D::default(),
            1 => Vector3D::new(0, 0, -4096),
            _ => Vector3D::new(coordinate(), coordinate(), coordinate()),
        };
        let scale = match sample % 5 {
            0 => Vector3D::default(),
            1 => Vector3D::new(i32::MIN, i32::MAX, 4096),
            _ => Vector3D::new(coordinate(), coordinate(), coordinate()),
        };
        let roll = coordinate();
        let channel = |value| vec![VectorKeyframe { frame: 0, value }];
        let segment = ActionSegmentData::Components {
            translation: channel(translation),
            scale: channel(scale),
            rotation: channel(rotation),
            roll: vec![ScalarKeyframe {
                frame: 0,
                value: roll,
            }],
        };
        let expected = translation_matrix(translation)
            .multiplied(direction_matrix(rotation))
            .multiplied(AffineTrans::rotation_z(roll))
            .multiplied(scale_matrix(scale));
        assert_eq!(segment.sample_transform(0), expected, "sample={sample}");
    }
}

#[test]
fn pattern_sampling_keeps_the_last_eligible_key_in_file_order() {
    let action = ActionData {
        frame_count: 10,
        segments: Vec::new(),
        pattern_keys: vec![[3, 2, 0], [1, 4, 0], [3, 8, 0], [5, 16, 0]],
    };
    for (frame, expected) in [(-1, 0), (0, 0), (1 << 16, 2), (3 << 16, 4), (5 << 16, 8)] {
        assert_eq!(action.sample_pattern(frame), expected, "frame={frame}");
    }
}

#[test]
fn descending_animation_channels_keep_keyframes_and_fractional_values_exact() {
    let scalar = [
        ScalarKeyframe {
            frame: 2,
            value: 12,
        },
        ScalarKeyframe { frame: 4, value: 0 },
    ];
    let vector = scalar.map(|key| VectorKeyframe {
        frame: key.frame,
        value: Vector3D::new(key.value, -key.value, key.value),
    });
    for (frame, expected) in [
        (-1, 12),
        (2 << 16, 12),
        (5 << 15, 9),
        (3 << 16, 6),
        (4 << 16, 0),
        (i32::MAX, 0),
    ] {
        assert_eq!(sample_scalar(&scalar, frame, 99), expected, "{frame}");
        assert_eq!(
            sample_vector(&vector, frame, Vector3D::default()),
            Vector3D::new(expected, -expected, expected),
            "{frame}"
        );
    }
}

#[test]
fn action_rotation_vector_describes_the_positive_z_axis() {
    assert_eq!(
        direction_matrix(Vector3D::new(0, 0, 4096)),
        AffineTrans::IDENTITY
    );
    assert_eq!(
        direction_matrix(Vector3D::new(4096, 0, 0)).transform(Vector3D::new(0, 0, 4096)),
        Vector3D::new(4096, 0, 0)
    );
    assert_eq!(
        direction_matrix(Vector3D::new(0, 0, -4096)).transform(Vector3D::new(0, 0, 4096)),
        Vector3D::new(0, 0, -4096)
    );
}

#[test]
fn empty_action_rotation_channel_keeps_the_bac_posture() {
    let segment = ActionSegmentData::Components {
        translation: Vec::new(),
        scale: Vec::new(),
        rotation: Vec::new(),
        roll: Vec::new(),
    };
    assert_eq!(segment.sample_transform(0), AffineTrans::IDENTITY);
}

#[test]
fn action_pattern_does_not_wrap_large_unsigned_frames() {
    let action = ActionData {
        frame_count: u16::MAX,
        segments: Vec::new(),
        pattern_keys: vec![[40_000, 0, 1]],
    };

    assert_eq!(action.sample_pattern(i32::MAX), 0);
}

fn pose_segment(keys: u16) -> ActionSegmentData {
    let mut translation = Vec::new();
    let mut scale = Vec::new();
    let mut rotation = Vec::new();
    let mut roll = Vec::new();
    for key in 0..keys {
        let frame = key * 4;
        let key = i32::from(key);
        let x = key % 7 - 3;
        translation.push(VectorKeyframe {
            frame,
            value: Vector3D::new(x * 40, -x * 20, key * 4),
        });
        scale.push(VectorKeyframe {
            frame,
            value: Vector3D::new(4096 + key * 3, -2048 - key, 3072 + key * 5),
        });
        rotation.push(VectorKeyframe {
            frame,
            value: Vector3D::new(x * 300, (key % 5 - 2) * 200, 4096),
        });
        roll.push(ScalarKeyframe {
            frame,
            value: key * 127 - 1024,
        });
    }
    ActionSegmentData::Components {
        translation,
        scale,
        rotation,
        roll,
    }
}

#[test]
#[ignore = "manual Micro3D action pose throughput measurement"]
fn action_pose_throughput() {
    for keys in [0, 1, 8, 128] {
        let segment = pose_segment(keys);
        let mut checksum = 0_u64;
        let started = std::time::Instant::now();
        for sample in 0..65_536 {
            let frame = ((sample % (i32::from(keys.max(1)) * 4 + 1)) << 16) | (sample & 0xffff);
            let pose = std::hint::black_box(&segment).sample_transform(frame);
            for value in std::hint::black_box(pose.values) {
                checksum = checksum
                    .wrapping_mul(31)
                    .wrapping_add(u64::from(value.cast_unsigned()));
            }
        }
        eprintln!(
            "keys={keys} elapsed={:?} checksum={checksum:016x}",
            started.elapsed()
        );
    }
}
