use super::*;
use crate::machine::font::UnicodeBitmapGlyph;
use crate::machine::graphics_dispatch::TextSource;
use std::cell::Cell;
use std::rc::Rc;

struct FontHeightContext(i32);

impl HostServices for FontHeightContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }

    fn lcd_ui_font_height(&self, _: i32) -> Option<i32> {
        Some(self.0)
    }
}

#[test]
fn compact_glyphs_stay_above_the_baseline_when_line_height_changes() {
    let program = Program::new();
    for size in [8, 0, 16] {
        for character in ['A', 'Б', 'Ё', 'Ê'] {
            let mut reference = None;
            for height in [8, 9, 12, 16, 18, 22, 36, 64] {
                let mut context = FontHeightContext(height);
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let (_, graphics, pixels) = unicode_graphics_target(&mut machine, size, 16, 80);
                machine
                    .graphics_unicode_draw_text(
                        graphics,
                        TextSource::Utf16(&[character as u16]),
                        0,
                        72,
                        4 | 64,
                    )
                    .unwrap();
                let actual = machine.graphics_int_array_snapshot(pixels).unwrap();
                let occupied_rows = (0..80)
                    .filter(|row| {
                        actual[row * 16..(row + 1) * 16]
                            .iter()
                            .any(|pixel| *pixel != 0)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(occupied_rows.first(), Some(&65));
                assert_eq!(occupied_rows.last(), Some(&71));
                if let Some(expected) = &reference {
                    assert_eq!(
                        &actual, expected,
                        "size={size}, height={height}, {character}"
                    );
                } else {
                    reference = Some(actual);
                }
            }
        }
    }
}

#[test]
fn clipping_the_top_bearing_preserves_compact_text() {
    use super::graphics_support::set_graphics_fields;

    let program = Program::new();
    for (size, height, bearing) in [(8, 9, 1), (0, 12, 4), (16, 16, 8), (8, 18, 10)] {
        let mut context = FontHeightContext(height);
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let text = ['A' as u16, 'П' as u16, 'Ё' as u16];
        let (_, reference, reference_pixels) = unicode_graphics_target(&mut machine, size, 48, 40);
        machine
            .graphics_unicode_draw_text(reference, TextSource::Utf16(&text), 3, 10, 20)
            .unwrap();
        let expected = machine
            .graphics_int_array_snapshot(reference_pixels)
            .unwrap();
        let (_, clipped, clipped_pixels) = unicode_graphics_target(&mut machine, size, 48, 40);
        set_graphics_fields(
            &mut machine,
            clipped,
            &[("clipY", 10 + bearing), ("clipH", 7)],
        );
        machine
            .graphics_unicode_draw_text(clipped, TextSource::Utf16(&text), 3, 10, 20)
            .unwrap();
        assert_eq!(
            machine.graphics_int_array_snapshot(clipped_pixels).unwrap(),
            expected
        );
        assert_eq!(
            expected.iter().filter(|pixel| **pixel != 0).count(),
            18 + 17 + 18
        );
    }
}

#[test]
fn profiled_compact_unicode_shares_the_system_font_baseline() {
    let program = Program::new();
    let mut reference = None;
    for height in [9, 12, 18, 36, 64] {
        let mut context = FontHeightContext(height);
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let (_, graphics, pixels) = unicode_graphics_target(&mut machine, 8, 24, 80);
        machine
            .graphics_unicode_draw_text(
                graphics,
                TextSource::Utf16(&['A' as u16, 'α' as u16]),
                0,
                72,
                4 | 64,
            )
            .unwrap();
        let actual = machine.graphics_int_array_snapshot(pixels).unwrap();
        if let Some(expected) = &reference {
            assert_eq!(&actual, expected, "height={height}");
        } else {
            reference = Some(actual);
        }
    }
}

#[test]
fn unicode_text_checks_cancellation_before_rendering_long_strings() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(2));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at,
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (_, graphics, pixels) = unicode_graphics_target(&mut machine, 0, 104, 16);
    let string = machine
        .intern_string_units(vec!['真' as u16; 4_096], &[], &[])
        .unwrap();
    let error = machine
        .graphics_unicode_draw_text(graphics, TextSource::JavaString(string), 0, 0, 20)
        .unwrap_err();
    assert_eq!(error.code(), "execution-cancelled");
    assert_eq!(checks.get(), 2);
    assert!(
        machine
            .graphics_int_array_snapshot(pixels)
            .unwrap()
            .iter()
            .all(|pixel| *pixel == 0)
    );
}

