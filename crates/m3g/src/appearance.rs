//! Images, texture sampling, fog and fixed-function material lighting.

use crate::math::rounded_u32;
use diagnostics::{Category, EmuError};
use std::sync::Arc;

mod lighting;
mod texture;
pub(crate) use lighting::PreparedLighting;
pub use lighting::{LightSource, MaterialState, shade_lit_vertex};
pub use texture::{BlendFunction, Texture2DState, WrapMode};

/// M3G `Image2D` pixel format.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ImageFormat {
    /// One alpha byte.
    Alpha,
    /// One luminance byte.
    Luminance,
    /// Luminance followed by alpha.
    LuminanceAlpha,
    /// RGB bytes.
    Rgb,
    /// RGBA bytes.
    Rgba,
}

impl ImageFormat {
    /// Number of tightly packed bytes per pixel or palette entry.
    #[must_use]
    pub const fn components(self) -> usize {
        match self {
            Self::Alpha | Self::Luminance => 1,
            Self::LuminanceAlpha => 2,
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }
}

/// Bounded, decoded `Image2D` state in straight-alpha ARGB form.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Image2DState {
    width: u32,
    height: u32,
    format: ImageFormat,
    mutable: bool,
    pixels: Arc<[u32]>,
}

impl Image2DState {
    /// Validates image dimensions and returns the bounded pixel count.
    pub fn validate_dimensions(width: u32, height: u32) -> Result<usize, EmuError> {
        if width == 0 || height == 0 {
            return Err(appearance_error(
                "image-size",
                "Image2D dimensions must be positive",
            ));
        }
        if width > 1_024 || height > 1_024 {
            return Err(image_limit());
        }
        Ok(width as usize * height as usize)
    }

    pub(crate) fn validate_checkpoint(&self) -> Result<(), EmuError> {
        if Self::validate_dimensions(self.width, self.height)? != self.pixels.len() {
            return Err(appearance_error(
                "checkpoint-image",
                "Checkpoint image pixels do not match their dimensions",
            ));
        }
        Ok(())
    }

    /// Creates a mutable image initialized to opaque white in every format.
    pub fn mutable(format: ImageFormat, width: u32, height: u32) -> Result<Self, EmuError> {
        let count = Self::validate_dimensions(width, height)?;
        Ok(Self {
            width,
            height,
            format,
            mutable: true,
            pixels: std::iter::repeat_n(0xffff_ffff, count).collect(),
        })
    }

    /// Copies the required tightly packed pixel prefix for an immutable image.
    pub fn from_bytes(
        format: ImageFormat,
        width: u32,
        height: u32,
        bytes: &[u8],
    ) -> Result<Self, EmuError> {
        let count = Self::validate_dimensions(width, height)?;
        let expected = count * format.components();
        if bytes.len() < expected {
            return Err(appearance_error(
                "image-size",
                "Image2D byte count is shorter than dimensions and format require",
            ));
        }
        let pixels = bytes[..expected]
            .chunks_exact(format.components())
            .map(|pixel| decode_pixel(format, pixel))
            .collect();
        Ok(Self {
            width,
            height,
            format,
            mutable: false,
            pixels,
        })
    }

    /// Copies the required one-byte index prefix and at most 256 palette entries.
    pub fn from_palette(
        format: ImageFormat,
        width: u32,
        height: u32,
        indices: &[u8],
        palette: &[u8],
    ) -> Result<Self, EmuError> {
        let count = Self::validate_dimensions(width, height)?;
        let components = format.components();
        let complete_palette_bytes = 256 * components;
        if indices.len() < count
            || (palette.len() < complete_palette_bytes && !palette.len().is_multiple_of(components))
        {
            return Err(appearance_error(
                "image-palette",
                format!(
                    "Image2D palette or index data is malformed: format={format:?} dimensions={width}x{height} indices={} minimum_indices={count} palette_bytes={}",
                    indices.len(),
                    palette.len(),
                ),
            ));
        }
        let palette = palette[..palette.len().min(complete_palette_bytes)]
            .chunks_exact(components)
            .map(|entry| decode_pixel(format, entry))
            .collect::<Vec<_>>();
        // The API leaves palette entries beyond the supplied prefix undefined.
        // Keep that case deterministic without rejecting an otherwise valid image.
        let undefined = decode_pixel(format, &[0; 4][..components]);
        let pixels = indices[..count]
            .iter()
            .map(|index| {
                palette
                    .get(usize::from(*index))
                    .copied()
                    .unwrap_or(undefined)
            })
            .collect();
        Ok(Self {
            width,
            height,
            format,
            mutable: false,
            pixels,
        })
    }

