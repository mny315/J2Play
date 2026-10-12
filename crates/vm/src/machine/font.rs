use super::{UNIFONT_BMP, UNIFONT_ENTRY_SIZE, UNIFONT_HEADER_SIZE};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct UnicodeBitmapGlyph {
    pub(super) width: i32,
    pub(super) rows: [u16; 16],
}

fn unifont_bmp_entry(codepoint: u16) -> Option<&'static [u8]> {
    debug_assert_eq!(&UNIFONT_BMP[..8], b"E240UF01");
    let offset = UNIFONT_HEADER_SIZE + usize::from(codepoint) * UNIFONT_ENTRY_SIZE;
    let entry = UNIFONT_BMP.get(offset..offset + UNIFONT_ENTRY_SIZE)?;
    matches!(entry[0], 8 | 16).then_some(entry)
}

pub(super) fn unifont_bmp_glyph_width(codepoint: u16) -> Option<i32> {
    unifont_bmp_entry(codepoint).map(|entry| i32::from(entry[0]))
}

pub(super) fn unifont_bmp_glyph(codepoint: u16) -> Option<UnicodeBitmapGlyph> {
    let entry = unifont_bmp_entry(codepoint)?;
    let mut rows = [0_u16; 16];
    for (row, bytes) in rows.iter_mut().zip(entry[1..].chunks_exact(2)) {
        *row = u16::from_be_bytes([bytes[0], bytes[1]]);
    }
    Some(UnicodeBitmapGlyph {
        width: i32::from(entry[0]),
        rows,
    })
}

pub(super) const NARROW_8_ROW_BOUNDARIES: [usize; 9] = [0, 4, 6, 8, 9, 10, 12, 14, 16];
pub(super) const NARROW_9_ROW_BOUNDARIES: [usize; 10] = [0, 3, 4, 6, 8, 9, 10, 12, 14, 16];
pub(super) const NARROW_5_COLUMN_BOUNDARIES: [usize; 6] = [0, 2, 3, 5, 6, 8];

/// Keeps narrow Unicode glyphs in the same compact raster cell as the
/// built-in 5x7 alphabet. A device profile's measured font height is a line
/// metric; stretching an 8x16 bitmap to that full height makes accented and
/// Cyrillic letters visibly larger than adjacent LCDUI Latin text.
pub(super) fn lcd_ui_unicode_raster_height(size: i32, glyph_width: i32, line_height: i32) -> i32 {
    if size == 8 && glyph_width == 8 {
        line_height.min(9)
    } else {
        line_height
    }
}

/// Resamples one row of the embedded 16-pixel font into an LCDUI cell.
///
/// Every source row in the target row's interval is combined. Sampling only
/// one row drops one-pixel horizontal strokes when the bitmap is reduced. The
/// narrow small-font tables also spend the limited rows on Unifont's actual
/// accent, letter and descender bands instead of its outer cell padding.
pub(super) fn lcd_ui_unicode_row(
    glyph: &UnicodeBitmapGlyph,
    target_row: i32,
    target_height: i32,
) -> Option<u16> {
    if target_height <= 0 || target_row < 0 || target_row >= target_height {
        return None;
    }

    let target_row = usize::try_from(target_row).ok()?;
    let target_height = usize::try_from(target_height).ok()?;
    let (source_start, source_end) = match (target_height, glyph.width) {
        (8, 8) => (
            NARROW_8_ROW_BOUNDARIES[target_row],
            NARROW_8_ROW_BOUNDARIES[target_row + 1],
        ),
        (9, 8) => (
            NARROW_9_ROW_BOUNDARIES[target_row],
            NARROW_9_ROW_BOUNDARIES[target_row + 1],
        ),
        _ => {
            let source_start = target_row.checked_mul(16)? / target_height;
            let next_boundary = target_row.checked_add(1)?.checked_mul(16)? / target_height;
            (
                source_start,
                next_boundary.max(source_start + 1).min(glyph.rows.len()),
            )
        }
    };
    Some(
        glyph.rows[source_start..source_end]
            .iter()
            .copied()
            .fold(0, |combined, row| combined | row),
    )
}

/// Returns whether a resampled target column contains source ink.
///
/// Combining the complete source-column interval is important for the 8-to-6
/// narrow-font reduction: nearest-neighbour sampling otherwise skips columns
/// three and seven, which can erase stems and diagonals from Cyrillic glyphs.
pub(super) fn lcd_ui_unicode_column_is_set(
    row_bits: u16,
    source_width: i32,
    target_column: i32,
    target_width: i32,
) -> bool {
    if !(1..=16).contains(&source_width)
        || target_width <= 0
        || target_column < 0
        || target_column >= target_width
    {
        return false;
    }

    let Ok(source_width) = usize::try_from(source_width) else {
        return false;
    };
    let Ok(target_column) = usize::try_from(target_column) else {
        return false;
    };
    let Ok(target_width) = usize::try_from(target_width) else {
        return false;
    };
    let Some(source_start) = target_column
        .checked_mul(source_width)
        .map(|value| value / target_width)
    else {
        return false;
    };
    let Some(next_boundary) = target_column
        .checked_add(1)
        .and_then(|value| value.checked_mul(source_width))
        .map(|value| value / target_width)
    else {
        return false;
    };
    let source_end = next_boundary.max(source_start + 1).min(source_width);
    column_range_is_set(row_bits, source_start, source_end)
}

/// Resamples an 8-pixel Unicode cell into the five ink columns used by the
/// small LCDUI alphabet, leaving the sixth advance column as spacing. The
/// symmetric boundaries retain both one-pixel side bearings and do not shift
/// a fallback glyph one pixel to the right relative to built-in glyphs.
pub(super) fn lcd_ui_small_unicode_column_is_set(row_bits: u16, target_column: i32) -> bool {
    let Ok(target_column) = usize::try_from(target_column) else {
        return false;
    };
    let Some((&source_start, &source_end)) = NARROW_5_COLUMN_BOUNDARIES
        .get(target_column)
        .zip(NARROW_5_COLUMN_BOUNDARIES.get(target_column + 1))
    else {
        return false;
    };
    column_range_is_set(row_bits, source_start, source_end)
}

fn column_range_is_set(row_bits: u16, start: usize, end: usize) -> bool {
    // Column zero is the high bit; u32 also permits the exclusive end at 16.
    let mask = (0xffff_u32 >> start) ^ (0xffff_u32 >> end);
    u32::from(row_bits) & mask != 0
}

pub(super) const fn system_font_has_glyph(codepoint: u16) -> bool {
    matches!(
        codepoint,
        0x20..=0x7e | 0x0401 | 0x0410..=0x044f | 0x0451
    )
}

pub(super) const fn lcd_ui_font_dimensions(size: i32) -> (i32, i32) {
    match size {
        8 => (8, 6),
        16 => (16, 12),
        _ => (12, 8),
    }
}
