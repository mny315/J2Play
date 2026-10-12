use super::graphics_support::direct_graphics_target;
use super::*;

mod cancellation;
mod throughput;

fn get_pixels_args(graphics: Handle, destination: Handle, values: [i32; 7]) -> Vec<Value> {
    [
        Value::Reference(Some(graphics)),
        Value::Reference(Some(destination)),
    ]
    .into_iter()
    .chain(values.map(Value::Int))
    .collect()
}

#[test]
fn nokia_get_pixels_reads_only_the_rectangle_with_each_format_and_row_order() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 4, 4);
    let colors = [0xffff_0000_u32, 0x8000_ff00, 0x0000_00ff, 0xffff_ffff];
    for (index, color) in [5, 6, 9, 10].into_iter().zip(colors) {
        machine
            .heap
            .managed
            .array_set(pixels, index, HeapValue::Int(color.cast_signed()))
            .unwrap();
    }
    for (kind, format, expected) in [
        (ArrayKind::Int, 8888, colors),
        (ArrayKind::Int, 888, [0xff_0000, 0xff00, 0xff, 0xff_ffff]),
        (ArrayKind::Short, 4444, [0xff00, 0x80f0, 0x000f, 0xffff]),
        (ArrayKind::Short, 444, [0x0f00, 0x00f0, 0x000f, 0x0fff]),
        (ArrayKind::Short, 555, [0x7c00, 0x03e0, 0x001f, 0x7fff]),
        (ArrayKind::Short, 1555, [0xfc00, 0x83e0, 0x001f, 0xffff]),
        (ArrayKind::Short, 565, [0xf800, 0x07e0, 0x001f, 0xffff]),
    ] {
        let expected = expected.map(|pixel| {
            if kind == ArrayKind::Short {
                i32::from(u16::try_from(pixel).unwrap().cast_signed())
            } else {
                pixel.cast_signed()
            }
        });
        for (offset, stride, destinations) in [
            (0, 2, [0, 1, 2, 3]),
            (4, -2, [4, 5, 2, 3]),
            (1, 0, [1, 2, 1, 2]),
        ] {
            let destination = machine
                .heap
                .managed
                .allocate_array(kind.clone(), 8)
                .unwrap();
            machine
                .graphics_nokia_get_pixels(&get_pixels_args(
                    graphics,
                    destination,
                    [offset, stride, 1, 1, 2, 2, format],
                ))
                .unwrap();
            let mut output = [0; 8];
            for (index, pixel) in destinations.into_iter().zip(expected) {
                output[index] = pixel;
            }
            for (index, pixel) in (0..8).zip(output) {
                assert_eq!(
                    machine.heap.managed.array_get(destination, index).unwrap(),
                    HeapValue::Int(pixel),
                    "format={format} offset={offset} stride={stride} index={index}"
                );
            }
        }
    }
}

#[test]
fn nokia_get_pixels_preserves_aliases_and_destination_bounds_errors() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
    for (index, color) in (0..9).zip(1..=9) {
        machine
            .heap
            .managed
            .array_set(pixels, index, HeapValue::Int(color))
            .unwrap();
    }
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 3)
        .unwrap();
    assert_eq!(
        machine
            .graphics_nokia_get_pixels(&get_pixels_args(
                graphics,
                destination,
                [0, 2, 1, 0, 2, 2, 8888],
            ))
            .unwrap_err()
            .code(),
        "array-index-out-of-bounds-exception"
    );
    assert_eq!(
        machine.graphics_int_array_snapshot(destination).unwrap(),
        [2, 3, 5]
    );
    // A zero-width request must not write or divide the empty snapshot into rows.
    machine
        .graphics_nokia_get_pixels(&get_pixels_args(
            graphics,
            destination,
            [0, 2, 1, 0, 0, 2, 8888],
        ))
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(destination).unwrap(),
        [2, 3, 5]
    );
    machine
        .graphics_nokia_get_pixels(&get_pixels_args(graphics, pixels, [4, 2, 1, 0, 2, 2, 8888]))
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        [1, 2, 3, 4, 2, 3, 5, 6, 9]
    );
}

