use super::{
    CallOutcome, EmuError, HeapValue, LCDUI_GRAPHICS_DRAW_IMAGE_FNV1A64,
    LCDUI_GRAPHICS_DRAW_RGB_FNV1A64, LCDUI_GRAPHICS_FILL_RECT_FNV1A64,
    LCDUI_GRAPHICS_SET_COLOR_FNV1A64, LCDUI_GRAPHICS_SET_RGB_COLOR_FNV1A64,
    LCDUI_GRAPHICS_TRANSLATE_FNV1A64, Machine, Method, Value, heap_error, image_axis_origin,
    int_argument, reference_argument, vm_error,
};

impl Machine<'_, '_> {
    pub(super) fn invoke_lcd_ui_graphics_scalar_intrinsic(
        method: &Method,
        args: &[Value],
    ) -> Result<Option<CallOutcome>, EmuError> {
        fn sat(left: i32, right: i32) -> i32 {
            (i64::from(left) + i64::from(right)).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                as i32
        }

        if !method.is_static || method.key.class != "javax/microedition/lcdui/Graphics" {
            return Ok(None);
        }

        let value = match (method.key.name.as_str(), method.key.descriptor.as_str()) {
            ("sat", "(II)I") => sat(int_argument(args, 0)?, int_argument(args, 1)?),
            ("clamp", "(III)I") => {
                let value = int_argument(args, 0)?;
                let minimum = int_argument(args, 1)?;
                let maximum = int_argument(args, 2)?;
                if value < minimum {
                    minimum
                } else if value > maximum {
                    maximum
                } else {
                    value
                }
            }
            ("origin", "(IIIZ)I") => {
                let position = int_argument(args, 0)?;
                let extent = int_argument(args, 1)?;
                let anchor = int_argument(args, 2)?;
                let horizontal = int_argument(args, 3)? != 0;
                image_axis_origin(position, extent, anchor, horizontal)?
            }
            _ => return Ok(None),
        };
        Ok(Some(CallOutcome::Return(Some(Value::Int(value)))))
    }

