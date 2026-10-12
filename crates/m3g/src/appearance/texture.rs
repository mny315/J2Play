//! Texture addressing, filtering and environment blending.

use super::{Image2DState, ImageFormat};
use crate::math::rounded_u32;
use crate::{Mat4, Vec4};
use std::sync::Arc;

/// Texture coordinate addressing mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WrapMode {
    /// Clamp to the border texel.
    Clamp,
    /// Repeat every integer coordinate interval.
    Repeat,
}

/// Texture/environment blend function.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BlendFunction {
    /// Replace fragment color with texture color.
    Replace,
    /// Multiply texture and fragment colors.
    Modulate,
    /// Interpolate RGB by texture alpha.
    Decal,
    /// Add colors with saturation.
    Add,
    /// Blend fragment RGB toward a constant using texture RGB.
    Blend,
}

/// Immutable sampling state for one texture unit.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Texture2DState {
    width: u32,
    height: u32,
    format: ImageFormat,
    mipmaps: Vec<MipLevel>,
    wrap_s: WrapMode,
    wrap_t: WrapMode,
    minification_linear: bool,
    magnification_linear: bool,
    blend: BlendFunction,
    blend_color: u32,
    transform: Mat4,
    identity_transform: bool,
    mipmap_filter: Option<bool>,
    color_key: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
struct MipLevel {
    width: u32,
    height: u32,
    pixels: Arc<[u32]>,
}

impl Texture2DState {
    /// Validates saved image bounds, mip levels and derived sampling state.
    pub fn validate_checkpoint(&self) -> Result<(), diagnostics::EmuError> {
        let mut dimensions = (self.width, self.height);
        if Image2DState::validate_dimensions(self.width, self.height).is_err() {
            return Err(save_state::error(
                "checkpoint-texture",
                "Checkpoint texture dimensions are invalid.",
            ));
        }
        // Disabling mip filtering can retain a complete chain. Changing the
        // color key while disabled discards it, leaving only the base image.
        let base_only = self.mipmaps.len() == 1 && self.mipmap_filter.is_none();
        let full_levels = self.width.max(self.height).ilog2() as usize + 1;
        if (!base_only && self.mipmaps.len() != full_levels)
            || self.identity_transform != (self.transform == Mat4::IDENTITY)
        {
            return Err(save_state::error(
                "checkpoint-texture",
                "Checkpoint texture sampling state is inconsistent.",
            ));
        }
        for level in &self.mipmaps {
            let pixels = (level.width as usize).checked_mul(level.height as usize);
            if (level.width, level.height) != dimensions || pixels != Some(level.pixels.len()) {
                return Err(save_state::error(
                    "checkpoint-texture",
                    "Checkpoint texture pixels are invalid.",
                ));
            }
            dimensions = ((dimensions.0 / 2).max(1), (dimensions.1 / 2).max(1));
        }
        Ok(())
    }

    /// Creates default nearest/repeat/modulate texture state.
    #[must_use]
    pub fn new(image: Image2DState) -> Self {
        let Image2DState {
            width,
            height,
            format,
            pixels,
            ..
        } = image;
        Self {
            width,
            height,
            format,
            mipmaps: vec![MipLevel {
                width,
                height,
                pixels,
            }],
            wrap_s: WrapMode::Repeat,
            wrap_t: WrapMode::Repeat,
            minification_linear: false,
            magnification_linear: false,
            blend: BlendFunction::Modulate,
            blend_color: 0,
            transform: Mat4::IDENTITY,
            identity_transform: true,
            mipmap_filter: None,
            color_key: None,
        }
    }

    /// Sets wrapping independently for S and T.
    pub fn set_wrapping(&mut self, s: WrapMode, t: WrapMode) {
        self.wrap_s = s;
        self.wrap_t = t;
    }

    /// Selects nearest or bilinear filtering for both shrinking and enlarging.
    pub fn set_linear_filter(&mut self, linear: bool) {
        self.set_image_filters(linear, linear);
    }

    /// Sets image filtering independently for minification and magnification.
    pub fn set_image_filters(&mut self, minification_linear: bool, magnification_linear: bool) {
        self.minification_linear = minification_linear;
        self.magnification_linear = magnification_linear;
    }

