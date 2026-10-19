//! Framebuffer ownership, viewport bounds, clearing and background image sampling.

use super::{
    ClipVertex, ColorDamage, CullMode, FrameBlend, MAX_CLIPPED_VERTICES, RenderLimits, RenderStats,
    SoftwareRenderer, render_error,
};
use crate::Image2DState;
use diagnostics::EmuError;

impl SoftwareRenderer {
    /// Host raster density. Coordinates and profile viewport limits remain logical.
    #[must_use]
    pub const fn resolution_scale(&self) -> u32 {
        self.resolution_scale
    }

    /// Selects original or double raster density, retaining color and depth.
    pub fn set_resolution_scale(&mut self, scale: u32) -> Result<(), EmuError> {
        if !matches!(scale, 1 | 2) {
            return Err(render_error(
                "invalid-render-scale",
                "render scale must be one or two",
            ));
        }
        if scale == self.resolution_scale {
            return Ok(());
        }
        let width = self
            .width()
            .checked_mul(scale)
            .ok_or_else(|| render_error("resource-limit", "scaled target overflow"))?;
        let height = self
            .height()
            .checked_mul(scale)
            .ok_or_else(|| render_error("resource-limit", "scaled target overflow"))?;
        let count = target_pixel_count(width, height)?;
        let mut color = Vec::new();
        let mut depth = Vec::new();
        color
            .try_reserve_exact(count)
            .map_err(|_| render_error("resource-limit", "scaled color allocation failed"))?;
        depth
            .try_reserve_exact(count)
            .map_err(|_| render_error("resource-limit", "scaled depth allocation failed"))?;
        for y in 0..height {
            for x in 0..width {
                let source = ((y * self.resolution_scale / scale) * self.width
                    + x * self.resolution_scale / scale) as usize;
                color.push(self.color[source]);
                depth.push(self.depth[source]);
            }
        }
        self.viewport = self
            .viewport
            .map(|v| (v / self.resolution_scale as i32).saturating_mul(scale as i32));
        self.scissor = self.scissor.map(|v| v / self.resolution_scale * scale);
        self.width = width;
        self.height = height;
        self.color = color;
        self.depth = depth;
        self.resolution_scale = scale;
        self.color_damage = ColorDamage::Clean;
        self.mark_color_damage([0, 0, width, height]);
        self.reset_stats();
        Ok(())
    }

    /// Resolves host samples to the guest's original RGB/ARGB image dimensions.
    #[must_use]
    pub fn resolved_pixels(&self) -> Vec<u32> {
        if self.resolution_scale == 1 {
            return self.color.clone();
        }
        let mut output = Vec::with_capacity((self.width() * self.height()) as usize);
        for y in 0..self.height() as usize {
            for x in 0..self.width() as usize {
                let i = y * 2 * self.width as usize + x * 2;
                let values = [
                    self.color[i],
                    self.color[i + 1],
                    self.color[i + self.width as usize],
                    self.color[i + self.width as usize + 1],
                ];
                let mut pixel = 0;
                for shift in [0, 8, 16, 24] {
                    let sum: u32 = values.iter().map(|v| (v >> shift) & 255).sum();
                    pixel |= ((sum + 2) / 4) << shift;
                }
                output.push(pixel);
            }
        }
        output
    }

    pub(super) fn fragment_limit(&self) -> u64 {
        self.limits
            .fragments
            .saturating_mul(u64::from(self.resolution_scale).pow(2))
    }

    /// Allocates a bounded color/depth target.
    pub fn new(width: u32, height: u32, limits: RenderLimits) -> Result<Self, EmuError> {
        let count = target_pixel_count(width, height)?;
        Ok(Self::with_validated_pixels(
            width,
            height,
            vec![0; count],
            limits,
        ))
    }