#[test]
fn unicode_text_can_cancel_during_rasterization_and_render_again() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (_, graphics, pixels) = unicode_graphics_target(&mut machine, 0, 16_384, 16);
    let string = machine
        .intern_string_units(vec!['真' as u16; 2_048], &[], &[])
        .unwrap();
    machine
        .graphics_unicode_draw_text(graphics, TextSource::JavaString(string), 0, 0, 20)
        .unwrap();
    let total_checks = checks.get();
    assert!(
        total_checks > 4,
        "rasterization did not poll for cancellation"
    );
    let complete = machine.graphics_int_array_snapshot(pixels).unwrap();

    for point in [total_checks / 2, total_checks] {
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(0));
        checks.set(0);
        cancel_at.set(point);
        assert_eq!(
            machine
                .graphics_unicode_draw_text(graphics, TextSource::JavaString(string), 0, 0, 20)
                .unwrap_err()
                .code(),
            "execution-cancelled",
        );
        let partial = machine.graphics_int_array_snapshot(pixels).unwrap();
        assert!(partial.iter().any(|pixel| *pixel != 0));
        assert_ne!(partial, complete);
    }

    checks.set(0);
    cancel_at.set(usize::MAX);
    machine
        .graphics_int_array_mut(pixels)
        .unwrap()
        .fill(HeapValue::Int(0));
    machine
        .graphics_unicode_draw_text(graphics, TextSource::JavaString(string), 0, 0, 20)
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        complete
    );
}

#[test]
fn unicode_text_keeps_alignment_and_translation_when_clipped() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let text = [
        'A' as u16,
        'Б' as u16,
        'ê' as u16,
        'α' as u16,
        '真' as u16,
        0xd800,
    ];
    let string = machine
        .intern_string_units(text.to_vec(), &[], &[])
        .unwrap();
    for size in [8, 0, 16] {
        let (height, advance) = lcd_ui_font_dimensions(size);
        let width = advance * 5 + height;
        let (_, reference, reference_pixels) = unicode_graphics_target(&mut machine, size, 128, 48);
        machine
            .graphics_unicode_draw_text(reference, TextSource::Utf16(&text), 8, 7, 20)
            .unwrap();
        let expected = machine
            .graphics_int_array_snapshot(reference_pixels)
            .unwrap();
        assert!(expected.iter().any(|pixel| *pixel != 0));
        for (horizontal, x_offset) in [(4, 0), (1, width / 2), (8, width)] {
            for (vertical, y_offset) in [(16, 0), (32, height), (64, height - 1)] {
                let (_, graphics, pixels) = unicode_graphics_target(&mut machine, size, 128, 48);
                for (field, value) in [
                    ("tx", -3),
                    ("ty", 4),
                    ("clipX", 13),
                    ("clipY", 10),
                    ("clipW", 54),
                    ("clipH", 11),
                ] {
                    machine
                        .heap
                        .managed
                        .set_field(
                            graphics,
                            &format!("javax/microedition/lcdui/Graphics.{field}:I"),
                            HeapValue::Int(value),
                        )
                        .unwrap();
                }
                machine
                    .graphics_unicode_draw_text(
                        graphics,
                        TextSource::JavaString(string),
                        11 + x_offset,
                        3 + y_offset,
                        horizontal | vertical,
                    )
                    .unwrap();
                let actual = machine.graphics_int_array_snapshot(pixels).unwrap();
                for (index, pixel) in actual.into_iter().enumerate() {
                    let in_clip =
                        (13..67).contains(&(index % 128)) && (10..21).contains(&(index / 128));
                    assert_eq!(
                        pixel,
                        if in_clip { expected[index] } else { 0 },
                        "size={size}, anchor={}, pixel={index}",
                        horizontal | vertical
                    );
                }
            }
        }
    }
}

