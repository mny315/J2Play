use super::image_blit::{ImageBlitRegion, composite_image_rows};
use super::pixel_access::int_array_mut;
use super::{
    Allocation, Arc, ArrayKind, EmuError, Handle, HeapValue, ImmutableImagePixels, Machine,
    RASTER_CANCELLATION_INTERVAL, ShapePainter, Value, check_raster_cancellation,
    composite_argb_slot, draw_argb_line, graphics_clip_rectangle, heap_error, image_origin,
    int_argument, nokia_direct_transform, raster_work_chunks, reference_argument, type_error,
    vm_error,
};

fn lcd_ui_arc_clamp_i64(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[inline]
fn fill_row(target: &mut [HeapValue], color: i32) -> Result<(), EmuError> {
    if color.cast_unsigned() >> 24 == 255 {
        target.fill(HeapValue::Int(color));
    } else {
        for slot in target {
            composite_argb_slot(slot, color)?;
        }
    }
    Ok(())
}

impl Machine<'_, '_> {
    pub(in crate::machine) fn graphics_fill_rect(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        self.execution.counters.fill_rect_calls =
            self.execution.counters.fill_rect_calls.saturating_add(1);
        let graphics = reference_argument(args, 0)?;
        let left = int_argument(args, 1)?;
        let top = int_argument(args, 2)?;
        let right = int_argument(args, 3)?;
        let bottom = int_argument(args, 4)?;
        let color = int_argument(args, 5)?;
        if right <= left || bottom <= top || color.cast_unsigned() >> 24 == 0 {
            return Ok(());
        }
        let (pixels, target_width, _) = self.graphics_target(graphics)?;
        let row_width = usize::try_from(i64::from(right) - i64::from(left)).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "invalid fillRect width",
            )
        })?;
        let target_pixels = int_array_mut(&mut self.heap.managed, pixels)?;
        for (row_index, row) in (top..bottom).enumerate() {
            let start = row
                .checked_mul(target_width)
                .and_then(|base| base.checked_add(left))
                .and_then(|index| usize::try_from(index).ok())
                .ok_or_else(|| {
                    vm_error(
                        "array-index-out-of-bounds-exception",
                        "fillRect target index overflow",
                    )
                })?;
            let end = start.checked_add(row_width).ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "fillRect target range overflow",
                )
            })?;
            let target_row = target_pixels.get_mut(start..end).ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "fillRect target row is out of bounds",
                )
            })?;
            let row_work = row_index * row_width;
            if row_width <= RASTER_CANCELLATION_INTERVAL - row_work % RASTER_CANCELLATION_INTERVAL {
                check_raster_cancellation(self.native_context, row_work)?;
                fill_row(target_row, color)?;
                continue;
            }
            for chunk in raster_work_chunks(row_work..row_work + row_width) {
                check_raster_cancellation(self.native_context, chunk.start)?;
                let target_chunk = &mut target_row[chunk.start - row_work..chunk.end - row_work];
                fill_row(target_chunk, color)?;
            }
        }
        Ok(())
    }

    pub(in crate::machine) fn graphics_draw_image(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        self.execution.counters.draw_image_calls =
            self.execution.counters.draw_image_calls.saturating_add(1);
        let graphics = reference_argument(args, 0)?;
        let image = reference_argument(args, 1)?;
        let origin_x = int_argument(args, 2)?;
        let origin_y = int_argument(args, 3)?;
        let source_width =
            self.graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?;
        let source_height =
            self.graphics_int_field(image, "javax/microedition/lcdui/Image.height:I")?;
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
            .saturating_add(source_width)
            .min(clip_x.saturating_add(clip_w));
        let bottom = origin_y
            .saturating_add(source_height)
            .min(clip_y.saturating_add(clip_h));
        if right <= left || bottom <= top {
            return Ok(());
        }

        // Cache immutable image backing once. Distinct mutable allocations
        // stay borrowed; copying an image onto itself needs a snapshot.
        let source_pixels = self.graphics_cached_image_pixels(image, source, args)?;
        // Drawing an image onto itself is not exposed by MIDP, but bootstrap
        // and defensive synthetic callers can construct that alias. Retain a
        // snapshot only for this overlap case; distinct mutable images can be
        // streamed directly from one managed allocation into the other.
        let overlapping_pixels = if source_pixels.is_none() && source == target {
            Some(Arc::new(ImmutableImagePixels::Argb(
                self.graphics_int_array_region_snapshot(
                    source,
                    source_width,
                    0,
                    0,
                    source_width,
                    source_height,
                )?
                .into_boxed_slice(),
            )))
        } else {
            None
        };
        let copy_width = usize::try_from(right - left).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "drawImage clipped width overflow",
            )
        })?;
        let source_x = left - origin_x;
        let source_y = top - origin_y;
        let region = ImageBlitRegion {
            copy_width,
            rows: bottom - top,
            source_offset: i64::from(source_y) * i64::from(source_width) + i64::from(source_x),
            source_stride: source_width,
            target_width,
            target_x: left,
            target_y: top,
        };

        if let Some(source_pixels) = source_pixels.or(overlapping_pixels) {
            let target_pixels = int_array_mut(&mut self.heap.managed, target)?;
            return match source_pixels.as_ref() {
                ImmutableImagePixels::Argb(pixels) => composite_image_rows(
                    self.native_context,
                    pixels,
                    |pixel| Ok(*pixel),
                    target_pixels,
                    region,
                ),
                ImmutableImagePixels::Indexed8 { palette, indices } => composite_image_rows(
                    self.native_context,
                    indices,
                    |index| {
                        palette.get(usize::from(*index)).copied().ok_or_else(|| {
                            vm_error(
                                "array-index-out-of-bounds-exception",
                                "drawImage palette index is out of bounds",
                            )
                        })
                    },
                    target_pixels,
                    region,
                ),
            };
        }

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
        composite_image_rows(
            self.native_context,
            source_pixels,
            |pixel| match pixel {
                HeapValue::Int(pixel) => Ok(*pixel),
                _ => Err(type_error()),
            },
            target_pixels,
            region,
        )
    }

    pub(in crate::machine) fn graphics_draw_nokia_image(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let image = reference_argument(args, 1)?;
        let x = int_argument(args, 2)?;
        let y = int_argument(args, 3)?;
        let anchor = int_argument(args, 4)?;
        let manipulation = int_argument(args, 5)?;
        let transform = nokia_direct_transform(manipulation)
            .ok_or_else(|| vm_error("illegal-argument", "unsupported DirectGraphics transform"))?;
        let source_width =
            self.graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?;
        let source_height =
            self.graphics_int_field(image, "javax/microedition/lcdui/Image.height:I")?;
        let (draw_width, draw_height) = if transform >= 4 {
            (source_height, source_width)
        } else {
            (source_width, source_height)
        };
        let translate_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
        let translate_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
        let (origin_x, origin_y) = image_origin(
            x.saturating_add(translate_x),
            y.saturating_add(translate_y),
            draw_width,
            draw_height,
            anchor,
        )?;

        self.graphics_draw_region(&[
            Value::Reference(Some(graphics)),
            Value::Reference(Some(image)),
            Value::Int(0),
            Value::Int(0),
            Value::Int(source_width),
            Value::Int(source_height),
            Value::Int(transform),
            Value::Int(origin_x),
            Value::Int(origin_y),
            Value::Int(draw_width),
            Value::Int(draw_height),
        ])
    }

    pub(in crate::machine) fn graphics_set_argb_color(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let color = int_argument(args, 1)?;
        self.heap
            .managed
            .set_field(
                graphics,
                "javax/microedition/lcdui/Graphics.color:I",
                HeapValue::Int(color),
            )
            .map_err(heap_error)
    }

    pub(in crate::machine) fn graphics_set_clip(&mut self, args: &[Value]) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let translate_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
        let translate_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
        let (_, target_width, target_height) = self.graphics_target(graphics)?;
        let [clip_x, clip_y, clip_w, clip_h] = graphics_clip_rectangle(
            int_argument(args, 1)?,
            int_argument(args, 2)?,
            int_argument(args, 3)?,
            int_argument(args, 4)?,
            translate_x,
            translate_y,
            target_width,
            target_height,
        );
        for (field, value) in [
            ("javax/microedition/lcdui/Graphics.clipX:I", clip_x),
            ("javax/microedition/lcdui/Graphics.clipY:I", clip_y),
            ("javax/microedition/lcdui/Graphics.clipW:I", clip_w),
            ("javax/microedition/lcdui/Graphics.clipH:I", clip_h),
        ] {
            self.heap
                .managed
                .set_field(graphics, field, HeapValue::Int(value))
                .map_err(heap_error)?;
        }
        Ok(())
    }

    pub(in crate::machine) fn graphics_draw_line(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let translate_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
        let translate_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
        let clip_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipX:I")?;
        let clip_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipY:I")?;
        let clip_w =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipW:I")?;
        let clip_h =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipH:I")?;
        let color =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.color:I")?;
        let dotted =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.stroke:I")? != 0;
        let (target, target_width, target_height) = self.graphics_target(graphics)?;
        let left = clip_x.max(0).min(target_width);
        let top = clip_y.max(0).min(target_height);
        let right = clip_x.saturating_add(clip_w).max(0).min(target_width);
        let bottom = clip_y.saturating_add(clip_h).max(0).min(target_height);
        let pixels = self.graphics_int_array_mut(target)?;
        draw_argb_line(
            pixels,
            target_width,
            (
                int_argument(args, 1)?.saturating_add(translate_x),
                int_argument(args, 2)?.saturating_add(translate_y),
            ),
            (
                int_argument(args, 3)?.saturating_add(translate_x),
                int_argument(args, 4)?.saturating_add(translate_y),
            ),
            (left, top, right, bottom),
            color,
            dotted,
        )
    }

    pub(in crate::machine) fn graphics_draw_arc(
        &mut self,
        args: &[Value],
        fill: bool,
    ) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let x = int_argument(args, 1)?;
        let y = int_argument(args, 2)?;
        let width = int_argument(args, 3)?;
        let height = int_argument(args, 4)?;
        let start = int_argument(args, 5)?;
        let angle = int_argument(args, 6)?;
        if width < 0 || height < 0 || angle == 0 || (fill && (width == 0 || height == 0)) {
            return Ok(());
        }

        let translate_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
        let translate_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
        let clip_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipX:I")?;
        let clip_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipY:I")?;
        let clip_w =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipW:I")?;
        let clip_h =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipH:I")?;
        let color =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.color:I")?;
        if color.cast_unsigned() >> 24 == 0 {
            return Ok(());
        }
        let dotted = !fill
            && self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.stroke:I")?
                != 0;
        let (target, target_width, target_height) = self.graphics_target(graphics)?;
        let clip = (
            clip_x.max(0).min(target_width),
            clip_y.max(0).min(target_height),
            clip_x.saturating_add(clip_w).max(0).min(target_width),
            clip_y.saturating_add(clip_h).max(0).min(target_height),
        );

        // Shared integer geometry keeps native wrappers and intrinsic calls
        // consistent with the deterministic LCDUI rasterizer.
        let center_x =
            lcd_ui_arc_clamp_i64(i64::from(x) + i64::from(translate_x) + i64::from(width) / 2);
        let center_y =
            lcd_ui_arc_clamp_i64(i64::from(y) + i64::from(translate_y) + i64::from(height) / 2);
        let radius_x = i64::from(width) / 2;
        let radius_y = i64::from(height) / 2;
        let clip = (
            clip.0
                .max(lcd_ui_arc_clamp_i64(i64::from(center_x) - radius_x)),
            clip.1
                .max(lcd_ui_arc_clamp_i64(i64::from(center_y) - radius_y)),
            clip.2
                .min(lcd_ui_arc_clamp_i64(i64::from(center_x) + radius_x + 1)),
            clip.3
                .min(lcd_ui_arc_clamp_i64(i64::from(center_y) + radius_y + 1)),
        );
        if clip.0 >= clip.2 || clip.1 >= clip.3 {
            return Ok(());
        }
        let max_steps = (i64::from(target_width) + i64::from(target_height))
            .saturating_mul(8)
            .max(1);

        let Allocation::Array {
            kind: ArrayKind::Int,
            elements: pixels,
        } = self.heap.managed.get_mut(target).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let check_work = |work| check_raster_cancellation(self.native_context, work);
        let mut painter = ShapePainter::new(pixels, target_width, clip, color)?;
        let mut previous = (center_x, center_y);
        for (index, (point, repeated)) in graphics::ellipse_arc_samples(
            (center_x, center_y),
            (width, height),
            start,
            angle,
            max_steps,
        )
        .enumerate()
        {
            check_work(index)?;
            if fill {
                painter.triangle([(center_x, center_y), previous, point], check_work)?;
                painter.line(previous, point, false, check_work)?;
            } else if index > 0 {
                painter.line(previous, point, dotted, check_work)?;
            }
            if !fill && repeated {
                painter.point(point.0, point.1)?;
            }
            previous = point;
        }
        if fill {
            painter.line((center_x, center_y), previous, false, check_work)?;
        }
        Ok(())
    }

    pub(in crate::machine) fn canvas_is_applied_current(
        &self,
        canvas: Handle,
    ) -> Result<bool, EmuError> {
        let display = match self
            .classes
            .static_fields
            .get("javax/microedition/lcdui/Display.INSTANCE:Ljavax/microedition/lcdui/Display;")
        {
            Some(Value::Reference(Some(display))) => *display,
            Some(Value::Reference(None)) | None => return Ok(false),
            Some(_) => return Err(type_error()),
        };
        match self
            .heap.managed
            .field(
                display,
                "javax/microedition/lcdui/Display.appliedCurrent:Ljavax/microedition/lcdui/Displayable;",
            )
            .map_err(heap_error)?
        {
            HeapValue::Reference(current) => Ok(current == Some(canvas)),
            _ => Err(type_error()),
        }
    }

    pub(in crate::machine) fn canvas_repaint(&mut self, args: &[Value]) -> Result<(), EmuError> {
        let canvas = reference_argument(args, 0)?;
        let framebuffer = self.graphics_reference_field(
            canvas,
            "javax/microedition/lcdui/Canvas.framebuffer:Ljavax/microedition/lcdui/Image;",
        )?;
        let framebuffer_width =
            self.graphics_int_field(framebuffer, "javax/microedition/lcdui/Image.width:I")?;
        let framebuffer_height =
            self.graphics_int_field(framebuffer, "javax/microedition/lcdui/Image.height:I")?;
        let (x, y, width, height) = if args.len() == 1 {
            (0, 0, framebuffer_width, framebuffer_height)
        } else {
            (
                int_argument(args, 1)?,
                int_argument(args, 2)?,
                int_argument(args, 3)?,
                int_argument(args, 4)?,
            )
        };
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let left = x.max(0).min(framebuffer_width);
        let top = y.max(0).min(framebuffer_height);
        let right = x.saturating_add(width).max(0).min(framebuffer_width);
        let bottom = y.saturating_add(height).max(0).min(framebuffer_height);
        if left >= right || top >= bottom {
            return Ok(());
        }
        let pending =
            self.graphics_int_field(canvas, "javax/microedition/lcdui/Canvas.pending:Z")? != 0;
        // Repaint requests are asynchronous and coalesce while a paint is
        // already pending. Only the request that creates new visible work may
        // consume a host frame interval; pacing every merged request stalls a
        // guest repaint loop before the display turn can service that work.
        // An unapplied Display transition likewise leaves the target hidden.
        if !pending && self.canvas_is_applied_current(canvas)? {
            self.native_context.pace_lcdui_frame_request()?;
        }
        let (left, top, right, bottom) = if pending {
            let old_left =
                self.graphics_int_field(canvas, "javax/microedition/lcdui/Canvas.damageX:I")?;
            let old_top =
                self.graphics_int_field(canvas, "javax/microedition/lcdui/Canvas.damageY:I")?;
            let old_width =
                self.graphics_int_field(canvas, "javax/microedition/lcdui/Canvas.damageW:I")?;
            let old_height =
                self.graphics_int_field(canvas, "javax/microedition/lcdui/Canvas.damageH:I")?;
            (
                left.min(old_left),
                top.min(old_top),
                right.max(old_left.saturating_add(old_width)),
                bottom.max(old_top.saturating_add(old_height)),
            )
        } else {
            (left, top, right, bottom)
        };
        for (field, value) in [
            ("javax/microedition/lcdui/Canvas.pending:Z", 1),
            ("javax/microedition/lcdui/Canvas.damageX:I", left),
            ("javax/microedition/lcdui/Canvas.damageY:I", top),
            (
                "javax/microedition/lcdui/Canvas.damageW:I",
                right.saturating_sub(left),
            ),
            (
                "javax/microedition/lcdui/Canvas.damageH:I",
                bottom.saturating_sub(top),
            ),
        ] {
            self.heap
                .managed
                .set_field(canvas, field, HeapValue::Int(value))
                .map_err(heap_error)?;
        }
        Ok(())
    }

    pub(in crate::machine) fn graphics_color_component(
        &self,
        args: &[Value],
        shift: u32,
    ) -> Result<i32, EmuError> {
        let graphics = reference_argument(args, 0)?;
        let color = self
            .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.color:I")?
            .cast_unsigned();
        Ok(((color >> shift) & 0xff).cast_signed())
    }
}
