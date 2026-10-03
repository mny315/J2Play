//! Deterministic bounded immediate-mode software rasterizer.

use crate::math::rounded_u32;
use crate::{FogState, Mat4, Texture2DState, Vec4};
use diagnostics::{Category, EmuError};

mod checkpoint;
mod clipping;
mod fragments;
mod primitives;
mod target;
mod triangles;

use clipping::{ClipVertex, MAX_CLIPPED_VERTICES};
pub use fragments::FrameBlend;

const SUBPIXEL_BITS: i32 = 8;
const SUBPIXEL_SCALE: i64 = 1 << SUBPIXEL_BITS;
const DEPTH_MAX: f64 = 16_777_215.0;

/// One immediate-mode vertex in homogeneous clip coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    /// Homogeneous clip-space position.
    pub position: Vec4,
    /// Straight-alpha ARGB color.
    pub color: u32,
    /// Texture coordinates for every software-backend unit.
    pub texture: [[f32; 3]; crate::MAX_TEXTURE_UNITS],
    /// Camera-space Z before projection, used for fog. Defaults to zero.
    pub eye_z: f32,
}

impl Vertex {
    /// Constructs a clip-space vertex.
    #[must_use]
    pub const fn new(position: Vec4, color: u32) -> Self {
        Self {
            position,
            color,
            texture: [[0.0, 0.0, 1.0]; 2],
            eye_z: 0.0,
        }
    }

    /// Constructs a clip-space vertex with unit-zero texture coordinates.
    #[must_use]
    pub const fn textured(position: Vec4, color: u32, s: f32, t: f32) -> Self {
        Self {
            position,
            color,
            texture: [[s, t, 0.0], [0.0, 0.0, 1.0]],
            eye_z: 0.0,
        }
    }

    /// Constructs a vertex carrying coordinates for both texture units.
    #[must_use]
    pub const fn with_textures(position: Vec4, color: u32, texture: [[f32; 3]; 2]) -> Self {
        Self {
            position,
            color,
            texture,
            eye_z: 0.0,
        }
    }

    /// Projects a camera-space vertex while retaining its Z for fog.
    pub fn project(&mut self, projection: Mat4) {
        self.eye_z = self.position.z;
        self.position = projection.transform(self.position);
    }

    fn validate_position(&self) -> Result<(), EmuError> {
        if ![
            self.position.x,
            self.position.y,
            self.position.z,
            self.position.w,
            self.eye_z,
        ]
        .into_iter()
        .all(f32::is_finite)
        {
            return Err(render_error(
                "non-finite-vertex",
                "vertex position or camera distance contains NaN or infinity",
            ));
        }
        Ok(())
    }
}

/// Triangle culling mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CullMode {
    /// Rasterize both orientations.
    #[default]
    None,
    /// Cull clockwise triangles in window coordinates.
    Clockwise,
    /// Cull counter-clockwise triangles in window coordinates.
    CounterClockwise,
}

/// Native-work limits applied independently from VM instruction limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RenderLimits {
    /// Maximum submitted triangles between counter resets.
    pub triangles: u64,
    /// Maximum candidate fragments between counter resets.
    pub fragments: u64,
    /// Maximum viewport dimensions selected by the active device profile.
    pub max_viewport_width: u32,
    pub max_viewport_height: u32,
}

impl Default for RenderLimits {
    fn default() -> Self {
        Self {
            triangles: 262_144,
            fragments: 10_485_760,
            max_viewport_width: 1_024,
            max_viewport_height: 1_024,
        }
    }
}

/// Observable renderer counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RenderStats {
    /// Submitted input triangles.
    pub submitted_triangles: u64,
    /// Triangles intersecting clip planes.
    pub clipped_triangles: u64,
    /// Culled or degenerate output triangles.
    pub culled_triangles: u64,
    /// Rasterized output triangles after clipping.
    pub rasterized_triangles: u64,
    /// Candidate pixel centers charged to the fragment work budget.
    pub tested_fragments: u64,
    /// Covered pixels which reached fragment shading.
    pub shaded_fragments: u64,
    /// Covered fragments rejected by depth.
    pub depth_rejected_fragments: u64,
    /// Fragments combined with an existing color.
    pub blended_fragments: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct ScreenVertex {
    x: i64,
    y: i64,
    depth: f64,
    inverse_w: f64,
    color_over_w: [f64; 4],
    texture_over_w: [[f64; 3]; 2],
    eye_z_over_w: f64,
}