fn direct_pixel_args(graphics: Handle, source: Handle, values: [i32; 9]) -> Vec<Value> {
    let mut args = vec![
        Value::Reference(Some(graphics)),
        Value::Reference(Some(source)),
    ];
    args.extend(values.map(Value::Int));
    args
}

#[test]
fn nokia_draw_pixels_checks_source_bounds_before_allocating() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
    let source = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 0)
        .unwrap();
    let mut args = vec![
        Value::Reference(Some(graphics)),
        Value::Reference(Some(source)),
    ];
    args.extend([0, 0, 0, 0, 0, i32::MAX, i32::MAX, 0, 8888].map(Value::Int));
    assert_eq!(
        machine
            .graphics_nokia_draw_pixels(&args)
            .unwrap_err()
            .code(),
        "array-index-out-of-bounds-exception"
    );
    assert_eq!(machine.graphics_int_array_snapshot(pixels).unwrap(), [0; 9]);
}

#[test]
fn nokia_draw_pixels_preserves_destination_alpha() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for (foreground, background, transparency, expected) in [
        (0x80ff_0000_u32, 0x0000_00ff_u32, 1, 0x80ff_0000_u32),
        (0x80ff_0000, 0x8000_00ff, 1, 0xc0aa_0055),
        (0x01ff_0000, 0x0100_00ff, 1, 0x0280_007f),
        (0x80ff_0000, 0xff00_00ff, 1, 0xff80_007f),
        (0x00ff_0000, 0x8000_00ff, 1, 0x8000_00ff),
        (0xffff_0000, 0x8000_00ff, 1, 0xffff_0000),
        (0x00ff_0000, 0x8000_00ff, 0, 0xffff_0000),
    ] {
        let (graphics, pixels) = direct_graphics_target(&mut machine, 1, 1);
        let source = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 1)
            .unwrap();
        machine
            .heap
            .managed
            .array_set(source, 0, HeapValue::Int(foreground.cast_signed()))
            .unwrap();
        machine
            .heap
            .managed
            .array_set(pixels, 0, HeapValue::Int(background.cast_signed()))
            .unwrap();
        machine
            .graphics_nokia_draw_pixels(&direct_pixel_args(
                graphics,
                source,
                [transparency, 0, 1, 0, 0, 1, 1, 0, 8888],
            ))
            .unwrap();
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            [expected.cast_signed()]
        );
    }
}

#[test]
fn nokia_draw_pixels_clips_repeated_rows_before_copying() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let source = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    let color = 0xff12_3456_u32.cast_signed();
    machine
        .heap
        .managed
        .array_set(source, 0, HeapValue::Int(color))
        .unwrap();
    for rotation in [0, 90, 180, 270] {
        let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
        machine
            .graphics_nokia_draw_pixels(&direct_pixel_args(
                graphics,
                source,
                [1, 0, 0, 0, 0, 1, i32::MAX, rotation, 8888],
            ))
            .unwrap();
        let expected = if rotation % 180 == 0 {
            [color, 0, 0, color, 0, 0, color, 0, 0]
        } else {
            [color, color, color, 0, 0, 0, 0, 0, 0]
        };
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            expected
        );
    }
}

