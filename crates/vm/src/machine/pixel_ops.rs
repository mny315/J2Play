use super::{ArrayKind, EmuError, HeapValue, type_error, vm_error};

// Transform the image prefix, preserving any trailing values in packed arrays.
// The caller already owns a snapshot, so row-preserving transforms reuse it.
pub(super) fn transform_image_pixels(
    mut source: Vec<i32>,
    width: i32,
    height: i32,
    transform: i32,
) -> Result<(i32, i32, Vec<i32>), EmuError> {
    if width <= 0 || height <= 0 || !(0..=7).contains(&transform) {
        return Err(vm_error("illegal-argument", "invalid image transform"));
    }
    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| vm_error("out-of-memory-error", "image dimensions overflow"))?;
    if source.len() < expected {
        return Err(vm_error(
            "array-index-out-of-bounds-exception",
            "image pixel buffer has an invalid length",
        ));
    }
    if transform < 4 {
        let region = &mut source[..expected];
        if matches!(transform, 1 | 3) {
            region.reverse();
        }
        if matches!(transform, 1 | 2) {
            for row in region.chunks_exact_mut(width as usize) {
                row.reverse();
            }
        }
        return Ok((width, height, source));
    }
    let mut output = Vec::with_capacity(source.len());
    // Transposition changes the row stride. All indices fit in the image
    // prefix validated above; usize arithmetic avoids an artificial i32 cap.
    let (input_width, input_height) = (width as usize, height as usize);
    for x in 0..input_width {
        let source_x = if transform >= 6 {
            input_width - 1 - x
        } else {
            x
        };
        for y in 0..input_height {
            let source_y = if transform & 1 != 0 {
                input_height - 1 - y
            } else {
                y
            };
            output.push(source[source_y * input_width + source_x]);
        }
    }
    output.extend_from_slice(&source[expected..]);
    Ok((height, width, output))
}

#[inline]
pub(super) fn blend_argb(source: i32, destination: i32) -> i32 {
    let alpha = source.cast_unsigned() >> 24;
    if alpha == 0 {
        return destination;
    }
    let source = source.cast_unsigned();
    let destination = destination.cast_unsigned();
    let inverse = 255 - alpha;
    let destination_alpha = destination >> 24;
    if destination_alpha != 255 {
        // ARGB stores straight color. Weight both colors by their opacity,
        // then normalize by the resulting source-over alpha.
        let source_weight = alpha * 255;
        let destination_weight = destination_alpha * inverse;
        let combined = source_weight + destination_weight;
        let blend = |shift: u32| -> u32 {
            (((source >> shift) & 255) * source_weight
                + ((destination >> shift) & 255) * destination_weight
                + combined / 2)
                / combined
        };
        return (((combined + 127) / 255) << 24 | blend(16) << 16 | blend(8) << 8 | blend(0))
            .cast_signed();
    }
    let red =
        (((source >> 16 & 255) * alpha + (destination >> 16 & 255) * inverse + 127) / 255) << 16;
    let green =
        (((source >> 8 & 255) * alpha + (destination >> 8 & 255) * inverse + 127) / 255) << 8;
    let blue = ((source & 255) * alpha + (destination & 255) * inverse + 127) / 255;
    (0xff00_0000 | red | green | blue).cast_signed()
}

#[inline]
pub(super) fn composite_argb_slot(slot: &mut HeapValue, color: i32) -> Result<(), EmuError> {
    let alpha = color.cast_unsigned() >> 24;
    if alpha == 0 {
        return Ok(());
    }
    if alpha == 255 {
        *slot = HeapValue::Int(color);
        return Ok(());
    }
    let HeapValue::Int(destination) = *slot else {
        return Err(type_error());
    };
    *slot = HeapValue::Int(blend_argb(color, destination));
    Ok(())
}

pub(super) fn paint_argb_pixel(
    pixels: &mut [HeapValue],
    width: i32,
    x: i32,
    y: i32,
    color: i32,
) -> Result<(), EmuError> {
    if color.cast_unsigned() >> 24 == 0 {
        return Ok(());
    }
    let index = y
        .checked_mul(width)
        .and_then(|base| base.checked_add(x))
        .and_then(|index| usize::try_from(index).ok())
        .ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "DirectGraphics target index overflow",
            )
        })?;
    let slot = pixels.get_mut(index).ok_or_else(|| {
        vm_error(
            "array-index-out-of-bounds-exception",
            "DirectGraphics target index is outside the pixel buffer",
        )
    })?;
    composite_argb_slot(slot, color)
}

pub(super) struct ShapePainter<'a> {
    pixels: &'a mut [HeapValue],
    width: i32,
    clip: (i32, i32, i32, i32),
    color: i32,
    covered: Vec<bool>,
}

