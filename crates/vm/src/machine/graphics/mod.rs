use super::{
    Allocation, Arc, ArrayKind, EmuError, Handle, HashMap, HeapValue, MAX_LCD_PIXELS, Machine,
    ShapePainter, Value, argb_to_nokia_pixel, composite_argb_slot, draw_argb_line,
    graphics_clip_rectangle, heap_error, image_origin, int_argument, lcd_ui_font_dimensions,
    lcd_ui_small_unicode_column_is_set, lcd_ui_unicode_column_is_set, lcd_ui_unicode_raster_height,
    lcd_ui_unicode_row, nokia_direct_transform, nokia_pixel_to_argb, paint_argb_pixel,
    reference_argument, system_font_has_glyph, transform_image_pixels, type_error,
    unicode_text_origin, unifont_bmp_glyph, unifont_bmp_glyph_width, vm_error,
};

mod canvas;
mod image_blit;
mod images;
mod native;
mod nokia_direct;
mod pixel_access;
mod polygons;
mod primitives;
mod rgb_regions;
mod sprite_collisions;
mod text;
mod tiled_layer;

pub(in crate::machine) use pixel_access::ImmutableImagePixels;
pub(in crate::machine) use text::TextSource;

const RASTER_CANCELLATION_INTERVAL: usize = 1_024;

// Work ranges are bounded by validated image buffers. Align blocks across
// row boundaries so cancellation checks stay outside the inner pixel loops.
fn raster_work_chunks(
    range: std::ops::Range<usize>,
) -> impl Iterator<Item = std::ops::Range<usize>> {
    let first = range.start / RASTER_CANCELLATION_INTERVAL;
    let end = range.end.div_ceil(RASTER_CANCELLATION_INTERVAL);
    (first..end).map(move |block| {
        (block * RASTER_CANCELLATION_INTERVAL).max(range.start)
            ..((block + 1) * RASTER_CANCELLATION_INTERVAL).min(range.end)
    })
}

fn check_raster_cancellation(
    host: &dyn natives::HostServices,
    work: usize,
) -> Result<(), EmuError> {
    if work.is_multiple_of(RASTER_CANCELLATION_INTERVAL) && host.execution_cancelled() {
        return Err(vm_error(
            "execution-cancelled",
            "graphics operation was cancelled by the host",
        ));
    }
    Ok(())
}