    /// Selects base-only sampling or nearest/linear mip level filtering.
    pub fn set_mipmap_filter(&mut self, enabled: bool, linear_levels: bool) {
        if enabled && self.mipmaps.len() == 1 {
            generate_mipmaps(&mut self.mipmaps, self.color_key);
        }
        self.mipmap_filter = enabled.then_some(linear_levels);
    }

    /// Shares already generated levels of the same immutable pixel snapshot.
    /// Image updates use copy-on-write, so changed pixels cannot reuse old levels.
    pub fn reuse_mipmaps(&mut self, previous: &Self) {
        if self.width == previous.width
            && self.height == previous.height
            && self.format == previous.format
            && self.color_key == previous.color_key
            && self.mipmaps.len() < previous.mipmaps.len()
            && Arc::ptr_eq(&self.mipmaps[0].pixels, &previous.mipmaps[0].pixels)
        {
            self.mipmaps.clone_from(&previous.mipmaps);
        }
    }

    /// Makes texels with the selected 24-bit RGB value fully transparent.
    ///
    /// The comparison happens while sampling, so callers can share the source
    /// pixels without allocating a color-keyed copy for every bind.
    pub fn set_color_key(&mut self, color_key: Option<u32>) {
        let color_key = color_key.map(|value| value & 0x00ff_ffff);
        if self.color_key != color_key {
            let rebuild_mipmaps = self.mipmap_filter.is_some();
            self.mipmaps.truncate(1);
            self.color_key = color_key;
            if rebuild_mipmaps {
                generate_mipmaps(&mut self.mipmaps, self.color_key);
            }
        }
    }

    /// Base image dimensions used for deterministic LOD selection.
    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Whether LOD can affect mip selection or the image filter.
    pub(crate) fn uses_level_of_detail(&self) -> bool {
        (self.mipmap_filter.is_some() && self.mipmaps.len() > 1)
            || self.minification_linear != self.magnification_linear
    }

    /// Host bytes owned by this decoded sampling state.
    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        self.mipmaps.iter().fold(0_usize, |total, level| {
            total.saturating_add(level.pixels.len().saturating_mul(size_of::<u32>()))
        })
    }

    /// Sets texture blend function.
    pub fn set_blend_function(&mut self, blend: BlendFunction) {
        self.blend = blend;
    }

    /// Sets the RGB constant used by `Blend`.
    pub fn set_blend_color(&mut self, color: u32) {
        self.blend_color = color & 0x00ff_ffff;
    }

    /// Copies the texture-coordinate transform.
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.identity_transform = transform == Mat4::IDENTITY;
    }

    /// Samples and combines a texture with interpolated fragment color.
    #[must_use]
    pub fn shade(&self, s: f64, t: f64, fragment: u32) -> u32 {
        self.shade_coordinates([s, t, 0.0], fragment)
    }

    /// Transforms, samples and combines a three-component texture coordinate.
    #[must_use]
    pub fn shade_coordinates(&self, coordinates: [f64; 3], fragment: u32) -> u32 {
        self.shade_coordinates_lod(coordinates, fragment, 0.0)
    }

    /// Samples at an explicit non-negative logarithmic mip level.
    #[must_use]
    pub fn shade_coordinates_lod(&self, coordinates: [f64; 3], fragment: u32, lod: f64) -> u32 {
        let input = Vec4::new(
            coordinates[0] as f32,
            coordinates[1] as f32,
            coordinates[2] as f32,
            1.0,
        );
        // Most bound textures use the identity transform. Preserve the f32
        // coordinate conversion while avoiding twelve fused operations per
        // fragment. Non-finite inputs retain the matrix path's IEEE behavior.
        let transformed = if self.identity_transform
            && [input.x, input.y, input.z]
                .iter()
                .all(|value| value.is_finite())
        {
            input
        } else {
            self.transform.transform(input)
        };
        let inverse = if transformed.w == 0.0 {
            1.0
        } else {
            transformed.w.recip()
        };
        let s = f64::from(transformed.x * inverse);
        let t = f64::from(transformed.y * inverse);
        let minifying = lod > 0.0;
        let linear = if minifying {
            self.minification_linear
        } else {
            self.magnification_linear
        };
        let maximum = self.mipmaps.len().saturating_sub(1) as f64;
        let lod = lod.clamp(0.0, maximum);
        let texture = match (minifying, self.mipmap_filter) {
            (true, Some(true)) => {
                let lower = lod.floor() as usize;
                let upper = lod.ceil() as usize;
                let first = self.sample_level(lower, s, t, linear);
                if lower == upper {
                    first
                } else {
                    let second = self.sample_level(upper, s, t, linear);
                    mix_argb(first, second, lod - lod.floor())
                }
            }
            (true, Some(false)) => self.sample_level(lod.round() as usize, s, t, linear),
            _ => self.sample_level(0, s, t, linear),
        };
        let format = if self.color_key.is_some() {
            ImageFormat::Rgba
        } else {
            self.format
        };
        blend_texture(self.blend, format, texture, fragment, self.blend_color)
    }

    fn sample_level(&self, level: usize, s: f64, t: f64, linear: bool) -> u32 {
        let index = level.min(self.mipmaps.len() - 1);
        let color_key = self.color_key.filter(|_| index == 0);
        let level = &self.mipmaps[index];
        if linear {
            self.sample_linear(level, s, t, color_key)
        } else {
            let x = texture_index(s, level.width, self.wrap_s);
            let y = texture_index(t, level.height, self.wrap_t);
            apply_color_key(level.pixels[y * level.width as usize + x], color_key)
        }
    }

    fn sample_linear(&self, level: &MipLevel, s: f64, t: f64, color_key: Option<u32>) -> u32 {
        let width = f64::from(level.width);
        let height = f64::from(level.height);
        let x = addressed(s, self.wrap_s) * width - 0.5;
        let y = addressed(t, self.wrap_t) * height - 0.5;
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        // These are already integral texel coordinates. Dividing by the image
        // size and multiplying again can round down to the preceding texel.
        let [left, right] = [x0, x0 + 1.0].map(|x| texel_index(x as i64, level.width, self.wrap_s));
        let [top, bottom] =
            [y0, y0 + 1.0].map(|y| texel_index(y as i64, level.height, self.wrap_t));
        let coordinates = [(left, top), (right, top), (left, bottom), (right, bottom)];
        let pixels = coordinates
            .map(|(x, y)| apply_color_key(level.pixels[y * level.width as usize + x], color_key));
        bilinear(pixels, fx, fy)
    }
}

