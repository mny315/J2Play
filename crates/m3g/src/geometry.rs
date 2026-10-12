//! Validated vertex/index storage, morphing, skinning and picking primitives.

use crate::{LightSource, Mat4, MaterialState, Vec3, Vec4, Vertex};
use diagnostics::{Category, EmuError};
use std::sync::Arc;

mod morphing;
mod picking;
mod skinning;
mod vertices;
pub use morphing::MorphedVertices;
pub use picking::{Ray, RayHit};
mod vertex_array;
pub use skinning::SkinInfluence;
pub use vertex_array::{VertexArrayState, VertexComponent};

pub(crate) const MAX_TRIANGLE_STRIP_INDICES: usize = 786_432;

/// Aggregated vertex attributes with JSR scale/bias semantics.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VertexBufferState {
    positions: Option<(VertexArrayState, f32, [f32; 3])>,
    normals: Option<VertexArrayState>,
    colors: Option<VertexArrayState>,
    texture_coordinates: [Option<(VertexArrayState, f32, [f32; 3])>; 2],
    default_color: u32,
}

impl Default for VertexBufferState {
    fn default() -> Self {
        Self {
            positions: None,
            normals: None,
            colors: None,
            texture_coordinates: [None, None],
            default_color: 0xffff_ffff,
        }
    }
}

impl VertexBufferState {
    pub(crate) fn validate_checkpoint(&self) -> Result<(), EmuError> {
        for (slot, array) in self.attribute_arrays().into_iter().enumerate() {
            if let Some(array) = array {
                array.validate_checkpoint()?;
                self.validate_attribute(slot, Some(array))?;
            }
        }
        for (_, scale, bias) in self
            .positions
            .iter()
            .chain(self.texture_coordinates.iter().flatten())
        {
            validate_scale_bias(*scale, bias)?;
        }
        Ok(())
    }

    /// Assigns positions with a finite scale and three-component bias.
    pub fn set_positions(
        &mut self,
        array: Option<VertexArrayState>,
        scale: f32,
        bias: [f32; 3],
    ) -> Result<(), EmuError> {
        validate_scale_bias(scale, &bias)?;
        self.validate_attribute(0, array.as_ref())?;
        self.positions = array.map(|array| (array, scale, bias));
        Ok(())
    }

    /// Assigns three-component normals.
    pub fn set_normals(&mut self, array: Option<VertexArrayState>) -> Result<(), EmuError> {
        self.validate_attribute(1, array.as_ref())?;
        self.normals = array;
        Ok(())
    }

    /// Assigns RGB or RGBA colors.
    pub fn set_colors(&mut self, array: Option<VertexArrayState>) -> Result<(), EmuError> {
        self.validate_attribute(2, array.as_ref())?;
        self.colors = array;
        Ok(())
    }

    /// Assigns texture coordinates to one software-backend texture unit.
    pub fn set_texture_coordinates(
        &mut self,
        unit: usize,
        array: Option<VertexArrayState>,
        scale: f32,
        bias: [f32; 3],
    ) -> Result<(), EmuError> {
        if unit >= self.texture_coordinates.len() {
            return Err(geometry_error(
                "texture-unit",
                "texture unit exceeds the software backend capacity",
            ));
        }
        validate_scale_bias(scale, &bias)?;
        self.validate_attribute(3 + unit, array.as_ref())?;
        self.texture_coordinates[unit] = array.map(|array| (array, scale, bias));
        Ok(())
    }

    /// Sets the fallback ARGB color.
    pub fn set_default_color(&mut self, color: u32) {
        self.default_color = color;
    }

