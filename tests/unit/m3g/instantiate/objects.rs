use super::*;

#[test]
fn vertex_encodings_preserve_signed_components_and_wrap_each_delta_channel() {
    for (size, minimum, maximum) in [(1, -128_i16, 127_i16), (2, i16::MIN, i16::MAX)] {
        let encoded = [[maximum, minimum, 1, -1], [1, -1, maximum, minimum]];
        let accumulated = [encoded[0], [minimum, maximum, minimum, maximum]];
        for components in 2..=4 {
            for encoding in [0, 1] {
                let mut data = vec![size, components as u8, encoding, 2, 0];
                for vertex in encoded {
                    for value in &vertex[..components] {
                        if size == 1 {
                            data.push(*value as u8);
                        } else {
                            data.extend_from_slice(&value.to_le_bytes());
                        }
                    }
                }
                let mut cursor = Cursor::new(&data);
                let array = decode_vertex_array(&mut cursor).unwrap();
                cursor.finish().unwrap();
                let expected = if encoding == 0 { encoded } else { accumulated };
                for (vertex, expected) in expected.iter().enumerate() {
                    for (component, value) in expected[..components].iter().enumerate() {
                        assert_eq!(array.component(vertex, component).unwrap(), *value);
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "manual vertex-array decoding throughput measurement"]
fn vertex_array_decode_throughput() {
    for vertices in [8, 4_096, 65_535] {
        for size in [1, 2] {
            for encoding in [0, 1] {
                let mut data = vec![size, 3, encoding];
                data.extend_from_slice(&(vertices as u16).to_le_bytes());
                for index in 0..vertices * 3 {
                    let value = (index * 7_919) as i16;
                    if size == 1 {
                        data.push(value as u8);
                    } else {
                        data.extend_from_slice(&value.to_le_bytes());
                    }
                }
                let iterations = match vertices {
                    8 => 8_192,
                    4_096 => 128,
                    _ => 8,
                };
                let mut array = None;
                let started = std::time::Instant::now();
                for _ in 0..iterations {
                    let mut cursor = Cursor::new(std::hint::black_box(&data));
                    array = Some(decode_vertex_array(&mut cursor).unwrap());
                    cursor.finish().unwrap();
                    std::hint::black_box(&array);
                }
                let elapsed = started.elapsed();
                let array = array.unwrap();
                let mut checksum = 0xcbf2_9ce4_8422_2325_u64;
                for vertex in 0..vertices {
                    for component in 0..3 {
                        checksum = (checksum
                            ^ u64::from(
                                array.component(vertex, component).unwrap().cast_unsigned(),
                            ))
                        .wrapping_mul(0x0100_0000_01b3);
                    }
                }
                eprintln!(
                    "vertex_decode vertices={vertices} size={size} encoding={encoding} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}

#[test]
fn keyframe_encodings_preserve_declared_component_values() {
    for encoding in [0_u8, 1, 2] {
        let mut data = vec![176, 192, encoding];
        for value in [10_u32, 0, 1, 2, 2] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        if encoding != 0 {
            for value in [10.0_f32, -2.0, 4.0, 8.0] {
                data.extend_from_slice(&value.to_le_bytes());
            }
        }
        data.extend_from_slice(&0_u32.to_le_bytes());
        if encoding == 0 {
            data.extend_from_slice(&10.0_f32.to_le_bytes());
            data.extend_from_slice(&6.0_f32.to_le_bytes());
        } else if encoding == 1 {
            data.extend_from_slice(&[0, u8::MAX]);
        } else {
            data.extend_from_slice(&0_u16.to_le_bytes());
            data.extend_from_slice(&u16::MAX.to_le_bytes());
        }
        data.extend_from_slice(&10_u32.to_le_bytes());
        if encoding == 0 {
            data.extend_from_slice(&14.0_f32.to_le_bytes());
            data.extend_from_slice(&(-2.0_f32).to_le_bytes());
        } else if encoding == 1 {
            data.extend_from_slice(&[u8::MAX, 0]);
        } else {
            data.extend_from_slice(&u16::MAX.to_le_bytes());
            data.extend_from_slice(&0_u16.to_le_bytes());
        }

        let mut cursor = Cursor::new(&data);
        let sequence = decode_keyframes(&mut cursor).unwrap();
        cursor.finish().unwrap();
        assert_eq!(sequence.keyframe(0).unwrap(), (0, [10.0, 6.0].as_slice()));
        assert_eq!(sequence.keyframe(1).unwrap(), (10, [14.0, -2.0].as_slice()));
    }
}

#[test]
fn strip_encoding_selects_explicit_indices_independently_of_their_count() {
    for encoding in [0_u8, 1, 2, 128, 129, 130] {
        for source in [&[7_u32][..], &[7, 3][..], &[7, 3, 5, 2][..]] {
            let explicit = encoding & 128 != 0;
            let mut data = vec![encoding];
            if explicit {
                data.extend_from_slice(&(source.len() as u32).to_le_bytes());
            }
            for index in if explicit { source } else { &source[..1] } {
                match encoding & 127 {
                    0 => data.extend_from_slice(&index.to_le_bytes()),
                    1 => data.push(*index as u8),
                    _ => data.extend_from_slice(&(*index as u16).to_le_bytes()),
                }
            }
            data.extend_from_slice(&1_u32.to_le_bytes());
            data.extend_from_slice(&3_u32.to_le_bytes());
            let mut cursor = Cursor::new(&data);
            let decoded = decode_strips(&mut cursor);
            cursor.finish().unwrap();
            if explicit && source.len() < 3 {
                assert_eq!(decoded.unwrap_err().code(), "invalid-strips");
            } else {
                assert_eq!(
                    decoded.unwrap().indices(),
                    if explicit { &[7, 3, 5] } else { &[7, 8, 9] }
                );
            }
        }
    }
}

#[test]
fn vertex_buffer_decodes_bias_before_scale() {
    let mut runtime = Runtime::default();
    let positions = runtime
        .create(
            None,
            ObjectKind::VertexArray(VertexArrayState::new(1, 3, VertexComponent::Short).unwrap()),
        )
        .unwrap();
    let texture = runtime
        .create(
            None,
            ObjectKind::VertexArray(VertexArrayState::new(1, 2, VertexComponent::Short).unwrap()),
        )
        .unwrap();
    let handles = [Some(positions), Some(texture)];
    let mut data = vec![0x12, 0x34, 0x56, 0x78];
    data.extend_from_slice(&1_u32.to_le_bytes());
    for value in [1.0_f32, 2.0, 3.0, 0.25] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&1_u32.to_le_bytes());
    data.extend_from_slice(&2_u32.to_le_bytes());
    for value in [0.1_f32, 0.2, 0.0, 0.5] {
        data.extend_from_slice(&value.to_le_bytes());
    }

    let mut cursor = Cursor::new(&data);
    let (state, arrays) = decode_vertex_buffer(&mut cursor, &handles, &runtime).unwrap();
    cursor.finish().unwrap();

    let (_, position_scale, position_bias) = state.positions().unwrap();
    assert_eq!(position_bias, [1.0, 2.0, 3.0]);
    assert_eq!(position_scale, 0.25);
    let (_, texture_scale, texture_bias) = state.texture_coordinates(0).unwrap();
    assert_eq!(texture_bias, [0.1, 0.2, 0.0]);
    assert_eq!(texture_scale, 0.5);
    assert_eq!(arrays, [Some(positions), None, None, Some(texture), None]);

    let unused_bias = data.len() - 8;
    data[unused_bias..unused_bias + 4].copy_from_slice(&1.0_f32.to_le_bytes());
    assert!(decode_vertex_buffer(&mut Cursor::new(&data), &handles, &runtime).is_err());
}

#[test]
fn skinned_mesh_preserves_first_seen_bone_order_when_deduplicating() {
    let mut runtime = Runtime::default();
    let handles = (0..5)
        .map(|_| runtime.create(None, ObjectKind::Object).unwrap())
        .map(Some)
        .collect::<Vec<_>>();
    let mut data = Vec::new();
    data.extend_from_slice(&1_u32.to_le_bytes());
    data.extend_from_slice(&4_u32.to_le_bytes());
    for (bone, first_vertex) in [(2_u32, 0_u32), (3, 1), (2, 2), (3, 3)] {
        data.extend_from_slice(&bone.to_le_bytes());
        data.extend_from_slice(&first_vertex.to_le_bytes());
        data.extend_from_slice(&1_u32.to_le_bytes());
        data.extend_from_slice(&256_i32.to_le_bytes());
    }
    let mesh = MeshData {
        vertices: 4,
        submeshes: vec![5],
        appearances: vec![0],
    };

    let mut cursor = Cursor::new(&data);
    let kind = decode_kind(
        ObjectType::SkinnedMesh,
        &mut cursor,
        Some(&mesh),
        &handles,
        &runtime,
    )
    .unwrap();
    cursor.finish().unwrap();

    let ObjectKind::SkinnedMesh {
        bones, influences, ..
    } = kind
    else {
        unreachable!();
    };
    assert_eq!(bones, [handles[1].unwrap(), handles[2].unwrap()]);
    assert_eq!(
        influences
            .iter()
            .map(|influence| influence.bone)
            .collect::<Vec<_>>(),
        [0, 1, 0, 1]
    );
}

#[test]
fn mesh_record_counts_are_checked_before_reserving_arrays() {
    let mut runtime = Runtime::default();
    let handle = runtime.create(None, ObjectKind::Object).unwrap();
    let mesh = MeshData {
        vertices: 1,
        submeshes: vec![1],
        appearances: vec![0],
    };
    for kind in [ObjectType::MorphingMesh, ObjectType::SkinnedMesh] {
        for count in [1_u32, u32::MAX] {
            let mut data = Vec::new();
            if kind == ObjectType::SkinnedMesh {
                data.extend_from_slice(&1_u32.to_le_bytes());
            }
            data.extend_from_slice(&count.to_le_bytes());
            let error = decode_kind(
                kind,
                &mut Cursor::new(&data),
                Some(&mesh),
                &[Some(handle)],
                &runtime,
            )
            .unwrap_err();
            assert_eq!(error.code(), "truncated-object-data");
        }
    }
}