fn apply_color_key(pixel: u32, color_key: Option<u32>) -> u32 {
    if color_key.is_some_and(|color_key| pixel & 0x00ff_ffff == color_key) {
        0
    } else {
        pixel
    }
}

fn generate_mipmaps(levels: &mut Vec<MipLevel>, color_key: Option<u32>) {
    while let Some(previous) = levels.last() {
        if previous.width <= 1 && previous.height <= 1 {
            break;
        }
        let width = (previous.width / 2).max(1);
        let height = (previous.height / 2).max(1);
        // Only base pixels carry source color keys. Generated levels already
        // contain filtered alpha; their blended RGB must not be keyed again.
        let source_key = color_key.filter(|_| levels.len() == 1);
        let mut pixels = Vec::with_capacity(width as usize * height as usize);
        for y in 0..height {
            for x in 0..width {
                let source_x = x * 2;
                let source_y = y * 2;
                let samples = [
                    (source_x, source_y),
                    ((source_x + 1).min(previous.width - 1), source_y),
                    (source_x, (source_y + 1).min(previous.height - 1)),
                    (
                        (source_x + 1).min(previous.width - 1),
                        (source_y + 1).min(previous.height - 1),
                    ),
                ];
                let mut channels = [0_u32; 4];
                for (sample_x, sample_y) in samples {
                    let pixel = apply_color_key(
                        previous.pixels
                            [sample_y as usize * previous.width as usize + sample_x as usize],
                        source_key,
                    );
                    for (channel, shift) in [24, 16, 8, 0].into_iter().enumerate() {
                        channels[channel] += (pixel >> shift) & 0xff;
                    }
                }
                pixels.push(
                    ((channels[0] + 2) / 4) << 24
                        | ((channels[1] + 2) / 4) << 16
                        | ((channels[2] + 2) / 4) << 8
                        | ((channels[3] + 2) / 4),
                );
            }
        }
        levels.push(MipLevel {
            width,
            height,
            pixels: pixels.into(),
        });
    }
}