    /// Converts straight-alpha ARGB pixels from a MIDP image into the requested format.
    pub fn from_argb(
        format: ImageFormat,
        width: u32,
        height: u32,
        pixels: &[u32],
    ) -> Result<Self, EmuError> {
        let count = Self::validate_dimensions(width, height)?;
        if pixels.len() != count {
            return Err(appearance_error(
                "image-size",
                "MIDP source pixel count does not match dimensions",
            ));
        }
        let pixels = pixels
            .iter()
            .map(|pixel| {
                let alpha = *pixel & 0xff00_0000;
                let red = (*pixel >> 16) & 0xff;
                let green = (*pixel >> 8) & 0xff;
                let blue = *pixel & 0xff;
                let luminance = (red * 77 + green * 150 + blue * 29 + 128) >> 8;
                match format {
                    ImageFormat::Alpha => alpha | 0x00ff_ffff,
                    ImageFormat::Luminance => {
                        0xff00_0000 | luminance << 16 | luminance << 8 | luminance
                    }
                    ImageFormat::LuminanceAlpha => {
                        alpha | luminance << 16 | luminance << 8 | luminance
                    }
                    ImageFormat::Rgb => 0xff00_0000 | *pixel & 0x00ff_ffff,
                    ImageFormat::Rgba => *pixel,
                }
            })
            .collect();
        Ok(Self {
            width,
            height,
            format,
            mutable: false,
            pixels,
        })
    }

    /// Adopts an already decoded immutable RGBA pixel buffer without copying it.
    pub fn from_shared_argb(width: u32, height: u32, pixels: Arc<[u32]>) -> Result<Self, EmuError> {
        let count = Self::validate_dimensions(width, height)?;
        if pixels.len() != count {
            return Err(appearance_error(
                "image-size",
                "shared ARGB source pixel count does not match dimensions",
            ));
        }
        Ok(Self {
            width,
            height,
            format: ImageFormat::Rgba,
            mutable: false,
            pixels,
        })
    }

    /// Image width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Image height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Pixel format.
    #[must_use]
    pub const fn format(&self) -> ImageFormat {
        self.format
    }

    /// Whether `set` is permitted.
    #[must_use]
    pub const fn is_mutable(&self) -> bool {
        self.mutable
    }

