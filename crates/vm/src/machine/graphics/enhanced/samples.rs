//! Compact host samples, with checked guest-array planes for general LCDUI draws.

use super::super::vm_error;
use super::{EmuError, HeapValue, type_error};

#[derive(Debug)]
pub(super) enum SamplePlanes {
    Compact([Vec<i32>; 4]),
    Guest([Vec<HeapValue>; 4]),
}

impl SamplePlanes {
    pub(super) fn new(values: &[HeapValue]) -> Result<Option<Self>, EmuError> {
        let mut planes: [Vec<i32>; 4] = std::array::from_fn(|_| Vec::new());
        for plane in &mut planes {
            if plane.try_reserve_exact(values.len()).is_err() {
                return Ok(None);
            }
            for value in values {
                let HeapValue::Int(pixel) = value else {
                    return Err(type_error());
                };
                plane.push(*pixel);
            }
        }
        Ok(Some(Self::Compact(planes)))
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Compact(planes) => planes[0].len(),
            Self::Guest(planes) => planes[0].len(),
        }
    }

    /// Promotion precedes the draw and is atomic on allocation failure. Once a
    /// surface needs arbitrary guest-array raster routines, retain their planes
    /// so repeated image copies do not pay conversion on every frame.
    pub(super) fn prepare_guest_planes(
        &mut self,
        mut check: impl FnMut(usize) -> Result<(), EmuError>,
    ) -> Result<(), EmuError> {
        if let Self::Compact(source) = self {
            let mut planes: [Vec<HeapValue>; 4] = std::array::from_fn(|_| Vec::new());
            let mut work = 0;
            for (plane, source) in planes.iter_mut().zip(source) {
                check(work)?;
                plane
                    .try_reserve_exact(source.len())
                    .map_err(|_| vm_error("resource-limit", "host sample allocation failed"))?;
                for chunk in source.chunks(super::super::RASTER_CANCELLATION_INTERVAL) {
                    check(work)?;
                    plane.extend(chunk.iter().copied().map(HeapValue::Int));
                    work += chunk.len();
                }
            }
            *self = Self::Guest(planes);
        }
        Ok(())
    }

    pub(super) fn guest_plane_mut(
        &mut self,
        phase: usize,
    ) -> Result<&mut Vec<HeapValue>, EmuError> {
        let Self::Guest(planes) = self else {
            return Err(type_error());
        };
        planes.get_mut(phase).ok_or_else(type_error)
    }

    pub(super) fn composite_pixel(&mut self, index: usize, color: i32) -> Result<(), EmuError> {
        match self {
            Self::Compact(planes) => composite_pixel(planes, index, color),
            Self::Guest(planes) => composite_pixel(planes, index, color),
        }
    }

    pub(super) fn fill(&mut self, start: usize, end: usize, color: i32) -> Result<(), EmuError> {
        match self {
            Self::Compact(planes) => fill(planes, start, end, color),
            Self::Guest(planes) => fill(planes, start, end, color),
        }
    }

    pub(super) fn composite_row(&mut self, start: usize, colors: &[i32]) -> Result<(), EmuError> {
        match self {
            Self::Compact(planes) => composite_row(planes, start, colors),
            Self::Guest(planes) => composite_row(planes, start, colors),
        }
    }

    pub(super) fn copy_row(&mut self, start: usize, colors: &[i32]) -> Result<(), EmuError> {
        match self {
            Self::Compact(planes) => copy_row(planes, start, colors),
            Self::Guest(planes) => copy_row(planes, start, colors),
        }
    }

    pub(super) fn interleave(&self, width: u32, height: u32) -> Result<Vec<u32>, EmuError> {
        match self {
            Self::Compact(planes) => interleave(planes, width as usize, height as usize),
            Self::Guest(planes) => interleave(planes, width as usize, height as usize),
        }
    }

    pub(super) fn interleave_into(
        &self,
        width: u32,
        height: u32,
        pixels: &mut Vec<u32>,
    ) -> Result<(), EmuError> {
        match self {
            Self::Compact(planes) => {
                interleave_into(planes, width as usize, height as usize, pixels)
            }
            Self::Guest(planes) => interleave_into(planes, width as usize, height as usize, pixels),
        }
    }

    pub(super) fn matches(&self, width: u32, pixels: &[u32]) -> bool {
        match self {
            Self::Compact(planes) => matches(planes, width as usize, pixels),
            Self::Guest(planes) => matches(planes, width as usize, pixels),
        }
    }

    pub(super) fn store(
        &mut self,
        stride: usize,
        pixels: &[u32],
        clip: [u32; 4],
        opaque: bool,
        destination: Option<&mut [HeapValue]>,
    ) {
        match self {
            Self::Compact(planes) => store(planes, stride, pixels, clip, opaque, destination),
            Self::Guest(planes) => store(planes, stride, pixels, clip, opaque, destination),
        }
    }
}

