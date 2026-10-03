//! Nokia `DirectGraphics` pixel transfer and format conversion.

use super::{
    Allocation, ArrayKind, EmuError, HeapValue, Machine, Value, argb_to_nokia_pixel,
    check_raster_cancellation, heap_error, int_argument, nokia_direct_transform,
    nokia_pixel_to_argb, paint_argb_pixel, raster_work_chunks, reference_argument, type_error,
    vm_error,
};

struct DirectPixelSource<'a> {
    elements: &'a [HeapValue],
    kind: &'a ArrayKind,
    offset: i64,
    scanlength: i64,
    width: i32,
    height: i32,
    transform: i32,
    format: i32,
    transparency: bool,
}

impl<'a> DirectPixelSource<'a> {
    fn new(allocation: &'a Allocation, args: &[Value]) -> Result<Self, EmuError> {
        let Allocation::Array { kind, elements } = allocation else {
            return Err(type_error());
        };
        let offset = i64::from(int_argument(args, 3)?);
        let scanlength = i64::from(int_argument(args, 4)?);
        let width = int_argument(args, 7)?;
        let height = int_argument(args, 8)?;
        let transform = nokia_direct_transform(int_argument(args, 9)?)
            .ok_or_else(|| vm_error("illegal-argument", "unsupported DirectGraphics transform"))?;
        let format = int_argument(args, 10)?;
        let transparency = int_argument(args, 2)? != 0;
        nokia_pixel_to_argb(0, kind, format, transparency)?;

        // The caller checked positive dimensions. Validate the entire source
        // range before clipping or allocating, including negative row strides.
        let last_row = offset + i64::from(height - 1) * scanlength;
        let low = offset.min(last_row);
        let high = offset.max(last_row) + i64::from(width);
        if low < 0 || usize::try_from(high).map_or(true, |high| high > elements.len()) {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "DirectGraphics source range is outside the array",
            ));
        }
        Ok(Self {
            elements,
            kind,
            offset,
            scanlength,
            width,
            height,
            transform,
            format,
            transparency,
        })
    }

    fn dimensions(&self) -> (i32, i32) {
        if self.transform >= 4 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }

    fn pixel(&self, x: i32, y: i32) -> Result<i32, EmuError> {
        let (source_x, source_y) = match self.transform {
            0 => (x, y),
            1 => (x, self.height - 1 - y),
            2 => (self.width - 1 - x, y),
            3 => (self.width - 1 - x, self.height - 1 - y),
            4 => (y, x),
            5 => (y, self.height - 1 - x),
            6 => (self.width - 1 - y, x),
            7 => (self.width - 1 - y, self.height - 1 - x),
            _ => unreachable!("DirectGraphics transform was validated"),
        };
        let index = self.offset + i64::from(source_y) * self.scanlength + i64::from(source_x);
        let Some(HeapValue::Int(value)) = usize::try_from(index)
            .ok()
            .and_then(|index| self.elements.get(index))
        else {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "DirectGraphics pixel index is outside the array",
            ));
        };
        nokia_pixel_to_argb(*value, self.kind, self.format, self.transparency)
    }
}

