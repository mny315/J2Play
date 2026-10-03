use super::*;

#[test]
fn clipped_lines_match_direct_rasterization_in_every_octant() {
    fn direct(start: (i32, i32), end: (i32, i32)) -> Vec<(i32, i32, u64)> {
        let (mut x, mut y) = start;
        let dx = (end.0 - x).abs();
        let dy = -(end.1 - y).abs();
        let sx = if x < end.0 { 1 } else { -1 };
        let sy = if y < end.1 { 1 } else { -1 };
        let mut error = dx + dy;
        let mut pixels = Vec::new();
        loop {
            pixels.push((x, y, pixels.len() as u64));
            if (x, y) == end {
                return pixels;
            }
            let twice = error * 2;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
        }
    }

    for x0 in [-8, -1, 0, 7, 15] {
        for y0 in [-8, -1, 0, 7, 15] {
            for x1 in -8..=15 {
                for y1 in -8..=15 {
                    let expected = direct((x0, y0), (x1, y1));
                    for clip in [
                        Rect {
                            x: 0,
                            y: 0,
                            width: 8,
                            height: 8,
                        },
                        Rect {
                            x: -2,
                            y: 3,
                            width: 13,
                            height: 1,
                        },
                        Rect {
                            x: 3,
                            y: -2,
                            width: 1,
                            height: 13,
                        },
                        Rect {
                            x: 0,
                            y: 0,
                            width: 0,
                            height: 8,
                        },
                    ] {
                        let visible = expected
                            .iter()
                            .copied()
                            .filter(|&(x, y, _)| {
                                x >= clip.x
                                    && y >= clip.y
                                    && x < clip.x + clip.width
                                    && y < clip.y + clip.height
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(
                            clipped_line_points((x0, y0), (x1, y1), clip).collect::<Vec<_>>(),
                            visible,
                            "{x0}/{y0} to {x1}/{y1}, {clip:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn clipped_lines_jump_over_extreme_hidden_spans() {
    let clip = Rect {
        x: 0,
        y: 0,
        width: 4,
        height: 4,
    };
    assert_eq!(
        clipped_line_points((i32::MIN, i32::MIN), (i32::MAX, i32::MAX), clip).collect::<Vec<_>>(),
        (0..4)
            .map(|i| (i, i, 2_147_483_648 + i as u64))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        clipped_line_points((i32::MAX, i32::MAX), (i32::MIN, i32::MIN), clip).collect::<Vec<_>>(),
        (0..4)
            .rev()
            .map(|i| (i, i, 2_147_483_647 - i as u64))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        clipped_line_points((i32::MIN, 2), (i32::MAX, 2), clip).collect::<Vec<_>>(),
        (0..4)
            .map(|i| (i, 2, 2_147_483_648 + i as u64))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        clipped_line_points((2, i32::MAX), (2, i32::MIN), clip).collect::<Vec<_>>(),
        (0..4)
            .rev()
            .map(|i| (2, i, 2_147_483_647 - i as u64))
            .collect::<Vec<_>>()
    );
    for coordinate in [i32::MIN, i32::MAX] {
        let clip = Rect {
            x: coordinate,
            y: coordinate,
            width: 1,
            height: 1,
        };
        assert_eq!(
            clipped_line_points((i32::MIN, i32::MIN), (i32::MAX, i32::MAX), clip)
                .collect::<Vec<_>>(),
            [(
                coordinate,
                coordinate,
                (i64::from(coordinate) - i64::from(i32::MIN)) as u64
            )]
        );
    }
}

#[test]
fn long_lines_preserve_visible_pixels_and_dotted_phase_when_clipped() {
    for first in 0..16 {
        for last in 0..16 {
            for (start, end) in [
                ((-256, first), (256, last)),
                ((256, last), (-256, first)),
                ((first, -256), (last, 256)),
                ((last, 256), (first, -256)),
            ] {
                for style in [StrokeStyle::Solid, StrokeStyle::Dotted] {
                    let mut full = Framebuffer::new(16, 16).unwrap();
                    let mut clipped = Framebuffer::new(16, 16).unwrap();
                    for (frame, apply_clip) in [(&mut full, false), (&mut clipped, true)] {
                        let mut graphics = frame.graphics();
                        graphics.set_color(0x123456);
                        graphics.set_stroke_style(style);
                        if apply_clip {
                            graphics.set_clip(3, 2, 8, 11);
                        }
                        graphics.draw_line(start.0, start.1, end.0, end.1);
                    }
                    for y in 0..16 {
                        for x in 0..16 {
                            let index = y * 16 + x;
                            let expected = if (3..11).contains(&x) && (2..13).contains(&y) {
                                full.pixels()[index]
                            } else {
                                0xffff_ffff
                            };
                            assert_eq!(
                                clipped.pixels()[index],
                                expected,
                                "{start:?} to {end:?}, {style:?}, pixel={x}/{y}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn rgb_with_repeated_rows_only_visits_the_visible_destination() {
    let mut framebuffer = Framebuffer::new(3, 3).unwrap();
    let pixel = 0xff12_3456;
    framebuffer
        .graphics()
        .draw_rgb(&[pixel], 0, 0, 1, i32::MIN, 1, u32::MAX, true)
        .unwrap();
    assert_eq!(
        framebuffer.pixels(),
        &[0xffff_ffff, pixel, 0xffff_ffff].repeat(3)
    );
    let mut graphics = framebuffer.graphics();
    graphics.set_clip(i32::MAX, 0, 1, 3);
    assert_eq!(
        graphics
            .draw_rgb(&[pixel], 0, 1, 0, 0, 1, 2, true)
            .unwrap_err()
            .code(),
        "image-size"
    );
    graphics.fill_rect(i32::MAX, 0, 1, 3);
}

#[test]
fn clipped_signed_rgb_rows_preserve_translation_and_alpha() {
    let pixels = [
        0xff12_3456,
        0x8012_3456,
        0x0012_3456,
        0xffab_cdef,
        0x80ab_cdef,
        0x00ab_cdef,
    ];
    for scan_length in [-3, 0, 3] {
        for process_alpha in [false, true] {
            let offset = if scan_length < 0 { 3 } else { 0 };
            let mut full = Framebuffer::new(5, 5).unwrap();
            let mut clipped = Framebuffer::new(5, 5).unwrap();
            for (frame, apply_clip) in [(&mut full, false), (&mut clipped, true)] {
                let mut graphics = frame.graphics();
                graphics.translate(2, 1);
                if apply_clip {
                    graphics.set_clip(0, 1, 2, 1);
                }
                graphics
                    .draw_rgb(&pixels, offset, scan_length, -1, 0, 3, 2, process_alpha)
                    .unwrap();
            }
            for y in 0..5 {
                for x in 0..5 {
                    let index = y * 5 + x;
                    let expected = if y == 2 && (2..4).contains(&x) {
                        full.pixels()[index]
                    } else {
                        0xffff_ffff
                    };
                    assert_eq!(clipped.pixels()[index], expected);
                }
            }
        }
    }
}

#[test]
fn clipped_regions_preserve_every_transform_and_anchor() {
    let pixels = (0_u32..20)
        .map(|index| 0x8000_0000 | (index * 0x070503))
        .collect::<Vec<_>>();
    let image = Image::from_argb(&pixels, 4, 5, true).unwrap();
    for transform in [
        Transform::None,
        Transform::MirrorRot180,
        Transform::Mirror,
        Transform::Rot180,
        Transform::MirrorRot270,
        Transform::Rot90,
        Transform::Rot270,
        Transform::MirrorRot90,
    ] {
        for anchor in [0, 1 | 2, 8 | 32] {
            for origin in [(-1, -1), (1, 2), (4, 5)] {
                let mut full = Framebuffer::new(8, 8).unwrap();
                let mut clipped = Framebuffer::new(8, 8).unwrap();
                for (frame, apply_clip) in [(&mut full, false), (&mut clipped, true)] {
                    let mut graphics = frame.graphics();
                    graphics.translate(1, -1);
                    if apply_clip {
                        graphics.set_clip(1, 3, 3, 3);
                    }
                    graphics
                        .draw_region(&image, 1, 1, 2, 3, transform, origin.0, origin.1, anchor)
                        .unwrap();
                }
                for y in 0..8 {
                    for x in 0..8 {
                        let index = y * 8 + x;
                        let expected = if (2..5).contains(&x) && (2..5).contains(&y) {
                            full.pixels()[index]
                        } else {
                            0xffff_ffff
                        };
                        assert_eq!(clipped.pixels()[index], expected, "{transform:?} {anchor}");
                    }
                }
            }
        }
    }
}