trait SamplePixel: Copy {
    fn from_argb(pixel: u32) -> Self;
    fn argb(self) -> Result<u32, EmuError>;
    fn composite(&mut self, color: i32) -> Result<(), EmuError>;
}

impl SamplePixel for i32 {
    fn from_argb(pixel: u32) -> Self {
        pixel.cast_signed()
    }
    fn argb(self) -> Result<u32, EmuError> {
        Ok(self.cast_unsigned())
    }
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn composite(&mut self, color: i32) -> Result<(), EmuError> {
        if color.cast_unsigned() >> 24 == 255 {
            *self = color;
        } else {
            *self = crate::machine::pixel_ops::blend_argb(color, *self);
        }
        Ok(())
    }
}

impl SamplePixel for HeapValue {
    fn from_argb(pixel: u32) -> Self {
        Self::Int(pixel.cast_signed())
    }
    fn argb(self) -> Result<u32, EmuError> {
        let Self::Int(pixel) = self else {
            return Err(type_error());
        };
        Ok(pixel.cast_unsigned())
    }
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn composite(&mut self, color: i32) -> Result<(), EmuError> {
        super::super::composite_argb_slot(self, color)
    }
}

fn composite_pixel<T: SamplePixel>(
    planes: &mut [Vec<T>; 4],
    index: usize,
    color: i32,
) -> Result<(), EmuError> {
    for plane in planes {
        plane
            .get_mut(index)
            .ok_or_else(type_error)?
            .composite(color)?;
    }
    Ok(())
}

fn fill<T: SamplePixel>(
    planes: &mut [Vec<T>; 4],
    start: usize,
    end: usize,
    color: i32,
) -> Result<(), EmuError> {
    for plane in planes {
        let row = plane.get_mut(start..end).ok_or_else(type_error)?;
        if color.cast_unsigned() >> 24 == 255 {
            row.fill(T::from_argb(color.cast_unsigned()));
        } else {
            for slot in row {
                slot.composite(color)?;
            }
        }
    }
    Ok(())
}

fn composite_row<T: SamplePixel>(
    planes: &mut [Vec<T>; 4],
    start: usize,
    colors: &[i32],
) -> Result<(), EmuError> {
    let end = start.checked_add(colors.len()).ok_or_else(type_error)?;
    for plane in planes {
        let row = plane.get_mut(start..end).ok_or_else(type_error)?;
        for (pixel, color) in row.iter_mut().zip(colors) {
            pixel.composite(*color)?;
        }
    }
    Ok(())
}

fn copy_row<T: SamplePixel>(
    planes: &mut [Vec<T>; 4],
    start: usize,
    colors: &[i32],
) -> Result<(), EmuError> {
    let end = start.checked_add(colors.len()).ok_or_else(type_error)?;
    for plane in planes {
        let row = plane.get_mut(start..end).ok_or_else(type_error)?;
        if let [color] = colors {
            row[0] = T::from_argb(color.cast_unsigned());
        } else {
            for (pixel, color) in row.iter_mut().zip(colors) {
                *pixel = T::from_argb(color.cast_unsigned());
            }
        }
    }
    Ok(())
}

fn interleave<T: SamplePixel>(
    planes: &[Vec<T>; 4],
    width: usize,
    height: usize,
) -> Result<Vec<u32>, EmuError> {
    let mut pixels = vec![0; width * height * 4];
    interleave_rows(planes, width, &mut pixels)?;
    Ok(pixels)
}

