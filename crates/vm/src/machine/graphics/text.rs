use super::{
    Allocation, ArrayKind, EmuError, Handle, Machine, check_raster_cancellation,
    composite_argb_slot, heap_error, lcd_ui_font_dimensions, lcd_ui_small_unicode_column_is_set,
    lcd_ui_unicode_column_is_set, lcd_ui_unicode_raster_height, lcd_ui_unicode_row,
    system_font_has_glyph, type_error, unicode_text_origin, unifont_bmp_glyph,
    unifont_bmp_glyph_width, vm_error,
};

#[derive(Clone, Copy)]
pub(in crate::machine) enum TextSource<'a> {
    JavaString(Handle),
    Utf16(&'a [u16]),
}

fn character_advance(codepoint: u16, height: i32, narrow_width: i32) -> i32 {
    if !system_font_has_glyph(codepoint) && unifont_bmp_glyph_width(codepoint) == Some(16) {
        height
    } else {
        narrow_width
    }
}

// Keep the optional whole-string scan separate from the glyph rasterizer.
#[inline(never)]
fn text_width(
    host: &dyn natives::HostServices,
    text: &[u16],
    height: i32,
    narrow_width: i32,
) -> Result<i32, EmuError> {
    let mut width = 0_i32;
    for (index, &codepoint) in text.iter().enumerate() {
        check_raster_cancellation(host, index)?;
        width = width.saturating_add(character_advance(codepoint, height, narrow_width));
    }
    Ok(width)
}

impl Machine<'_, '_> {
    pub(in crate::machine) fn active_lcd_ui_font_dimensions(&self, size: i32) -> (i32, i32) {
        let (default_height, width) = lcd_ui_font_dimensions(size);
        let height = self
            .native_context
            .lcd_ui_font_height(size)
            .filter(|height| (1..=64).contains(height))
            .unwrap_or(default_height);
        (height, width)
    }

    pub(in crate::machine) fn unicode_fallback_char_width(
        &self,
        font: Handle,
        codepoint: u16,
    ) -> Result<Option<i32>, EmuError> {
        if system_font_has_glyph(codepoint) {
            return Ok(None);
        }
        let Some(glyph_width) = unifont_bmp_glyph_width(codepoint) else {
            return Ok(None);
        };
        let size = self.graphics_int_field(font, "javax/microedition/lcdui/Font.size:I")?;
        let (height, narrow_width) = self.active_lcd_ui_font_dimensions(size);
        Ok(Some(if glyph_width == 16 {
            height
        } else {
            narrow_width
        }))
    }

