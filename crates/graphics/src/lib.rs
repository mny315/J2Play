//! Deterministic, host-independent MIDP software graphics.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::missing_errors_doc,
    clippy::similar_names,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use diagnostics::{Category, EmuError};

mod bmp;
mod font;
mod gif;
mod jpeg;
mod png;
mod primitives;
mod triangles;

use bmp::decode_bmp;
use font::glyph;
use gif::decode_gif;
use jpeg::decode_jpeg;
use png::{decode_png, decode_png_with_midp_compatibility, encode_png};

pub use font::{system_font_glyph, system_font_glyph as compact_lcd_ui_font_glyph};
pub use primitives::{clipped_line_points, ellipse_arc_samples, ellipse_point};
pub use triangles::clipped_triangle_rows;

const MAX_IMAGE_PIXELS: usize = 4_194_304;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrokeStyle {
    Solid,
    Dotted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Transform {
    None = 0,
    MirrorRot180 = 1,
    Mirror = 2,
    Rot180 = 3,
    MirrorRot270 = 4,
    Rot90 = 5,
    Rot270 = 6,
    MirrorRot90 = 7,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    #[must_use]
    pub fn intersect(self, rhs: Self) -> Self {
        let x = i64::from(self.x.max(rhs.x));
        let y = i64::from(self.y.max(rhs.y));
        let right = (i64::from(self.x) + i64::from(self.width.max(0)))
            .min(i64::from(rhs.x) + i64::from(rhs.width.max(0)));
        let bottom = (i64::from(self.y) + i64::from(self.height.max(0)))
            .min(i64::from(rhs.y) + i64::from(rhs.height.max(0)));
        Self {
            x: clamp_i64(x),
            y: clamp_i64(y),
            width: clamp_i64((right - x).max(0)),
            height: clamp_i64((bottom - y).max(0)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
    mutable: bool,
}

impl Image {
    /// Creates a mutable opaque image, initially white.
    pub fn mutable(width: u32, height: u32) -> Result<Self, EmuError> {
        let len = checked_pixels(width, height)?;
        Ok(Self {
            width,
            height,
            pixels: vec![0xffff_ffff; len],
            mutable: true,
        })
    }
    /// Creates an immutable image from MIDP ARGB integers.
    pub fn from_argb(
        pixels: &[u32],
        width: u32,
        height: u32,
        process_alpha: bool,
    ) -> Result<Self, EmuError> {
        let len = checked_pixels(width, height)?;
        if pixels.len() != len {
            return Err(error(
                "image-size",
                "RGB array does not match image dimensions",
            ));
        }
        let pixels = pixels
            .iter()
            .map(|p| if process_alpha { *p } else { *p | 0xff00_0000 })
            .collect();
        Ok(Self {
            width,
            height,
            pixels,
            mutable: false,
        })
    }
    /// Decodes bounded PNG (grayscale, RGB, RGBA, grayscale+alpha or indexed), including Adam7.
    pub fn from_png(bytes: &[u8]) -> Result<Self, EmuError> {
        decode_png(bytes)
    }
    /// Decodes a bounded image using MIDP handset compatibility behavior.
    pub fn from_midp_encoded(bytes: &[u8]) -> Result<Self, EmuError> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            decode_png_with_midp_compatibility(bytes)
        } else {
            Self::from_encoded(bytes)
        }
    }
    /// Decodes a bounded self-identifying MIDP image (PNG, JPEG, GIF or BMP).
    pub fn from_encoded(bytes: &[u8]) -> Result<Self, EmuError> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            decode_png(bytes)
        } else if bytes.starts_with(&[0xff, 0xd8]) {
            decode_jpeg(bytes)
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            decode_gif(bytes)
        } else if bytes.starts_with(b"BM") {
            decode_bmp(bytes)
        } else {
            Err(error("image-format", "unsupported image signature"))
        }
    }
    /// Returns a drawing context for a mutable image.
    pub fn graphics(&mut self) -> Result<Graphics<'_>, EmuError> {
        if !self.mutable {
            return Err(error("immutable-image", "immutable image has no Graphics"));
        }
        Ok(Graphics::new(self))
    }
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
    #[must_use]
    pub const fn is_mutable(&self) -> bool {
        self.mutable
    }
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
    /// Transfers the decoded ARGB buffer without copying its pixels.
    #[must_use]
    pub fn into_pixels(self) -> Vec<u32> {
        self.pixels
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Framebuffer {
    image: Image,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Result<Self, EmuError> {
        Ok(Self {
            image: Image::mutable(width, height)?,
        })
    }
    #[must_use]
    pub fn width(&self) -> u32 {
        self.image.width
    }
    #[must_use]
    pub fn height(&self) -> u32 {
        self.image.height
    }
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.image.pixels
    }
    pub fn graphics(&mut self) -> Graphics<'_> {
        Graphics::new(&mut self.image)
    }
    /// Encodes an RGBA PNG with deterministic filtering/compression.
    #[must_use]
    pub fn to_png(&self) -> Vec<u8> {
        encode_png(&self.image)
    }
}

