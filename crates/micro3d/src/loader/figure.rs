//! Figure geometry, skeletons and declared allocation bounds.

use super::{
    AffineTrans, BitReader, Cursor, EmuError, LoaderLimits, TRAILER_BYTES, Vector3D, bounded_file,
    checked_mul, loader_error, validate_retained_bytes, validate_trailer,
};

mod packed;
mod patterns;
mod version_three;

use packed::{decode_normals, decode_version_five_polygons, decode_vertices};
use patterns::{annotate_pattern_materials, parse_patterns, validate_pattern_polygon_counts};
use version_three::parse_version_three;

const BONE_BYTES: usize = 28;

/// One decoded triangle. UV values remain integer texel coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Face {
    pub indices: [u32; 3],
    pub uv: [[u32; 2]; 3],
    pub attributes: u32,
    pub color: Option<u32>,
    /// MBAC external-appearance group. Group zero is always visible; groups
    /// one and above are enabled by the corresponding Figure pattern bit.
    pub pattern: u16,
    /// Texture material slot for textured polygons. Flat-colored polygons do
    /// not reference a material texture.
    pub material: Option<u16>,
}

/// One skeleton segment from the MBAC bone table.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bone {
    pub vertex_count: u16,
    pub parent: i16,
    pub transform: AffineTrans,
}

/// Atomically decoded Figure resource.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FigureData {
    pub format_version: u16,
    /// Packed bit width of texture coordinates stored in the MBAC polygon stream.
    pub uv_bits: u8,
    pub vertices: Vec<Vector3D>,
    pub normals: Vec<Vector3D>,
    pub faces: Vec<Face>,
    pub bones: Vec<Bone>,
    pub pattern_count: u16,
    pub material_count: u16,
}

impl FigureData {
    /// Parses the documented MBAC v5 2/{0,2}/3 encodings used by deployed content.
    pub fn parse(bytes: &[u8], limits: LoaderLimits) -> Result<Self, EmuError> {
        bounded_file(bytes, limits)?;
        let mut cursor = Cursor::new(bytes);
        if cursor.take(2)? != b"MB" {
            return Err(loader_error(
                "figure-magic",
                "Figure resource does not start with MB",
            ));
        }
        let format_version = cursor.u16()?;
        if format_version == 3 {
            return parse_version_three(bytes, cursor, limits);
        }
        if format_version != 5 {
            return Err(loader_error(
                "figure-version",
                format!("unsupported MBAC format version {format_version}"),
            ));
        }
        let vertex_format = cursor.u8()?;
        let normal_format = cursor.u8()?;
        let polygon_format = cursor.u8()?;
        let bone_format = cursor.u8()?;
        if vertex_format != 2
            || !matches!(normal_format, 0 | 2)
            || polygon_format != 3
            || bone_format != 1
        {
            return Err(loader_error(
                "figure-encoding",
                format!(
                    "unsupported MBAC encodings {vertex_format}/{normal_format}/{polygon_format}/{bone_format}"
                ),
            ));
        }
        let vertex_count = usize::from(cursor.u16()?);
        let triangle_count = usize::from(cursor.u16()?);
        let quad_count = usize::from(cursor.u16()?);
        let bone_count = usize::from(cursor.u16()?);
        let flat_triangle_count = usize::from(cursor.u16()?);
        let flat_quad_count = usize::from(cursor.u16()?);
        let material_count = cursor.u16()?;
        let pattern_count = cursor.u16()?;
        let color_count = usize::from(cursor.u16()?);
        let decoded_faces = validate_counts(
            vertex_count,
            triangle_count,
            quad_count,
            flat_triangle_count,
            flat_quad_count,
            bone_count,
            limits,
        )?;
        validate_figure_bytes(
            vertex_count,
            if normal_format == 0 { 0 } else { vertex_count },
            decoded_faces,
            bone_count,
            limits,
        )?;
        let pattern_polygons =
            parse_patterns(&mut cursor, pattern_count, usize::from(material_count))?;
        validate_pattern_polygon_counts(
            &pattern_polygons,
            triangle_count,
            quad_count,
            flat_triangle_count,
            flat_quad_count,
        )?;

        let bone_bytes = checked_mul(bone_count, BONE_BYTES, "bone table")?;
        let body_end = bytes
            .len()
            .checked_sub(TRAILER_BYTES)
            .and_then(|value| value.checked_sub(bone_bytes))
            .ok_or_else(|| {
                loader_error("figure-truncated", "Figure has no complete body/trailer")
            })?;
        if cursor.position() >= body_end {
            return Err(loader_error("figure-truncated", "Figure body is empty"));
        }

        let (vertices, vertex_end) = decode_vertices(bytes, cursor.position(), vertex_count)?;
        cursor.set_position(vertex_end)?;
        let (normals, normal_end) = if normal_format == 0 {
            (Vec::new(), cursor.position())
        } else {
            decode_normals(bytes, cursor.position(), vertex_count)?
        };
        cursor.set_position(normal_end)?;
        let (mut faces, uv_bits, face_end) = decode_version_five_polygons(
            bytes,
            cursor.position(),
            triangle_count,
            quad_count,
            flat_triangle_count,
            flat_quad_count,
            color_count,
            vertex_count,
        )?;
        annotate_pattern_materials(
            &mut faces,
            &pattern_polygons,
            triangle_count,
            quad_count,
            flat_triangle_count,
            flat_quad_count,
        )?;
        if face_end > body_end {
            return Err(loader_error(
                "figure-face-overrun",
                format!(
                    "Figure faces from byte {} end at byte {face_end}, beyond bone table at byte {body_end}",
                    cursor.position()
                ),
            ));
        }
        faces.shrink_to_fit();
        let mut bone_cursor = Cursor::with_position(bytes, body_end)?;
        let mut bones = Vec::with_capacity(bone_count);
        let mut covered_vertices = 0_usize;
        for index in 0..bone_count {
            let count = bone_cursor.u16()?;
            let parent = normalize_bone_parent(index, bone_cursor.i16()?, 5)?;
            covered_vertices = covered_vertices
                .checked_add(usize::from(count))
                .ok_or_else(|| {
                    loader_error("figure-bone-overflow", "Figure bone coverage overflow")
                })?;
            let mut values = [0_i32; 12];
            for value in &mut values {
                *value = i32::from(bone_cursor.i16()?);
            }
            bones.push(Bone {
                vertex_count: count,
                parent,
                transform: AffineTrans::new(values),
            });
        }
        normalize_single_bone_vertex_range(&mut bones, &mut covered_vertices, vertex_count);
        if bone_count != 0 && covered_vertices != vertex_count {
            let ranges = bones
                .iter()
                .take(16)
                .map(|bone| (bone.vertex_count, bone.parent))
                .collect::<Vec<_>>();
            return Err(loader_error(
                "figure-bone-coverage",
                format!(
                    "Version 5 Figure bone ranges cover {covered_vertices} of {vertex_count} vertices; first ranges={ranges:?}"
                ),
            ));
        }
        validate_trailer(&bytes[bytes.len() - TRAILER_BYTES..])?;
        let figure = Self {
            format_version,
            uv_bits,
            vertices,
            normals,
            faces,
            bones,
            pattern_count,
            material_count,
        };
        validate_retained_bytes(figure.allocated_bytes(), limits, "Figure")?;
        Ok(figure)
    }

    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        self.vertices
            .capacity()
            .saturating_mul(size_of::<Vector3D>())
            .saturating_add(
                self.normals
                    .capacity()
                    .saturating_mul(size_of::<Vector3D>()),
            )
            .saturating_add(self.faces.capacity().saturating_mul(size_of::<Face>()))
            .saturating_add(self.bones.capacity().saturating_mul(size_of::<Bone>()))
    }
}

