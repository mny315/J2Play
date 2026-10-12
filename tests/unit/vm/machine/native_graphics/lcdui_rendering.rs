use super::graphics_support::{direct_graphics_target, set_graphics_fields};
use super::*;
use std::cell::Cell;
use std::rc::Rc;

mod draw_rgb;
mod image_blits;
mod images;
mod intrinsics;
mod raster_cancellation;
mod shape_spans;

#[test]
fn fill_rect_rejects_extreme_width_without_overflow_or_writing() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
    for (left, right) in [(i32::MIN, i32::MAX), (-1, i32::MAX)] {
        let error = machine
            .graphics_fill_rect(&[
                Value::Reference(Some(graphics)),
                Value::Int(left),
                Value::Int(0),
                Value::Int(right),
                Value::Int(1),
                Value::Int(-1),
            ])
            .unwrap_err();
        assert_eq!(error.code(), "array-index-out-of-bounds-exception");
        assert_eq!(machine.graphics_int_array_snapshot(pixels).unwrap(), [0; 9]);
    }
}

#[test]
fn lcd_ui_triangle_can_cancel_before_finishing_one_long_row() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 8192, 1);
    for coordinates in [[0, 0, 8191, 0, 4096, 1], [0, 0, 8191, 0, 4096, 0]] {
        let mut args = vec![Value::Reference(Some(graphics))];
        args.extend(coordinates.map(Value::Int));
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(0));
        checks.set(0);
        cancel_at.set(2);
        assert_eq!(
            machine.graphics_fill_triangle(&args).unwrap_err().code(),
            "execution-cancelled"
        );
        let pixels = machine.graphics_int_array_snapshot(pixels).unwrap();
        assert!(pixels.contains(&0));
        assert!(pixels.contains(&0xff12_3456_u32.cast_signed()));
    }
}

#[test]
fn polygon_rasterization_checks_cancellation_during_fill_and_outline() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 16, 16);
    let points = [(0, 0), (15, 15), (0, 15), (15, 0)].repeat(512);
    let color = 0xff12_3456_u32.cast_signed();
    for fill in [false, true] {
        checks.set(0);
        cancel_at.set(usize::MAX);
        machine
            .graphics_render_nokia_polygon(graphics, &points, color, fill)
            .unwrap();
        let complete = machine.graphics_int_array_snapshot(pixels).unwrap();
        let total_checks = checks.get();
        assert!(
            total_checks > 4,
            "long polygon did not poll for cancellation"
        );

        for point in [1, total_checks / 2, total_checks] {
            machine
                .graphics_int_array_mut(pixels)
                .unwrap()
                .fill(HeapValue::Int(0));
            checks.set(0);
            cancel_at.set(point);
            assert_eq!(
                machine
                    .graphics_render_nokia_polygon(graphics, &points, color, fill)
                    .unwrap_err()
                    .code(),
                "execution-cancelled"
            );
            if point == 1 {
                assert_eq!(
                    machine.graphics_int_array_snapshot(pixels).unwrap(),
                    [0; 256]
                );
            }
        }
        checks.set(0);
        cancel_at.set(usize::MAX);
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(0));
        machine
            .graphics_render_nokia_polygon(graphics, &points, color, fill)
            .unwrap();
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            complete
        );
    }
}

#[test]
fn polygon_coordinate_validation_checks_cancellation() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let coordinates = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 8192)
        .unwrap();
    assert_eq!(
        machine
            .graphics_nokia_coordinate_slice(coordinates, 0, 8192)
            .unwrap(),
        vec![HeapValue::Int(0); 8192]
    );
    let total_checks = checks.get();
    assert!(
        total_checks > 1,
        "coordinate validation did not poll for cancellation"
    );
    for point in [1, total_checks] {
        checks.set(0);
        cancel_at.set(point);
        assert_eq!(
            machine
                .graphics_nokia_coordinate_slice(coordinates, 0, 8192)
                .unwrap_err()
                .code(),
            "execution-cancelled"
        );
    }
}

#[test]
fn polygon_outline_can_cancel_before_finishing_one_long_edge() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 8192, 1);
    let points = [(0, 0), (8191, 0)];
    let color = 0xff12_3456_u32.cast_signed();
    machine
        .graphics_render_nokia_polygon(graphics, &points, color, false)
        .unwrap();
    let total_checks = checks.get();
    let mut interrupted_edge = false;
    for point in 1..=total_checks {
        checks.set(0);
        cancel_at.set(point);
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(0));
        assert_eq!(
            machine
                .graphics_render_nokia_polygon(graphics, &points, color, false)
                .unwrap_err()
                .code(),
            "execution-cancelled"
        );
        let output = machine.graphics_int_array_snapshot(pixels).unwrap();
        interrupted_edge |= output.contains(&color) && output.contains(&0);
    }
    assert!(
        interrupted_edge,
        "cancellation was only checked between complete edges"
    );
}

