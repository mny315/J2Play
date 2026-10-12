use super::{StrokeStyle, ellipse_point};
use crate::{Framebuffer, Graphics, clamp_i64};

#[test]
fn ellipse_points_match_integer_formula_at_every_angle_and_coordinate_limits() {
    let sine = |angle: i64| {
        let angle = angle.rem_euclid(360);
        let (x, sign) = if angle <= 180 {
            (angle, 1)
        } else {
            (angle - 180, -1)
        };
        let product = x * (180 - x);
        sign * 4 * product * 65_536 / (40_500 - product)
    };
    let scale = |radius: i32, factor: i64| {
        let value = i64::from(radius) * factor;
        (value + if value >= 0 { 32_768 } else { -32_768 }) / 65_536
    };
    for center in [(0, 0), (i32::MIN, i32::MAX), (i32::MAX, i32::MIN)] {
        for radii in [
            (0, 0),
            (1, 1),
            (2, 3),
            (113, 159),
            (i32::MIN, i32::MAX),
            (i32::MAX, i32::MAX),
        ] {
            for angle in (-1080..=1080_i64).chain([i64::MIN, i64::MAX]) {
                let normalized = angle.rem_euclid(360);
                let clamp =
                    |value: i64| value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
                let expected = (
                    clamp(i64::from(center.0) + scale(radii.0, sine(normalized + 90))),
                    clamp(i64::from(center.1) - scale(radii.1, sine(normalized))),
                );
                assert_eq!(
                    ellipse_point(center, radii, angle),
                    expected,
                    "{center:?}, {radii:?}, {angle}"
                );
            }
        }
    }
}

#[test]
#[ignore = "release throughput fixture"]
fn ellipse_arc_throughput() {
    for radii in [(4, 3), (119, 159), (i32::MAX, 1)] {
        let mut checksum = 0_u64;
        let start = std::time::Instant::now();
        for angle in -524_288..524_288_i64 {
            let (x, y) = ellipse_point(
                (17, -29),
                std::hint::black_box(radii),
                std::hint::black_box(angle),
            );
            checksum = checksum.wrapping_mul(31).wrapping_add(u64::from(x as u32));
            checksum = checksum.wrapping_mul(31).wrapping_add(u64::from(y as u32));
        }
        println!(
            "ellipse-point radii={radii:?} ns={} checksum={checksum:016x}",
            start.elapsed().as_nanos()
        );
    }
    for (name, width, height, start_angle, sweep) in [
        ("small", 8, 7, 0, 360),
        ("partial", 31, 27, 17, 137),
        ("large", 239, 319, 53, -270),
        ("flat", 239, 1, 0, 360),
        ("extreme", i32::MAX, 2, 0, 360),
    ] {
        for mode in 0..3 {
            let mut frame = Framebuffer::new(240, 320).unwrap();
            let start = std::time::Instant::now();
            for index in 0..512_u32 {
                let mut graphics = frame.graphics();
                graphics.set_color(index * 7919);
                if mode == 2 {
                    graphics.set_stroke_style(StrokeStyle::Dotted);
                }
                if mode == 0 {
                    graphics.fill_arc(-3, 1, width, height, start_angle, sweep);
                } else {
                    graphics.draw_arc(-3, 1, width, height, start_angle, sweep);
                }
                std::hint::black_box(&frame);
            }
            let elapsed = start.elapsed();
            let checksum = frame
                .pixels()
                .iter()
                .fold(0xcbf2_9ce4_8422_2325_u64, |hash, pixel| {
                    (hash ^ u64::from(*pixel)).wrapping_mul(0x100_0000_01b3)
                });
            println!(
                "ellipse-arc case={name} mode={mode} ns={} checksum={checksum:016x}",
                elapsed.as_nanos()
            );
        }
    }
}

