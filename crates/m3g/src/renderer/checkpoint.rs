//! Validation of decoded render targets and their work counters.

use super::target::{target_pixel_count, valid_viewport_size};
use super::{MAX_CLIPPED_VERTICES, RenderLimits, SoftwareRenderer, render_error};
use diagnostics::EmuError;

impl SoftwareRenderer {
    /// Validates a decoded render target before it can be used for drawing.
    pub fn validate_checkpoint(&self, limits: RenderLimits) -> Result<(), EmuError> {
        let pixels = target_pixel_count(self.width, self.height).map_err(|_| {
            render_error("checkpoint-target", "Checkpoint render target is invalid")
        })?;
        if self.color.len() != pixels
            || self.depth.len() != pixels
            || self.limits != limits
            || self.scissor[0]
                .checked_add(self.scissor[2])
                .is_none_or(|edge| edge > self.width)
            || self.scissor[1]
                .checked_add(self.scissor[3])
                .is_none_or(|edge| edge > self.height)
            || !self
                .depth_range
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            || !self.depth_offset.iter().all(|value| value.is_finite())
        {
            return Err(render_error(
                "checkpoint-target",
                "Checkpoint render target is invalid",
            ));
        }
        // The constructor starts with the full target. Subsequent viewport
        // selections must fit the profile, including their signed storage.
        let initial_viewport = [0, 0, self.width as i32, self.height as i32];
        if self.viewport != initial_viewport
            && !valid_viewport_size(
                self.viewport[2].cast_unsigned(),
                self.viewport[3].cast_unsigned(),
                limits,
            )
        {
            return Err(render_error(
                "checkpoint-target",
                "Checkpoint viewport dimensions are invalid",
            ));
        }
        let stats = self.stats;
        // Clipping may fan one submitted triangle out into several outputs.
        // Failed draws may also leave a submission without an output triangle.
        let output_triangles = stats
            .submitted_triangles
            .saturating_mul((MAX_CLIPPED_VERTICES - 2) as u64);
        if stats.submitted_triangles > limits.triangles
            || stats.clipped_triangles > stats.submitted_triangles
            || stats.culled_triangles > output_triangles
            || stats.rasterized_triangles > output_triangles
            || stats.tested_fragments > limits.fragments
            || stats.shaded_fragments > stats.tested_fragments
            || stats.depth_rejected_fragments > stats.tested_fragments
            || stats.blended_fragments > stats.shaded_fragments
        {
            return Err(render_error(
                "checkpoint-target",
                "Checkpoint renderer counters exceed their work bounds",
            ));
        }
        for texture in self.textures.iter().flatten() {
            texture.validate_checkpoint()?;
        }
        Ok(())
    }
}
