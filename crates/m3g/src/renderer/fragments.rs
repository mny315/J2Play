//! Depth rejection, fog, alpha testing and framebuffer compositing.

use super::SoftwareRenderer;

/// Framebuffer blending equation shared by JSR-184 and `Micro3D`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FrameBlend {
    Add,
    /// JSR-184 `SRC_ALPHA`, `ONE_MINUS_SRC_ALPHA` for all four channels.
    Alpha,
    /// JSR-184 `SRC_ALPHA`, `ONE` for all four channels.
    AlphaAdd,
    Half,
    Modulate,
    ModulateX2,
    #[default]
    Replace,
    /// Source-over color and alpha accumulation used by `Micro3D`.
    SourceOver,
    Subtract,
}

impl SoftwareRenderer {
    pub(super) fn depth_rejects_fragment(&mut self, pixel_index: usize, depth: u32) -> bool {
        let rejected = self.depth_test
            && if self.depth_test_allows_equal {
                depth > self.depth[pixel_index]
            } else {
                depth >= self.depth[pixel_index]
            };
        if rejected {
            self.stats.depth_rejected_fragments += 1;
        }
        rejected
    }

    /// Completes shading after depth testing and texture application. Reports
    /// whether the fragment survived alpha rejection, including depth-only writes.
    // Release raster fixtures benefit from eliminating per-pixel calls and spills.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub(super) fn write_fragment(
        &mut self,
        pixel_index: usize,
        depth: u32,
        mut source: u32,
        eye_z: f64,
    ) -> bool {
        if let Some(fog) = self.fog {
            source = fog.apply(source, eye_z.abs());
        }
        if ((source >> 24) as u8) < self.alpha_threshold {
            return false;
        }
        if let Some(alpha) = self.fragment_alpha_override {
            source = source & 0x00ff_ffff | u32::from(alpha) << 24;
        }
        if self.depth_write {
            self.depth[pixel_index] = depth;
        }
        if self.color_write || self.alpha_write {
            let destination = self.color[pixel_index];
            let mut output = blend_frame(self.blending, source, destination);
            if !self.color_write {
                output = output & 0xff00_0000 | destination & 0x00ff_ffff;
            }
            if !self.alpha_write {
                output = output & 0x00ff_ffff | destination & 0xff00_0000;
            }
            if self.blending != FrameBlend::Replace {
                self.stats.blended_fragments += 1;
            }
            self.color[pixel_index] = output;
        }
        true
    }
}

pub(super) fn blend_frame(mode: FrameBlend, source: u32, destination: u32) -> u32 {
    if mode == FrameBlend::Replace {
        return source;
    }
    let source_alpha = (source >> 24) & 0xff;
    if matches!(mode, FrameBlend::Alpha | FrameBlend::SourceOver) {
        if source_alpha == 0 {
            return destination;
        }
        if source_alpha == 255 {
            return source;
        }
        // Two 16-bit lanes hold independent channel numerators. Their maximum
        // is 255*255, so rounding and normalization cannot carry into a neighbor.
        let inverse_alpha = 255 - source_alpha;
        let channels = |shift: u32| {
            let source = (source >> shift) & 0x00ff_00ff;
            let destination = (destination >> shift) & 0x00ff_00ff;
            let sum = source * source_alpha + destination * inverse_alpha + 0x0080_0080;
            ((sum + ((sum >> 8) & 0x00ff_00ff)) >> 8) & 0x00ff_00ff
        };
        let output = channels(0) | channels(8) << 8;
        return if mode == FrameBlend::SourceOver {
            let alpha = source_alpha + (((destination >> 24) * inverse_alpha + 127) / 255);
            output & 0x00ff_ffff | alpha << 24
        } else {
            output
        };
    }
    let inverse_alpha = 255 - source_alpha;
    let mut result = 0_u32;
    for shift in [24, 16, 8, 0] {
        let source_component = (source >> shift) & 0xff;
        let destination_component = (destination >> shift) & 0xff;
        let value = match mode {
            FrameBlend::Add if shift == 24 => source_component.max(destination_component),
            FrameBlend::Add => (source_component + destination_component).min(255),
            FrameBlend::SourceOver if shift == 24 => {
                source_alpha + (destination_component * inverse_alpha + 127) / 255
            }
            FrameBlend::Alpha | FrameBlend::SourceOver => {
                (source_component * source_alpha + destination_component * inverse_alpha + 127)
                    / 255
            }
            FrameBlend::AlphaAdd => {
                ((source_component * source_alpha + 127) / 255 + destination_component).min(255)
            }
            FrameBlend::Half => (source_component + destination_component).div_ceil(2),
            FrameBlend::Modulate => (source_component * destination_component + 127) / 255,
            FrameBlend::ModulateX2 => {
                ((2 * source_component * destination_component + 127) / 255).min(255)
            }
            FrameBlend::Replace => unreachable!(),
            FrameBlend::Subtract if shift == 24 => destination_component,
            FrameBlend::Subtract => destination_component.saturating_sub(source_component),
        };
        result |= value << shift;
    }
    result
}