impl<'a> ShapePainter<'a> {
    pub(super) fn new(
        pixels: &'a mut [HeapValue],
        width: i32,
        clip: (i32, i32, i32, i32),
        color: i32,
    ) -> Result<Self, EmuError> {
        let mut covered = Vec::new();
        if (1..255).contains(&(color.cast_unsigned() >> 24)) && clip.0 < clip.2 && clip.1 < clip.3 {
            let count = usize::try_from(i64::from(clip.2) - i64::from(clip.0))
                .ok()
                .and_then(|width| {
                    usize::try_from(i64::from(clip.3) - i64::from(clip.1))
                        .ok()
                        .and_then(|height| width.checked_mul(height))
                })
                .filter(|count| *count <= pixels.len())
                .ok_or_else(|| {
                    vm_error("out-of-memory-error", "invalid shape coverage dimensions")
                })?;
            covered
                .try_reserve_exact(count)
                .map_err(|_| vm_error("out-of-memory-error", "shape coverage allocation failed"))?;
            covered.resize(count, false);
        }
        Ok(Self {
            pixels,
            width,
            clip,
            color,
            covered,
        })
    }

    pub(super) fn point(&mut self, x: i32, y: i32) -> Result<(), EmuError> {
        if x < self.clip.0 || y < self.clip.1 || x >= self.clip.2 || y >= self.clip.3 {
            return Ok(());
        }
        if !self.covered.is_empty() {
            // One API shape can rasterize a pixel through several edges or
            // spans. Composite it once, regardless of those internal overlaps.
            let row_width = (i64::from(self.clip.2) - i64::from(self.clip.0)) as usize;
            let index = (i64::from(y) - i64::from(self.clip.1)) as usize * row_width
                + (i64::from(x) - i64::from(self.clip.0)) as usize;
            if std::mem::replace(&mut self.covered[index], true) {
                return Ok(());
            }
        }
        paint_argb_pixel(self.pixels, self.width, x, y, self.color)
    }

    pub(super) fn span(
        &mut self,
        y: i32,
        xs: std::ops::RangeInclusive<i32>,
        mut check_work: impl FnMut(usize) -> Result<(), EmuError>,
    ) -> Result<(), EmuError> {
        if xs.is_empty() || y < self.clip.1 || y >= self.clip.3 || self.clip.0 >= self.clip.2 {
            return Ok(());
        }
        let left = (*xs.start()).max(self.clip.0);
        let right = (*xs.end()).min(self.clip.2 - 1);
        if left > right {
            return Ok(());
        }
        if left == right {
            check_work(0)?;
            return self.point(left, y);
        }
        let offsets = y.checked_mul(self.width).and_then(|row| {
            Some((
                usize::try_from(row.checked_add(left)?).ok()?,
                usize::try_from(row.checked_add(right)?).ok()?,
            ))
        });
        if let Some((first, last)) = offsets
            && let Some(pixels) = self.pixels.get_mut(first..=last)
        {
            if self.covered.is_empty() {
                for (index, chunk) in pixels.chunks_mut(1_024).enumerate() {
                    check_work(index * 1_024)?;
                    if self.color.cast_unsigned() >> 24 == 255 {
                        chunk.fill(HeapValue::Int(self.color));
                    }
                }
            } else {
                let row_width = (i64::from(self.clip.2) - i64::from(self.clip.0)) as usize;
                let start = (i64::from(y) - i64::from(self.clip.1)) as usize * row_width
                    + (i64::from(left) - i64::from(self.clip.0)) as usize;
                let covered = &mut self.covered[start..start + pixels.len()];
                for (index, (chunk, coverage)) in pixels
                    .chunks_mut(1_024)
                    .zip(covered.chunks_mut(1_024))
                    .enumerate()
                {
                    check_work(index * 1_024)?;
                    for (pixel, seen) in chunk.iter_mut().zip(coverage) {
                        if !std::mem::replace(seen, true) {
                            composite_argb_slot(pixel, self.color)?;
                        }
                    }
                }
            }
            return Ok(());
        }
        // Malformed backing buffers retain the same error and partial writes
        // as individual points; valid rows use the checked slice above.
        for (work, x) in (left..=right).enumerate() {
            check_work(work)?;
            self.point(x, y)?;
        }
        Ok(())
    }

    pub(super) fn line(
        &mut self,
        start: (i32, i32),
        end: (i32, i32),
        dotted: bool,
        mut check_work: impl FnMut(usize) -> Result<(), EmuError>,
    ) -> Result<(), EmuError> {
        let mut visited = 0;
        rasterize_line(start, end, self.clip, dotted, |x, y| {
            check_work(visited)?;
            visited += 1;
            self.point(x, y)
        })
    }

    pub(super) fn triangle(
        &mut self,
        vertices: [(i32, i32); 3],
        mut check_work: impl FnMut(usize) -> Result<(), EmuError>,
    ) -> Result<(), EmuError> {
        let clip = graphics::Rect {
            x: self.clip.0,
            y: self.clip.1,
            width: self.clip.2.saturating_sub(self.clip.0),
            height: self.clip.3.saturating_sub(self.clip.1),
        };
        for (y, xs) in graphics::clipped_triangle_rows(vertices, clip) {
            check_work(0)?;
            self.span(y, xs, &mut check_work)?;
        }
        Ok(())
    }
}