    pub(in crate::machine) fn graphics_unicode_draw_text(
        &mut self,
        graphics: Handle,
        text: TextSource<'_>,
        x: i32,
        y: i32,
        anchor: i32,
    ) -> Result<(), EmuError> {
        // String payloads and target pixels occupy separate heap storage;
        // drawing can borrow the UTF-16 units without copying the whole String.
        let text = match text {
            TextSource::JavaString(string) => self
                .heap
                .string_values
                .get(&string)
                .ok_or_else(type_error)?
                .as_slice(),
            TextSource::Utf16(text) => text,
        };
        let font = self.graphics_reference_field(
            graphics,
            "javax/microedition/lcdui/Graphics.font:Ljavax/microedition/lcdui/Font;",
        )?;
        let size = self.graphics_int_field(font, "javax/microedition/lcdui/Font.size:I")?;
        let (height, narrow_width) = self.active_lcd_ui_font_dimensions(size);
        // Only HCENTER and RIGHT need the full width. Left-aligned drawing
        // stops at the clip edge without scanning the invisible suffix.
        let width = if anchor & (1 | 8) != 0 {
            text_width(self.native_context, text, height, narrow_width)?
        } else {
            if !text.is_empty() {
                check_raster_cancellation(self.native_context, 0)?;
            }
            0
        };
        let translate_x =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")?;
        let translate_y =
            self.graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")?;
        let (origin_x, origin_y) = unicode_text_origin(
            x.saturating_add(translate_x),
            y.saturating_add(translate_y),
            width,
            height,
            anchor,
        )?;

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
        let (target, target_width, target_height) = self.graphics_target(graphics)?;
        let clip_right = clip_x.saturating_add(clip_w).min(target_width);
        let clip_bottom = clip_y.saturating_add(clip_h).min(target_height);
        let clip_left = clip_x.max(0);
        let clip_top = clip_y.max(0);
        if clip_left >= clip_right || clip_top >= clip_bottom || color.cast_unsigned() >> 24 == 0 {
            return Ok(());
        }

        let first_row = clip_top.saturating_sub(origin_y).clamp(0, height);
        let last_row = clip_bottom.saturating_sub(origin_y).clamp(0, height);
        if first_row >= last_row {
            return Ok(());
        }
        let question = unifont_bmp_glyph(u16::from(b'?')).expect("embedded question glyph");
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements: target_pixels,
        } = self.heap.managed.get_mut(target).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        // Font.getBaselinePosition() is height - 1. The compact alphabet has
        // seven ink rows above that baseline, followed by one leading row.
        // Align it to the font metric, not the top of a taller line cell;
        // otherwise clipping the cell's top bearing cuts into the letters.
        let system_glyph_top = (height - 8).max(0);
        let mut raster_rows = 0;
        let mut pen_x = origin_x;
        for (index, &codepoint) in text.iter().enumerate() {
            check_raster_cancellation(self.native_context, index)?;
            if pen_x >= clip_right {
                break;
            }
            let advance = character_advance(codepoint, height, narrow_width);
            let next_pen_x = pen_x.saturating_add(advance);
            if next_pen_x <= clip_left {
                pen_x = next_pen_x;
                continue;
            }
            let glyph = unifont_bmp_glyph(codepoint).unwrap_or(question);
            let system_glyph =
                char::from_u32(u32::from(codepoint)).and_then(graphics::compact_lcd_ui_font_glyph);
            let compact_unicode = system_glyph.is_none() && size == 8 && glyph.width == 8;
            let raster_height = lcd_ui_unicode_raster_height(size, glyph.width, height);
            let unicode_glyph_top = height - raster_height;
            let first_column = clip_left.saturating_sub(pen_x).clamp(0, advance);
            let last_column = clip_right.saturating_sub(pen_x).clamp(0, advance);
            for target_row in first_row..last_row {
                check_raster_cancellation(self.native_context, raster_rows)?;
                raster_rows += 1;
                let pixel_y = origin_y.saturating_add(target_row);
                let source_bits = if let Some(rows) = system_glyph {
                    usize::try_from(target_row - system_glyph_top)
                        .ok()
                        .and_then(|row| rows.get(row))
                        .map_or(0, |bits| u16::from(*bits) << 11)
                } else {
                    lcd_ui_unicode_row(&glyph, target_row - unicode_glyph_top, raster_height)
                        .unwrap_or(0)
                };
                if source_bits == 0 {
                    continue;
                }
                for target_column in first_column..last_column {
                    let pixel_x = pen_x.saturating_add(target_column);
                    let pixel_is_set = if system_glyph.is_some() {
                        target_column < 5 && source_bits & (1 << (15 - target_column)) != 0
                    } else if compact_unicode {
                        lcd_ui_small_unicode_column_is_set(source_bits, target_column)
                    } else {
                        lcd_ui_unicode_column_is_set(
                            source_bits,
                            glyph.width,
                            target_column,
                            advance,
                        )
                    };
                    if !pixel_is_set {
                        continue;
                    }
                    let index = pixel_y
                        .checked_mul(target_width)
                        .and_then(|base| base.checked_add(pixel_x))
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or_else(|| {
                            vm_error(
                                "array-index-out-of-bounds-exception",
                                "Unicode glyph target index overflow",
                            )
                        })?;
                    let slot = target_pixels.get_mut(index).ok_or_else(|| {
                        vm_error(
                            "array-index-out-of-bounds-exception",
                            "Unicode glyph target is out of bounds",
                        )
                    })?;
                    composite_argb_slot(slot, color)?;
                }
            }
            pen_x = next_pen_x;
        }
        Ok(())
    }
}