fn normalize_bone_parent(index: usize, encoded: i16, version: u16) -> Result<i16, EmuError> {
    // Both -1 and a self-reference on the first bone are emitted as the root
    // sentinel by deployed MBAC toolchains. Normalize the latter before the
    // runtime builds its acyclic, zero-based hierarchy.
    let parent = if version == 5 && index == 0 && encoded == 0 {
        -1
    } else {
        encoded
    };
    if parent >= i16::try_from(index).unwrap_or(i16::MAX) || parent < -1 {
        return Err(loader_error(
            "figure-bone-parent",
            format!("Version {version} Figure bone {index} has invalid parent {encoded}"),
        ));
    }
    Ok(parent)
}

fn normalize_single_bone_vertex_range(
    bones: &mut [Bone],
    covered_vertices: &mut usize,
    vertex_count: usize,
) {
    // A deployed MBAC v5 exporter writes zero for the only bone's range to
    // mark a static Figure. Its remaining bone payload is padding rather than
    // a bind matrix, so the static root owns every vertex with identity pose.
    if *covered_vertices == 0
        && let [root] = bones
        && root.vertex_count == 0
        && let Ok(count) = u16::try_from(vertex_count)
    {
        root.vertex_count = count;
        root.transform = AffineTrans::IDENTITY;
        *covered_vertices = vertex_count;
    }
}

fn validate_counts(
    vertices: usize,
    triangles: usize,
    quads: usize,
    flat_triangles: usize,
    flat_quads: usize,
    bones: usize,
    limits: LoaderLimits,
) -> Result<usize, EmuError> {
    let decoded_faces = triangles
        .checked_add(quads.saturating_mul(2))
        .and_then(|value| value.checked_add(flat_triangles))
        .and_then(|value| value.checked_add(flat_quads.saturating_mul(2)))
        .ok_or_else(|| loader_error("figure-face-overflow", "Figure face count overflow"))?;
    if vertices == 0
        || vertices > limits.vertices
        || decoded_faces > limits.faces
        || bones > limits.bones
    {
        return Err(loader_error(
            "figure-budget",
            "Figure object counts exceed parser budgets",
        ));
    }
    Ok(decoded_faces)
}

fn validate_figure_bytes(
    vertices: usize,
    normals: usize,
    faces: usize,
    bones: usize,
    limits: LoaderLimits,
) -> Result<(), EmuError> {
    let bytes = [
        (vertices, size_of::<Vector3D>()),
        (normals, size_of::<Vector3D>()),
        (faces, size_of::<Face>()),
        (bones, size_of::<Bone>()),
    ]
    .into_iter()
    .try_fold(0_usize, |total, (count, item_bytes)| {
        total
            .checked_add(checked_mul(count, item_bytes, "Figure decoded table")?)
            .ok_or_else(|| loader_error("resource-overflow", "Figure decoded size overflow"))
    })?;
    validate_retained_bytes(bytes, limits, "Figure")
}

#[cfg(test)]
#[path = "../../../../tests/unit/micro3d/loader/figure/mod.rs"]
mod tests;
