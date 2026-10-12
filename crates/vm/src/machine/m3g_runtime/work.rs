//! Per-operation geometry work and cooperative cancellation.

use super::{EmuError, Machine, vm_error};

pub(in crate::machine) struct GeometryWork {
    vertices: usize,
    indices: u64,
}

impl GeometryWork {
    pub(in crate::machine) fn new(limits: &crate::machine::Limits) -> Self {
        Self {
            vertices: limits.m3g_vertices,
            indices: limits.m3g_render.triangles.saturating_mul(3),
        }
    }

    pub(in crate::machine) fn vertices(&mut self, count: usize) -> Result<(), EmuError> {
        self.vertices = self.vertices.checked_sub(count).ok_or_else(|| {
            EmuError::new(
                diagnostics::Category::M3g,
                "render-budget",
                "M3G geometry exceeds the profile vertex budget",
            )
        })?;
        Ok(())
    }

    pub(in crate::machine) fn indices(&mut self, count: usize) -> Result<(), EmuError> {
        // Three indices per triangle also permits independent three-vertex strips.
        // Degenerate strips still consume work while their indices are scanned.
        self.indices = self.indices.checked_sub(count as u64).ok_or_else(|| {
            EmuError::new(
                diagnostics::Category::M3g,
                "render-budget",
                "M3G geometry exceeds the profile index budget",
            )
        })?;
        Ok(())
    }
}

impl Machine<'_, '_> {
    pub(in crate::machine) fn m3g_check_geometry_cancellation(&self) -> Result<(), EmuError> {
        if self.native_context.execution_cancelled() {
            Err(vm_error("execution-cancelled", "M3G scene work cancelled"))
        } else {
            Ok(())
        }
    }
}