#[test]
fn filled_ellipses_have_no_gaps_between_radial_samples() {
    for (width, height) in [(256, 256), (384, 128), (128, 384)] {
        let mut frame = Framebuffer::new(width + 1, height + 1).unwrap();
        frame
            .graphics()
            .fill_arc(0, 0, width as i32, height as i32, 0, 360);
        for y in 0..=height {
            let row = &frame.pixels()[(y * (width + 1)) as usize..][..=width as usize];
            assert_eq!(
                row[(width / 2) as usize],
                0xff00_0000,
                "ellipse {width}x{height} is missing row {y}"
            );
            let first = row.iter().position(|&pixel| pixel == 0xff00_0000).unwrap();
            let last = row.iter().rposition(|&pixel| pixel == 0xff00_0000).unwrap();
            assert!(
                row[first..=last].iter().all(|&pixel| pixel == 0xff00_0000),
                "ellipse {width}x{height} has a hole in row {y}"
            );
        }
    }
}

#[test]
fn filled_arcs_include_their_solid_outline() {
    for (width, height) in [(8, 7), (27, 19), (256, 128)] {
        for (start, sweep) in [(-45, 180), (17, 37), (53, -270), (0, 360)] {
            let mut outline = Framebuffer::new(280, 160).unwrap();
            let mut filled = outline.clone();
            outline
                .graphics()
                .draw_arc(3, 5, width, height, start, sweep);
            filled
                .graphics()
                .fill_arc(3, 5, width, height, start, sweep);
            assert!(outline.pixels().iter().any(|pixel| *pixel != 0xffff_ffff));
            for (index, (&edge, &area)) in outline.pixels().iter().zip(filled.pixels()).enumerate()
            {
                assert!(
                    edge == 0xffff_ffff || area == edge,
                    "missing arc boundary at {index}: {width}x{height}, {start}/{sweep}"
                );
            }
        }
    }
}

#[test]
fn filled_triangle_includes_its_bottom_boundary() {
    let mut frame = Framebuffer::new(5, 5).unwrap();
    frame.graphics().fill_triangle(0, 4, 2, 0, 4, 4);
    assert_eq!(&frame.pixels()[20..25], &[0xff00_0000; 5]);
}

#[test]
fn filled_shapes_ignore_stroke_style_without_changing_it() {
    for shape in 0..4 {
        let mut outputs = Vec::new();
        for stroke in [StrokeStyle::Solid, StrokeStyle::Dotted] {
            let mut frame = Framebuffer::new(32, 32).unwrap();
            {
                let mut graphics = frame.graphics();
                graphics.set_color(0x0012_3456);
                graphics.set_stroke_style(stroke);
                match shape {
                    0 => graphics.fill_arc(2, 2, 27, 25, 30, 240),
                    1 => graphics.fill_triangle(1, 1, 29, 11, 7, 29),
                    2 => graphics.fill_triangle(1, 7, 12, 7, 29, 7),
                    _ => graphics.fill_round_rect(1, 1, 29, 29, 12, 12),
                }
                assert_eq!(graphics.stroke, stroke);
            }
            outputs.push(frame.pixels().to_vec());
        }
        assert!(outputs[0].iter().any(|pixel| *pixel != 0xffff_ffff));
        assert_eq!(
            outputs[0], outputs[1],
            "stroke style affected filled shape {shape}"
        );
    }
}

#[test]
fn filled_curves_with_nonpositive_extents_leave_pixels_unchanged() {
    let mut frame = Framebuffer::new(8, 8).unwrap();
    for (width, height) in [(0, 0), (0, 7), (7, 0), (-1, 7), (7, -1)] {
        frame.graphics().fill_arc(0, 0, width, height, 0, 360);
        frame.graphics().fill_round_rect(0, 0, width, height, 4, 4);
        assert_eq!(frame.pixels(), &[0xffff_ffff; 64], "{width}/{height}");
    }
}