#[test]
fn nokia_draw_pixels_preserves_transforms_clipping_and_negative_strides() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let colors = [
        0xff11_0000_u32,
        0xff00_2200,
        0xff00_0033,
        0xff44_4400,
        0xff00_5555,
        0xff66_0066,
    ]
    .map(u32::cast_signed);
    let rotations = [
        (0, 3, [0, 1, 2, 3, 4, 5]),
        (90, 2, [2, 5, 1, 4, 0, 3]),
        (180, 3, [5, 4, 3, 2, 1, 0]),
        (270, 2, [3, 0, 4, 1, 5, 2]),
    ];
    for (kind, format, values) in [
        (ArrayKind::Int, 8888, colors),
        (
            ArrayKind::Short,
            444,
            [0x100, 0x020, 0x003, 0x440, 0x055, 0x606],
        ),
    ] {
        let source = machine.heap.managed.allocate_array(kind, 8).unwrap();
        for (index, value) in [1, 2, 3, 5, 6, 7].into_iter().zip(values) {
            machine
                .heap
                .managed
                .array_set(source, index, HeapValue::Int(value))
                .unwrap();
        }
        for stride in [4, -4] {
            let logical = if stride > 0 {
                colors
            } else {
                [
                    colors[3], colors[4], colors[5], colors[0], colors[1], colors[2],
                ]
            };
            for (rotation, width, order) in rotations {
                for flips in [0, 0x2000, 0x4000, 0x6000] {
                    let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
                    for (name, value) in [
                        ("tx", 1),
                        ("ty", -1),
                        ("clipX", 1),
                        ("clipY", 1),
                        ("clipW", 2),
                        ("clipH", 2),
                    ] {
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
                    let mut rows = order
                        .chunks_exact(width)
                        .map(|row| row.iter().map(|&index| logical[index]).collect::<Vec<_>>())
                        .collect::<Vec<_>>();
                    if flips & 0x4000 != 0 {
                        rows.reverse();
                    }
                    if flips & 0x2000 != 0 {
                        for row in &mut rows {
                            row.reverse();
                        }
                    }
                    let mut expected = [0; 9];
                    for (y, row) in rows.iter().enumerate().skip(1) {
                        for (x, &pixel) in row.iter().enumerate().skip(1) {
                            expected[y * 3 + x] = pixel;
                        }
                    }
                    machine
                        .graphics_nokia_draw_pixels(&direct_pixel_args(
                            graphics,
                            source,
                            [
                                1,
                                if stride > 0 { 1 } else { 5 },
                                stride,
                                -1,
                                1,
                                3,
                                2,
                                rotation | flips,
                                format,
                            ],
                        ))
                        .unwrap();
                    assert_eq!(
                        machine.graphics_int_array_snapshot(pixels).unwrap(),
                        expected,
                        "format={format} stride={stride} rotation={rotation} flips={flips}"
                    );
                }
            }
        }
    }
}

#[test]
fn nokia_draw_pixels_preserves_overlapping_source_rows() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 3);
    let colors = [1_u32, 2, 3, 4, 5, 6, 7, 8, 9].map(|n| (0xff00_0000 | n).cast_signed());
    for (index, color) in (0..).zip(colors) {
        machine
            .heap
            .managed
            .array_set(pixels, index, HeapValue::Int(color))
            .unwrap();
    }
    machine
        .graphics_nokia_draw_pixels(&direct_pixel_args(
            graphics,
            pixels,
            [1, 0, 3, 0, 1, 3, 2, 0, 8888],
        ))
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        [
            colors[0], colors[1], colors[2], colors[0], colors[1], colors[2], colors[3], colors[4],
            colors[5]
        ]
    );
}

#[test]
fn nokia_direct_pixel_formats_convert_without_phone_specific_state() {
    assert_eq!(
        nokia_pixel_to_argb(
            0xf800_u16.cast_signed().into(),
            &ArrayKind::Short,
            565,
            false
        )
        .unwrap(),
        0xffff_0000_u32.cast_signed()
    );
    assert_eq!(
        nokia_pixel_to_argb(
            0x8123_u16.cast_signed().into(),
            &ArrayKind::Short,
            4444,
            true
        )
        .unwrap(),
        0x8811_2233_u32.cast_signed()
    );
    assert_eq!(
        argb_to_nokia_pixel(0xff12_3456_u32.cast_signed(), &ArrayKind::Int, 888).unwrap(),
        0x0012_3456
    );
    assert_eq!(
        argb_to_nokia_pixel(0xffff_0000_u32.cast_signed(), &ArrayKind::Short, 565).unwrap(),
        i32::from(0xf800_u16.cast_signed())
    );
}