    /// Immutable straight-alpha ARGB pixels.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// Logical bytes retained by this image's pixel storage.
    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        self.pixels.len().saturating_mul(size_of::<u32>())
    }

    /// Replaces the complete mutable image from a render target.
    pub fn load_argb(&mut self, pixels: &[u32]) -> Result<(), EmuError> {
        if !self.mutable {
            return Err(appearance_error(
                "immutable-image",
                "cannot render into an immutable Image2D",
            ));
        }
        if pixels.len() != self.pixels.len() {
            return Err(appearance_error(
                "image-size",
                "render target pixel count does not match Image2D dimensions",
            ));
        }
        let destination = Arc::make_mut(&mut self.pixels);
        if self.format == ImageFormat::Rgb {
            for (destination, source) in destination.iter_mut().zip(pixels) {
                *destination = *source | 0xff00_0000;
            }
        } else {
            destination.copy_from_slice(pixels);
        }
        Ok(())
    }

    /// Replaces a rectangular region of a mutable image atomically.
    pub fn set(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        bytes: &[u8],
    ) -> Result<(), EmuError> {
        let expected = self.validate_update(x, y, width, height)?;
        if bytes.len() < expected {
            return Err(appearance_error(
                "image-size",
                "Image2D update byte count is shorter than the region requires",
            ));
        }
        let components = self.format.components();
        let source_stride = width as usize * components;
        let pixels = Arc::make_mut(&mut self.pixels);
        for (row, source) in bytes[..expected].chunks_exact(source_stride).enumerate() {
            let destination = (y as usize + row) * self.width as usize + x as usize;
            for (destination, source) in pixels[destination..destination + width as usize]
                .iter_mut()
                .zip(source.chunks_exact(components))
            {
                *destination = decode_pixel(self.format, source);
            }
        }
        Ok(())
    }

    /// Validates a mutable update region and returns its required source byte count.
    pub fn validate_update(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<usize, EmuError> {
        if !self.mutable {
            return Err(appearance_error(
                "immutable-image",
                "cannot update immutable Image2D",
            ));
        }
        if width == 0
            || height == 0
            || x.checked_add(width).is_none_or(|right| right > self.width)
            || y.checked_add(height)
                .is_none_or(|bottom| bottom > self.height)
        {
            return Err(appearance_error(
                "image-region",
                "Image2D update region is empty or outside the image",
            ));
        }
        Ok(width as usize * height as usize * self.format.components())
    }
}

/// Fog equation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FogMode {
    /// Linear between near and far distances.
    Linear,
    /// Exponential by density.
    Exponential,
}

/// Fixed-function fog state.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FogState {
    /// RGB fog color.
    pub color: u32,
    /// Equation mode.
    pub mode: FogMode,
    /// Linear near distance.
    pub near: f32,
    /// Linear far distance.
    pub far: f32,
    /// Exponential density.
    pub density: f32,
}

impl FogState {
    /// Applies fog to straight-alpha ARGB color at a camera-space distance.
    #[must_use]
    pub fn apply(self, color: u32, distance: f64) -> u32 {
        let factor = match self.mode {
            FogMode::Linear => {
                let denominator = f64::from(self.far) - f64::from(self.near);
                if denominator == 0.0 {
                    0.0
                } else {
                    ((f64::from(self.far) - distance) / denominator).clamp(0.0, 1.0)
                }
            }
            FogMode::Exponential => (-f64::from(self.density) * distance.max(0.0))
                .exp()
                .clamp(0.0, 1.0),
        };
        mix_rgb(self.color, color, factor)
    }
}

fn image_limit() -> EmuError {
    appearance_error("resource-limit", "Image2D exceeds profile pixel budget")
}

fn decode_pixel(format: ImageFormat, bytes: &[u8]) -> u32 {
    match format {
        ImageFormat::Alpha => u32::from(bytes[0]) << 24 | 0x00ff_ffff,
        ImageFormat::Luminance => {
            let value = u32::from(bytes[0]);
            0xff00_0000 | value << 16 | value << 8 | value
        }
        ImageFormat::LuminanceAlpha => {
            let value = u32::from(bytes[0]);
            u32::from(bytes[1]) << 24 | value << 16 | value << 8 | value
        }
        ImageFormat::Rgb => {
            0xff00_0000 | u32::from(bytes[0]) << 16 | u32::from(bytes[1]) << 8 | u32::from(bytes[2])
        }
        ImageFormat::Rgba => {
            u32::from(bytes[3]) << 24
                | u32::from(bytes[0]) << 16
                | u32::from(bytes[1]) << 8
                | u32::from(bytes[2])
        }
    }
}

fn mix_rgb(first: u32, second: u32, second_weight: f64) -> u32 {
    let mut output = second & 0xff00_0000;
    for shift in [16, 8, 0] {
        let left = f64::from((first >> shift) & 0xff);
        let right = f64::from((second >> shift) & 0xff);
        let value = rounded_u32(second_weight.mul_add(right - left, left), 255);
        output |= value << shift;
    }
    output
}

fn appearance_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::M3g, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/m3g/appearance/mod.rs"]
mod tests;