impl Machine<'_, '_> {
    pub(in crate::machine) fn graphics_nokia_draw_pixels(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let source = reference_argument(args, 1)?;
        let x = int_argument(args, 5)?;
        let y = int_argument(args, 6)?;
        let width = int_argument(args, 7)?;
        let height = int_argument(args, 8)?;
        if width < 0 || height < 0 {
            return Err(vm_error(
                "illegal-argument",
                "negative DirectGraphics pixel dimensions",
            ));
        }
        if width == 0 || height == 0 {
            return Ok(());
        }
        let source =
            DirectPixelSource::new(self.heap.managed.get(source).map_err(heap_error)?, args)?;
        let (width, height) = source.dimensions();
        let translate_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
        let translate_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
        let origin_x = x.saturating_add(translate_x);
        let origin_y = y.saturating_add(translate_y);
        let clip_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipX:I")?;
        let clip_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipY:I")?;
        let clip_w =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipW:I")?;
        let clip_h =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipH:I")?;
        let (target, target_width, target_height) = self.graphics_target(graphics)?;
        let left = origin_x.max(clip_x).max(0);
        let top = origin_y.max(clip_y).max(0);
        let right = origin_x
            .saturating_add(width)
            .min(clip_x.saturating_add(clip_w))
            .min(target_width);
        let bottom = origin_y
            .saturating_add(height)
            .min(clip_y.saturating_add(clip_h))
            .min(target_height);
        if left >= right || top >= bottom {
            return Ok(());
        }

        // Snapshot only the visible region, including when rows repeat with
        // scanlength zero. The target allocation bounds temporary storage,
        // and taking the snapshot first also preserves source/target aliasing.
        let target_length = self.heap.managed.array_length(target).map_err(heap_error)?;
        let count = usize::try_from(i64::from(right - left) * i64::from(bottom - top))
            .ok()
            .filter(|count| *count <= target_length)
            .ok_or_else(|| {
                vm_error(
                    "out-of-memory-error",
                    "invalid DirectGraphics target dimensions",
                )
            })?;
        let mut snapshot = Vec::new();
        snapshot.try_reserve_exact(count).map_err(|_| {
            vm_error(
                "out-of-memory-error",
                "DirectGraphics pixel snapshot allocation failed",
            )
        })?;
        for target_y in top..bottom {
            let row_work = snapshot.len();
            for chunk in raster_work_chunks(row_work..row_work + (right - left) as usize) {
                check_raster_cancellation(self.native_context, chunk.start)?;
                for index in chunk {
                    let target_x = left + (index - row_work) as i32;
                    snapshot.push(source.pixel(target_x - origin_x, target_y - origin_y)?);
                }
            }
        }
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements: target_pixels,
        } = self.heap.managed.get_mut(target).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        for (target_y, row) in (top..bottom).zip(snapshot.chunks_exact((right - left) as usize)) {
            let row_work = (target_y - top) as usize * row.len();
            for chunk in raster_work_chunks(row_work..row_work + row.len()) {
                check_raster_cancellation(self.native_context, chunk.start)?;
                let start = chunk.start - row_work;
                for (column, &pixel) in (start..).zip(&row[start..chunk.end - row_work]) {
                    let target_x = left + column as i32;
                    paint_argb_pixel(target_pixels, target_width, target_x, target_y, pixel)?;
                }
            }
        }
        Ok(())
    }

    pub(in crate::machine) fn graphics_nokia_get_pixels(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let destination = reference_argument(args, 1)?;
        let offset = int_argument(args, 2)?;
        let scanlength = int_argument(args, 3)?;
        let x = int_argument(args, 4)?;
        let y = int_argument(args, 5)?;
        let width = int_argument(args, 6)?;
        let height = int_argument(args, 7)?;
        let format = int_argument(args, 8)?;
        if width < 0 || height < 0 {
            return Err(vm_error(
                "illegal-argument",
                "negative DirectGraphics pixel dimensions",
            ));
        }
        let translate_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
        let translate_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
        let source_x = x.saturating_add(translate_x);
        let source_y = y.saturating_add(translate_y);
        let (source, source_width, source_height) = self.graphics_target(graphics)?;
        if source_x < 0
            || source_y < 0
            || source_x.saturating_add(width) > source_width
            || source_y.saturating_add(height) > source_height
        {
            return Err(vm_error(
                "illegal-argument",
                "DirectGraphics source region is outside the graphics target",
            ));
        }
        // Preserve aliases while copying only the requested rectangle.
        let source_pixels = self.graphics_int_array_region_snapshot(
            source,
            source_width,
            source_x,
            source_y,
            width,
            height,
        )?;
        let Allocation::Array {
            kind: destination_kind,
            elements,
        } = self.heap.managed.get_mut(destination).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if width == 0 || height == 0 {
            return Ok(());
        }
        for (row, source_row) in (0..height).zip(source_pixels.chunks_exact(width as usize)) {
            let row_work = row as usize * source_row.len();
            for chunk in raster_work_chunks(row_work..row_work + source_row.len()) {
                check_raster_cancellation(self.native_context, chunk.start)?;
                let start = chunk.start - row_work;
                for (column, &pixel) in (start..).zip(&source_row[start..chunk.end - row_work]) {
                    let destination_index = offset
                        .checked_add(row.checked_mul(scanlength).ok_or_else(|| {
                            vm_error(
                                "array-index-out-of-bounds-exception",
                                "DirectGraphics destination row overflow",
                            )
                        })?)
                        .and_then(|base| base.checked_add(column as i32))
                        .and_then(|index| usize::try_from(index).ok())
                        .ok_or_else(|| {
                            vm_error(
                                "array-index-out-of-bounds-exception",
                                "DirectGraphics destination index overflow",
                            )
                        })?;
                    let value = argb_to_nokia_pixel(pixel, destination_kind, format)?;
                    let slot = elements.get_mut(destination_index).ok_or_else(|| {
                        vm_error(
                            "array-index-out-of-bounds-exception",
                            "DirectGraphics destination index is outside the array",
                        )
                    })?;
                    *slot = HeapValue::Int(value);
                }
            }
        }
        Ok(())
    }
}
