//! MBAC version 3 tables with unpacked coordinates and texture indices.

use super::{
    AffineTrans, BONE_BYTES, Bone, Cursor, EmuError, Face, FigureData, LoaderLimits, TRAILER_BYTES,
    Vector3D, checked_mul, loader_error, normalize_bone_parent, validate_counts,
    validate_figure_bytes, validate_retained_bytes, validate_trailer,
};

pub(super) fn parse_version_three(
    bytes: &[u8],
    mut cursor: Cursor<'_>,
    limits: LoaderLimits,
) -> Result<FigureData, EmuError> {
    let vertex_count = usize::from(cursor.u16()?);
    let triangle_count = usize::from(cursor.u16()?);
    let quad_count = usize::from(cursor.u16()?);
    let bone_count = usize::from(cursor.u16()?);
    let decoded_faces = validate_counts(
        vertex_count,
        triangle_count,
        quad_count,
        0,
        0,
        bone_count,
        limits,
    )?;
    validate_figure_bytes(vertex_count, 0, decoded_faces, bone_count, limits)?;
    let bone_bytes = checked_mul(bone_count, BONE_BYTES, "bone table")?;
    let body_end = bytes
        .len()
        .checked_sub(TRAILER_BYTES + bone_bytes)
        .ok_or_else(|| loader_error("figure-truncated", "Version 3 Figure is truncated"))?;
    let vertex_bytes = checked_mul(vertex_count, 6, "version 3 vertex table")?;
    if cursor.position().saturating_add(vertex_bytes) > body_end {
        return Err(loader_error(
            "figure-truncated",
            "Version 3 vertex table is truncated",
        ));
    }
    let mut vertices = Vec::with_capacity(vertex_count);
    for _ in 0..vertex_count {
        let x = i32::from(cursor.i16()?);
        let y = i32::from(cursor.i16()?);
        let z = i32::from(cursor.i16()?);
        vertices.push(Vector3D::new(x, y, z));
    }
    let mut faces = Vec::with_capacity(triangle_count.saturating_add(quad_count * 2));
    for _ in 0..triangle_count {
        let attributes = u32::from(cursor.u16()?);
        let indices = [cursor.u16()?, cursor.u16()?, cursor.u16()?];
        let uv = [cursor.u16()?, cursor.u16()?, cursor.u16()?];
        faces.push(version_three_face(attributes, indices, uv, vertex_count)?);
    }
    for _ in 0..quad_count {
        let attributes = u32::from(cursor.u16()?);
        let indices = [cursor.u16()?, cursor.u16()?, cursor.u16()?, cursor.u16()?];
        let uv = [cursor.u16()?, cursor.u16()?, cursor.u16()?, cursor.u16()?];
        faces.push(version_three_face(
            attributes,
            [indices[0], indices[1], indices[2]],
            [uv[0], uv[1], uv[2]],
            vertex_count,
        )?);
        faces.push(version_three_face(
            attributes,
            [indices[2], indices[1], indices[3]],
            [uv[2], uv[1], uv[3]],
            vertex_count,
        )?);
    }
    if cursor.position() != body_end {
        return Err(loader_error(
            "figure-version-3-body",
            "Version 3 polygon table does not end at the bone table",
        ));
    }
    let mut bones = Vec::with_capacity(bone_count);
    let mut covered_vertices = 0_usize;
    for index in 0..bone_count {
        let count = cursor.u16()?;
        let parent = normalize_bone_parent(index, cursor.i16()?, 3)?;
        covered_vertices = covered_vertices
            .checked_add(usize::from(count))
            .ok_or_else(|| loader_error("figure-bone-overflow", "Figure bone coverage overflow"))?;
        let mut values = [0_i32; 12];
        for value in &mut values {
            *value = i32::from(cursor.i16()?);
        }
        bones.push(Bone {
            vertex_count: count,
            parent,
            transform: AffineTrans::new(values),
        });
    }
    if bone_count != 0 && covered_vertices != vertex_count {
        let ranges = bones
            .iter()
            .take(16)
            .map(|bone| (bone.vertex_count, bone.parent))
            .collect::<Vec<_>>();
        return Err(loader_error(
            "figure-bone-coverage",
            format!(
                "Version 3 Figure bone ranges cover {covered_vertices} of {vertex_count} vertices; first ranges={ranges:?}"
            ),
        ));
    }
    if cursor.position() != bytes.len() - TRAILER_BYTES {
        return Err(loader_error(
            "figure-version-3-body",
            "Version 3 bone table does not end at the trailer",
        ));
    }
    validate_trailer(&bytes[bytes.len() - TRAILER_BYTES..])?;
    let figure = FigureData {
        format_version: 3,
        uv_bits: 8,
        vertices,
        normals: Vec::new(),
        faces,
        bones,
        pattern_count: 0,
        material_count: 1,
    };
    validate_retained_bytes(figure.allocated_bytes(), limits, "Figure")?;
    Ok(figure)
}

fn version_three_face(
    attributes: u32,
    indices: [u16; 3],
    packed_uv: [u16; 3],
    vertex_count: usize,
) -> Result<Face, EmuError> {
    if indices
        .iter()
        .any(|index| usize::from(*index) >= vertex_count)
    {
        return Err(loader_error(
            "figure-index",
            "Version 3 polygon references an invalid vertex",
        ));
    }
    Ok(Face {
        indices: indices.map(u32::from),
        uv: packed_uv.map(|value| [u32::from(value & 0xff), u32::from(value >> 8)]),
        attributes,
        color: None,
        pattern: 0,
        material: Some(0),
    })
}