    /// Adopts an already scaled host color attachment without upscaling a
    /// temporary native attachment. Public dimensions remain logical.
    pub fn from_scaled_pixels(
        width: u32,
        height: u32,
        scale: u32,
        pixels: Vec<u32>,
        limits: RenderLimits,
    ) -> Result<Self, EmuError> {
        if !matches!(scale, 1 | 2) {
            return Err(render_error(
                "invalid-render-scale",
                "render scale must be one or two",
            ));
        }
        let width = width
            .checked_mul(scale)
            .ok_or_else(|| render_error("invalid-target", "target width overflow"))?;
        let height = height
            .checked_mul(scale)
            .ok_or_else(|| render_error("invalid-target", "target height overflow"))?;
        let mut renderer = Self::from_pixels(width, height, pixels, limits)?;
        renderer.resolution_scale = scale;
        renderer.mark_color_damage([0, 0, width, height]);
        Ok(renderer)
    }

    /// Adopts an existing color attachment without allocating or copying its
    /// pixels. Dimensions are validated before allocating the depth attachment.
    pub fn from_pixels(
        width: u32,
        height: u32,
        pixels: Vec<u32>,
        limits: RenderLimits,
    ) -> Result<Self, EmuError> {
        if pixels.len() != target_pixel_count(width, height)? {
            return Err(render_error(
                "invalid-target",
                "source pixel count does not match the render target",
            ));
        }
        Ok(Self::with_validated_pixels(width, height, pixels, limits))
    }

    fn with_validated_pixels(
        width: u32,
        height: u32,
        color: Vec<u32>,
        limits: RenderLimits,
    ) -> Self {
        let depth = vec![u32::MAX; color.len()];
        Self {
            resolution_scale: 1,
            width,
            height,
            color,
            depth,
            viewport: [0, 0, width as i32, height as i32],
            scissor: [0, 0, width, height],
            depth_range: [0.0, 1.0],
            depth_test: true,
            depth_write: true,
            depth_test_allows_equal: false,
            color_write: true,
            alpha_write: true,
            blending: FrameBlend::Replace,
            depth_offset: [0.0; 2],
            cull_mode: CullMode::None,
            smooth_shading: true,
            textures: [None, None],
            fog: None,
            alpha_threshold: 0,
            fragment_alpha_override: None,
            limits,
            stats: RenderStats::default(),
            color_damage: ColorDamage::Dirty([0, 0, width, height]),
            clip_a: [ClipVertex::default(); MAX_CLIPPED_VERTICES],
            clip_b: [ClipVertex::default(); MAX_CLIPPED_VERTICES],
        }
    }