fn interleave_into<T: SamplePixel>(
    planes: &[Vec<T>; 4],
    width: usize,
    height: usize,
    pixels: &mut Vec<u32>,
) -> Result<(), EmuError> {
    let count = width * height * 4;
    pixels
        .try_reserve_exact(count.saturating_sub(pixels.len()))
        .map_err(|_| vm_error("resource-limit", "host frame allocation failed"))?;
    pixels.resize(count, 0);
    interleave_rows(planes, width, pixels)
}

// Keep once-per-frame copies outside the larger presentation handler.
#[inline(never)]
fn interleave_rows<T: SamplePixel>(
    planes: &[Vec<T>; 4],
    width: usize,
    pixels: &mut [u32],
) -> Result<(), EmuError> {
    let [upper_left, upper_right, lower_left, lower_right] = planes;
    let rows = upper_left
        .chunks_exact(width)
        .zip(upper_right.chunks_exact(width))
        .zip(
            lower_left
                .chunks_exact(width)
                .zip(lower_right.chunks_exact(width)),
        );
    for (((upper_left, upper_right), (lower_left, lower_right)), target) in
        rows.zip(pixels.chunks_exact_mut(width * 4))
    {
        let (upper, lower) = target.split_at_mut(width * 2);
        // Finish one physical row before the next. Android release fixtures
        // favor two source streams at a time when presentation storage is reused.
        interleave_row(upper_left, upper_right, upper)?;
        interleave_row(lower_left, lower_right, lower)?;
    }
    Ok(())
}

#[inline]
fn interleave_row<T: SamplePixel>(
    left: &[T],
    right: &[T],
    pixels: &mut [u32],
) -> Result<(), EmuError> {
    for ((left, right), pixel) in left.iter().zip(right).zip(pixels.chunks_exact_mut(2)) {
        pixel.copy_from_slice(&[left.argb()?, right.argb()?]);
    }
    Ok(())
}

fn matches<T: SamplePixel>(planes: &[Vec<T>; 4], width: usize, pixels: &[u32]) -> bool {
    planes.iter().enumerate().all(|(phase, plane)| {
        plane.chunks_exact(width).enumerate().all(|(row, values)| {
            let start = (row * 2 + phase / 2) * width * 2;
            values
                .iter()
                .zip(pixels[start..start + width * 2].chunks_exact(2))
                .all(|(value, pair)| value.argb().is_ok_and(|pixel| pixel == pair[phase % 2]))
        })
    })
}

fn store<T: SamplePixel>(
    planes: &mut [Vec<T>; 4],
    stride: usize,
    pixels: &[u32],
    [x, y, width, height]: [u32; 4],
    opaque: bool,
    mut destination: Option<&mut [HeapValue]>,
) {
    let mask = if opaque { 0xff00_0000 } else { 0 };
    let [upper_left, upper_right, lower_left, lower_right] = planes;
    for row in y as usize..(y + height) as usize {
        let start = row * stride + x as usize;
        let end = start + width as usize;
        let upper = &pixels[row * 4 * stride + x as usize * 2..][..width as usize * 2];
        let lower = &pixels[(row * 4 + 2) * stride + x as usize * 2..][..width as usize * 2];
        for (offset, ((upper, lower), ((a, b), (c, d)))) in upper
            .chunks_exact(2)
            .zip(lower.chunks_exact(2))
            .zip(
                upper_left[start..end]
                    .iter_mut()
                    .zip(&mut upper_right[start..end])
                    .zip(
                        lower_left[start..end]
                            .iter_mut()
                            .zip(&mut lower_right[start..end]),
                    ),
            )
            .enumerate()
        {
            let values = [
                upper[0] | mask,
                upper[1] | mask,
                lower[0] | mask,
                lower[1] | mask,
            ];
            *a = T::from_argb(values[0]);
            *b = T::from_argb(values[1]);
            *c = T::from_argb(values[2]);
            *d = T::from_argb(values[3]);
            if let Some(destination) = &mut destination {
                let rb: u32 = values.iter().map(|v| v & 0x00ff_00ff).sum();
                let ga: u32 = values.iter().map(|v| (v >> 8) & 0x00ff_00ff).sum();
                let pixel = ((rb + 0x0002_0002) >> 2) & 0x00ff_00ff
                    | (((ga + 0x0002_0002) >> 2) & 0x00ff_00ff) << 8;
                destination[start + offset] = HeapValue::Int(pixel.cast_signed());
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/unit/vm/machine/sample_planes.rs"]
mod tests;
