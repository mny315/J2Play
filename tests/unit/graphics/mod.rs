use super::font::UNKNOWN_GLYPH;
use super::*;
use sha2::{Digest, Sha256};

mod clipping;

#[test]
fn clip_translate_and_primitives_are_stable() {
    let mut f = Framebuffer::new(12, 10).unwrap();
    {
        let mut g = f.graphics();
        g.set_color(0x112233);
        g.fill_rect(0, 0, 12, 10);
        g.translate(2, 1);
        g.set_clip(1, 1, 7, 6);
        g.clip_rect(2, 2, 4, 3);
        g.set_color(0xff0000);
        g.fill_rect(-20, -20, 50, 50);
        g.set_color(0x00ff00);
        g.draw_line(0, 0, 8, 6);
    }
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(
                f.pixels()
                    .iter()
                    .flat_map(|p| p.to_be_bytes())
                    .collect::<Vec<_>>()
            )
        ),
        "ca5a395da804aad5d3f1b11e2e641ee9ee448f59a2e1e92317237ddf3848bd43"
    );
}

#[test]
fn all_transforms_use_asymmetric_matrix() {
    let image = Image::from_argb(
        &[
            0xff010000, 0xff020000, 0xff030000, 0xff040000, 0xff050000, 0xff060000,
        ],
        2,
        3,
        true,
    )
    .unwrap();
    let expected = [
        vec![1, 2, 3, 4, 5, 6],
        vec![5, 6, 3, 4, 1, 2],
        vec![2, 1, 4, 3, 6, 5],
        vec![6, 5, 4, 3, 2, 1],
        vec![1, 3, 5, 2, 4, 6],
        vec![5, 3, 1, 6, 4, 2],
        vec![2, 4, 6, 1, 3, 5],
        vec![6, 4, 2, 5, 3, 1],
    ];
    for (n, t) in [
        Transform::None,
        Transform::MirrorRot180,
        Transform::Mirror,
        Transform::Rot180,
        Transform::MirrorRot270,
        Transform::Rot90,
        Transform::Rot270,
        Transform::MirrorRot90,
    ]
    .into_iter()
    .enumerate()
    {
        let swapped = n >= 4;
        let mut f =
            Framebuffer::new(if swapped { 3 } else { 2 }, if swapped { 2 } else { 3 }).unwrap();
        f.graphics()
            .draw_region(&image, 0, 0, 2, 3, t, 0, 0, 0)
            .unwrap();
        assert_eq!(
            f.pixels()
                .iter()
                .map(|p| (p >> 16) & 255)
                .collect::<Vec<_>>(),
            expected[n]
        );
    }
}

#[test]
fn image_alpha_blends_against_the_framebuffer() {
    let image = Image::from_argb(&[0x8000_0000], 1, 1, true).unwrap();
    let mut f = Framebuffer::new(1, 1).unwrap();
    f.graphics().draw_image(&image, 0, 0, 0).unwrap();
    assert_eq!(f.pixels()[0], 0xff7f_7f7f);
}

#[test]
fn font_metrics_measure_text_and_saturate_at_the_pixel_limit() {
    assert_eq!(Font::system_small().string_width("ABC"), 18);
    let font = Font {
        char_width: u32::MAX,
        ..Font::system_small()
    };
    assert_eq!(font.string_width(""), 0);
    assert_eq!(font.string_width("я🙂"), u32::MAX);
}

#[test]
fn system_font_covers_latin_text_and_all_decimal_digits() {
    for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,:;!+-/()[]".chars() {
        assert_ne!(glyph(ch), UNKNOWN_GLYPH, "missing glyph for {ch:?}");
    }
    for (lower, upper) in ('a'..='z').zip('A'..='Z') {
        assert_eq!(glyph(lower), glyph(upper));
    }
    let digits = ('0'..='9').map(glyph).collect::<Vec<_>>();
    for (index, left) in digits.iter().enumerate() {
        for right in &digits[index + 1..] {
            assert_ne!(left, right);
        }
    }
    for (lower, upper) in ('а'..='я').zip('А'..='Я') {
        assert_eq!(glyph(lower), glyph(upper));
        assert_ne!(glyph(lower), UNKNOWN_GLYPH);
    }
    assert_eq!(glyph('ё'), glyph('Ё'));
    assert_ne!(glyph('ё'), UNKNOWN_GLYPH);
}

