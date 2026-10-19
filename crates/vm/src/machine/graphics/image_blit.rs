//! Checked row compositing shared by image draws and signed-stride RGB transfers.

use super::enhanced::SampleSurface;

use super::{
    EmuError, HeapValue, ImmutableImagePixels, RASTER_CANCELLATION_INTERVAL,
    check_raster_cancellation, composite_argb_slot, raster_work_chunks, vm_error,
};

#[derive(Clone, Copy)]
pub(super) struct ImageBlitRegion {
    pub copy_width: usize,
    pub rows: i32,
    pub source_offset: i64,
    pub source_stride: i32,
    pub target_width: i32,
    pub target_x: i32,
    pub target_y: i32,
}

trait SampleWriter {
    fn begin(&mut self);
    fn push(&mut self, color: i32);
    fn finish(&mut self, start: usize) -> Result<(), EmuError>;
}

// The single-surface raster carries no sample pointers or scratch storage.
impl SampleWriter for () {
    fn begin(&mut self) {}
    fn push(&mut self, _: i32) {}
    fn finish(&mut self, _: usize) -> Result<(), EmuError> {
        Ok(())
    }
}

struct RowSamples<'a, const OPAQUE: bool> {
    surface: &'a mut SampleSurface,
    colors: Vec<i32>,
}

impl<'a, const OPAQUE: bool> RowSamples<'a, OPAQUE> {
    fn new(surface: &'a mut SampleSurface, width: usize) -> Result<Self, EmuError> {
        let colors = std::mem::take(&mut surface.row_scratch);
        let mut writer = Self { surface, colors };
        writer.colors.clear();
        writer
            .colors
            .try_reserve_exact(width.min(RASTER_CANCELLATION_INTERVAL))
            .map_err(|_| vm_error("resource-limit", "image sample scratch allocation failed"))?;
        Ok(writer)
    }
}

impl<const OPAQUE: bool> Drop for RowSamples<'_, OPAQUE> {
    fn drop(&mut self) {
        self.colors.clear();
        std::mem::swap(&mut self.surface.row_scratch, &mut self.colors);
    }
}

impl<const OPAQUE: bool> SampleWriter for RowSamples<'_, OPAQUE> {
    fn begin(&mut self) {
        self.colors.clear();
    }
    fn push(&mut self, color: i32) {
        self.colors.push(color);
    }
    fn finish(&mut self, start: usize) -> Result<(), EmuError> {
        if OPAQUE {
            self.surface.copy_row(start, &self.colors)
        } else {
            self.surface.composite_row(start, &self.colors)
        }
    }
}

#[inline]
fn composite_transformed_rows<T, S: SampleWriter>(
    host: &dyn natives::HostServices,
    source_pixels: &[T],
    mut source_pixel: impl FnMut(&T) -> Result<i32, EmuError>,
    target_pixels: &mut [HeapValue],
    region: ImageBlitRegion,
    source_step: i32,
    mut samples: S,
) -> Result<(), EmuError> {
    if region.copy_width == 0 {
        return Ok(());
    }
    let step = source_step.unsigned_abs() as usize;
    let source_span = (region.copy_width - 1)
        .checked_mul(step)
        .filter(|_| step != 0)
        .ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "image blit source step is invalid",
            )
        })?;
    for row in 0..region.rows {
        let source_start = region
            .source_offset
            .checked_add(i64::from(row) * i64::from(region.source_stride))
            .and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "image blit source index overflow",
                )
            })?;
        let source_start = if source_step < 0 {
            source_start.checked_sub(source_span)
        } else {
            Some(source_start)
        }
        .ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "image blit source range overflow",
            )
        })?;
        let source_end = source_start
            .checked_add(source_span)
            .and_then(|end| end.checked_add(1))
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "image blit source range overflow",
                )
            })?;
        let target_start = region
            .target_y
            .checked_add(row)
            .and_then(|row| row.checked_mul(region.target_width))
            .and_then(|base| base.checked_add(region.target_x))
            .and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "image blit target index overflow",
                )
            })?;
        let target_end = target_start.checked_add(region.copy_width).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "image blit target range overflow",
            )
        })?;
        let source_row = source_pixels.get(source_start..source_end).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "image blit source row is out of bounds",
            )
        })?;
        let target_row = target_pixels
            .get_mut(target_start..target_end)
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "image blit target row is out of bounds",
                )
            })?;
        let row_work = row as usize * region.copy_width;
        if source_step == 1 {
            composite_row(
                host,
                row_work,
                source_row.iter(),
                target_row,
                &mut source_pixel,
                target_start,
                &mut samples,
            )?;
        } else if source_step == -1 {
            composite_row(
                host,
                row_work,
                source_row.iter().rev(),
                target_row,
                &mut source_pixel,
                target_start,
                &mut samples,
            )?;
        } else if source_step < 0 {
            composite_row(
                host,
                row_work,
                source_row.iter().rev().step_by(step),
                target_row,
                &mut source_pixel,
                target_start,
                &mut samples,
            )?;
        } else {
            composite_row(
                host,
                row_work,
                source_row.iter().step_by(step),
                target_row,
                &mut source_pixel,
                target_start,
                &mut samples,
            )?;
        }
    }
    Ok(())
}

