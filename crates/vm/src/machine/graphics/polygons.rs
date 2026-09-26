//! MIDP triangles and Nokia polygon geometry, clipping and rasterization.

use super::{
    Allocation, ArrayKind, EmuError, Handle, HeapValue, Machine, ShapePainter, Value,
    check_raster_cancellation, composite_argb_slot, heap_error, int_argument, reference_argument,
    type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn graphics_nokia_polygon(
        &mut self,
        args: &[Value],
        fill: bool,
    ) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let x_points = reference_argument(args, 1)?;
        let x_offset = int_argument(args, 2)?;
        let y_points = reference_argument(args, 3)?;
        let y_offset = int_argument(args, 4)?;
        let count = int_argument(args, 5)?;
        let color = int_argument(args, 6)?;
        let xs = self.graphics_nokia_coordinate_slice(x_points, x_offset, count)?;
        let ys = self.graphics_nokia_coordinate_slice(y_points, y_offset, count)?;
        let translate_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
        let translate_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
        let mut points = Vec::with_capacity(xs.len());
        for (index, (x, y)) in xs.iter().zip(ys).enumerate() {
            check_raster_cancellation(self.native_context, index)?;
            let (HeapValue::Int(x), HeapValue::Int(y)) = (x, y) else {
                return Err(type_error());
            };
            points.push((x.saturating_add(translate_x), y.saturating_add(translate_y)));
        }
        self.graphics_render_nokia_polygon(graphics, &points, color, fill)
    }

    pub(in crate::machine) fn graphics_nokia_triangle(
        &mut self,
        args: &[Value],
        fill: bool,
    ) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        self.graphics_triangle(graphics, args, int_argument(args, 7)?, fill)
    }

    pub(in crate::machine) fn graphics_fill_triangle(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let color =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.color:I")?;
        if color.cast_unsigned() >> 24 == 0 {
            return Ok(());
        }
        let translate_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
        let translate_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
        let points = [
            (
                int_argument(args, 1)?.saturating_add(translate_x),
                int_argument(args, 2)?.saturating_add(translate_y),
            ),
            (
                int_argument(args, 3)?.saturating_add(translate_x),
                int_argument(args, 4)?.saturating_add(translate_y),
            ),
            (
                int_argument(args, 5)?.saturating_add(translate_x),
                int_argument(args, 6)?.saturating_add(translate_y),
            ),
        ];

        let clip_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipX:I")?;
        let clip_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipY:I")?;
        let clip_w =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipW:I")?;
        let clip_h =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipH:I")?;
        let (target, target_width, target_height) = self.graphics_target(graphics)?;
        let clip_left = clip_x.max(0).min(target_width);
        let clip_top = clip_y.max(0).min(target_height);
        let clip_right = clip_x.saturating_add(clip_w).max(0).min(target_width);
        let clip_bottom = clip_y.saturating_add(clip_h).max(0).min(target_height);
        if clip_left >= clip_right || clip_top >= clip_bottom {
            return Ok(());
        }
        let clip = graphics::Rect {
            x: clip_left,
            y: clip_top,
            width: clip_right - clip_left,
            height: clip_bottom - clip_top,
        };
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements: pixels,
        } = self.heap.managed.get_mut(target).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        for (work, (y, xs)) in graphics::clipped_triangle_rows(points, clip).enumerate() {
            if xs.is_empty() {
                check_raster_cancellation(self.native_context, work)?;
                continue;
            }
            // The clip is inside the validated target, so all coordinates are
            // nonnegative. Use u64 for offsets before checking the actual array.
            let row = y as u64 * target_width as u64;
            let span = usize::try_from(row + *xs.start() as u64)
                .ok()
                .zip(usize::try_from(row + *xs.end() as u64).ok())
                .and_then(|(first, last)| pixels.get_mut(first..=last))
                .ok_or_else(|| {
                    vm_error(
                        "array-index-out-of-bounds-exception",
                        "triangle span is outside the pixel buffer",
                    )
                })?;
            for chunk in span.chunks_mut(1_024) {
                check_raster_cancellation(self.native_context, 0)?;
                if color.cast_unsigned() >> 24 == 255 {
                    chunk.fill(HeapValue::Int(color));
                } else {
                    for pixel in chunk {
                        composite_argb_slot(pixel, color)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(in crate::machine) fn graphics_triangle(
        &mut self,
        graphics: Handle,
        args: &[Value],
        color: i32,
        fill: bool,
    ) -> Result<(), EmuError> {
        let translate_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
        let translate_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
        let points = [
            (
                int_argument(args, 1)?.saturating_add(translate_x),
                int_argument(args, 2)?.saturating_add(translate_y),
            ),
            (
                int_argument(args, 3)?.saturating_add(translate_x),
                int_argument(args, 4)?.saturating_add(translate_y),
            ),
            (
                int_argument(args, 5)?.saturating_add(translate_x),
                int_argument(args, 6)?.saturating_add(translate_y),
            ),
        ];
        self.graphics_render_nokia_polygon(graphics, &points, color, fill)
    }

    pub(in crate::machine) fn graphics_nokia_coordinate_slice(
        &self,
        array: Handle,
        offset: i32,
        count: i32,
    ) -> Result<&[HeapValue], EmuError> {
        let start = usize::try_from(offset).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative DirectGraphics coordinate offset",
            )
        })?;
        let count = usize::try_from(count).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative DirectGraphics point count",
            )
        })?;
        let end = start.checked_add(count).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "DirectGraphics coordinate range overflow",
            )
        })?;
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let coordinates = elements.get(start..end).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "DirectGraphics coordinate range is outside the array",
            )
        })?;
        for (index, value) in coordinates.iter().enumerate() {
            check_raster_cancellation(self.native_context, index)?;
            if !matches!(value, HeapValue::Int(_)) {
                return Err(type_error());
            }
        }
        Ok(coordinates)
    }

    pub(in crate::machine) fn graphics_render_nokia_polygon(
        &mut self,
        graphics: Handle,
        points: &[(i32, i32)],
        color: i32,
        fill: bool,
    ) -> Result<(), EmuError> {
        if points.is_empty() || color.cast_unsigned() >> 24 == 0 {
            return Ok(());
        }
        let clip_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipX:I")?;
        let clip_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipY:I")?;
        let clip_w =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipW:I")?;
        let clip_h =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipH:I")?;
        let (target, target_width, target_height) = self.graphics_target(graphics)?;
        let (min_x, min_y, max_x, max_y) = points.iter().enumerate().try_fold(
            (i32::MAX, i32::MAX, i32::MIN, i32::MIN),
            |(min_x, min_y, max_x, max_y), (index, &(x, y))| {
                check_raster_cancellation(self.native_context, index)?;
                Ok::<_, EmuError>((min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y)))
            },
        )?;
        let clip_left = clip_x.max(0).min(target_width).max(min_x);
        let clip_top = clip_y.max(0).min(target_height).max(min_y);
        let clip_right = clip_x
            .saturating_add(clip_w)
            .max(0)
            .min(target_width)
            .min(max_x.saturating_add(1));
        let clip_bottom = clip_y
            .saturating_add(clip_h)
            .max(0)
            .min(target_height)
            .min(max_y.saturating_add(1));
        if clip_left >= clip_right || clip_top >= clip_bottom {
            return Ok(());
        }

        let Allocation::Array {
            kind: ArrayKind::Int,
            elements: pixels,
        } = self.heap.managed.get_mut(target).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let check_work = |work| check_raster_cancellation(self.native_context, work);
        let mut painter = ShapePainter::new(
            pixels,
            target_width,
            (clip_left, clip_top, clip_right, clip_bottom),
            color,
        )?;
        if fill && points.len() >= 3 {
            let mut intersections = Vec::with_capacity(points.len());
            for y in clip_top..clip_bottom {
                intersections.clear();
                for index in 0..points.len() {
                    check_work(index)?;
                    let (x1, y1) = points[index];
                    let (x2, y2) = points[(index + 1) % points.len()];
                    if (y1 <= y && y2 > y) || (y2 <= y && y1 > y) {
                        let numerator =
                            (i128::from(y) - i128::from(y1)) * (i128::from(x2) - i128::from(x1));
                        let x = i128::from(x1) + numerator / (i128::from(y2) - i128::from(y1));
                        intersections.push(i32::try_from(x).unwrap_or(if x < 0 {
                            i32::MIN
                        } else {
                            i32::MAX
                        }));
                    }
                }
                intersections.sort_unstable();
                for (index, pair) in intersections.chunks_exact(2).enumerate() {
                    check_work(index)?;
                    let start = pair[0].max(clip_left);
                    let end = pair[1].min(clip_right.saturating_sub(1));
                    painter.span(y, start..=end, check_work)?;
                }
            }
        }

        if points.len() == 1 {
            let (x, y) = points[0];
            if x >= clip_left && x < clip_right && y >= clip_top && y < clip_bottom {
                painter.point(x, y)?;
            }
            return Ok(());
        }
        for index in 0..points.len() {
            check_work(index)?;
            painter.line(
                points[index],
                points[(index + 1) % points.len()],
                false,
                check_work,
            )?;
        }
        Ok(())
    }
}