pub struct Graphics<'a> {
    target: &'a mut Image,
    color: u32,
    tx: i32,
    ty: i32,
    clip: Rect,
    stroke: StrokeStyle,
}

impl<'a> Graphics<'a> {
    fn new(target: &'a mut Image) -> Self {
        let clip = Rect {
            x: 0,
            y: 0,
            width: target.width as i32,
            height: target.height as i32,
        };
        Self {
            target,
            color: 0xff00_0000,
            tx: 0,
            ty: 0,
            clip,
            stroke: StrokeStyle::Solid,
        }
    }
    pub fn set_color(&mut self, rgb: u32) {
        self.color = 0xff00_0000 | (rgb & 0x00ff_ffff);
    }
    #[must_use]
    pub const fn color(&self) -> u32 {
        self.color
    }
    pub fn set_stroke_style(&mut self, style: StrokeStyle) {
        self.stroke = style;
    }
    pub fn translate(&mut self, x: i32, y: i32) {
        self.tx = self.tx.saturating_add(x);
        self.ty = self.ty.saturating_add(y);
    }
    #[must_use]
    pub const fn translation(&self) -> (i32, i32) {
        (self.tx, self.ty)
    }
    pub fn set_clip(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: self.target.width as i32,
            height: self.target.height as i32,
        };
        self.clip = Rect {
            x: x.saturating_add(self.tx),
            y: y.saturating_add(self.ty),
            width: width.max(0),
            height: height.max(0),
        }
        .intersect(bounds);
    }
    pub fn clip_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.clip = self.clip.intersect(Rect {
            x: x.saturating_add(self.tx),
            y: y.saturating_add(self.ty),
            width: width.max(0),
            height: height.max(0),
        });
    }
    #[must_use]
    pub fn clip(&self) -> Rect {
        Rect {
            x: self.clip.x.saturating_sub(self.tx),
            y: self.clip.y.saturating_sub(self.ty),
            ..self.clip
        }
    }
    pub fn draw_rgb(
        &mut self,
        pixels: &[u32],
        offset: i32,
        scan_length: i32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        process_alpha: bool,
    ) -> Result<(), EmuError> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        let first = i64::from(offset);
        let final_row = first + i64::from(height - 1) * i64::from(scan_length);
        let low = first.min(final_row);
        let high = first.max(final_row) + i64::from(width);
        if low < 0 || high > pixels.len() as i64 {
            return Err(error("image-size", "RGB array region is outside input"));
        }
        let ox = x.saturating_add(self.tx);
        let oy = y.saturating_add(self.ty);
        let columns = clipped_raster_axis(ox, width, self.clip.x, self.clip.width);
        let rows = clipped_raster_axis(oy, height, self.clip.y, self.clip.height);
        if columns.is_empty() || rows.is_empty() {
            return Ok(());
        }
        let count = (columns.end - columns.start) as usize;
        let target_x = (i64::from(ox) + i64::from(columns.start)) as usize;
        for row in rows {
            let source = (first
                + i64::from(row) * i64::from(scan_length)
                + i64::from(columns.start)) as usize;
            let target_y = (i64::from(oy) + i64::from(row)) as usize;
            let target = target_y * self.target.width as usize + target_x;
            let output = &mut self.target.pixels[target..target + count];
            for (&pixel, destination) in pixels[source..source + count].iter().zip(output) {
                blend_pixel(
                    destination,
                    if process_alpha {
                        pixel
                    } else {
                        pixel | 0xff00_0000
                    },
                );
            }
        }
        Ok(())
    }
    pub fn copy_area(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        dest_x: i32,
        dest_y: i32,
        anchor: u32,
    ) -> Result<(), EmuError> {
        let sx = x.saturating_add(self.tx);
        let sy = y.saturating_add(self.ty);
        if sx < 0
            || sy < 0
            || (sx as u32)
                .checked_add(width)
                .is_none_or(|right| right > self.target.width)
            || (sy as u32)
                .checked_add(height)
                .is_none_or(|bottom| bottom > self.target.height)
        {
            return Err(error("image-region", "copy source is outside target"));
        }
        if width == 0 || height == 0 {
            return Err(error("image-region", "source region is outside image"));
        }
        let (ox, oy) = anchor_origin(
            dest_x.saturating_add(self.tx),
            dest_y.saturating_add(self.ty),
            width as i32,
            height as i32,
            anchor,
        )?;
        let columns = clipped_raster_axis(ox, width, self.clip.x, self.clip.width);
        let rows = clipped_raster_axis(oy, height, self.clip.y, self.clip.height);
        if columns.is_empty() || rows.is_empty() {
            return Ok(());
        }
        let stride = self.target.width as usize;
        let count = (columns.end - columns.start) as usize;
        // Mutable targets are opaque. Copy bottom-up when moving down so
        // overlapping rows retain the original source; copy_within handles
        // horizontal overlap without a temporary image.
        for index in 0..rows.end - rows.start {
            let row = if oy > sy {
                rows.end - 1 - index
            } else {
                rows.start + index
            };
            let source =
                (sy as usize + row as usize) * stride + sx as usize + columns.start as usize;
            let target = (oy + row as i32) as usize * stride + (ox + columns.start as i32) as usize;
            self.target
                .pixels
                .copy_within(source..source + count, target);
        }
        Ok(())
    }
    pub fn draw_image(
        &mut self,
        image: &Image,
        x: i32,
        y: i32,
        anchor: u32,
    ) -> Result<(), EmuError> {
        self.draw_region(
            image,
            0,
            0,
            image.width,
            image.height,
            Transform::None,
            x,
            y,
            anchor,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw_region(
        &mut self,
        image: &Image,
        sx: u32,
        sy: u32,
        width: u32,
        height: u32,
        transform: Transform,
        x: i32,
        y: i32,
        anchor: u32,
    ) -> Result<(), EmuError> {
        if width == 0
            || height == 0
            || sx.checked_add(width).is_none_or(|v| v > image.width)
            || sy.checked_add(height).is_none_or(|v| v > image.height)
        {
            return Err(error("image-region", "source region is outside image"));
        }
        let swapped = matches!(
            transform,
            Transform::Rot90 | Transform::Rot270 | Transform::MirrorRot90 | Transform::MirrorRot270
        );
        let (dw, dh) = if swapped {
            (height, width)
        } else {
            (width, height)
        };
        let (ox, oy) = anchor_origin(
            x.saturating_add(self.tx),
            y.saturating_add(self.ty),
            dw as i32,
            dh as i32,
            anchor,
        )?;
        let columns = clipped_raster_axis(ox, dw, self.clip.x, self.clip.width);
        let rows = clipped_raster_axis(oy, dh, self.clip.y, self.clip.height);
        if columns.is_empty() || rows.is_empty() {
            return Ok(());
        }
        let count = (columns.end - columns.start) as usize;
        let target_x = (i64::from(ox) + i64::from(columns.start)) as usize;
        for dy in rows {
            let target_y = (i64::from(oy) + i64::from(dy)) as usize;
            let target = target_y * self.target.width as usize + target_x;
            let output = &mut self.target.pixels[target..target + count];
            for (dx, destination) in columns.clone().zip(output) {
                let (rx, ry) = inverse_transform(dx, dy, width, height, transform);
                let pixel = image.pixels[((sy + ry) * image.width + sx + rx) as usize];
                blend_pixel(destination, pixel);
            }
        }
        Ok(())
    }
    pub fn draw_char(&mut self, ch: char, x: i32, y: i32) {
        let glyph = glyph(ch);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    self.put(
                        x.saturating_add(self.tx).saturating_add(col),
                        y.saturating_add(self.ty).saturating_add(row as i32),
                        self.color,
                    );
                }
            }
        }
    }
    pub fn draw_string(&mut self, text: &str, x: i32, y: i32) {
        for (n, ch) in text.chars().enumerate() {
            let offset = i32::try_from(n).unwrap_or(i32::MAX).saturating_mul(6);
            self.draw_char(ch, x.saturating_add(offset), y);
        }
    }
    fn put(&mut self, x: i32, y: i32, pixel: u32) {
        if x >= self.clip.x
            && y >= self.clip.y
            && x < self.clip.x + self.clip.width
            && y < self.clip.y + self.clip.height
        {
            self.target.pixels[(y as u32 * self.target.width + x as u32) as usize] = pixel;
        }
    }
}