    /// Returns the common vertex count, or zero when no arrays are attached.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.attribute_arrays()
            .into_iter()
            .flatten()
            .next()
            .map_or(0, VertexArrayState::vertex_count)
    }

    /// Position array and its scale/bias tuple.
    #[must_use]
    pub fn positions(&self) -> Option<(&VertexArrayState, f32, [f32; 3])> {
        self.positions
            .as_ref()
            .map(|(array, scale, bias)| (array, *scale, *bias))
    }

    /// Normal array.
    #[must_use]
    pub fn normals(&self) -> Option<&VertexArrayState> {
        self.normals.as_ref()
    }

    /// Color array.
    #[must_use]
    pub fn colors(&self) -> Option<&VertexArrayState> {
        self.colors.as_ref()
    }

    /// Texture array and scale/bias tuple for a valid unit.
    #[must_use]
    pub fn texture_coordinates(&self, unit: usize) -> Option<(&VertexArrayState, f32, [f32; 3])> {
        self.texture_coordinates
            .get(unit)
            .and_then(Option::as_ref)
            .map(|(array, scale, bias)| (array, *scale, *bias))
    }

    /// Current fallback color.
    #[must_use]
    pub const fn default_color(&self) -> u32 {
        self.default_color
    }

    /// Logical bytes retained by decoded attribute snapshots.
    #[must_use]
    pub(crate) fn allocated_bytes(&self) -> usize {
        self.attribute_arrays()
            .into_iter()
            .flatten()
            .fold(0_usize, |total, array| {
                total.saturating_add(array.allocated_bytes())
            })
    }

    /// Whether this buffer contains a normal attribute.
    #[must_use]
    pub const fn has_normals(&self) -> bool {
        self.normals.is_some()
    }

    fn validate_count(&self, replaced: usize, count: usize) -> Result<(), EmuError> {
        if self
            .attribute_arrays()
            .into_iter()
            .enumerate()
            .any(|(slot, array)| {
                slot != replaced && array.is_some_and(|array| array.vertex_count() != count)
            })
        {
            return Err(geometry_error(
                "attribute-count",
                "all VertexBuffer arrays must have the same vertex count",
            ));
        }
        Ok(())
    }

    fn validate_attribute(
        &self,
        slot: usize,
        array: Option<&VertexArrayState>,
    ) -> Result<(), EmuError> {
        let Some(array) = array else { return Ok(()) };
        let components = array.component_count();
        let (valid, message) = match slot {
            0 => (
                components == 3,
                "position array must contain three components",
            ),
            1 => (
                components == 3,
                "normal array must contain three components",
            ),
            2 => (
                array.component_type() == VertexComponent::Byte && (3..=4).contains(&components),
                "color array must contain three or four byte components",
            ),
            _ => (
                (2..=3).contains(&components),
                "texture coordinate array must contain two or three components",
            ),
        };
        if !valid {
            return Err(geometry_error("attribute-shape", message));
        }
        self.validate_count(slot, array.vertex_count())
    }

    fn attribute_arrays(&self) -> [Option<&VertexArrayState>; 5] {
        [
            self.positions.as_ref().map(|(array, ..)| array),
            self.normals.as_ref(),
            self.colors.as_ref(),
            self.texture_coordinates[0]
                .as_ref()
                .map(|(array, ..)| array),
            self.texture_coordinates[1]
                .as_ref()
                .map(|(array, ..)| array),
        ]
    }
}

/// Validated strip indices and deterministic triangle expansion.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TriangleStripArrayState {
    indices: Arc<[u32]>,
    strips: Arc<[usize]>,
    triangle_count: usize,
}

/// Zero-allocation triangle expansion over validated strip storage.
#[derive(Debug)]
pub struct TriangleIter<'a> {
    indices: &'a [u32],
    strips: &'a [usize],
    strip_index: usize,
    strip_offset: usize,
    triangle_index: usize,
    remaining: usize,
}

