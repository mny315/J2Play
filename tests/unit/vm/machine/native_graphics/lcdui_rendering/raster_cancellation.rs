#![allow(
    clippy::redundant_closure_for_method_calls,
    reason = "method items cannot satisfy the callbacks' independent Machine lifetimes"
)]

use super::*;

fn check_operation(
    program: &Program,
    width: i32,
    height: i32,
    prepare: impl FnOnce(&mut Machine<'_, '_>, Handle, Handle) -> Vec<Value>,
    draw: impl Fn(&mut Machine<'_, '_>, &[Value]) -> Result<(), EmuError>,
) {
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, output) = direct_graphics_target(&mut machine, width, height);
    let args = prepare(&mut machine, graphics, output);
    let initial = machine.graphics_int_array_snapshot(output).unwrap();
    checks.set(0);
    draw(&mut machine, &args).unwrap();
    let total = checks.get();
    assert!(total > 4, "long raster operation did not poll cancellation");
    assert!(total <= 96, "thin rows polled the host once per pixel");
    let complete = machine.graphics_int_array_snapshot(output).unwrap();
    let mut interrupted_write = false;
    for point in [1, 2, total / 2, total * 3 / 4, total] {
        for (slot, &pixel) in machine
            .graphics_int_array_mut(output)
            .unwrap()
            .iter_mut()
            .zip(&initial)
        {
            *slot = HeapValue::Int(pixel);
        }
        checks.set(0);
        cancel_at.set(point);
        assert_eq!(
            draw(&mut machine, &args).unwrap_err().code(),
            "execution-cancelled"
        );
        assert_eq!(checks.get(), point);
        let partial = machine.graphics_int_array_snapshot(output).unwrap();
        if point == 1 {
            assert_eq!(partial, initial);
        }
        interrupted_write |= partial != initial && partial != complete;
    }
    if initial != complete {
        assert!(
            interrupted_write,
            "cancellation was only checked before writing"
        );
    }
    for (slot, &pixel) in machine
        .graphics_int_array_mut(output)
        .unwrap()
        .iter_mut()
        .zip(&initial)
    {
        *slot = HeapValue::Int(pixel);
    }
    cancel_at.set(usize::MAX);
    draw(&mut machine, &args).unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(output).unwrap(),
        complete
    );
}

#[test]
fn fill_rect_cancels_inside_wide_and_thin_opaque_and_alpha_regions() {
    let program = Program::new();
    for (width, height) in [(8192, 1), (1, 8192), (3, 2731)] {
        for color in [0xff12_3456_u32, 0x8012_3456] {
            check_operation(
                &program,
                width,
                height,
                |_, graphics, _| {
                    let mut args = vec![Value::Reference(Some(graphics))];
                    args.extend([0, 0, width, height, color.cast_signed()].map(Value::Int));
                    args
                },
                |machine, args| machine.graphics_fill_rect(args),
            );
        }
    }
}

#[test]
fn rgb_blits_cancel_during_alias_copy_and_painting_with_signed_strides() {
    let program = Program::new();
    for (width, height) in [(8192, 1), (1, 8192), (3, 2731)] {
        for alias in [false, true] {
            for stride in [width, -width, 0] {
                for alpha in [false, true] {
                    check_operation(
                        &program,
                        width,
                        height,
                        |machine, graphics, output| {
                            let source = if alias {
                                output
                            } else {
                                machine
                                    .heap
                                    .managed
                                    .allocate_array(ArrayKind::Int, width * height)
                                    .unwrap()
                            };
                            for (index, slot) in machine
                                .graphics_int_array_mut(source)
                                .unwrap()
                                .iter_mut()
                                .enumerate()
                            {
                                *slot = HeapValue::Int(
                                    (0x8012_3400_u32 | (index as u32 & 255)).cast_signed(),
                                );
                            }
                            let offset = if stride < 0 { (height - 1) * width } else { 0 };
                            let mut args = vec![
                                Value::Reference(Some(graphics)),
                                Value::Reference(Some(source)),
                            ];
                            args.extend(
                                [offset, stride, 0, 0, width, height, i32::from(alpha)]
                                    .map(Value::Int),
                            );
                            args
                        },
                        |machine, args| machine.graphics_draw_rgb(args),
                    );
                }
            }
        }
    }
}