#[inline]
fn composite_row<'a, T: 'a, S: SampleWriter>(
    host: &dyn natives::HostServices,
    row_work: usize,
    mut source: impl Iterator<Item = &'a T>,
    target: &mut [HeapValue],
    mut source_pixel: impl FnMut(&T) -> Result<i32, EmuError>,
    target_start: usize,
    samples: &mut S,
) -> Result<(), EmuError> {
    if target.len() <= RASTER_CANCELLATION_INTERVAL - row_work % RASTER_CANCELLATION_INTERVAL {
        check_raster_cancellation(host, row_work)?;
        samples.begin();
        let result = (|| {
            for (pixel, slot) in source.zip(target) {
                let color = source_pixel(pixel)?;
                composite_argb_slot(slot, color)?;
                samples.push(color);
            }
            Ok(())
        })();
        let flushed = samples.finish(target_start);
        return result.and(flushed);
    }
    for chunk in raster_work_chunks(row_work..row_work + target.len()) {
        check_raster_cancellation(host, chunk.start)?;
        let target_chunk = &mut target[chunk.start - row_work..chunk.end - row_work];
        // Advance the bounded destination first so ending a chunk cannot
        // consume an extra source pixel from the next one.
        samples.begin();
        let result = (|| {
            for (slot, pixel) in target_chunk.iter_mut().zip(source.by_ref()) {
                let color = source_pixel(pixel)?;
                composite_argb_slot(slot, color)?;
                samples.push(color);
            }
            Ok(())
        })();
        let flushed = samples.finish(target_start + chunk.start - row_work);
        result.and(flushed)?;
    }
    Ok(())
}

#[inline]
pub(super) fn composite_transformed_image(
    host: &dyn natives::HostServices,
    source: &ImmutableImagePixels,
    target: &mut [HeapValue],
    region: ImageBlitRegion,
    source_step: i32,
    samples: Option<&mut SampleSurface>,
) -> Result<(), EmuError> {
    if let Some(samples) = samples {
        composite_transformed_image_inner(
            host,
            source,
            target,
            region,
            source_step,
            RowSamples::<false>::new(samples, region.copy_width)?,
        )
    } else {
        composite_transformed_image_inner(host, source, target, region, source_step, ())
    }
}

#[inline]
fn composite_transformed_image_inner<S: SampleWriter>(
    host: &dyn natives::HostServices,
    source: &ImmutableImagePixels,
    target: &mut [HeapValue],
    region: ImageBlitRegion,
    source_step: i32,
    samples: S,
) -> Result<(), EmuError> {
    match source {
        ImmutableImagePixels::Argb(pixels) => composite_transformed_rows(
            host,
            pixels,
            |pixel| Ok(*pixel),
            target,
            region,
            source_step,
            samples,
        ),
        ImmutableImagePixels::Indexed8 { palette, indices } => composite_transformed_rows(
            host,
            indices,
            |index| {
                palette.get(usize::from(*index)).copied().ok_or_else(|| {
                    vm_error(
                        "array-index-out-of-bounds-exception",
                        "image blit palette index is out of bounds",
                    )
                })
            },
            target,
            region,
            source_step,
            samples,
        ),
    }
}

