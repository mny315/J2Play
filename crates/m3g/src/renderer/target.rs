//! Framebuffer ownership, viewport bounds, clearing and background image sampling.

use super::{
    ClipVertex, CullMode, FrameBlend, MAX_CLIPPED_VERTICES, RenderLimits, RenderStats,
    SoftwareRenderer, render_error,
};
use crate::Image2DState;
use diagnostics::EmuError;

impl SoftwareRenderer {
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
            clip_a: [ClipVertex::default(); MAX_CLIPPED_VERTICES],
            clip_b: [ClipVertex::default(); MAX_CLIPPED_VERTICES],
        }
    }

    /// Target width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Target height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Immutable ARGB pixels.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.color
    }

    /// Replaces the color attachment after exact dimension validation.
    pub fn load_pixels(&mut self, pixels: &[u32]) -> Result<(), EmuError> {
        if pixels.len() != self.color.len() {
            return Err(render_error(
                "invalid-target",
                "source pixel count does not match the render target",
            ));
        }
        self.color.copy_from_slice(pixels);
        Ok(())
    }

    /// Adopts a validated color attachment while preserving depth and render state.
    pub fn replace_pixels(&mut self, pixels: Vec<u32>) -> Result<(), EmuError> {
        if pixels.len() != self.color.len() {
            return Err(render_error(
                "invalid-target",
                "source pixel count does not match the render target",
            ));
        }
        self.color = pixels;
        Ok(())
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
        self.viewport = [x, y, width as i32, height as i32];
        Ok(())
    }

    /// Restricts every color/depth write to a target-space rectangle.
    pub fn set_scissor(&mut self, x: u32, y: u32, width: u32, height: u32) -> Result<(), EmuError> {
        if x > self.width
            || y > self.height
            || x.checked_add(width).is_none_or(|right| right > self.width)
            || y.checked_add(height)
                .is_none_or(|bottom| bottom > self.height)
        {
            return Err(render_error(
                "invalid-scissor",
                "scissor is outside the target",
            ));
        }
        self.scissor = [x, y, width, height];
        Ok(())
    }

    /// Clears selected buffers within the viewport and scissor.
    pub fn clear(&mut self, color: Option<u32>, depth: bool) {
        let [start_x, start_y, end_x, end_y] = self.viewport_bounds();
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