#[test]
fn system_font_covers_common_western_european_diacritics() {
    for (lower, upper) in [
        ('à', 'À'),
        ('á', 'Á'),
        ('â', 'Â'),
        ('ã', 'Ã'),
        ('č', 'Č'),
        ('ç', 'Ç'),
        ('é', 'É'),
        ('ê', 'Ê'),
        ('ì', 'Ì'),
        ('í', 'Í'),
        ('ñ', 'Ñ'),
        ('ó', 'Ó'),
        ('ô', 'Ô'),
        ('õ', 'Õ'),
        ('š', 'Š'),
        ('ú', 'Ú'),
    ] {
        assert_eq!(system_font_glyph(lower), system_font_glyph(upper));
        assert!(system_font_glyph(lower).is_some());
        assert_ne!(glyph(lower), UNKNOWN_GLYPH);
    }
    assert_eq!(
        system_font_glyph('Ê'),
        Some([
            0b01010, 0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ])
    );
    assert_eq!(
        system_font_glyph('Č'),
        Some([
            0b01010, 0b01110, 0b10001, 0b10000, 0b10000, 0b10001, 0b01110,
        ])
    );
    assert_eq!(
        system_font_glyph('Š'),
        Some([
            0b01010, 0b01111, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ])
    );
}

#[test]
fn lcd_ui_keeps_cyrillic_in_the_compact_system_font_cell() {
    assert_eq!(compact_lcd_ui_font_glyph('Б'), system_font_glyph('Б'));
    assert_eq!(compact_lcd_ui_font_glyph('ё'), system_font_glyph('Ё'));
    assert_eq!(compact_lcd_ui_font_glyph('у'), system_font_glyph('У'));
    assert_eq!(compact_lcd_ui_font_glyph('A'), system_font_glyph('A'));
}

#[test]
fn copy_area_preserves_overlapping_sources_with_clipping_translation_and_anchors() {
    let original = (0..42).map(|pixel| 0xff00_0000 | pixel).collect::<Vec<_>>();
    for (tx, ty) in [(0, 0), (2, -1)] {
        for (anchor, anchor_x, anchor_y) in [(0, 0, 0), (3, 2, 2), (40, 5, 4)] {
            for clip in [(0, 0, 7, 6), (2, 2, 3, 2)] {
                for dest_y in -4..=6 {
                    for dest_x in -5..=7 {
                        let mut image = Image::mutable(7, 6).unwrap();
                        image.pixels.copy_from_slice(&original);
                        let mut expected = original.clone();
                        for row in 0..4 {
                            for column in 0..5 {
                                let px = dest_x + column;
                                let py = dest_y + row;
                                if px >= clip.0
                                    && py >= clip.1
                                    && px < clip.0 + clip.2
                                    && py < clip.1 + clip.3
                                {
                                    expected[(py * 7 + px) as usize] =
                                        original[((row + 1) * 7 + column + 1) as usize];
                                }
                            }
                        }
                        let mut graphics = image.graphics().unwrap();
                        graphics.translate(tx, ty);
                        graphics.set_clip(clip.0 - tx, clip.1 - ty, clip.2, clip.3);
                        graphics
                            .copy_area(
                                1 - tx,
                                1 - ty,
                                5,
                                4,
                                dest_x + anchor_x - tx,
                                dest_y + anchor_y - ty,
                                anchor,
                            )
                            .unwrap();
                        assert_eq!(image.pixels(), expected);
                    }
                }
            }
        }
    }
}