#[test]
fn arc_rasterization_checks_cancellation_during_fill_and_outline() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 32, 32);
    let mut args = vec![Value::Reference(Some(graphics))];
    args.extend([0, 0, 31, 31, 0, 360].map(Value::Int));
    for fill in [false, true] {
        checks.set(0);
        cancel_at.set(usize::MAX);
        machine.graphics_draw_arc(&args, fill).unwrap();
        let total_checks = checks.get();
        assert!(total_checks > 2, "arc did not poll for cancellation");
        for point in [1, total_checks / 2, total_checks] {
            checks.set(0);
            cancel_at.set(point);
            machine
                .graphics_int_array_mut(pixels)
                .unwrap()
                .fill(HeapValue::Int(0));
            assert_eq!(
                machine.graphics_draw_arc(&args, fill).unwrap_err().code(),
                "execution-cancelled"
            );
            if point == 1 {
                assert_eq!(
                    machine.graphics_int_array_snapshot(pixels).unwrap(),
                    [0; 1024]
                );
            }
        }
    }
}

#[test]
fn filled_arc_ignores_the_stroke_style_without_changing_it() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 32, 32);
    for (start, sweep) in [(0, 360), (30, 120), (-30, -120)] {
        let mut args = vec![Value::Reference(Some(graphics))];
        args.extend([2, 2, 27, 25, start, sweep].map(Value::Int));
        let mut outputs = Vec::new();
        for stroke in [0, 1] {
            machine
                .heap
                .managed
                .set_field(
                    graphics,
                    "javax/microedition/lcdui/Graphics.stroke:I",
                    HeapValue::Int(stroke),
                )
                .unwrap();
            machine
                .graphics_int_array_mut(pixels)
                .unwrap()
                .fill(HeapValue::Int(0));
            machine.graphics_draw_arc(&args, true).unwrap();
            outputs.push(machine.graphics_int_array_snapshot(pixels).unwrap());
            assert_eq!(
                machine
                    .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.stroke:I")
                    .unwrap(),
                stroke
            );
        }
        assert!(outputs[0].iter().any(|pixel| *pixel != 0));
        assert_eq!(
            outputs[0], outputs[1],
            "stroke style changed arc fill at {start}/{sweep}"
        );
    }
}

#[test]
fn filled_arc_with_nonpositive_dimensions_draws_nothing() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 8, 8);
    for (width, height) in [(0, 0), (0, 7), (7, 0), (-1, 7), (7, -1)] {
        let mut args = vec![Value::Reference(Some(graphics))];
        args.extend([0, 0, width, height, 0, 360].map(Value::Int));
        machine.graphics_draw_arc(&args, true).unwrap();
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            [0; 64],
            "{width}/{height}"
        );
    }
    for (width, height) in [(0, 0), (0, 7), (7, 0)] {
        let mut args = vec![Value::Reference(Some(graphics))];
        args.extend([0, 0, width, height, 0, 360].map(Value::Int));
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(0));
        machine.graphics_draw_arc(&args, false).unwrap();
        assert!(
            machine
                .graphics_int_array_snapshot(pixels)
                .unwrap()
                .iter()
                .any(|pixel| *pixel != 0)
        );
    }
}

