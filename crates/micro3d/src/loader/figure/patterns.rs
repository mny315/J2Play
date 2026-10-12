//! Pattern visibility and texture-material ranges in the Figure face table.

use super::{Cursor, EmuError, Face, checked_mul, loader_error};

#[derive(Debug)]
pub(super) struct PatternPolygons<'a> {
    bytes: &'a [u8],
    row_bytes: usize,
}

struct PatternPolygonCounts<'a> {
    flat_triangles: usize,
    flat_quads: usize,
    materials: &'a [[u8; 4]],
}

impl PatternPolygons<'_> {
    fn iter(&self) -> impl ExactSizeIterator<Item = PatternPolygonCounts<'_>> {
        self.bytes
            .chunks_exact(self.row_bytes)
            .map(|row| PatternPolygonCounts {
                flat_triangles: usize::from(u16::from_le_bytes([row[0], row[1]])),
                flat_quads: usize::from(u16::from_le_bytes([row[2], row[3]])),
                materials: row[4..].as_chunks::<4>().0,
            })
    }
}

impl PatternPolygonCounts<'_> {
    fn materials(&self) -> impl ExactSizeIterator<Item = [usize; 2]> {
        self.materials.iter().map(|&[a, b, c, d]| {
            [
                usize::from(u16::from_le_bytes([a, b])),
                usize::from(u16::from_le_bytes([c, d])),
            ]
        })
    }
}

pub(super) fn parse_patterns<'a>(
    cursor: &mut Cursor<'a>,
    pattern_count: u16,
    material_count: usize,
) -> Result<PatternPolygons<'a>, EmuError> {
    let row_bytes = checked_mul(material_count, 4, "Figure material counts")?
        .checked_add(4)
        .ok_or_else(|| loader_error("resource-overflow", "Figure pattern row size overflow"))?;
    let bytes = cursor.take(checked_mul(
        usize::from(pattern_count),
        row_bytes,
        "Figure patterns",
    )?)?;
    // The table is needed only while validating and annotating decoded faces.
    // Keep its checked wire representation instead of expanding every u16
    // count into a host-sized integer and allocating one Vec per pattern.
    Ok(PatternPolygons { bytes, row_bytes })
}

pub(super) fn validate_pattern_polygon_counts(
    patterns: &PatternPolygons<'_>,
    triangles: usize,
    quads: usize,
    flat_triangles: usize,
    flat_quads: usize,
) -> Result<(), EmuError> {
    let mut decoded = [0_usize; 4];
    for pattern in patterns.iter() {
        decoded[2] = decoded[2]
            .checked_add(pattern.flat_triangles)
            .ok_or_else(|| {
                loader_error("figure-pattern-overflow", "flat triangle count overflow")
            })?;
        decoded[3] = decoded[3]
            .checked_add(pattern.flat_quads)
            .ok_or_else(|| loader_error("figure-pattern-overflow", "flat quad count overflow"))?;
        for [material_triangles, material_quads] in pattern.materials() {
            decoded[0] = decoded[0].checked_add(material_triangles).ok_or_else(|| {
                loader_error(
                    "figure-pattern-overflow",
                    "textured triangle count overflow",
                )
            })?;
            decoded[1] = decoded[1].checked_add(material_quads).ok_or_else(|| {
                loader_error("figure-pattern-overflow", "textured quad count overflow")
            })?;
        }
    }
    if decoded != [triangles, quads, flat_triangles, flat_quads] {
        return Err(loader_error(
            "figure-pattern-count",
            format!(
                "pattern/material polygon counts {decoded:?} do not match header {:?}",
                [triangles, quads, flat_triangles, flat_quads]
            ),
        ));
    }
    Ok(())
}

pub(super) fn annotate_pattern_materials(
    faces: &mut [Face],
    patterns: &PatternPolygons<'_>,
    triangles: usize,
    quads: usize,
    flat_triangles: usize,
    flat_quads: usize,
) -> Result<(), EmuError> {
    let textured_faces = triangles
        .checked_add(quads.saturating_mul(2))
        .ok_or_else(|| loader_error("figure-face-overflow", "textured face count overflow"))?;
    let mut textured_triangle = 0_usize;
    let mut textured_quad = triangles;
    let mut flat_triangle = textured_faces;
    let mut flat_quad = textured_faces.saturating_add(flat_triangles);

    let annotate = |faces: &mut [Face],
                    cursor: &mut usize,
                    count: usize,
                    pattern: u16,
                    material: Option<u16>|
     -> Result<(), EmuError> {
        let end = cursor
            .checked_add(count)
            .filter(|end| *end <= faces.len())
            .ok_or_else(|| {
                loader_error(
                    "figure-pattern-range",
                    "pattern/material polygon range exceeds decoded faces",
                )
            })?;
        for face in &mut faces[*cursor..end] {
            face.pattern = pattern;
            face.material = material;
        }
        *cursor = end;
        Ok(())
    };

    for (pattern_index, pattern) in patterns.iter().enumerate() {
        let pattern_index = u16::try_from(pattern_index)
            .map_err(|_| loader_error("figure-pattern-count", "pattern index exceeds u16"))?;
        for (material_index, [material_triangles, material_quads]) in
            pattern.materials().enumerate()
        {
            let material_index = u16::try_from(material_index)
                .map_err(|_| loader_error("figure-material-count", "material index exceeds u16"))?;
            annotate(
                faces,
                &mut textured_triangle,
                material_triangles,
                pattern_index,
                Some(material_index),
            )?;
            annotate(
                faces,
                &mut textured_quad,
                material_quads.saturating_mul(2),
                pattern_index,
                Some(material_index),
            )?;
        }
        annotate(
            faces,
            &mut flat_triangle,
            pattern.flat_triangles,
            pattern_index,
            None,
        )?;
        annotate(
            faces,
            &mut flat_quad,
            pattern.flat_quads.saturating_mul(2),
            pattern_index,
            None,
        )?;
    }

    let expected = [
        triangles,
        textured_faces,
        textured_faces.saturating_add(flat_triangles),
        textured_faces
            .saturating_add(flat_triangles)
            .saturating_add(flat_quads.saturating_mul(2)),
    ];
    if [textured_triangle, textured_quad, flat_triangle, flat_quad] != expected {
        return Err(loader_error(
            "figure-pattern-range",
            "pattern/material polygon ranges do not cover the decoded faces",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/unit/micro3d/loader/figure/patterns.rs"]
mod tests;