#[test]
fn complete_primitive_scene_has_golden_hash() {
    let mut framebuffer = Framebuffer::new(32, 24).unwrap();
    {
        let mut graphics = framebuffer.graphics();
        graphics.set_color(0x0010_2030);
        graphics.fill_rect(0, 0, 32, 24);
        graphics.set_color(0x00ff_8800);
        graphics.draw_rect(1, 1, 12, 8);
        graphics.fill_triangle(2, 20, 15, 5, 20, 21);
        graphics.set_color(0x0000_ccff);
        graphics.draw_round_rect(14, 1, 16, 10, 6, 6);
        graphics.fill_arc(20, 12, 10, 9, 30, 240);
        graphics.set_stroke_style(StrokeStyle::Dotted);
        graphics.draw_line(0, 23, 31, 0);
        graphics
            .draw_rgb(
                &[0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff],
                0,
                2,
                0,
                10,
                2,
                2,
                true,
            )
            .unwrap();
        graphics.copy_area(0, 10, 2, 2, 31, 24, 40).unwrap();
    }
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(
                framebuffer
                    .pixels()
                    .iter()
                    .flat_map(|pixel| pixel.to_be_bytes())
                    .collect::<Vec<_>>()
            )
        ),
        "7c2f8aefe15a2d423a7b21891293aaf5566e2c2c6d07e443e47d12c0e0a1c5d0"
    );
}

#[test]
fn hostile_coordinates_are_clipped_without_panicking() {
    let mut framebuffer = Framebuffer::new(4, 4).unwrap();
    let mut graphics = framebuffer.graphics();
    graphics.translate(i32::MAX, i32::MAX);
    graphics.translate(1, 1);
    graphics.set_clip(i32::MIN, i32::MIN, i32::MAX, i32::MAX);
    let _ = graphics.clip();
    graphics.draw_line(i32::MIN, i32::MIN, i32::MAX, i32::MAX);
    graphics.draw_rect(i32::MAX, i32::MAX, i32::MAX, i32::MAX);
    graphics.draw_round_rect(i32::MAX, i32::MIN, i32::MAX, i32::MAX, i32::MAX, i32::MAX);
    graphics.fill_round_rect(i32::MIN, i32::MAX, i32::MAX, i32::MAX, i32::MAX, i32::MAX);
    graphics.draw_char('X', i32::MAX, i32::MAX);
    graphics.draw_string("XX", i32::MAX, i32::MAX);
    assert!(anchor_origin(i32::MIN, i32::MIN, i32::MAX, i32::MAX, 40).is_ok());

    let mut framebuffer = Framebuffer::new(4, 4).unwrap();
    {
        let mut graphics = framebuffer.graphics();
        graphics.set_color(0x12_34_56);
        graphics.set_clip(0, 3, 4, 1);
        graphics.draw_line(i32::MIN, i32::MIN, i32::MAX, i32::MAX);
    }
    assert_eq!(framebuffer.pixels()[15], 0xff12_3456);

    {
        let mut graphics = framebuffer.graphics();
        graphics.set_color(0x65_43_21);
        graphics.fill_triangle(i32::MIN, i32::MIN, i32::MAX, i32::MIN, 0, i32::MAX);
    }
    assert!(
        framebuffer
            .pixels()
            .iter()
            .all(|pixel| *pixel == 0xff65_4321)
    );
}

#[test]
fn signed_rgb_scanlines_reverse_row_order() {
    let pixels = [0xff01_0000, 0xff02_0000, 0xff03_0000, 0xff04_0000];
    let mut framebuffer = Framebuffer::new(2, 2).unwrap();
    framebuffer
        .graphics()
        .draw_rgb(&pixels, 2, -2, 0, 0, 2, 2, true)
        .unwrap();
    assert_eq!(
        framebuffer.pixels(),
        &[0xff03_0000, 0xff04_0000, 0xff01_0000, 0xff02_0000]
    );
}

#[test]
fn degenerate_triangle_and_conflicting_anchors_are_controlled() {
    let mut framebuffer = Framebuffer::new(5, 2).unwrap();
    let mut graphics = framebuffer.graphics();
    graphics.set_color(0x12_34_56);
    graphics.fill_triangle(1, 1, 4, 1, 2, 1);
    assert_eq!(&graphics.target.pixels[6..10], &[0xff12_3456; 4]);
    let image = Image::from_argb(&[0xffff_ffff], 1, 1, true).unwrap();
    assert!(graphics.draw_image(&image, 0, 0, 1 | 4).is_err());
    assert!(graphics.draw_image(&image, 0, 0, 2 | 16).is_err());
}