#[test]
fn left_aligned_text_does_not_measure_the_clipped_suffix() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::new(Cell::new(usize::MAX)),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (_, graphics, pixels) = unicode_graphics_target(&mut machine, 0, 104, 16);
    let prefix = ['A' as u16, '真' as u16, 0xd800].repeat(16);
    let mut extended = prefix.clone();
    extended.resize(65_536, '大' as u16);
    for anchor in [0, 20] {
        let mut reference = None;
        for text in [&prefix, &extended] {
            checks.set(0);
            machine
                .graphics_int_array_mut(pixels)
                .unwrap()
                .fill(HeapValue::Int(0));
            machine
                .graphics_unicode_draw_text(graphics, TextSource::Utf16(text), 0, 0, anchor)
                .unwrap();
            let actual = machine.graphics_int_array_snapshot(pixels).unwrap();
            assert!(actual.iter().any(|pixel| *pixel != 0));
            let result = (actual, checks.get());
            if let Some(expected) = &reference {
                assert_eq!(&result, expected);
            } else {
                reference = Some(result);
            }
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn unicode_font_raster_throughput() {
    let program = Program::new();
    for character in ['A', 'α', '真'] {
        for (size, height) in [(8, 8), (8, 9), (0, 12), (16, 16), (0, 64)] {
            for length in [1, 16] {
                let mut context = FontHeightContext(height);
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let (_, graphics, pixels) = unicode_graphics_target(&mut machine, size, 240, 80);
                let text = vec![character as u16; length];
                let started = std::time::Instant::now();
                for _ in 0..4096 {
                    machine
                        .graphics_unicode_draw_text(
                            graphics,
                            TextSource::Utf16(std::hint::black_box(&text)),
                            0,
                            0,
                            20,
                        )
                        .unwrap();
                }
                let elapsed = started.elapsed();
                let checksum = machine
                    .graphics_int_array_snapshot(pixels)
                    .unwrap()
                    .iter()
                    .fold(0xcbf2_9ce4_8422_2325_u64, |sum, pixel| {
                        sum.wrapping_mul(0x0000_0100_0000_01b3) ^ u64::from(pixel.cast_unsigned())
                    });
                eprintln!(
                    "font character={character} size={size} height={height} length={length} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn unicode_text_throughput() {
    use std::hint::black_box;
    use std::time::Instant;

    let program = Program::new();
    for character in ['A', '真'] {
        for length in [16, 4_096] {
            for anchor in [20, 17, 24] {
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let (_, graphics, pixels) = unicode_graphics_target(&mut machine, 0, 240, 20);
                let string = machine
                    .intern_string_units(vec![character as u16; length], &[], &[])
                    .unwrap();
                let (height, narrow_width) = machine.active_lcd_ui_font_dimensions(0);
                let width = length as i32
                    * if character == 'A' {
                        narrow_width
                    } else {
                        height
                    };
                let x = match anchor {
                    17 => width / 2,
                    24 => width,
                    _ => 0,
                };
                let started = Instant::now();
                for _ in 0..1_024 {
                    machine
                        .graphics_unicode_draw_text(
                            graphics,
                            TextSource::JavaString(black_box(string)),
                            black_box(x),
                            0,
                            black_box(anchor),
                        )
                        .unwrap();
                }
                let elapsed = started.elapsed();
                let checksum = machine
                    .graphics_int_array_snapshot(pixels)
                    .unwrap()
                    .into_iter()
                    .fold(0_u64, |sum, pixel| {
                        sum.rotate_left(1) ^ u64::from(pixel.cast_unsigned())
                    });
                eprintln!(
                    "text character={character} length={length} anchor={anchor} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}

fn unicode_graphics_target(
    machine: &mut Machine<'_, '_>,
    size: i32,
    width: i32,
    height: i32,
) -> (Handle, Handle, Handle) {
    let font = machine
        .heap
        .managed
        .allocate_object(
            "javax/microedition/lcdui/Font",
            HashMap::from([(
                "javax/microedition/lcdui/Font.size:I".into(),
                HeapValue::Int(size),
            )]),
        )
        .unwrap();
    let target_pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, width * height)
        .unwrap();
    let target_image = machine
        .heap
        .managed
        .allocate_object(
            "javax/microedition/lcdui/Image",
            HashMap::from([
                (
                    "javax/microedition/lcdui/Image.width:I".into(),
                    HeapValue::Int(width),
                ),
                (
                    "javax/microedition/lcdui/Image.height:I".into(),
                    HeapValue::Int(height),
                ),
                (
                    "javax/microedition/lcdui/Image.pixels:[I".into(),
                    HeapValue::Reference(Some(target_pixels)),
                ),
            ]),
        )
        .unwrap();
    let graphics = machine
        .heap
        .managed
        .allocate_object(
            "javax/microedition/lcdui/Graphics",
            HashMap::from([
                (
                    "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;"
                        .into(),
                    HeapValue::Reference(Some(target_image)),
                ),
                (
                    "javax/microedition/lcdui/Graphics.font:Ljavax/microedition/lcdui/Font;".into(),
                    HeapValue::Reference(Some(font)),
                ),
                (
                    "javax/microedition/lcdui/Graphics.color:I".into(),
                    HeapValue::Int(-1),
                ),
                (
                    "javax/microedition/lcdui/Graphics.tx:I".into(),
                    HeapValue::Int(0),
                ),
                (
                    "javax/microedition/lcdui/Graphics.ty:I".into(),
                    HeapValue::Int(0),
                ),
                (
                    "javax/microedition/lcdui/Graphics.clipX:I".into(),
                    HeapValue::Int(0),
                ),
                (
                    "javax/microedition/lcdui/Graphics.clipY:I".into(),
                    HeapValue::Int(0),
                ),
                (
                    "javax/microedition/lcdui/Graphics.clipW:I".into(),
                    HeapValue::Int(width),
                ),
                (
                    "javax/microedition/lcdui/Graphics.clipH:I".into(),
                    HeapValue::Int(height),
                ),
            ]),
        )
        .unwrap();

    (font, graphics, target_pixels)
}

#[test]
pub(crate) fn embedded_unifont_has_distinct_cjk_glyphs() {
    let expected = ['真', '大', '圣', '斩', '魔', '录', '影', '忍', '传'];
    let glyphs = expected
        .into_iter()
        .map(|character| unifont_bmp_glyph(character as u16).unwrap())
        .collect::<Vec<_>>();
    assert!(glyphs.iter().all(|glyph| glyph.width == 16));
    assert_eq!(
        glyphs
            .iter()
            .map(|glyph| glyph.rows)
            .collect::<HashSet<_>>()
            .len(),
        expected.len()
    );
}

#[test]
pub(crate) fn small_unicode_downsampling_preserves_relative_font_metrics() {
    let capital = unifont_bmp_glyph('Б' as u16).unwrap();
    let lowercase = unifont_bmp_glyph('а' as u16).unwrap();
    let accented = unifont_bmp_glyph('ё' as u16).unwrap();
    let descender = unifont_bmp_glyph('Д' as u16).unwrap();
    let occupied_rows = |glyph: &UnicodeBitmapGlyph, height| {
        (0..height)
            .filter(|row| lcd_ui_unicode_row(glyph, *row, height).is_some_and(|bits| bits != 0))
            .collect::<Vec<_>>()
    };

    assert_eq!(occupied_rows(&capital, 8), [1, 2, 3, 4, 5, 6]);
    assert_eq!(occupied_rows(&lowercase, 8), [2, 3, 4, 5, 6]);
    assert_eq!(occupied_rows(&accented, 8), [0, 2, 3, 4, 5, 6]);
    assert_eq!(occupied_rows(&descender, 8), [1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(occupied_rows(&capital, 9), [2, 3, 4, 5, 6, 7]);
    assert_eq!(occupied_rows(&lowercase, 9), [3, 4, 5, 6, 7]);
    assert_eq!(occupied_rows(&accented, 9), [0, 1, 3, 4, 5, 6, 7]);
    assert_eq!(occupied_rows(&descender, 9), [2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(lcd_ui_font_dimensions(8), (8, 6));
}

#[test]
fn unicode_column_sampling_matches_every_source_bit_at_all_font_widths() {
    for source_width in 1..=16 {
        for target_width in 1..=64 {
            for column in 0..target_width {
                let start = column * source_width / target_width;
                let end = ((column + 1) * source_width / target_width)
                    .max(start + 1)
                    .min(source_width);
                for source_column in 0..16 {
                    let row = 1_u16 << (15 - source_column);
                    assert_eq!(
                        lcd_ui_unicode_column_is_set(row, source_width, column, target_width),
                        (start..end).contains(&source_column),
                        "source={source_width}, target={target_width}, column={column}, bit={source_column}"
                    );
                }
            }
        }
    }
    for (column, range) in [0..2, 2..3, 3..5, 5..6, 6..8].into_iter().enumerate() {
        for source_column in 0..16 {
            assert_eq!(
                lcd_ui_small_unicode_column_is_set(1 << (15 - source_column), column as i32),
                range.contains(&source_column)
            );
        }
    }
}

#[test]
pub(crate) fn narrow_unicode_downsampling_keeps_skipped_source_columns() {
    assert!(lcd_ui_unicode_column_is_set(1 << 12, 8, 2, 6));
    assert!(lcd_ui_unicode_column_is_set(1 << 8, 8, 5, 6));
    assert!(!lcd_ui_unicode_column_is_set(1 << 12, 8, 3, 6));
}

#[test]
pub(crate) fn profiled_small_font_keeps_narrow_unicode_in_the_compact_cell() {
    assert_eq!(lcd_ui_unicode_raster_height(8, 8, 8), 8);
    assert_eq!(lcd_ui_unicode_raster_height(8, 8, 9), 9);
    assert_eq!(lcd_ui_unicode_raster_height(8, 8, 12), 9);
    assert_eq!(lcd_ui_unicode_raster_height(8, 16, 12), 12);
    assert_eq!(lcd_ui_unicode_raster_height(0, 8, 15), 15);

    // The compact fallback uses five ink columns and leaves the sixth
    // character-cell column clear, just like the built-in 5x7 alphabet.
    assert!(lcd_ui_small_unicode_column_is_set(1 << 14, 0));
    assert!(lcd_ui_small_unicode_column_is_set(1 << 12, 2));
    assert!(lcd_ui_small_unicode_column_is_set(1 << 9, 4));
    assert!(!lcd_ui_small_unicode_column_is_set(u16::MAX, 5));
}

#[test]
pub(crate) fn unicode_fallback_mixes_lcd_ui_latin_with_cjk_without_host_fonts() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (font, graphics, target_pixels) = unicode_graphics_target(&mut machine, 0, 104, 16);

    assert_eq!(
        machine
            .unicode_fallback_char_width(font, '真' as u16)
            .unwrap(),
        Some(12)
    );
    assert_eq!(
        machine
            .unicode_fallback_char_width(font, 'A' as u16)
            .unwrap(),
        None
    );
    assert_eq!(
        machine
            .unicode_fallback_char_width(font, 'ç' as u16)
            .unwrap(),
        Some(8)
    );
    let french = "Français".encode_utf16().collect::<Vec<_>>();
    machine
        .graphics_unicode_draw_text(graphics, TextSource::Utf16(&french), 0, 0, 20)
        .unwrap();
    let expected_f = graphics::compact_lcd_ui_font_glyph('F').unwrap();
    let Allocation::Array { elements, .. } = machine.heap.managed.get(target_pixels).unwrap()
    else {
        panic!("target pixels must be an int array");
    };
    for row in 0..12 {
        for column in 0..8 {
            let expected = (4..11).contains(&row)
                && column < 5
                && expected_f[row - 4] & (1_u8 << (4 - column)) != 0;
            assert_eq!(elements[row * 104 + column] == HeapValue::Int(-1), expected);
        }
    }
    machine
        .graphics_unicode_draw_text(graphics, TextSource::Utf16(&['Б' as u16]), 64, 0, 20)
        .unwrap();
    let Allocation::Array { elements, .. } = machine.heap.managed.get(target_pixels).unwrap()
    else {
        panic!("target pixels must be an int array");
    };
    let expected_be = graphics::compact_lcd_ui_font_glyph('Б').unwrap();
    for row in 0..12 {
        for column in 0..8 {
            let expected = (4..11).contains(&row)
                && column < 5
                && expected_be[row - 4] & (1_u8 << (4 - column)) != 0;
            assert_eq!(
                elements[row * 104 + 64 + column] == HeapValue::Int(-1),
                expected
            );
        }
    }
    machine
        .graphics_unicode_draw_text(
            graphics,
            TextSource::Utf16(&['真' as u16, '大' as u16]),
            72,
            0,
            20,
        )
        .unwrap();
    let Allocation::Array { elements, .. } = machine.heap.managed.get(target_pixels).unwrap()
    else {
        panic!("target pixels must be an int array");
    };
    let cjk_cells = [72, 84].map(|left| {
        (0..16)
            .flat_map(|row| {
                elements[row * 104 + left..row * 104 + left + 12]
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>()
    });
    for cell in &cjk_cells {
        assert!(
            cell.contains(&HeapValue::Int(-1)),
            "CJK glyph must be drawn"
        );
    }
    assert_ne!(cjk_cells[0], cjk_cells[1]);
    machine
        .graphics_unicode_draw_text(graphics, TextSource::Utf16(&['Ê' as u16]), 96, 0, 20)
        .unwrap();
    let expected_e_circumflex = graphics::compact_lcd_ui_font_glyph('Ê').unwrap();
    let Allocation::Array { elements, .. } = machine.heap.managed.get(target_pixels).unwrap()
    else {
        panic!("target pixels must be an int array");
    };
    for row in 0..12 {
        for column in 0..8 {
            let expected = (4..11).contains(&row)
                && column < 5
                && expected_e_circumflex[row - 4] & (1_u8 << (4 - column)) != 0;
            assert_eq!(
                elements[row * 104 + 96 + column] == HeapValue::Int(-1),
                expected
            );
        }
    }
}
