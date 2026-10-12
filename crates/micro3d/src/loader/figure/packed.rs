//! MBAC version 5 packed vertices, normals and polygons.

use super::{BitReader, EmuError, Face, Vector3D, loader_error};

pub(super) fn decode_vertices(
    bytes: &[u8],
    offset: usize,
    count: usize,
) -> Result<(Vec<Vector3D>, usize), EmuError> {
    let mut reader = BitReader::new(bytes, offset)?;
    let mut vertices = Vec::with_capacity(count);
    while vertices.len() < count {
        let block = usize::try_from(reader.unsigned(6)?).unwrap_or(usize::MAX) + 1;
        let range = usize::try_from(reader.unsigned(2)?).unwrap_or(usize::MAX);
        let bits = [8, 10, 13, 16]
            .get(range)
            .copied()
            .ok_or_else(|| loader_error("figure-vertex-range", "invalid vertex block range"))?;
        if block > count - vertices.len() {
            return Err(loader_error(
                "figure-vertex-count",
                "vertex block exceeds declared count",
            ));
        }
        for _ in 0..block {
            let x = reader.signed(bits)?;
            let y = reader.signed(bits)?;
            let z = reader.signed(bits)?;
            vertices.push(Vector3D::new(x, y, z));
        }
    }
    Ok((vertices, reader.aligned_position()))
}