    /// Target width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width / self.resolution_scale
    }

    /// Target height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height / self.resolution_scale
    }

    /// Immutable ARGB pixels.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.color
    }

    /// Replaces the color attachment after exact dimension validation.
    pub fn load_pixels(&mut self, pixels: &[u32]) -> Result<(), EmuError> {
        if self.resolution_scale == 2 && pixels.len() * 4 == self.color.len() {
            let width = self.width() as usize;
            for (row, source) in pixels.chunks_exact(width).enumerate() {
                for (x, &pixel) in source.iter().enumerate() {
                    let index = row * 2 * self.width as usize + x * 2;
                    self.color[index..index + 2].fill(pixel);
                    self.color[index + self.width as usize..index + self.width as usize + 2]
                        .fill(pixel);
                }
            }
            self.mark_color_damage([0, 0, self.width, self.height]);
            return Ok(());
        }
        if pixels.len() != self.color.len() {
            return Err(render_error(
                "invalid-target",
                "source pixel count does not match the render target",
            ));
        }
        self.color.copy_from_slice(pixels);
        self.mark_color_damage([0, 0, self.width, self.height]);
        Ok(())
    }

    /// Adopts a validated color attachment while preserving depth and render state.
    pub fn replace_pixels(&mut self, pixels: Vec<u32>) -> Result<(), EmuError> {
        if self.resolution_scale == 2 && pixels.len() * 4 == self.color.len() {
            return self.load_pixels(&pixels);
        }
        if pixels.len() != self.color.len() {
            return Err(render_error(
                "invalid-target",
                "source pixel count does not match the render target",
            ));
        }
        self.color = pixels;
        self.mark_color_damage([0, 0, self.width, self.height]);
        Ok(())
    }

    /// Whether target color has unpublished writes. Host-only tracking
    /// is rebuilt when restored attachments are loaded.
    #[must_use]
    pub fn has_color_damage(&self) -> bool {
        !matches!(self.color_damage, ColorDamage::Clean)
    }

    /// Completes publication into an RGB target, returning logical pixel bounds.
    /// RGB target alpha becomes opaque between render calls, as when reloading
    /// an LCDUI Graphics target. Depth and unmodified samples stay resident.
    pub fn take_rgb_damage(&mut self) -> Option<[u32; 4]> {
        let [left, top, right, bottom] =
            match std::mem::replace(&mut self.color_damage, ColorDamage::Clean) {
                ColorDamage::Unknown => [0, 0, self.width, self.height],
                ColorDamage::Clean => return None,
                ColorDamage::Dirty(bounds) => bounds,
            };
        let scale = self.resolution_scale;
        let (left, top) = (left / scale, top / scale);
        let (right, bottom) = (right.div_ceil(scale), bottom.div_ceil(scale));
        for y in top * scale..bottom * scale {
            let start = (y * self.width + left * scale) as usize;
            let end = (y * self.width + right * scale) as usize;
            for pixel in &mut self.color[start..end] {
                *pixel |= 0xff00_0000;
            }
        }
        Some([left, top, right - left, bottom - top])
    }

    pub(super) fn mark_color_damage(&mut self, bounds: [u32; 4]) {
        if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
            return;
        }
        self.color_damage = match self.color_damage {
            ColorDamage::Unknown => ColorDamage::Unknown,
            ColorDamage::Clean => ColorDamage::Dirty(bounds),
            ColorDamage::Dirty(old) => ColorDamage::Dirty([
                old[0].min(bounds[0]),
                old[1].min(bounds[1]),
                old[2].max(bounds[2]),
                old[3].max(bounds[3]),
            ]),
        };
    }

    /// Immutable fixed-point depth values.
    #[must_use]
    pub fn depth(&self) -> &[u32] {
        &self.depth
    }

    /// Native bytes retained by target attachments and bound texture copies.
    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        self.color
            .len()
            .saturating_add(self.depth.len())
            .saturating_mul(size_of::<u32>())
            .saturating_add(
                self.textures
                    .iter()
                    .flatten()
                    .fold(0_usize, |total, texture| {
                        total.saturating_add(texture.allocated_bytes())
                    }),
            )
    }

    /// Configures the viewport after complete bounds validation.
    pub fn set_viewport(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<(), EmuError> {
        if !valid_viewport_size(width, height, self.limits) {
            return Err(render_error(
                "invalid-viewport",
                "viewport dimensions exceed the active profile limits",
            ));
        }
        let scale = self.resolution_scale as i32;
        self.viewport = [
            x.saturating_mul(scale),
            y.saturating_mul(scale),
            (width as i32).saturating_mul(scale),
            (height as i32).saturating_mul(scale),
        ];
        Ok(())
    }

    /// Restricts every color/depth write to a target-space rectangle.
    pub fn set_scissor(&mut self, x: u32, y: u32, width: u32, height: u32) -> Result<(), EmuError> {
        if x > self.width()
            || y > self.height()
            || x.checked_add(width)
                .is_none_or(|right| right > self.width())
            || y.checked_add(height)
                .is_none_or(|bottom| bottom > self.height())
        {
            return Err(render_error(
                "invalid-scissor",
                "scissor is outside the target",
            ));
        }
        self.scissor = [x, y, width, height].map(|value| value * self.resolution_scale);
        Ok(())
    }

    /// Clears selected buffers within the viewport and scissor.
    pub fn clear(&mut self, color: Option<u32>, depth: bool) {
        let [start_x, start_y, end_x, end_y] = self.viewport_bounds();
        if color.is_some() {
            self.mark_color_damage([start_x, start_y, end_x, end_y]);
        }
        for y in start_y..end_y {
            let start = y as usize * self.width as usize + start_x as usize;
            let end = y as usize * self.width as usize + end_x as usize;
            if let Some(value) = color {
                self.color[start..end].fill(value);
            }
            if depth {
                self.depth[start..end].fill(u32::MAX);
            }
        }
    }

    /// Draws a background image into the viewport with independent border/repeat modes.
    pub fn draw_background(
        &mut self,
        image: &Image2DState,
        crop: [i32; 4],
        repeat_x: bool,
        repeat_y: bool,
    ) -> Result<(), EmuError> {
        let [crop_x, crop_y, crop_width, crop_height] = crop;
        if crop_width < 0 || crop_height < 0 {
            return Err(render_error(
                "invalid-background-crop",
                "background crop dimensions cannot be negative",
            ));
        }
        if crop_width == 0 || crop_height == 0 {
            return Ok(());
        }
        let viewport_width = i64::from(self.viewport[2]);
        let viewport_height = i64::from(self.viewport[3]);
        let [start_x, start_y, end_x, end_y] = self.viewport_bounds();
        if start_x == end_x || start_y == end_y {
            return Ok(());
        }
        // Horizontal sampling is identical on every row. Keep only columns
        // that address the image, bounded by the clipped target width.
        let columns: Vec<_> = (start_x..end_x)
            .filter_map(|target_x| {
                let destination_x = i64::from(target_x) - i64::from(self.viewport[0]);
                let raw_x =
                    i64::from(crop_x) + destination_x * i64::from(crop_width) / viewport_width;
                background_coordinate(raw_x, image.width(), repeat_x)
                    .map(|source_x| (target_x, source_x))
            })
            .collect();
        if columns.is_empty() {
            return Ok(());
        }
        self.mark_color_damage([start_x, start_y, end_x, end_y]);
        for target_y in start_y..end_y {
            let destination_y = i64::from(target_y) - i64::from(self.viewport[1]);
            let raw_y =
                i64::from(crop_y) + destination_y * i64::from(crop_height) / viewport_height;
            let Some(source_y) = background_coordinate(raw_y, image.height(), repeat_y) else {
                continue;
            };
            for &(target_x, source_x) in &columns {
                let destination = target_y as usize * self.width as usize + target_x as usize;
                let source = source_y as usize * image.width() as usize + source_x as usize;
                self.color[destination] = image.pixels()[source];
            }
        }
        Ok(())
    }

    pub(super) fn viewport_bounds(&self) -> [u32; 4] {
        let left = i64::from(self.viewport[0])
            .max(i64::from(self.scissor[0]))
            .clamp(0, i64::from(self.width));
        let top = i64::from(self.viewport[1])
            .max(i64::from(self.scissor[1]))
            .clamp(0, i64::from(self.height));
        let right = (i64::from(self.viewport[0]) + i64::from(self.viewport[2]))
            .min(i64::from(self.scissor[0] + self.scissor[2]))
            .min(i64::from(self.width))
            .max(left);
        let bottom = (i64::from(self.viewport[1]) + i64::from(self.viewport[3]))
            .min(i64::from(self.scissor[1] + self.scissor[3]))
            .min(i64::from(self.height))
            .max(top);
        [left as u32, top as u32, right as u32, bottom as u32]
    }
}

pub(super) fn target_pixel_count(width: u32, height: u32) -> Result<usize, EmuError> {
    if width == 0 || height == 0 {
        return Err(render_error(
            "invalid-target",
            "render target dimensions must be non-zero",
        ));
    }
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .ok_or_else(|| render_error("resource-limit", "render target size overflow"))?;
    if pixels > 4_194_304 {
        return Err(render_error(
            "resource-limit",
            "render target exceeds pixel budget",
        ));
    }
    Ok(pixels)
}

pub(super) fn valid_viewport_size(width: u32, height: u32, limits: RenderLimits) -> bool {
    width > 0
        && height > 0
        && width <= limits.max_viewport_width
        && height <= limits.max_viewport_height
        && i32::try_from(width).is_ok()
        && i32::try_from(height).is_ok()
}

fn background_coordinate(value: i64, dimension: u32, repeat: bool) -> Option<u32> {
    let dimension = i64::from(dimension);
    if repeat {
        Some(value.rem_euclid(dimension) as u32)
    } else {
        (0..dimension).contains(&value).then_some(value as u32)
    }
}
