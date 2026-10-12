use super::*;

fn push_bits(bytes: &mut Vec<u8>, bit: &mut usize, value: u64, width: usize) {
    for source in 0..width {
        if *bit / 8 == bytes.len() {
            bytes.push(0);
        }
        bytes[*bit / 8] |= (((value >> source) & 1) as u8) << (*bit % 8);
        *bit += 1;
    }
}

#[test]
fn version_five_vertices_preserve_resource_axes() {
    let mut bytes = Vec::new();
    let mut bit = 0;
    push_bits(&mut bytes, &mut bit, 0, 6);
    push_bits(&mut bytes, &mut bit, 0, 2);
    for value in [7_i8, -11, 13] {
        push_bits(&mut bytes, &mut bit, u64::from(value.cast_unsigned()), 8);
    }

    let (vertices, end) = decode_vertices(&bytes, 0, 1).unwrap();
    assert_eq!(end, bytes.len());
    assert_eq!(vertices, [Vector3D::new(7, -11, 13)]);
}

#[test]
fn version_five_normals_preserve_resource_axes() {
    let mut bytes = Vec::new();
    let mut bit = 0;
    for direction in 0..6 {
        push_bits(&mut bytes, &mut bit, 64, 7);
        push_bits(&mut bytes, &mut bit, direction, 3);
    }

    let (normals, end) = decode_normals(&bytes, 0, 6).unwrap();
    assert_eq!(end, bytes.len());
    assert_eq!(
        normals,
        [
            Vector3D::new(4096, 0, 0),
            Vector3D::new(-4096, 0, 0),
            Vector3D::new(0, 4096, 0),
            Vector3D::new(0, -4096, 0),
            Vector3D::new(0, 0, 4096),
            Vector3D::new(0, 0, -4096),
        ]
    );
}

#[test]
fn version_five_flat_faces_preserve_color_and_textured_stream_alignment() {
    let mut bytes = Vec::new();
    let mut bit = 0;
    for value in [1, 2, 8, 1, 0] {
        push_bits(&mut bytes, &mut bit, value, 8);
    }
    for value in [10, 20, 30, 40, 50, 60] {
        push_bits(&mut bytes, &mut bit, value, 8);
    }
    push_bits(&mut bytes, &mut bit, 1, 1);
    for value in [0, 1, 2] {
        push_bits(&mut bytes, &mut bit, value, 2);
    }
    push_bits(&mut bytes, &mut bit, 1, 1);

    for value in [1, 2, 8, 0] {
        push_bits(&mut bytes, &mut bit, value, 8);
    }
    push_bits(&mut bytes, &mut bit, 1, 1);
    for value in [0, 1, 2] {
        push_bits(&mut bytes, &mut bit, value, 2);
    }
    for value in [1, 2, 3, 4, 5, 6] {
        push_bits(&mut bytes, &mut bit, value, 8);
    }

    let (faces, uv_bits, end) = decode_version_five_polygons(&bytes, 0, 1, 0, 1, 0, 2, 3).unwrap();
    assert_eq!(uv_bits, 8);
    assert_eq!(end, bytes.len());
    assert_eq!(faces.len(), 2);
    assert_eq!(faces[0].indices, [0, 1, 2]);
    assert_eq!(faces[0].uv, [[1, 2], [3, 4], [5, 6]]);
    assert_eq!(faces[0].color, None);
    assert_eq!(faces[1].indices, [0, 1, 2]);
    assert_eq!(faces[1].attributes, 2);
    assert_eq!(faces[1].color, Some(0xff28_323c));
}

#[test]
fn packed_quad_expands_to_two_faces_with_triangle_strip_winding() {
    let mut bytes = Vec::new();
    let mut bit = 0;
    for value in [3, 2, 8, 0] {
        push_bits(&mut bytes, &mut bit, value, 8);
    }
    push_bits(&mut bytes, &mut bit, 7, 3);
    for index in [0, 3, 1, 2] {
        push_bits(&mut bytes, &mut bit, index, 2);
    }
    for uv in [1, 2, 3, 4, 5, 6, 7, 8] {
        push_bits(&mut bytes, &mut bit, uv, 8);
    }
    let (faces, uv_bits, end) = decode_version_five_polygons(&bytes, 0, 0, 1, 0, 0, 0, 4).unwrap();
    assert_eq!(end, bytes.len());
    assert_eq!(uv_bits, 8);
    assert_eq!(faces.len(), 2);
    assert_eq!(faces[0].indices, [0, 3, 1]);
    assert_eq!(faces[1].indices, [1, 3, 2]);
    assert_eq!(faces[0].uv, [[1, 2], [3, 4], [5, 6]]);
    assert_eq!(faces[1].uv, [[5, 6], [3, 4], [7, 8]]);
    assert!(faces.iter().all(|face| face.attributes == 7));
}