impl ScreenVertex {
    fn from_clip(
        source: ClipVertex,
        viewport: [i32; 4],
        depth_range: [f64; 2],
    ) -> Result<Option<Self>, EmuError> {
        // Homogeneous coordinates may use any finite nonzero scale. An
        // absolute epsilon here would reject otherwise identical projections.
        if source.position.w == 0.0 {
            return Ok(None);
        }
        let inverse_w = f64::from(source.position.w).recip();
        let ndc_x = f64::from(source.position.x) * inverse_w;
        let ndc_y = f64::from(source.position.y) * inverse_w;
        let ndc_z = f64::from(source.position.z) * inverse_w;
        let normalized_depth = ((ndc_z + 1.0) * 0.5).clamp(0.0, 1.0);
        Ok(Some(Self {
            x: quantize_subpixel(
                f64::from(viewport[0]) + (ndc_x + 1.0) * 0.5 * f64::from(viewport[2]),
            )?,
            y: quantize_subpixel(
                f64::from(viewport[1]) + (1.0 - ndc_y) * 0.5 * f64::from(viewport[3]),
            )?,
            depth: normalized_depth.mul_add(depth_range[1] - depth_range[0], depth_range[0]),
            inverse_w,
            color_over_w: source.color.map(|component| component * inverse_w),
            texture_over_w: source
                .texture
                .map(|coordinates| coordinates.map(|component| component * inverse_w)),
            eye_z_over_w: source.eye_z * inverse_w,
        }))
    }
}

/// Host-independent ARGB/depth render target and raster state.
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SoftwareRenderer {
    width: u32,
    height: u32,
    color: Vec<u32>,
    depth: Vec<u32>,
    viewport: [i32; 4],
    scissor: [u32; 4],
    depth_range: [f64; 2],
    depth_test: bool,
    depth_write: bool,
    depth_test_allows_equal: bool,
    color_write: bool,
    alpha_write: bool,
    blending: FrameBlend,
    depth_offset: [f64; 2],
    cull_mode: CullMode,
    smooth_shading: bool,
    textures: [Option<Texture2DState>; crate::MAX_TEXTURE_UNITS],
    fog: Option<FogState>,
    alpha_threshold: u8,
    fragment_alpha_override: Option<u8>,
    limits: RenderLimits,
    stats: RenderStats,
    #[serde(skip)]
    clip_a: [ClipVertex; MAX_CLIPPED_VERTICES],
    #[serde(skip)]
    clip_b: [ClipVertex; MAX_CLIPPED_VERTICES],
}

impl SoftwareRenderer {
    /// Current telemetry counters.
    #[must_use]
    pub const fn stats(&self) -> RenderStats {
        self.stats
    }

    /// Resets per-render-call work counters without touching buffers or state.
    pub fn reset_stats(&mut self) {
        self.stats = RenderStats::default();
    }

    /// Sets depth test/write, color write and GL source-alpha blending state.
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn set_compositing(
        &mut self,
        depth_test: bool,
        depth_write: bool,
        color_write: bool,
        blending: bool,
    ) {
        self.depth_test = depth_test;
        self.depth_write = depth_write;
        self.color_write = color_write;
        self.alpha_write = color_write;
        self.blending = if blending {
            FrameBlend::Alpha
        } else {
            FrameBlend::Replace
        };
    }

    /// Configures the complete JSR compositing state.
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn set_compositing_mode(
        &mut self,
        depth_test: bool,
        depth_write: bool,
        color_write: bool,
        alpha_write: bool,
        blending: FrameBlend,
        depth_offset_factor: f32,
        depth_offset_units: f32,
    ) -> Result<(), EmuError> {
        if !depth_offset_factor.is_finite() || !depth_offset_units.is_finite() {
            return Err(render_error(
                "invalid-compositing",
                "polygon depth offset must be finite",
            ));
        }
        self.depth_test = depth_test;
        self.depth_write = depth_write;
        self.color_write = color_write;
        self.alpha_write = alpha_write;
        self.blending = blending;
        self.depth_offset = [
            f64::from(depth_offset_factor),
            f64::from(depth_offset_units),
        ];
        Ok(())
    }