fn blend_pixel(destination: &mut u32, source: u32) {
    let alpha = source >> 24;
    if alpha == 255 {
        *destination = source;
    } else if alpha != 0 {
        let inverse_alpha = 255 - alpha;
        let blend = |source: u32, destination: u32| {
            (source * alpha + destination * inverse_alpha + 127) / 255
        };
        *destination = 0xff00_0000
            | blend((source >> 16) & 255, (*destination >> 16) & 255) << 16
            | blend((source >> 8) & 255, (*destination >> 8) & 255) << 8
            | blend(source & 255, *destination & 255);
    }
}

pub struct Font {
    pub height: u32,
    pub baseline: u32,
    pub char_width: u32,
}
impl Font {
    #[must_use]
    pub const fn system_small() -> Self {
        Self {
            height: 8,
            baseline: 7,
            char_width: 6,
        }
    }
    /// Returns the text width, saturating at the largest representable pixel width.
    #[must_use]
    pub fn string_width(&self, value: &str) -> u32 {
        self.char_width
            .saturating_mul(u32::try_from(value.chars().count()).unwrap_or(u32::MAX))
    }
}

#[cfg(test)]
#[allow(clippy::trivially_copy_pass_by_ref, clippy::unreadable_literal)]
#[path = "../../../tests/unit/graphics/mod.rs"]
mod tests;

