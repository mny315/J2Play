//! Shared immutable vertex components with bounded typed access.

use super::{EmuError, Vec4, geometry_error, geometry_limit, vertex_bounds};
use std::sync::Arc;

/// Serialized component width of a `VertexArray`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VertexComponent {
    /// Signed 8-bit components.
    Byte,
    /// Signed 16-bit components.
    Short,
}

impl VertexComponent {
    pub(super) fn normal(self, value: f32) -> f32 {
        let divisor = match self {
            Self::Byte => 255.0,
            Self::Short => 65_535.0,
        };
        (value * 2.0 + 1.0) / divisor
    }
}

/// Bounded signed component array.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VertexArrayState {
    vertex_count: usize,
    component_count: usize,
    component_type: VertexComponent,
    values: Arc<[i16]>,
}

impl VertexArrayState {
    /// Allocates a zero-initialized array.
    pub fn new(
        vertex_count: usize,
        component_count: usize,
        component_type: VertexComponent,
    ) -> Result<Self, EmuError> {
        let length = component_length(vertex_count, component_count)?;
        Ok(Self {
            vertex_count,
            component_count,
            component_type,
            values: std::iter::repeat_n(0, length).collect(),
        })
    }

    pub(crate) fn validate_checkpoint(&self) -> Result<(), EmuError> {
        if component_length(self.vertex_count, self.component_count)? != self.values.len()
            || self.component_type == VertexComponent::Byte
                && self
                    .values
                    .iter()
                    .any(|value| i8::try_from(*value).is_err())
        {
            return Err(geometry_error(
                "checkpoint-vertex-array",
                "Checkpoint vertex components do not match their declared shape",
            ));
        }
        Ok(())
    }

    /// Vertex count.
    #[must_use]
    pub const fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Components per vertex.
    #[must_use]
    pub const fn component_count(&self) -> usize {
        self.component_count
    }

    /// Component type.
    #[must_use]
    pub const fn component_type(&self) -> VertexComponent {
        self.component_type
    }

    /// Logical bytes retained by decoded component storage.
    #[must_use]
    pub(crate) fn allocated_bytes(&self) -> usize {
        self.values.len().saturating_mul(size_of::<i16>())
    }

    /// Replaces a consecutive vertex range from signed bytes.
    pub fn set_bytes(&mut self, first: usize, count: usize, values: &[i8]) -> Result<(), EmuError> {
        if self.component_type != VertexComponent::Byte {
            return Err(geometry_error(
                "component-type",
                "byte data requires an 8-bit VertexArray",
            ));
        }
        let range = self.checked_range(first, count, values.len())?;
        if range.is_empty() {
            return Ok(());
        }
        for (destination, source) in Arc::make_mut(&mut self.values)[range]
            .iter_mut()
            .zip(values)
        {
            *destination = i16::from(*source);
        }
        Ok(())
    }

    /// Replaces a consecutive vertex range from signed shorts.
    pub fn set_shorts(
        &mut self,
        first: usize,
        count: usize,
        values: &[i16],
    ) -> Result<(), EmuError> {
        if self.component_type != VertexComponent::Short {
            return Err(geometry_error(
                "component-type",
                "short data requires a 16-bit VertexArray",
            ));
        }
        let range = self.checked_range(first, count, values.len())?;
        if range.is_empty() {
            return Ok(());
        }
        let components = range.len();
        Arc::make_mut(&mut self.values)[range].copy_from_slice(&values[..components]);
        Ok(())
    }

    /// Transfers integer components without an intermediate typed array.
    /// Values are narrowed to the array's signed component width; only the
    /// requested vertex range is consumed.
    pub fn set_integers(
        &mut self,
        first: usize,
        count: usize,
        values: impl ExactSizeIterator<Item = i32>,
    ) -> Result<(), EmuError> {
        let range = self.checked_range(first, count, values.len())?;
        if range.is_empty() {
            return Ok(());
        }
        for (destination, source) in Arc::make_mut(&mut self.values)[range]
            .iter_mut()
            .zip(values)
        {
            *destination = match self.component_type {
                VertexComponent::Byte => i16::from(source as i8),
                VertexComponent::Short => source as i16,
            };
        }
        Ok(())
    }

    /// Returns one component sign-extended to `i16`.
    pub fn component(&self, vertex: usize, component: usize) -> Result<i16, EmuError> {
        if vertex >= self.vertex_count || component >= self.component_count {
            return Err(vertex_bounds());
        }
        Ok(self.values[vertex * self.component_count + component])
    }

    /// Borrows one complete attribute with a single vertex bounds check.
    pub(super) fn vertex_components(&self, vertex: usize) -> Result<&[i16], EmuError> {
        if vertex >= self.vertex_count {
            return Err(vertex_bounds());
        }
        let start = vertex * self.component_count;
        Ok(&self.values[start..start + self.component_count])
    }

    /// Borrows a validated range of sign-extended components of the requested type.
    pub fn components(
        &self,
        first: usize,
        count: usize,
        expected: VertexComponent,
    ) -> Result<&[i16], EmuError> {
        if self.component_type != expected {
            return Err(geometry_error(
                "component-type",
                "VertexArray component type does not match the transfer",
            ));
        }
        let range = self.checked_range(first, count, self.values.len())?;
        Ok(&self.values[range])
    }

    /// Maps both signed endpoints exactly to -1 and +1, including the bias
    /// that makes an integer zero a small positive normal component.
    pub(super) fn normal(&self, index: usize) -> Result<Vec4, EmuError> {
        let [x, y, z, ..] = self.vertex_components(index)? else {
            return Err(vertex_bounds());
        };
        let component = |value| self.component_type.normal(f32::from(value));
        Ok(Vec4::new(component(*x), component(*y), component(*z), 0.0))
    }

    fn checked_range(
        &self,
        first: usize,
        count: usize,
        supplied: usize,
    ) -> Result<std::ops::Range<usize>, EmuError> {
        let end_vertex = first.checked_add(count).ok_or_else(geometry_limit)?;
        let components = count
            .checked_mul(self.component_count)
            .ok_or_else(geometry_limit)?;
        if end_vertex > self.vertex_count {
            return Err(geometry_error(
                "vertex-bounds",
                "VertexArray range exceeds its vertex count",
            ));
        }
        if supplied < components {
            return Err(geometry_error(
                "attribute-length",
                "VertexArray transfer array is too short",
            ));
        }
        let start = first * self.component_count;
        Ok(start..start + components)
    }
}

fn component_length(vertex_count: usize, component_count: usize) -> Result<usize, EmuError> {
    if vertex_count == 0
        || u16::try_from(vertex_count).is_err()
        || !(2..=4).contains(&component_count)
    {
        return Err(geometry_error(
            "invalid-vertex-array",
            "vertex or component count is outside the M3G range",
        ));
    }
    vertex_count
        .checked_mul(component_count)
        .ok_or_else(geometry_limit)
}

#[cfg(test)]
#[path = "../../../../tests/unit/m3g/geometry/vertex_array.rs"]
mod tests;