#[test]
fn rounded_rectangle_outlines_ignore_negative_dimensions() {
    for stroke in [StrokeStyle::Solid, StrokeStyle::Dotted] {
        for (width, height) in [(-1, 5), (5, -1), (-1, -1), (i32::MIN, 5), (5, i32::MIN)] {
            let mut frame = Framebuffer::new(8, 8).unwrap();
            let mut graphics = frame.graphics();
            graphics.translate(1, 1);
            graphics.set_stroke_style(stroke);
            graphics.draw_round_rect(2, 2, width, height, 4, 4);
            assert_eq!(frame.pixels(), &[0xffff_ffff; 64], "{width}/{height}");
        }
    }
}

// The original per-step rasterizer is an independent reference for skipping
// repeated angles. In particular, a duplicate point can affect dotted outlines.
fn reference_arc(graphics: &mut Graphics<'_>, width: i32, height: i32, start: i32, angle: i32) {
    if angle == 0 {
        return;
    }
    let cx = clamp_i64(-4 + i64::from(graphics.tx) + i64::from(width) / 2);
    let cy = clamp_i64(3 + i64::from(graphics.ty) + i64::from(height) / 2);
    let sweep = i64::from(angle.clamp(-360, 360));
    let steps = (i64::from(width.max(height)) * sweep.abs() / 45).clamp(
        1,
        i64::from(graphics.target.width + graphics.target.height) * 8,
    );
    let mut previous = (cx, cy);
    for n in 0..=steps {
        let point = ellipse_point(
            (cx, cy),
            (width / 2, height / 2),
            i64::from(start) + sweep * n / steps,
        );
        if n > 0 {
            graphics.draw_absolute_line(previous.0, previous.1, point.0, point.1, graphics.stroke);
        }
        previous = point;
    }
}

#[test]
fn arc_outline_sampling_preserves_pixels_with_sparse_and_repeated_angles() {
    for (width, height) in [
        (0, 0),
        (0, 17),
        (19, 0),
        (1, 1),
        (4, 9),
        (27, 23),
        (47, 31),
        (127, 19),
        (i32::MAX, 2),
        (2, i32::MAX),
    ] {
        for (start, angle) in [
            (0, 0),
            (0, 1),
            (17, 37),
            (-23, -37),
            (45, 359),
            (0, 360),
            (30, 720),
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MAX),
        ] {
            for context in 0..4 {
                for stroke in [StrokeStyle::Solid, StrokeStyle::Dotted] {
                    let mut expected = Framebuffer::new(32, 24).unwrap();
                    let mut actual = expected.clone();
                    for (frame, reference) in [(&mut expected, true), (&mut actual, false)] {
                        let mut graphics = frame.graphics();
                        graphics.set_stroke_style(stroke);
                        match context {
                            1 => {
                                graphics.translate(3, -2);
                                graphics.set_clip(4, 5, 13, 11);
                            }
                            2 => graphics.translate(i32::MAX, i32::MIN),
                            3 => graphics.set_clip(8, 8, 0, 0),
                            _ => {}
                        }
                        if reference {
                            reference_arc(&mut graphics, width, height, start, angle);
                        } else {
                            graphics.draw_arc(-4, 3, width, height, start, angle);
                        }
                    }
                    assert_eq!(
                        actual, expected,
                        "{width}x{height}, {start}/{angle}, context {context}, {stroke:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn very_wide_arc_with_a_tiny_clip_draws_only_the_visible_pixel() {
    let mut frame = Framebuffer::new(crate::MAX_IMAGE_PIXELS as u32, 1).unwrap();
    let mut graphics = frame.graphics();
    graphics.set_clip(0, 0, 1, 1);
    graphics.fill_arc(-i32::MAX / 2, 0, i32::MAX, 1, 0, 360);
    assert_eq!(frame.pixels()[0], 0xff00_0000);
    assert!(
        frame.pixels()[1..]
            .iter()
            .all(|&pixel| pixel == 0xffff_ffff)
    );
}