impl Iterator for TriangleIter<'_> {
    type Item = [usize; 3];

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let length = *self.strips.get(self.strip_index)?;
            if self.triangle_index + 2 >= length {
                self.strip_offset += length;
                self.strip_index += 1;
                self.triangle_index = 0;
                continue;
            }
            let index = self.triangle_index;
            self.triangle_index += 1;
            let strip = &self.indices[self.strip_offset..self.strip_offset + length];
            let mut triangle = [
                strip[index] as usize,
                strip[index + 1] as usize,
                strip[index + 2] as usize,
            ];
            if index % 2 == 1 {
                triangle.swap(0, 1);
            }
            if triangle[0] != triangle[1]
                && triangle[1] != triangle[2]
                && triangle[0] != triangle[2]
            {
                self.remaining -= 1;
                return Some(triangle);
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for TriangleIter<'_> {}

impl TriangleStripArrayState {
    pub(crate) fn validate_checkpoint(&self) -> Result<(), EmuError> {
        if strip_index_count(&self.strips)? != self.indices.len()
            || self.triangle_count != triangle_count(&self.indices, &self.strips)
        {
            return Err(geometry_error(
                "checkpoint-strips",
                "Checkpoint triangle strips do not match their index data",
            ));
        }
        Ok(())
    }

    /// Creates explicit indexed strips.
    pub fn new(mut indices: Vec<u32>, strips: Vec<usize>) -> Result<Self, EmuError> {
        let required_indices = strip_index_count(&strips)?;
        if required_indices > indices.len() || indices.len() > MAX_TRIANGLE_STRIP_INDICES {
            return Err(geometry_error(
                "invalid-strips",
                "triangle strip lengths exceed bounded index data",
            ));
        }
        // JSR-184 only requires the source array to contain the combined
        // strip length. Content exporters commonly reuse a larger scratch
        // array, so the unused suffix must not become part of the IndexBuffer
        // or participate in deferred vertex-bounds validation.
        indices.truncate(required_indices);
        let triangle_count = triangle_count(&indices, &strips);
        Ok(Self {
            indices: indices.into(),
            strips: strips.into(),
            triangle_count,
        })
    }

    /// Creates consecutive strips beginning at `first_index`.
    pub fn implicit(first_index: u32, strips: Vec<usize>) -> Result<Self, EmuError> {
        let count = strip_index_count(&strips)?;
        let end = u32::try_from(count)
            .ok()
            .and_then(|count| first_index.checked_add(count))
            .ok_or_else(geometry_limit)?;
        // Consecutive indices cannot form degenerate triangles. Build their
        // final shared storage only after validating the complete size.
        let triangle_count = count - 2 * strips.len();
        Ok(Self {
            indices: (first_index..end).collect(),
            strips: strips.into(),
            triangle_count,
        })
    }

    /// Expands strips with alternating winding, omitting degenerate triangles.
    pub fn triangles(&self, vertex_count: usize) -> Result<TriangleIter<'_>, EmuError> {
        if self
            .indices
            .iter()
            .any(|index| usize::try_from(*index).map_or(true, |index| index >= vertex_count))
        {
            return Err(geometry_error(
                "index-bounds",
                "triangle strip references a missing vertex",
            ));
        }
        Ok(TriangleIter {
            indices: &self.indices,
            strips: &self.strips,
            strip_index: 0,
            strip_offset: 0,
            triangle_index: 0,
            remaining: self.triangle_count,
        })
    }

    /// Number of serialized indices.
    #[must_use]
    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    /// Original strip indices before triangle expansion.
    #[must_use]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Logical bytes retained by index and strip tables.
    #[must_use]
    pub(crate) fn allocated_bytes(&self) -> usize {
        self.indices
            .len()
            .saturating_mul(size_of::<u32>())
            .saturating_add(self.strips.len().saturating_mul(size_of::<usize>()))
    }
}

fn strip_index_count(strips: &[usize]) -> Result<usize, EmuError> {
    let count = strips.iter().try_fold(0_usize, |sum, &length| {
        if length < 3 {
            return None;
        }
        sum.checked_add(length)
    });
    count
        .filter(|count| (3..=MAX_TRIANGLE_STRIP_INDICES).contains(count))
        .ok_or_else(|| {
            geometry_error(
                "invalid-strips",
                "triangle strip lengths exceed bounded index data",
            )
        })
}

fn triangle_count(indices: &[u32], strips: &[usize]) -> usize {
    let mut count = 0_usize;
    let mut offset = 0_usize;
    for length in strips {
        let strip = &indices[offset..offset + length];
        for index in 0..length - 2 {
            if strip[index] != strip[index + 1]
                && strip[index + 1] != strip[index + 2]
                && strip[index] != strip[index + 2]
            {
                count += 1;
            }
        }
        offset += length;
    }
    count
}

fn validate_scale_bias(scale: f32, bias: &[f32; 3]) -> Result<(), EmuError> {
    if !scale.is_finite() || !bias.iter().all(|value| value.is_finite()) {
        return Err(geometry_error(
            "non-finite-attribute",
            "attribute scale or bias contains NaN or infinity",
        ));
    }
    Ok(())
}

fn geometry_limit() -> EmuError {
    geometry_error("resource-limit", "geometry exceeds configured budget")
}

fn vertex_bounds() -> EmuError {
    geometry_error("vertex-bounds", "vertex component index is out of range")
}

fn geometry_error(code: &'static str, message: &'static str) -> EmuError {
    EmuError::new(Category::M3g, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/m3g/geometry/mod.rs"]
mod tests;