fn image_source(
    machine: &mut Machine<'_, '_>,
    graphics: Handle,
    output: Handle,
    width: i32,
    height: i32,
    storage: &str,
) -> Handle {
    let pixels = (0..width * height)
        .map(|index| {
            let color = if storage == "indexed" {
                index % 8
            } else {
                index
            };
            (0xff00_0000_u32 | color as u32).cast_signed()
        })
        .collect::<Vec<_>>();
    if storage == "alias" {
        for (slot, &pixel) in machine
            .graphics_int_array_mut(output)
            .unwrap()
            .iter_mut()
            .zip(&pixels)
        {
            *slot = HeapValue::Int(pixel);
        }
    }
    let image = machine
        .allocate_image(
            width,
            height,
            pixels,
            matches!(storage, "mutable" | "alias"),
            &[Value::Reference(Some(graphics))],
        )
        .unwrap();
    if storage == "alias" {
        machine
            .heap
            .managed
            .set_field(
                image,
                "javax/microedition/lcdui/Image.pixels:[I",
                HeapValue::Reference(Some(output)),
            )
            .unwrap();
    }
    image
}

#[test]
fn image_blits_cancel_with_each_backing_and_aliases() {
    let program = program_with_bootstrap();
    for (width, height) in [(8192, 1), (1, 8192), (3, 2731)] {
        for storage in ["mutable", "argb", "indexed", "alias"] {
            check_operation(
                &program,
                width,
                height,
                |machine, graphics, output| {
                    let source = image_source(machine, graphics, output, width, height, storage);
                    vec![
                        Value::Reference(Some(graphics)),
                        Value::Reference(Some(source)),
                        Value::Int(0),
                        Value::Int(0),
                    ]
                },
                |machine, args| machine.graphics_draw_image(args),
            );
        }
    }
}

#[test]
fn transformed_image_blits_cancel_with_each_backing_and_axis_direction() {
    let program = program_with_bootstrap();
    for (width, height) in [(8192, 1), (1, 8192), (3, 2731)] {
        for storage in ["mutable", "argb", "indexed", "alias"] {
            for transform in 0..8 {
                check_operation(
                    &program,
                    width,
                    height,
                    |machine, graphics, output| {
                        let (source_width, source_height) = if transform < 4 {
                            (width, height)
                        } else {
                            (height, width)
                        };
                        let source = image_source(
                            machine,
                            graphics,
                            output,
                            source_width,
                            source_height,
                            storage,
                        );
                        let mut args = vec![
                            Value::Reference(Some(graphics)),
                            Value::Reference(Some(source)),
                        ];
                        args.extend(
                            [
                                0,
                                0,
                                source_width,
                                source_height,
                                transform,
                                0,
                                0,
                                width,
                                height,
                            ]
                            .map(Value::Int),
                        );
                        args
                    },
                    |machine, args| machine.graphics_draw_region(args),
                );
            }
        }
    }
}

#[test]
#[ignore = "manual fillRect throughput measurement"]
fn fill_rect_throughput() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, output) = direct_graphics_target(&mut machine, 240, 320);
    for color in [0xff12_3456_u32, 0x8012_3456] {
        let mut args = vec![Value::Reference(Some(graphics))];
        args.extend([0, 0, 240, 320, color.cast_signed()].map(Value::Int));
        let started = std::time::Instant::now();
        for _ in 0..128 {
            machine
                .graphics_fill_rect(std::hint::black_box(&args))
                .unwrap();
        }
        let elapsed = started.elapsed();
        let checksum = machine
            .graphics_int_array_snapshot(output)
            .unwrap()
            .into_iter()
            .fold(0_u64, |hash, pixel| {
                hash.wrapping_mul(31)
                    .wrapping_add(u64::from(pixel.cast_unsigned()))
            });
        eprintln!("fill color={color:08x} elapsed={elapsed:?} checksum={checksum:016x}");
    }
}