    /// Sets the normalized viewport depth mapping range.
    pub fn set_depth_range(&mut self, near: f32, far: f32) -> Result<(), EmuError> {
        if !near.is_finite()
            || !far.is_finite()
            || !(0.0..=1.0).contains(&near)
            || !(0.0..=1.0).contains(&far)
        {
            return Err(render_error(
                "invalid-depth-range",
                "invalid normalized depth range",
            ));
        }
        self.depth_range = [f64::from(near), f64::from(far)];
        Ok(())
    }

    /// Selects whether fragments at exactly the stored depth pass the depth test.
    pub fn set_depth_test_allows_equal(&mut self, enabled: bool) {
        self.depth_test_allows_equal = enabled;
    }

    /// Sets triangle orientation culling.
    pub fn set_cull_mode(&mut self, mode: CullMode) {
        self.cull_mode = mode;
    }

    /// Selects smooth interpolation or flat shading from the final input vertex.
    pub fn set_smooth_shading(&mut self, smooth: bool) {
        self.smooth_shading = smooth;
    }

    /// Binds or unbinds texture unit zero.
    pub fn set_texture(&mut self, texture: Option<Texture2DState>) {
        self.textures[0] = texture;
    }

    /// Current sampling state, also available for sharing unchanged mip levels.
    #[must_use]
    pub fn texture_unit(&self, unit: usize) -> Option<&Texture2DState> {
        self.textures.get(unit).and_then(Option::as_ref)
    }

    /// Binds or unbinds one profile-supported texture unit.
    pub fn set_texture_unit(
        &mut self,
        unit: usize,
        texture: Option<Texture2DState>,
    ) -> Result<(), EmuError> {
        let slot = self.textures.get_mut(unit).ok_or_else(|| {
            render_error(
                "texture-unit",
                "texture unit exceeds the software backend capacity",
            )
        })?;
        *slot = texture;
        Ok(())
    }

    /// Atomically binds both profile texture units after native-byte validation.
    pub fn set_texture_units(
        &mut self,
        textures: [Option<Texture2DState>; crate::MAX_TEXTURE_UNITS],
    ) -> Result<(), EmuError> {
        let attachment_bytes = self
            .color
            .len()
            .saturating_add(self.depth.len())
            .saturating_mul(size_of::<u32>());
        let texture_bytes = textures.iter().flatten().fold(0_usize, |total, texture| {
            total.saturating_add(texture.allocated_bytes())
        });
        if attachment_bytes.saturating_add(texture_bytes) > 8 * 1024 * 1024 {
            return Err(render_error(
                "resource-limit",
                "renderer attachments exceed the suite native-byte budget",
            ));
        }
        self.textures = textures;
        Ok(())
    }

    /// Sets optional fixed-function fog.
    pub fn set_fog(&mut self, fog: Option<FogState>) {
        self.fog = fog;
    }

    /// Sets the inclusive 8-bit alpha rejection threshold.
    pub fn set_alpha_threshold(&mut self, threshold: u8) {
        self.alpha_threshold = threshold;
    }

    /// Replaces fragment alpha after alpha testing but before frame blending.
    pub fn set_fragment_alpha_override(&mut self, alpha: Option<u8>) {
        self.fragment_alpha_override = alpha;
    }
}

fn quantize_subpixel(value: f64) -> Result<i64, EmuError> {
    let scaled = value * SUBPIXEL_SCALE as f64;
    // Edge functions subtract two products of coordinate differences. Leave
    // a sign bit of headroom so both products and their difference fit i64,
    // even when the viewport allows vertices on opposite sides of the origin.
    let limit = f64::from(i32::MAX / 2);
    if !scaled.is_finite() || scaled < -limit || scaled > limit {
        return Err(render_error(
            "coordinate-overflow",
            "post-transform vertex exceeds raster guard range",
        ));
    }
    Ok(scaled.round() as i64)
}

fn unpack_color(color: u32) -> [f64; 4] {
    [
        f64::from((color >> 24) & 0xff),
        f64::from((color >> 16) & 0xff),
        f64::from((color >> 8) & 0xff),
        f64::from(color & 0xff),
    ]
}

fn pack_color(components: [f64; 4]) -> u32 {
    let bytes = components.map(|value| rounded_u32(value, 255));
    bytes[0] << 24 | bytes[1] << 16 | bytes[2] << 8 | bytes[3]
}

fn render_error(code: &'static str, message: &'static str) -> EmuError {
    EmuError::new(Category::M3g, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/m3g/renderer/mod.rs"]
mod tests;