#[test]
fn line_clipping_preserves_pixels_and_dotted_phase() {
    let color = 0xff12_3456_u32.cast_signed();
    for first in 0..16 {
        for last in 0..16 {
            for (start, end) in [
                ((0, first), (15, last)),
                ((15, last), (0, first)),
                ((first, 0), (last, 15)),
                ((last, 15), (first, 0)),
            ] {
                for dotted in [false, true] {
                    let mut full = vec![HeapValue::Int(0); 256];
                    draw_argb_line(&mut full, 16, start, end, (0, 0, 16, 16), color, dotted)
                        .unwrap();
                    assert!(
                        full.contains(&HeapValue::Int(color)),
                        "{start:?} to {end:?}"
                    );
                    for clip in [(3, 2, 11, 13), (0, 3, 16, 4), (7, 0, 8, 16)] {
                        let mut clipped = vec![HeapValue::Int(0); 256];
                        draw_argb_line(&mut clipped, 16, start, end, clip, color, dotted).unwrap();
                        for y in 0..16 {
                            for x in 0..16 {
                                let index = (y * 16 + x) as usize;
                                let expected =
                                    if x >= clip.0 && y >= clip.1 && x < clip.2 && y < clip.3 {
                                        full[index]
                                    } else {
                                        HeapValue::Int(0)
                                    };
                                assert_eq!(
                                    clipped[index], expected,
                                    "{start:?} to {end:?}, {clip:?}, dotted={dotted}, pixel={x}/{y}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn nokia_polygon_clips_extreme_coordinates_without_overflow() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let color = 0xff12_3456_u32.cast_signed();
    for points in [
        vec![(i32::MIN, i32::MIN), (i32::MAX, i32::MIN), (0, i32::MAX)],
        vec![
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MIN),
            (i32::MAX, i32::MAX),
            (i32::MIN, i32::MAX),
        ],
    ] {
        for reverse in [false, true] {
            let mut points = points.clone();
            if reverse {
                points.reverse();
            }
            let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
            machine
                .graphics_render_nokia_polygon(graphics, &points, color, true)
                .unwrap();
            assert_eq!(
                machine.graphics_int_array_snapshot(pixels).unwrap(),
                [color; 9]
            );
        }
    }
}

#[test]
fn lcd_ui_triangle_preserves_edge_signs_at_extreme_coordinates() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for reverse in [false, true] {
        let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
        let mut points = [(i32::MIN, i32::MIN), (i32::MAX, i32::MIN), (0, i32::MAX)];
        if reverse {
            points.reverse();
        }
        let mut args = vec![Value::Reference(Some(graphics))];
        args.extend(
            points
                .into_iter()
                .flat_map(|(x, y)| [Value::Int(x), Value::Int(y)]),
        );
        machine.graphics_fill_triangle(&args).unwrap();
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            [0xff12_3456_u32.cast_signed(); 9]
        );
    }
}

#[test]
fn lcd_ui_fills_apply_direct_graphics_alpha() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for triangle in [false, true] {
        for (color, expected) in [
            (0x00ff_0000_u32, 0xff00_00ff_u32),
            (0x80ff_0000, 0xff80_007f),
            (0xffff_0000, 0xffff_0000),
        ] {
            let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
            for index in 0..9 {
                machine
                    .heap
                    .managed
                    .array_set(pixels, index, HeapValue::Int(0xff00_00ff_u32.cast_signed()))
                    .unwrap();
            }
            machine
                .graphics_set_argb_color(&[
                    Value::Reference(Some(graphics)),
                    Value::Int(color.cast_signed()),
                ])
                .unwrap();
            if triangle {
                let mut args = vec![Value::Reference(Some(graphics))];
                args.extend([-3, -3, 9, -3, -3, 9].map(Value::Int));
                machine.graphics_fill_triangle(&args).unwrap();
            } else {
                machine
                    .graphics_fill_rect(&[
                        Value::Reference(Some(graphics)),
                        Value::Int(0),
                        Value::Int(0),
                        Value::Int(3),
                        Value::Int(3),
                        Value::Int(color.cast_signed()),
                    ])
                    .unwrap();
            }
            assert_eq!(
                machine.graphics_int_array_snapshot(pixels).unwrap(),
                [expected.cast_signed(); 9],
                "triangle={triangle} color={color:#010x}"
            );
        }
    }
}

#[test]
fn nokia_polygon_blends_each_covered_pixel_once() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for points in [
        vec![(0, 0), (2, 0), (0, 2)],
        vec![(0, 0), (2, 0), (2, 2), (0, 2)],
        vec![(0, 0), (2, 2), (2, 0), (0, 2)],
        vec![(0, 0), (2, 0)],
        vec![(1, 1)],
    ] {
        for fill in [false, true] {
            let [opaque, translucent] = [0xffff_0000_u32, 0x80ff_0000].map(|color| {
                let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
                machine
                    .graphics_int_array_mut(pixels)
                    .unwrap()
                    .fill(HeapValue::Int(0xff00_00ff_u32.cast_signed()));
                machine
                    .graphics_render_nokia_polygon(graphics, &points, color.cast_signed(), fill)
                    .unwrap();
                machine.graphics_int_array_snapshot(pixels).unwrap()
            });
            let expected = opaque
                .into_iter()
                .map(|pixel| {
                    if pixel == 0xffff_0000_u32.cast_signed() {
                        0xff80_007f_u32.cast_signed()
                    } else {
                        pixel
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(translucent, expected, "points={points:?} fill={fill}");
        }
    }
}

#[test]
fn lcd_ui_arc_blends_each_covered_pixel_once() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for (start, sweep) in [(0, 360), (-45, 180), (270, -360)] {
        for fill in [false, true] {
            let [opaque, translucent] = [0xffff_0000_u32, 0x80ff_0000].map(|color| {
                let (graphics, pixels) = direct_graphics_target(&mut machine, 7, 7);
                for (name, value) in [("clipX", 1), ("clipY", 1), ("clipW", 5), ("clipH", 5)] {
                    machine
                        .heap
                        .managed
                        .set_field(
                            graphics,
                            &format!("javax/microedition/lcdui/Graphics.{name}:I"),
                            HeapValue::Int(value),
                        )
                        .unwrap();
                }
                machine
                    .graphics_int_array_mut(pixels)
                    .unwrap()
                    .fill(HeapValue::Int(0xff00_00ff_u32.cast_signed()));
                machine
                    .graphics_set_argb_color(&[
                        Value::Reference(Some(graphics)),
                        Value::Int(color.cast_signed()),
                    ])
                    .unwrap();
                let mut args = vec![Value::Reference(Some(graphics))];
                args.extend([0, 0, 6, 6, start, sweep].map(Value::Int));
                machine.graphics_draw_arc(&args, fill).unwrap();
                machine.graphics_int_array_snapshot(pixels).unwrap()
            });
            let expected = opaque
                .into_iter()
                .map(|pixel| {
                    if pixel == 0xffff_0000_u32.cast_signed() {
                        0xff80_007f_u32.cast_signed()
                    } else {
                        pixel
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(
                translucent, expected,
                "start={start} sweep={sweep} fill={fill}"
            );
        }
    }
}

#[test]
fn large_arc_fills_match_shared_geometry_with_one_alpha_blend() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 257, 129);
    set_graphics_fields(
        &mut machine,
        graphics,
        &[
            ("color", 0x8000_0000_u32.cast_signed()),
            ("stroke", 1),
            ("tx", -5),
            ("ty", 3),
            ("clipX", 7),
            ("clipY", 5),
            ("clipW", 243),
            ("clipH", 120),
        ],
    );
    for (start, sweep) in [(0, 360), (17, 123), (53, -270)] {
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(-1));
        let mut args = vec![Value::Reference(Some(graphics))];
        args.extend([0, 0, 256, 128, start, sweep].map(Value::Int));
        machine.graphics_draw_arc(&args, true).unwrap();
        let mut expected = graphics::Framebuffer::new(257, 129).unwrap();
        {
            let mut drawing = expected.graphics();
            drawing.set_clip(7, 5, 243, 120);
            drawing.translate(-5, 3);
            drawing.fill_arc(0, 0, 256, 128, start, sweep);
        }
        for (actual, &expected) in machine
            .graphics_int_array_snapshot(pixels)
            .unwrap()
            .iter()
            .zip(expected.pixels())
        {
            assert_eq!(
                actual.cast_unsigned(),
                if expected == 0xff00_0000 {
                    0xff7f_7f7f
                } else {
                    expected
                }
            );
        }
    }
}

#[test]
fn separate_polygon_calls_composite_independently() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
    machine
        .graphics_int_array_mut(pixels)
        .unwrap()
        .fill(HeapValue::Int(0xff00_00ff_u32.cast_signed()));
    for expected in [0xff80_007f_u32, 0xffc0_003f] {
        machine
            .graphics_render_nokia_polygon(
                graphics,
                &[(0, 0), (2, 0), (2, 2), (0, 2)],
                0x80ff_0000_u32.cast_signed(),
                true,
            )
            .unwrap();
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            [expected.cast_signed(); 9]
        );
    }
}

#[test]
fn translucent_shape_coverage_is_bounded_by_the_target_buffer() {
    let mut pixels = [HeapValue::Int(0)];
    let error = ShapePainter::new(
        &mut pixels,
        i32::MAX,
        (0, 0, i32::MAX, i32::MAX),
        0x80ff_0000_u32.cast_signed(),
    )
    .err()
    .expect("oversized coverage must be rejected");
    assert_eq!(error.code(), "out-of-memory-error");
    assert!(!is_managed_heap_limit_error(&error));
    assert_eq!(pixels, [HeapValue::Int(0)]);
}

#[test]
fn degenerate_lcd_ui_triangle_blends_each_pixel_once() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for points in [[0, 0, 1, 0, 2, 0], [2, 0, 1, 0, 0, 0], [-2, 0, 1, 0, 4, 0]] {
        let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 1);
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(0xff00_00ff_u32.cast_signed()));
        machine
            .graphics_set_argb_color(&[
                Value::Reference(Some(graphics)),
                Value::Int(0x80ff_0000_u32.cast_signed()),
            ])
            .unwrap();
        let mut args = vec![Value::Reference(Some(graphics))];
        args.extend(points.map(Value::Int));
        machine.graphics_fill_triangle(&args).unwrap();
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            [0xff80_007f_u32.cast_signed(); 3]
        );
    }
}