fn checked_pixels(width: u32, height: u32) -> Result<usize, EmuError> {
    let n = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| error("image-size", "image dimensions overflow"))?;
    if width == 0 || height == 0 || n > MAX_IMAGE_PIXELS {
        Err(error("image-size", "invalid or oversized image"))
    } else {
        Ok(n)
    }
}
fn clamp_i64(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
fn clipped_raster_axis(
    origin: i32,
    length: u32,
    clip_origin: i32,
    clip_length: i32,
) -> std::ops::Range<u32> {
    let first = i64::from(clip_origin) - i64::from(origin);
    let end = first + i64::from(clip_length);
    first.clamp(0, i64::from(length)) as u32..end.clamp(0, i64::from(length)) as u32
}
fn error(code: &'static str, message: &'static str) -> EmuError {
    EmuError::new(Category::Api, code, message)
}
fn anchor_origin(x: i32, y: i32, w: i32, h: i32, a: u32) -> Result<(i32, i32), EmuError> {
    const HC: u32 = 1;
    const VC: u32 = 2;
    const L: u32 = 4;
    const R: u32 = 8;
    const T: u32 = 16;
    const B: u32 = 32;
    if a & !(HC | VC | L | R | T | B) != 0
        || (a & (HC | L | R)).count_ones() > 1
        || (a & (VC | T | B)).count_ones() > 1
    {
        return Err(error("graphics-anchor", "invalid anchor"));
    }
    Ok((
        if a & R != 0 {
            x.saturating_sub(w)
        } else if a & HC != 0 {
            x.saturating_sub(w / 2)
        } else {
            x
        },
        if a & B != 0 {
            y.saturating_sub(h)
        } else if a & VC != 0 {
            y.saturating_sub(h / 2)
        } else {
            y
        },
    ))
}
fn inverse_transform(x: u32, y: u32, w: u32, h: u32, t: Transform) -> (u32, u32) {
    match t {
        Transform::None => (x, y),
        Transform::MirrorRot180 => (x, h - 1 - y),
        Transform::Mirror => (w - 1 - x, y),
        Transform::Rot180 => (w - 1 - x, h - 1 - y),
        Transform::MirrorRot270 => (y, x),
        Transform::Rot90 => (y, h - 1 - x),
        Transform::Rot270 => (w - 1 - y, x),
        Transform::MirrorRot90 => (w - 1 - y, h - 1 - x),
    }
}