pub(super) fn draw_argb_line(
    pixels: &mut [HeapValue],
    width: i32,
    start: (i32, i32),
    end: (i32, i32),
    clip: (i32, i32, i32, i32),
    color: i32,
    dotted: bool,
) -> Result<(), EmuError> {
    rasterize_line(start, end, clip, dotted, |x, y| {
        paint_argb_pixel(pixels, width, x, y, color)
    })
}

fn rasterize_line(
    start: (i32, i32),
    end: (i32, i32),
    clip: (i32, i32, i32, i32),
    dotted: bool,
    mut paint: impl FnMut(i32, i32) -> Result<(), EmuError>,
) -> Result<(), EmuError> {
    let clip = graphics::Rect {
        x: clip.0,
        y: clip.1,
        width: clip.2.saturating_sub(clip.0),
        height: clip.3.saturating_sub(clip.1),
    };
    for (x, y, step) in graphics::clipped_line_points(start, end, clip) {
        if !dotted || step.is_multiple_of(2) {
            paint(x, y)?;
        }
    }
    Ok(())
}

pub(super) fn nokia_pixel_to_argb(
    value: i32,
    kind: &ArrayKind,
    format: i32,
    transparency: bool,
) -> Result<i32, EmuError> {
    let value = value.cast_unsigned();
    let argb = match (kind, format) {
        (ArrayKind::Int, 8888) => value,
        (ArrayKind::Int, 888) => 0xff00_0000 | (value & 0x00ff_ffff),
        (ArrayKind::Short, 4444) => {
            let alpha = (value >> 12) & 0xf;
            let red = (value >> 8) & 0xf;
            let green = (value >> 4) & 0xf;
            let blue = value & 0xf;
            (alpha * 17) << 24 | (red * 17) << 16 | (green * 17) << 8 | (blue * 17)
        }
        (ArrayKind::Short, 444) => {
            let red = (value >> 8) & 0xf;
            let green = (value >> 4) & 0xf;
            let blue = value & 0xf;
            0xff00_0000 | (red * 17) << 16 | (green * 17) << 8 | (blue * 17)
        }
        (ArrayKind::Short, 555) => {
            0xff00_0000
                | expand_5_bits((value >> 10) & 0x1f) << 16
                | expand_5_bits((value >> 5) & 0x1f) << 8
                | expand_5_bits(value & 0x1f)
        }
        (ArrayKind::Short, 1555) => {
            let alpha = if value & 0x8000 == 0 { 0 } else { 255 };
            alpha << 24
                | expand_5_bits((value >> 10) & 0x1f) << 16
                | expand_5_bits((value >> 5) & 0x1f) << 8
                | expand_5_bits(value & 0x1f)
        }
        (ArrayKind::Short, 565) => {
            0xff00_0000
                | expand_5_bits((value >> 11) & 0x1f) << 16
                | expand_6_bits((value >> 5) & 0x3f) << 8
                | expand_5_bits(value & 0x1f)
        }
        _ => {
            return Err(vm_error(
                "illegal-argument",
                "pixel format does not match the DirectGraphics array type",
            ));
        }
    };
    Ok(if transparency {
        argb.cast_signed()
    } else {
        (argb | 0xff00_0000).cast_signed()
    })
}

pub(super) fn argb_to_nokia_pixel(
    value: i32,
    kind: &ArrayKind,
    format: i32,
) -> Result<i32, EmuError> {
    let value = value.cast_unsigned();
    let packed = match (kind, format) {
        (ArrayKind::Int, 8888) => return Ok(value.cast_signed()),
        (ArrayKind::Int, 888) => return Ok((value & 0x00ff_ffff).cast_signed()),
        (ArrayKind::Short, 4444) => {
            (value >> 28 & 0xf) << 12
                | (value >> 20 & 0xf) << 8
                | (value >> 12 & 0xf) << 4
                | value >> 4 & 0xf
        }
        (ArrayKind::Short, 444) => {
            (value >> 20 & 0xf) << 8 | (value >> 12 & 0xf) << 4 | value >> 4 & 0xf
        }
        (ArrayKind::Short, 555) => {
            (value >> 19 & 0x1f) << 10 | (value >> 11 & 0x1f) << 5 | value >> 3 & 0x1f
        }
        (ArrayKind::Short, 1555) => {
            (u32::from(value >> 24 != 0) << 15)
                | (value >> 19 & 0x1f) << 10
                | (value >> 11 & 0x1f) << 5
                | value >> 3 & 0x1f
        }
        (ArrayKind::Short, 565) => {
            (value >> 19 & 0x1f) << 11 | (value >> 10 & 0x3f) << 5 | value >> 3 & 0x1f
        }
        _ => {
            return Err(vm_error(
                "illegal-argument",
                "pixel format does not match the DirectGraphics array type",
            ));
        }
    };
    Ok(i32::from((packed as u16) as i16))
}

pub(super) fn expand_5_bits(value: u32) -> u32 {
    (value << 3) | (value >> 2)
}

pub(super) fn expand_6_bits(value: u32) -> u32 {
    (value << 2) | (value >> 4)
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/pixel_ops.rs"]
mod tests;