#[test]
fn individual_operation_png_goldens() {
    fn snapshot(draw: impl FnOnce(&mut Graphics<'_>)) -> String {
        let mut framebuffer = Framebuffer::new(16, 12).unwrap();
        draw(&mut framebuffer.graphics());
        assert!(
            framebuffer
                .pixels()
                .iter()
                .any(|pixel| *pixel != 0xffff_ffff),
            "each golden operation must draw visible pixels"
        );
        format!("{:x}", Sha256::digest(framebuffer.to_png()))
    }
    let tile = Image::from_argb(
        &[
            0xff01_0000,
            0xff02_0000,
            0xff03_0000,
            0xff04_0000,
            0xff05_0000,
            0xff06_0000,
        ],
        2,
        3,
        true,
    )
    .unwrap();
    let hashes = vec![
        snapshot(|g| {
            g.set_color(0xff0000);
            g.draw_rect(1, 1, 8, 6);
            g.fill_rect(3, 3, 4, 2);
        }),
        snapshot(|g| {
            g.set_color(0x00ff00);
            g.draw_round_rect(1, 1, 12, 9, 5, 5);
            g.fill_round_rect(5, 3, 8, 7, 4, 4);
        }),
        snapshot(|g| {
            g.set_color(0x0000ff);
            g.draw_arc(1, 1, 10, 8, 30, 240);
            g.fill_arc(7, 4, 8, 7, -45, 180);
        }),
        snapshot(|g| {
            g.set_color(0xffff00);
            g.fill_triangle(1, 10, 7, 1, 14, 10);
        }),
        snapshot(|g| {
            g.draw_rgb(
                &[0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff],
                2,
                -2,
                0,
                0,
                2,
                2,
                true,
            )
            .unwrap();
        }),
        snapshot(|g| {
            g.set_color(0xabcdef);
            g.fill_rect(0, 0, 4, 3);
            g.copy_area(0, 0, 4, 3, 16, 12, 40).unwrap();
        }),
        snapshot(|g| {
            g.draw_image(&tile, 1, 1, 0).unwrap();
            g.draw_region(&tile, 0, 0, 2, 3, Transform::Rot90, 8, 2, 0)
                .unwrap();
        }),
        snapshot(|g| {
            g.set_color(0x000000);
            g.draw_string("ABC", 2, 2);
        }),
        snapshot(|g| {
            g.translate(2, 1);
            g.set_clip(1, 1, 8, 7);
            g.clip_rect(2, 2, 4, 3);
            g.set_color(0xff00ff);
            g.fill_rect(-20, -20, 50, 50);
        }),
    ];
    assert_eq!(
        hashes,
        [
            "d3e421c8cfbf713c4f7de6b38ae00253cf5356a060fab529d40f9bfcfd5dffcc",
            "c020176892b210ae60637e736dbbcb4b9f12f08907aa9d7d67ab3c42995cea94",
            "62a283f40d72c53b13937bb08765a17d55e185e8b23200697817c84a349e9a76",
            "7e97e0f786aa27b1207d742d5ebb1e7f26523096a4b8c1363f6833636b31ce58",
            "95215bb1a3bfd22c4ecf140272ea5845e3fc80ac7ccadada8281bbf2d48e78f1",
            "eaab261b7a0b70e44a5b9faead882e80df078c26513a1213f777f50e4a0782e4",
            "dcef10715d39a0c6b06fe0d4b579c7bb9fbb30317ff6bc7a1f2087f905192498",
            "4c6f644196cf206c05d5935fc178451d0dd07e938ea0891c574dfa4709f4b76d",
            "435105fe536e6805f57e694e12f9cedba05d42b3857f8559c8d55e17f9df212a",
        ]
    );
}