fn mix_argb(first: u32, second: u32, amount: f64) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    [24, 16, 8, 0].into_iter().fold(0_u32, |result, shift| {
        let left = f64::from((first >> shift) & 0xff);
        let right = f64::from((second >> shift) & 0xff);
        result | rounded_u32(amount.mul_add(right - left, left), 255) << shift
    })
}

fn addressed(value: f64, mode: WrapMode) -> f64 {
    match mode {
        WrapMode::Clamp => value.clamp(0.0, 1.0 - f64::EPSILON),
        WrapMode::Repeat if (0.0..1.0).contains(&value) => value,
        // The period is exactly one: removing the integral part avoids a
        // general floating-point remainder on every repeated texture sample.
        WrapMode::Repeat => value - value.floor(),
    }
}

fn texture_index(value: f64, size: u32, mode: WrapMode) -> usize {
    if mode == WrapMode::Clamp {
        // Addressed nearest coordinates are non-negative and below size, so
        // truncation is floor and needs neither a libm call nor integer modulo.
        return (addressed(value, mode) * f64::from(size)) as usize;
    }
    // Repeat addressing is also non-negative. Its only possible upper-end
    // result is exactly size: adding one to a tiny negative remainder can
    // round to 1.0. Wrap that boundary without another floor or integer divide.
    let index = (addressed(value, mode) * f64::from(size)) as usize;
    if index < size as usize { index } else { 0 }
}

fn texel_index(raw: i64, size: u32, mode: WrapMode) -> usize {
    match mode {
        WrapMode::Clamp => raw.clamp(0, i64::from(size) - 1) as usize,
        // Sampling normalizes the coordinate before selecting its two
        // neighbors, so only -1 and size can lie outside the image.
        WrapMode::Repeat if raw < 0 => size as usize - 1,
        WrapMode::Repeat if raw >= i64::from(size) => 0,
        WrapMode::Repeat => raw as usize,
    }
}

fn bilinear(pixels: [u32; 4], x: f64, y: f64) -> u32 {
    let mut output = 0_u32;
    for shift in [24, 16, 8, 0] {
        let values = pixels.map(|pixel| f64::from((pixel >> shift) & 0xff));
        let top = x.mul_add(values[1] - values[0], values[0]);
        let bottom = x.mul_add(values[3] - values[2], values[2]);
        let value = rounded_u32(y.mul_add(bottom - top, top), 255);
        output |= value << shift;
    }
    output
}

fn blend_texture(
    function: BlendFunction,
    format: ImageFormat,
    texture: u32,
    fragment: u32,
    blend_color: u32,
) -> u32 {
    let texture_alpha = (texture >> 24) & 0xff;
    let mut output = match function {
        BlendFunction::Replace if matches!(format, ImageFormat::Luminance | ImageFormat::Rgb) => {
            fragment & 0xff00_0000
        }
        BlendFunction::Replace => texture & 0xff00_0000,
        BlendFunction::Decal => fragment & 0xff00_0000,
        _ => ((texture_alpha * ((fragment >> 24) & 0xff) + 127) / 255) << 24,
    };
    if format == ImageFormat::Alpha {
        return output | fragment & 0x00ff_ffff;
    }
    if function == BlendFunction::Replace
        || (function == BlendFunction::Modulate && fragment & 0x00ff_ffff == 0x00ff_ffff)
    {
        return output | texture & 0x00ff_ffff;
    }
    for shift in [16, 8, 0] {
        let texel = (texture >> shift) & 0xff;
        let base = (fragment >> shift) & 0xff;
        let value = match function {
            BlendFunction::Replace => texel,
            BlendFunction::Modulate => (texel * base + 127) / 255,
            BlendFunction::Decal => {
                (texel * texture_alpha + base * (255 - texture_alpha) + 127) / 255
            }
            BlendFunction::Add => (texel + base).min(255),
            BlendFunction::Blend => {
                let constant = (blend_color >> shift) & 0xff;
                (base * (255 - texel) + constant * texel + 127) / 255
            }
        };
        output |= value << shift;
    }
    output
}

#[cfg(test)]
#[path = "../../../../tests/unit/m3g/appearance/texture.rs"]
mod tests;