// Keep contiguous copies separate so their pixel loops carry no transform state.
#[inline]
pub(super) fn composite_image_rows<T>(
    host: &dyn natives::HostServices,
    source_pixels: &[T],
    source_pixel: impl FnMut(&T) -> Result<i32, EmuError>,
    target_pixels: &mut [HeapValue],
    region: ImageBlitRegion,
    samples: Option<&mut SampleSurface>,
) -> Result<(), EmuError> {
    composite_image_rows_mode::<_, false>(
        host,
        source_pixels,
        source_pixel,
        target_pixels,
        region,
        samples,
    )
}

#[inline]
fn composite_image_rows_mode<T, const OPAQUE: bool>(
    host: &dyn natives::HostServices,
    source_pixels: &[T],
    source_pixel: impl FnMut(&T) -> Result<i32, EmuError>,
    target_pixels: &mut [HeapValue],
    region: ImageBlitRegion,
    samples: Option<&mut SampleSurface>,
) -> Result<(), EmuError> {
    if let Some(samples) = samples {
        composite_image_rows_inner(
            host,
            source_pixels,
            source_pixel,
            target_pixels,
            region,
            RowSamples::<OPAQUE>::new(samples, region.copy_width)?,
        )
    } else {
        composite_image_rows_inner(host, source_pixels, source_pixel, target_pixels, region, ())
    }
}

#[inline]
fn composite_image_rows_inner<T, S: SampleWriter>(
    host: &dyn natives::HostServices,
    source_pixels: &[T],
    mut source_pixel: impl FnMut(&T) -> Result<i32, EmuError>,
    target_pixels: &mut [HeapValue],
    region: ImageBlitRegion,
    mut samples: S,
) -> Result<(), EmuError> {
    for row in 0..region.rows {
        let source_start = region
            .source_offset
            .checked_add(i64::from(row) * i64::from(region.source_stride))
            .and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "image blit source index overflow",
                )
            })?;
        let source_end = source_start.checked_add(region.copy_width).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "image blit source range overflow",
            )
        })?;
        let target_start = region
            .target_y
            .checked_add(row)
            .and_then(|row| row.checked_mul(region.target_width))
            .and_then(|base| base.checked_add(region.target_x))
            .and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "image blit target index overflow",
                )
            })?;
        let target_end = target_start.checked_add(region.copy_width).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "image blit target range overflow",
            )
        })?;
        let source_row = source_pixels.get(source_start..source_end).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "image blit source row is out of bounds",
            )
        })?;
        let target_row = target_pixels
            .get_mut(target_start..target_end)
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "image blit target row is out of bounds",
                )
            })?;
        let row_work = row as usize * region.copy_width;
        composite_row(
            host,
            row_work,
            source_row.iter(),
            target_row,
            &mut source_pixel,
            target_start,
            &mut samples,
        )?;
    }
    Ok(())
}

#[inline]
pub(super) fn composite_rgb_rows<T>(
    host: &dyn natives::HostServices,
    source: &[T],
    mut source_pixel: impl FnMut(&T) -> Result<i32, EmuError>,
    target: &mut [HeapValue],
    region: ImageBlitRegion,
    process_alpha: bool,
    samples: Option<&mut SampleSurface>,
) -> Result<(), EmuError> {
    // Resolve the drawRGB mode once so opaque copies do not test alpha per pixel.
    if process_alpha {
        composite_image_rows(host, source, source_pixel, target, region, samples)
    } else {
        composite_image_rows_mode::<_, true>(
            host,
            source,
            |pixel| Ok(source_pixel(pixel)? | 0xff00_0000_u32.cast_signed()),
            target,
            region,
            samples,
        )
    }
}