    pub(super) fn invoke_lcd_ui_graphics_public_intrinsic(
        &mut self,
        method: &Method,
        args: &[Value],
    ) -> Result<Option<CallOutcome>, EmuError> {
        if method.is_static
            || method.is_native
            || method.is_synchronized
            || !method.exception_table.is_empty()
            || method.key.class != "javax/microedition/lcdui/Graphics"
        {
            return Ok(None);
        }

        let graphics = reference_argument(args, 0)?;
        match (method.key.name.as_str(), method.key.descriptor.as_str()) {
            ("setColor", "(I)V")
                if method.code.len() == 12
                    && method.code_fingerprint == LCDUI_GRAPHICS_SET_COLOR_FNV1A64 =>
            {
                let color = int_argument(args, 1)? & 0x00ff_ffff | 0xff00_0000_u32.cast_signed();
                self.graphics_set_argb_color(&[
                    Value::Reference(Some(graphics)),
                    Value::Int(color),
                ])?;
            }
            ("setColor", "(III)V")
                if method.code.len() == 53
                    && method.code_fingerprint == LCDUI_GRAPHICS_SET_RGB_COLOR_FNV1A64 =>
            {
                let red = int_argument(args, 1)?;
                let green = int_argument(args, 2)?;
                let blue = int_argument(args, 3)?;
                if !(0..=255).contains(&red)
                    || !(0..=255).contains(&green)
                    || !(0..=255).contains(&blue)
                {
                    return Err(vm_error(
                        "illegal-argument",
                        "Graphics color component is outside 0..=255",
                    ));
                }
                let color = 0xff00_0000_u32.cast_signed() | (red << 16) | (green << 8) | blue;
                self.graphics_set_argb_color(&[
                    Value::Reference(Some(graphics)),
                    Value::Int(color),
                ])?;
            }
            ("translate", "(II)V")
                if method.code.len() == 25
                    && method.code_fingerprint == LCDUI_GRAPHICS_TRANSLATE_FNV1A64 =>
            {
                let translate_x = self
                    .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?
                    .saturating_add(int_argument(args, 1)?);
                self.heap
                    .managed
                    .set_field(
                        graphics,
                        "javax/microedition/lcdui/Graphics.tx:I",
                        HeapValue::Int(translate_x),
                    )
                    .map_err(heap_error)?;
                let translate_y = self
                    .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?
                    .saturating_add(int_argument(args, 2)?);
                self.heap
                    .managed
                    .set_field(
                        graphics,
                        "javax/microedition/lcdui/Graphics.ty:I",
                        HeapValue::Int(translate_y),
                    )
                    .map_err(heap_error)?;
            }
            ("fillRect", "(IIII)V")
                if method.code.len() == 130
                    && method.code_fingerprint == LCDUI_GRAPHICS_FILL_RECT_FNV1A64 =>
            {
                fn clamp(value: i32, minimum: i32, maximum: i32) -> i32 {
                    if value < minimum {
                        minimum
                    } else if value > maximum {
                        maximum
                    } else {
                        value
                    }
                }

                let translate_x =
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
                let translate_y =
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
                let clip_x =
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipX:I")?;
                let clip_y =
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipY:I")?;
                let clip_right = clip_x.wrapping_add(
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipW:I")?,
                );
                let clip_bottom = clip_y.wrapping_add(
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipH:I")?,
                );
                let x = int_argument(args, 1)?.saturating_add(translate_x);
                let y = int_argument(args, 2)?.saturating_add(translate_y);
                let left = clamp(x, clip_x, clip_right);
                let top = clamp(y, clip_y, clip_bottom);
                let right = clamp(x.saturating_add(int_argument(args, 3)?), clip_x, clip_right);
                let bottom = clamp(
                    y.saturating_add(int_argument(args, 4)?),
                    clip_y,
                    clip_bottom,
                );
                let color =
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.color:I")?;
                self.graphics_fill_rect(&[
                    Value::Reference(Some(graphics)),
                    Value::Int(left),
                    Value::Int(top),
                    Value::Int(right),
                    Value::Int(bottom),
                    Value::Int(color),
                ])?;
            }
            ("drawRGB", "([IIIIIIIZ)V")
                if method.code.len() == 142
                    && method.code_fingerprint == LCDUI_GRAPHICS_DRAW_RGB_FNV1A64 =>
            {
                // Preserve the public bootstrap's validation order before its
                // private pixel helper: null/negative extents first, then the
                // complete offset/scanline window. A scanline may be negative
                // or wider than the array when the requested height is one.
                let source = reference_argument(args, 1)?;
                let offset = int_argument(args, 2)?;
                let scanlength = int_argument(args, 3)?;
                let width = int_argument(args, 6)?;
                let height = int_argument(args, 7)?;
                if width < 0 || height < 0 {
                    return Err(vm_error(
                        "illegal-argument",
                        "Graphics.drawRGB dimensions are negative",
                    ));
                }
                let last_row = i64::from(offset) + i64::from(height - 1) * i64::from(scanlength);
                let low = i64::from(offset).min(last_row);
                let high = i64::from(offset).max(last_row) + i64::from(width);
                if height > 0
                    && (low < 0
                        || high
                            > i64::try_from(
                                self.heap.managed.array_length(source).map_err(heap_error)?,
                            )
                            .unwrap_or(i64::MAX))
                {
                    return Err(vm_error(
                        "array-index-out-of-bounds-exception",
                        "Graphics.drawRGB source range is outside the array",
                    ));
                }
                let translate_x =
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
                let translate_y =
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
                self.graphics_draw_rgb(&[
                    Value::Reference(Some(graphics)),
                    Value::Reference(Some(source)),
                    Value::Int(offset),
                    Value::Int(scanlength),
                    Value::Int(int_argument(args, 4)?.saturating_add(translate_x)),
                    Value::Int(int_argument(args, 5)?.saturating_add(translate_y)),
                    Value::Int(width),
                    Value::Int(height),
                    Value::Int(int_argument(args, 8)?),
                ])?;
            }
            ("drawImage", "(Ljavax/microedition/lcdui/Image;III)V")
                if method.code.len() == 62
                    && method.code_fingerprint == LCDUI_GRAPHICS_DRAW_IMAGE_FNV1A64 =>
            {
                let image = reference_argument(args, 1)?;
                let translate_x =
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
                let translate_y =
                    self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
                let width =
                    self.graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?;
                let height =
                    self.graphics_int_field(image, "javax/microedition/lcdui/Image.height:I")?;
                let anchor = int_argument(args, 4)?;
                let origin_x = image_axis_origin(
                    int_argument(args, 2)?.saturating_add(translate_x),
                    width,
                    anchor,
                    true,
                )?;
                let origin_y = image_axis_origin(
                    int_argument(args, 3)?.saturating_add(translate_y),
                    height,
                    anchor,
                    false,
                )?;
                self.graphics_draw_image(&[
                    Value::Reference(Some(graphics)),
                    Value::Reference(Some(image)),
                    Value::Int(origin_x),
                    Value::Int(origin_y),
                ])?;
            }
            ("drawRegion", "(Ljavax/microedition/lcdui/Image;IIIIIIII)V")
                if method.code.len() == 192 && method.code_fingerprint == 0x3663_ad31_9c91_6642 =>
            {
                // This fingerprint guards our Rust-owned bootstrap wrapper,
                // never an application method or archive identity.
                self.graphics_draw_region_public(args)?;
            }
            _ => return Ok(None),
        }
        Ok(Some(CallOutcome::Return(None)))
    }

    fn graphics_draw_region_public(&mut self, args: &[Value]) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let image = reference_argument(args, 1)?;
        let target = self.graphics_reference_field(
            graphics,
            "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;",
        )?;
        let x = int_argument(args, 2)?;
        let y = int_argument(args, 3)?;
        let width = int_argument(args, 4)?;
        let height = int_argument(args, 5)?;
        let transform = int_argument(args, 6)?;
        if image == target
            || width < 0
            || height < 0
            || x < 0
            || y < 0
            || x > self
                .graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?
                .wrapping_sub(width)
            || y > self
                .graphics_int_field(image, "javax/microedition/lcdui/Image.height:I")?
                .wrapping_sub(height)
            || !(0..=7).contains(&transform)
        {
            return Err(vm_error(
                "illegal-argument",
                "invalid Graphics.drawRegion source, extent or transform",
            ));
        }
        if width == 0 || height == 0 {
            return Ok(());
        }
        let (draw_width, draw_height) = if transform >= 4 {
            (height, width)
        } else {
            (width, height)
        };
        let anchor = int_argument(args, 9)?;
        let origin_x = image_axis_origin(
            int_argument(args, 7)?.saturating_add(
                self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?,
            ),
            draw_width,
            anchor,
            true,
        )?;
        let origin_y = image_axis_origin(
            int_argument(args, 8)?.saturating_add(
                self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?,
            ),
            draw_height,
            anchor,
            false,
        )?;
        self.graphics_draw_region(&[
            Value::Reference(Some(graphics)),
            Value::Reference(Some(image)),
            Value::Int(x),
            Value::Int(y),
            Value::Int(width),
            Value::Int(height),
            Value::Int(transform),
            Value::Int(origin_x),
            Value::Int(origin_y),
            Value::Int(draw_width),
            Value::Int(draw_height),
        ])
    }
}
