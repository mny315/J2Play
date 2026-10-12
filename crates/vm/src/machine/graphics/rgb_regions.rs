use super::image_blit::{ImageBlitRegion, composite_rgb_rows, composite_transformed_image};
use super::pixel_access::int_array_mut;
use super::{
    Allocation, Arc, ArrayKind, EmuError, HeapValue, ImmutableImagePixels, Machine, Value,
    heap_error, int_argument, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn graphics_draw_rgb(&mut self, args: &[Value]) -> Result<(), EmuError> {
        self.execution.counters.draw_rgb_calls =
            self.execution.counters.draw_rgb_calls.saturating_add(1);
        let graphics = reference_argument(args, 0)?;
        let source = reference_argument(args, 1)?;
        let offset = int_argument(args, 2)?;
        let scanlength = int_argument(args, 3)?;
        let origin_x = int_argument(args, 4)?;
        let origin_y = int_argument(args, 5)?;
        let width = int_argument(args, 6)?;
        let height = int_argument(args, 7)?;
        let process_alpha = int_argument(args, 8)? != 0;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get(source).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        // Check the complete source window before clipping or copying. Java
        // int inputs fit this arithmetic in i64, including negative stride.
        let last_row = i64::from(offset) + i64::from(height - 1) * i64::from(scanlength);
        let low = i64::from(offset).min(last_row);
        let high = i64::from(offset).max(last_row) + i64::from(width);
        if low < 0 || usize::try_from(high).map_or(true, |end| end > elements.len()) {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "drawRGB source range is out of bounds",
            ));
        }

        let (target, target_width, _) = self.graphics_target(graphics)?;
        let clip_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipX:I")?;
        let clip_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipY:I")?;
        let clip_w =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipW:I")?;
        let clip_h =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipH:I")?;

        let left = origin_x.max(clip_x);
        let top = origin_y.max(clip_y);
        let right = origin_x
            .saturating_add(width)
            .min(clip_x.saturating_add(clip_w));
        let bottom = origin_y
            .saturating_add(height)
            .min(clip_y.saturating_add(clip_h));
        if right <= left || bottom <= top {
            return Ok(());
        }
        let copy_width = usize::try_from(right - left).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "drawRGB clipped width overflow",
            )
        })?;
        let source_x = left - origin_x;
        let source_y = top - origin_y;

        let region = ImageBlitRegion {
            copy_width,
            rows: bottom - top,
            source_offset: i64::from(offset)
                + i64::from(source_y) * i64::from(scanlength)
                + i64::from(source_x),
            source_stride: scanlength,
            target_width,
            target_x: left,
            target_y: top,
        };

        // Distinct allocations can be borrowed together without a source snapshot.
        if source != target {
            let (source_allocation, target_allocation) = self
                .heap
                .managed
                .get_pair_mut(source, target)
                .map_err(heap_error)?;
            let Allocation::Array {
                kind: ArrayKind::Int,
                elements: source_pixels,
            } = source_allocation
            else {
                return Err(type_error());
            };
            let Allocation::Array {
                kind: ArrayKind::Int,
                elements: target_pixels,
            } = target_allocation
            else {
                return Err(type_error());
            };

            return composite_rgb_rows(
                self.native_context,
                source_pixels,
                |pixel| match pixel {
                    HeapValue::Int(pixel) => Ok(*pixel),
                    _ => Err(type_error()),
                },
                target_pixels,
                region,
                process_alpha,
            );
        }

        // Internal callers can alias the target. Snapshot only visible pixels:
        // repeated source rows must not amplify a tiny array without a bound.
        self.graphics_int_array_strided_snapshot_into_scratch(
            source,
            region.source_offset as i32,
            scanlength,
            right - left,
            bottom - top,
        )?;
        let source_pixels = std::mem::take(&mut self.graphics.draw_rgb_scratch);
        let result = (|| -> Result<(), EmuError> {
            let target_pixels = int_array_mut(&mut self.heap.managed, target)?;
            composite_rgb_rows(
                self.native_context,
                &source_pixels,
                |pixel| Ok(*pixel),
                target_pixels,
                ImageBlitRegion {
                    source_offset: 0,
                    source_stride: copy_width as i32,
                    ..region
                },
                process_alpha,
            )
        })();
        self.graphics.draw_rgb_scratch = source_pixels;
        result
    }

    pub(in crate::machine) fn graphics_draw_region(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        self.execution.counters.draw_region_calls =
            self.execution.counters.draw_region_calls.saturating_add(1);
        let graphics = reference_argument(args, 0)?;
        let image = reference_argument(args, 1)?;
        let sx = int_argument(args, 2)?;
        let sy = int_argument(args, 3)?;
        let width = int_argument(args, 4)?;
        let height = int_argument(args, 5)?;
        let transform = int_argument(args, 6)?;
        let origin_x = int_argument(args, 7)?;
        let origin_y = int_argument(args, 8)?;
        let draw_width = int_argument(args, 9)?;
        let draw_height = int_argument(args, 10)?;
        let source_width =
            self.graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?;
        let source =
            self.graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")?;
        let (target, target_width, _) = self.graphics_target(graphics)?;
        let clip_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipX:I")?;
        let clip_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipY:I")?;
        let clip_w =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipW:I")?;
        let clip_h =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipH:I")?;
        let left = origin_x.max(clip_x);
        let top = origin_y.max(clip_y);
        let right = origin_x
            .saturating_add(draw_width)
            .min(clip_x.saturating_add(clip_w));
        let bottom = origin_y
            .saturating_add(draw_height)
            .min(clip_y.saturating_add(clip_h));
        if right <= left || bottom <= top {
            return Ok(());
        }
        // After clipping, reuse immutable pixels or snapshot the mutable region.
        let (source_pixels, source_stride, source_origin_x, source_origin_y) =
            if let Some(cached) = self.graphics_cached_image_pixels(image, source, args)? {
                (cached, source_width, sx, sy)
            } else {
                (
                    Arc::new(ImmutableImagePixels::Argb(
                        self.graphics_int_array_region_snapshot(
                            source,
                            source_width,
                            sx,
                            sy,
                            width,
                            height,
                        )?
                        .into_boxed_slice(),
                    )),
                    width,
                    0,
                    0,
                )
            };
        // A transform changes only the first source pixel and the signed steps
        // between columns and rows. Resolve it once, outside the pixel loops.
        let dx = i64::from(left) - i64::from(origin_x);
        let dy = i64::from(top) - i64::from(origin_y);
        let last_x = i64::from(width) - 1;
        let last_y = i64::from(height) - 1;
        if source_stride <= 0 {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "drawRegion source width is invalid",
            ));
        }
        let (rx, ry, source_step, row_stride) = match transform {
            0 => (dx, dy, 1, source_stride),
            1 => (dx, last_y - dy, 1, -source_stride),
            2 => (last_x - dx, dy, -1, source_stride),
            3 => (last_x - dx, last_y - dy, -1, -source_stride),
            4 => (dy, dx, source_stride, 1),
            5 => (dy, last_y - dx, -source_stride, 1),
            6 => (last_x - dy, dx, source_stride, -1),
            _ => (last_x - dy, last_y - dx, -source_stride, -1),
        };
        let region = ImageBlitRegion {
            copy_width: (right - left) as usize,
            rows: bottom - top,
            source_offset: (i64::from(source_origin_y) + ry)
                .checked_mul(i64::from(source_stride))
                .and_then(|base| base.checked_add(i64::from(source_origin_x) + rx))
                .ok_or_else(|| {
                    vm_error(
                        "array-index-out-of-bounds-exception",
                        "drawRegion source index overflow",
                    )
                })?,
            source_stride: row_stride,
            target_width,
            target_x: left,
            target_y: top,
        };
        let target_pixels = int_array_mut(&mut self.heap.managed, target)?;
        composite_transformed_image(
            self.native_context,
            &source_pixels,
            target_pixels,
            region,
            source_step,
        )
    }
}
