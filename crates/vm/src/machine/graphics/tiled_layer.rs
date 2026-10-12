use super::{
    Allocation, ArrayKind, EmuError, HeapValue, Machine, Value, check_raster_cancellation,
    heap_error, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn graphics_tiled_layer_paint(
        &mut self,
        args: &[Value],
    ) -> Result<bool, EmuError> {
        let layer = reference_argument(args, 0)?;
        let graphics = reference_argument(args, 1)?;
        if self.graphics_int_field(layer, "javax/microedition/lcdui/game/Layer.visible:Z")? == 0 {
            return Ok(true);
        }

        let cols =
            self.graphics_int_field(layer, "javax/microedition/lcdui/game/TiledLayer.cols:I")?;
        let rows =
            self.graphics_int_field(layer, "javax/microedition/lcdui/game/TiledLayer.rows:I")?;
        let tile_width =
            self.graphics_int_field(layer, "javax/microedition/lcdui/game/TiledLayer.tw:I")?;
        let tile_height =
            self.graphics_int_field(layer, "javax/microedition/lcdui/game/TiledLayer.th:I")?;
        if cols <= 0 || rows <= 0 || tile_width <= 0 || tile_height <= 0 {
            return Ok(false);
        }

        let image = self.graphics_reference_field(
            layer,
            "javax/microedition/lcdui/game/TiledLayer.image:Ljavax/microedition/lcdui/Image;",
        )?;
        let target_image = self.graphics_reference_field(
            graphics,
            "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;",
        )?;
        // Graphics.drawRegion rejects self-copy. Fall back to the Java method
        // so the exception remains conditional on encountering a nonempty tile.
        if image == target_image {
            return Ok(false);
        }
        let cells = self
            .graphics_reference_field(layer, "javax/microedition/lcdui/game/TiledLayer.cells:[I")?;
        let animated = self.graphics_reference_field(
            layer,
            "javax/microedition/lcdui/game/TiledLayer.animated:[I",
        )?;
        let image_width =
            self.graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?;
        let sheet_cols = image_width / tile_width;
        if sheet_cols <= 0 {
            return Ok(false);
        }

        let layer_x = self.graphics_int_field(layer, "javax/microedition/lcdui/game/Layer.x:I")?;
        let layer_y = self.graphics_int_field(layer, "javax/microedition/lcdui/game/Layer.y:I")?;
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
        if clip_w <= 0 || clip_h <= 0 {
            return Ok(true);
        }

        let last_x = i64::from(layer_x) + i64::from(cols - 1) * i64::from(tile_width);
        let last_y = i64::from(layer_y) + i64::from(rows - 1) * i64::from(tile_height);
        let base_x = i64::from(layer_x) + i64::from(translate_x);
        let base_y = i64::from(layer_y) + i64::from(translate_y);
        let translated_last_x = last_x + i64::from(translate_x);
        let translated_last_y = last_y + i64::from(translate_y);
        if [
            last_x,
            last_y,
            base_x,
            base_y,
            translated_last_x,
            translated_last_y,
        ]
        .into_iter()
        .any(|value| i32::try_from(value).is_err())
        {
            // Saturation makes extreme coordinates non-linear. Preserve the
            // Java implementation for that rare edge case.
            return Ok(false);
        }

        let ceil_div = |value: i64, divisor: i64| -(-value).div_euclid(divisor);
        let clip_right = i64::from(clip_x) + i64::from(clip_w);
        let clip_bottom = i64::from(clip_y) + i64::from(clip_h);
        let first_col = (i64::from(clip_x) - base_x)
            .div_euclid(i64::from(tile_width))
            .clamp(0, i64::from(cols));
        let last_col =
            ceil_div(clip_right - base_x, i64::from(tile_width)).clamp(0, i64::from(cols));
        let first_row = (i64::from(clip_y) - base_y)
            .div_euclid(i64::from(tile_height))
            .clamp(0, i64::from(rows));
        let last_row =
            ceil_div(clip_bottom - base_y, i64::from(tile_height)).clamp(0, i64::from(rows));

        let mut work = 0_usize;
        for row in first_row..last_row {
            for col in first_col..last_col {
                // Empty cells do not call the renderer, but scanning them must
                // still honor Stop, including a long row or narrow clipped column.
                check_raster_cancellation(self.native_context, work)?;
                work = work.wrapping_add(1);
                let index = usize::try_from(row * i64::from(cols) + col).map_err(|_| {
                    vm_error(
                        "array-index-out-of-bounds-exception",
                        "TiledLayer cell index overflow",
                    )
                })?;
                let tile = match self.heap.managed.get(cells).map_err(heap_error)? {
                    Allocation::Array {
                        kind: ArrayKind::Int,
                        elements,
                    } => match elements.get(index) {
                        Some(HeapValue::Int(tile)) => *tile,
                        Some(_) => return Err(type_error()),
                        None => {
                            return Err(vm_error(
                                "array-index-out-of-bounds-exception",
                                "TiledLayer cell index is out of bounds",
                            ));
                        }
                    },
                    _ => return Err(type_error()),
                };
                let tile = if tile < 0 {
                    let animated_index = tile
                        .checked_neg()
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or_else(|| {
                            vm_error(
                                "array-index-out-of-bounds-exception",
                                "TiledLayer animated tile index overflow",
                            )
                        })?;
                    match self.heap.managed.get(animated).map_err(heap_error)? {
                        Allocation::Array {
                            kind: ArrayKind::Int,
                            elements,
                        } => match elements.get(animated_index) {
                            Some(HeapValue::Int(tile)) => *tile,
                            Some(_) => return Err(type_error()),
                            None => {
                                return Err(vm_error(
                                    "array-index-out-of-bounds-exception",
                                    "TiledLayer animated tile index is out of bounds",
                                ));
                            }
                        },
                        _ => return Err(type_error()),
                    }
                } else {
                    tile
                };
                if tile <= 0 {
                    continue;
                }

                let raw = tile - 1;
                let source_x = raw.rem_euclid(sheet_cols).wrapping_mul(tile_width);
                let source_y = (raw / sheet_cols).wrapping_mul(tile_height);
                let origin_x = i32::try_from(base_x + col * i64::from(tile_width))
                    .map_err(|_| vm_error("integer-overflow", "TiledLayer x overflow"))?;
                let origin_y = i32::try_from(base_y + row * i64::from(tile_height))
                    .map_err(|_| vm_error("integer-overflow", "TiledLayer y overflow"))?;
                self.graphics_draw_region(&[
                    Value::Reference(Some(graphics)),
                    Value::Reference(Some(image)),
                    Value::Int(source_x),
                    Value::Int(source_y),
                    Value::Int(tile_width),
                    Value::Int(tile_height),
                    Value::Int(0),
                    Value::Int(origin_x),
                    Value::Int(origin_y),
                    Value::Int(tile_width),
                    Value::Int(tile_height),
                ])?;
            }
        }
        Ok(true)
    }
}