pub(super) fn decode_normals(
    bytes: &[u8],
    offset: usize,
    count: usize,
) -> Result<(Vec<Vector3D>, usize), EmuError> {
    let mut reader = BitReader::new(bytes, offset)?;
    let mut normals = Vec::with_capacity(count);
    for _ in 0..count {
        let x = reader.signed(7)?;
        if x == -64 {
            let direction = reader.unsigned(3)?;
            let encoded = match direction {
                0 => Vector3D::new(4096, 0, 0),
                1 => Vector3D::new(-4096, 0, 0),
                2 => Vector3D::new(0, 4096, 0),
                3 => Vector3D::new(0, -4096, 0),
                4 => Vector3D::new(0, 0, 4096),
                5 => Vector3D::new(0, 0, -4096),
                _ => {
                    return Err(loader_error(
                        "figure-normal",
                        "reserved packed normal direction",
                    ));
                }
            };
            normals.push(encoded);
        } else {
            let y = reader.signed(7)?;
            let negative_z = reader.unsigned(1)? != 0;
            let x_scaled = x.saturating_mul(64);
            let y_scaled = y.saturating_mul(64);
            let square = i64::from(4096_i32).pow(2)
                - i64::from(x_scaled).pow(2)
                - i64::from(y_scaled).pow(2);
            let z = u64::try_from(square).unwrap_or(0).isqrt() as i32;
            normals.push(Vector3D::new(
                x_scaled,
                y_scaled,
                if negative_z { -z } else { z },
            ));
        }
    }
    Ok((normals, reader.aligned_position()))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn decode_version_five_polygons(
    bytes: &[u8],
    offset: usize,
    triangles: usize,
    quads: usize,
    flat_triangles: usize,
    flat_quads: usize,
    color_count: usize,
    vertex_count: usize,
) -> Result<(Vec<Face>, u8, usize), EmuError> {
    let mut reader = BitReader::new(bytes, offset)?;
    let output_count = triangles
        .checked_add(quads.saturating_mul(2))
        .and_then(|count| count.checked_add(flat_triangles))
        .and_then(|count| count.checked_add(flat_quads.saturating_mul(2)))
        .ok_or_else(|| loader_error("figure-face-overflow", "decoded face count overflow"))?;
    let mut output = Vec::with_capacity(output_count);
    if flat_triangles != 0 || flat_quads != 0 {
        let attribute_bits = usize::try_from(reader.unsigned(8)?).unwrap_or(usize::MAX);
        let index_bits = usize::try_from(reader.unsigned(8)?).unwrap_or(usize::MAX);
        let color_bits = usize::try_from(reader.unsigned(8)?).unwrap_or(usize::MAX);
        let color_index_bits = usize::try_from(reader.unsigned(8)?).unwrap_or(usize::MAX);
        let _metadata = reader.unsigned(8)?;
        if attribute_bits > 32
            || index_bits == 0
            || index_bits > 32
            || color_bits > 8
            || color_index_bits > 32
            || color_count == 0
        {
            return Err(loader_error(
                "figure-flat-bit-width",
                "Figure flat-polygon bit width is invalid",
            ));
        }
        let mut colors = Vec::with_capacity(color_count);
        for _ in 0..color_count {
            let red = u32::try_from(reader.unsigned(color_bits)?).unwrap_or(0);
            let green = u32::try_from(reader.unsigned(color_bits)?).unwrap_or(0);
            let blue = u32::try_from(reader.unsigned(color_bits)?).unwrap_or(0);
            colors.push(0xff00_0000 | red << 16 | green << 8 | blue);
        }
        for vertices in
            std::iter::repeat_n(3, flat_triangles).chain(std::iter::repeat_n(4, flat_quads))
        {
            let attributes = u32::try_from(reader.unsigned(attribute_bits)?).map_err(|_| {
                loader_error("figure-flat-attribute", "flat face attributes exceed u32")
            })? << 1;
            let mut indices = [0_u32; 4];
            for index in indices.iter_mut().take(vertices) {
                *index = u32::try_from(reader.unsigned(index_bits)?).map_err(|_| {
                    loader_error("figure-flat-index", "flat face index exceeds u32")
                })?;
                if usize::try_from(*index).unwrap_or(usize::MAX) >= vertex_count {
                    return Err(loader_error(
                        "figure-flat-index",
                        "flat face index is outside the vertex table",
                    ));
                }
            }
            let color_index =
                usize::try_from(reader.unsigned(color_index_bits)?).unwrap_or(usize::MAX);
            let color = *colors.get(color_index).ok_or_else(|| {
                loader_error("figure-flat-color", "flat face color index is invalid")
            })?;
            output.push(Face {
                indices: [indices[0], indices[1], indices[2]],
                uv: [[0; 2]; 3],
                attributes,
                color: Some(color),
                pattern: 0,
                material: None,
            });
            if vertices == 4 {
                output.push(Face {
                    indices: [indices[2], indices[1], indices[3]],
                    uv: [[0; 2]; 3],
                    attributes,
                    color: Some(color),
                    pattern: 0,
                    material: None,
                });
            }
        }
    }

    let flat_count = output.len();
    let uv_bits = if triangles != 0 || quads != 0 {
        let attribute_bits = usize::try_from(reader.unsigned(8)?).unwrap_or(usize::MAX);
        let index_bits = usize::try_from(reader.unsigned(8)?).unwrap_or(usize::MAX);
        let uv_bits = usize::try_from(reader.unsigned(8)?).unwrap_or(usize::MAX);
        let _metadata = reader.unsigned(8)?;
        if attribute_bits > 32 || index_bits == 0 || index_bits > 32 || uv_bits > 32 {
            return Err(loader_error(
                "figure-bit-width",
                "Figure textured-polygon bit width is invalid",
            ));
        }
        for _ in 0..triangles {
            output.push(
                read_face(
                    &mut reader,
                    3,
                    attribute_bits,
                    index_bits,
                    uv_bits,
                    vertex_count,
                )?[0],
            );
        }
        for _ in 0..quads {
            let decoded = read_face(
                &mut reader,
                4,
                attribute_bits,
                index_bits,
                uv_bits,
                vertex_count,
            )?;
            output.extend(decoded);
        }
        u8::try_from(uv_bits).unwrap_or(32)
    } else {
        0
    };
    // The file stores flat faces first, while pattern/material ranges address
    // textured faces first. Reorder the one allocated table in place.
    output.rotate_left(flat_count);
    Ok((output, uv_bits, reader.aligned_position()))
}

fn read_face(
    reader: &mut BitReader<'_>,
    vertices: usize,
    attribute_bits: usize,
    index_bits: usize,
    uv_bits: usize,
    vertex_count: usize,
) -> Result<[Face; 2], EmuError> {
    let attributes = u32::try_from(reader.unsigned(attribute_bits)?)
        .map_err(|_| loader_error("figure-face-attribute", "face attributes exceed u32"))?;
    let mut indices = [0_u32; 4];
    for index in indices.iter_mut().take(vertices) {
        *index = u32::try_from(reader.unsigned(index_bits)?)
            .map_err(|_| loader_error("figure-index", "face index exceeds u32"))?;
        if usize::try_from(*index).unwrap_or(usize::MAX) >= vertex_count {
            return Err(loader_error(
                "figure-index",
                "face index is outside the vertex table",
            ));
        }
    }
    let mut uv = [[0_u32; 2]; 4];
    for coordinate in uv.iter_mut().take(vertices) {
        coordinate[0] = u32::try_from(reader.unsigned(uv_bits)?)
            .map_err(|_| loader_error("figure-uv", "texture coordinate exceeds u32"))?;
        coordinate[1] = u32::try_from(reader.unsigned(uv_bits)?)
            .map_err(|_| loader_error("figure-uv", "texture coordinate exceeds u32"))?;
    }
    let first = Face {
        indices: [indices[0], indices[1], indices[2]],
        uv: [uv[0], uv[1], uv[2]],
        attributes,
        color: None,
        pattern: 0,
        material: Some(0),
    };
    if vertices == 3 {
        // The triangle caller reads only element zero. A fixed pair also
        // handles quads without allocating a temporary Vec per polygon.
        Ok([first, first])
    } else {
        Ok([
            first,
            Face {
                // MBAC stores quad vertices in triangle-strip order. Reverse
                // the shared edge for the second triangle so both halves
                // retain the same winding.
                indices: [indices[2], indices[1], indices[3]],
                uv: [uv[2], uv[1], uv[3]],
                attributes,
                color: None,
                pattern: 0,
                material: Some(0),
            },
        ])
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/micro3d/loader/figure/packed.rs"]
mod tests;
