use super::*;

#[test]
fn bone_root_accepts_both_deployed_sentinel_encodings() {
    assert_eq!(normalize_bone_parent(0, -1, 5).unwrap(), -1);
    assert_eq!(normalize_bone_parent(0, 0, 5).unwrap(), -1);
    assert_eq!(
        normalize_bone_parent(0, 0, 3).unwrap_err().code(),
        "figure-bone-parent"
    );
    assert_eq!(normalize_bone_parent(2, 0, 5).unwrap(), 0);
    assert_eq!(normalize_bone_parent(2, 1, 5).unwrap(), 1);
    assert_eq!(
        normalize_bone_parent(2, 2, 5).unwrap_err().code(),
        "figure-bone-parent"
    );
}

#[test]
fn zero_single_bone_range_covers_the_static_figure() {
    let mut bones = [Bone {
        vertex_count: 0,
        parent: -1,
        transform: AffineTrans::new([0; 12]),
    }];
    let mut covered = 0;

    normalize_single_bone_vertex_range(&mut bones, &mut covered, 137);

    assert_eq!(bones[0].vertex_count, 137);
    assert_eq!(bones[0].transform, AffineTrans::IDENTITY);
    assert_eq!(covered, 137);
}

#[test]
fn version_three_triangle_decodes_with_packed_uv() {
    let mut bytes = b"MB\x03\0".to_vec();
    for value in [3_u16, 1, 0, 1] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for vertex in [[0_i16, 0, 0], [10, 0, 0], [0, 10, 20]] {
        for value in vertex {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    for value in [7_u16, 0, 1, 2, 0x0201, 0x0403, 0x0605] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&3_u16.to_le_bytes());
    bytes.extend_from_slice(&(-1_i16).to_le_bytes());
    for value in [4096_i16, 0, 0, 0, 0, 4096, 0, 0, 0, 0, 4096, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");

    let figure = FigureData::parse(&bytes, LoaderLimits::default()).unwrap();
    assert_eq!(figure.faces.len(), 1);
    assert_eq!(figure.faces[0].indices, [0, 1, 2]);
    assert_eq!(figure.faces[0].uv, [[1, 2], [3, 4], [5, 6]]);
    assert_eq!(figure.faces[0].attributes, 7);
    assert_eq!(figure.vertices[2], Vector3D::new(0, 10, 20));
    let limits = LoaderLimits {
        decoded_bytes: 3 * size_of::<Vector3D>() + size_of::<Face>() + size_of::<Bone>(),
        ..LoaderLimits::default()
    };
    assert_eq!(FigureData::parse(&bytes, limits).unwrap(), figure);
    assert_eq!(
        FigureData::parse(
            &bytes,
            LoaderLimits {
                decoded_bytes: limits.decoded_bytes - 1,
                ..limits
            }
        )
        .unwrap_err()
        .code(),
        "resource-budget"
    );
}

#[test]
fn version_three_quad_preserves_triangle_strip_winding() {
    let mut bytes = b"MB\x03\0".to_vec();
    for value in [4_u16, 0, 1, 1] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for vertex in [[0_i16, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]] {
        for value in vertex {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    for value in [7_u16, 0, 3, 1, 2, 0x0201, 0x0403, 0x0605, 0x0807] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(&(-1_i16).to_le_bytes());
    for value in [4096_i16, 0, 0, 0, 0, 4096, 0, 0, 0, 0, 4096, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");

    let figure = FigureData::parse(&bytes, LoaderLimits::default()).unwrap();
    assert_eq!(figure.faces.len(), 2);
    assert_eq!(figure.faces[0].indices, [0, 3, 1]);
    assert_eq!(figure.faces[1].indices, [1, 3, 2]);
    assert_eq!(figure.faces[1].uv, [[5, 6], [3, 4], [7, 8]]);
}

#[test]
fn figure_geometry_budget_is_checked_before_decoding_the_body() {
    for (version, normals) in [(3_u16, 0_u8), (5, 0), (5, 2)] {
        let mut bytes = b"MB".to_vec();
        bytes.extend_from_slice(&version.to_le_bytes());
        if version == 5 {
            bytes.extend_from_slice(&[2, normals, 3, 1]);
        }
        for count in [10_u16, 2, 1, 1] {
            bytes.extend_from_slice(&count.to_le_bytes());
        }
        if version == 5 {
            // No flat polygons, patterns, materials or palette entries.
            bytes.extend_from_slice(&[0; 10]);
        }
        let required = 10 * size_of::<Vector3D>() * if normals == 0 { 1 } else { 2 }
            + 4 * size_of::<Face>()
            + size_of::<Bone>();
        assert_eq!(
            FigureData::parse(
                &bytes,
                LoaderLimits {
                    decoded_bytes: required - 1,
                    ..LoaderLimits::default()
                }
            )
            .unwrap_err()
            .code(),
            "resource-budget"
        );
    }
}

#[test]
fn packed_figure_with_and_without_normals_fits_its_exact_decoded_budget() {
    for normals in [0, 2] {
        let mut bytes = b"MB\x05\0".to_vec();
        bytes.extend_from_slice(&[2, normals, 3, 1]);
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&[0; 16]);
        bytes.extend_from_slice(&[0; 4]);
        if normals != 0 {
            bytes.extend_from_slice(&[0; 2]);
        }
        bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");
        let limits = LoaderLimits {
            decoded_bytes: size_of::<Vector3D>() * if normals == 0 { 1 } else { 2 },
            ..LoaderLimits::default()
        };
        let figure = FigureData::parse(&bytes, limits).unwrap();
        assert_eq!(figure.vertices, [Vector3D::default()]);
        assert_eq!(figure.normals.len(), usize::from(normals != 0));
        assert_eq!(figure.allocated_bytes(), limits.decoded_bytes);
        assert_eq!(
            FigureData::parse(
                &bytes,
                LoaderLimits {
                    decoded_bytes: limits.decoded_bytes - 1,
                    ..limits
                }
            )
            .unwrap_err()
            .code(),
            "resource-budget"
        );
    }
}
