use super::super::graphics_support::set_graphics_fields;
use super::*;

fn region_program() -> Program {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(7, 6) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    program
}

fn region_source(
    machine: &mut Machine<'_, '_>,
    graphics: Handle,
    mutable: bool,
    indexed: bool,
) -> Handle {
    let pixels = (0..360_u32)
        .map(|index| {
            let alpha = [0_u32, 128, 255][index as usize % 3];
            let color = if indexed { index % 16 } else { index };
            (alpha << 24 | color.wrapping_mul(0x0003_0711) & 0x00ff_ffff).cast_signed()
        })
        .collect();
    machine
        .allocate_image(20, 18, pixels, mutable, &[Value::Reference(Some(graphics))])
        .unwrap()
}

fn region_arguments(
    graphics: Handle,
    source: Handle,
    transform: i32,
    origin: (i32, i32),
) -> [Value; 11] {
    let (width, height) = if transform < 4 { (3, 2) } else { (2, 3) };
    [
        Value::Reference(Some(graphics)),
        Value::Reference(Some(source)),
        Value::Int(1),
        Value::Int(2),
        Value::Int(3),
        Value::Int(2),
        Value::Int(transform),
        Value::Int(origin.0),
        Value::Int(origin.1),
        Value::Int(width),
        Value::Int(height),
    ]
}

#[test]
fn normal_image_regions_match_independent_rasters_for_every_sample_and_transform() {
    const WIDTH: i32 = 7;
    const HEIGHT: i32 = 6;
    let program = region_program();
    for (mutable, indexed) in [(false, false), (false, true), (true, false)] {
        for promoted in [false, true] {
            for transform in 0..8 {
                for origin in [(-2, -1), (1, 1), (5, 4)] {
                    for clip in [(0, 0, WIDTH, HEIGHT), (1, 1, 4, 3), (3, 2, 0, 0)] {
                        let mut host = EnhancedHost::default();
                        let mut machine = program.machine(Limits::default(), false, &mut host);
                        let (graphics, pixels) =
                            direct_graphics_target(&mut machine, WIDTH, HEIGHT);
                        let canonical: Vec<i32> = (0..WIDTH * HEIGHT)
                            .map(|index| {
                                let index = index as u32;
                                ([0x0000_0000_u32, 0x4000_0000, 0x8000_0000, 0xff00_0000]
                                    [index as usize % 4]
                                    | index.wrapping_mul(0x0003_751b) & 0x00ff_ffff)
                                    .cast_signed()
                            })
                            .collect();
                        for (slot, &color) in machine
                            .graphics_int_array_mut(pixels)
                            .unwrap()
                            .iter_mut()
                            .zip(&canonical)
                        {
                            *slot = HeapValue::Int(color);
                        }
                        assert!(
                            machine
                                .ensure_enhanced_target(pixels, WIDTH as u32, HEIGHT as u32)
                                .unwrap()
                        );
                        if promoted {
                            machine.enhanced_2d(graphics, None, 0, |_| Ok(())).unwrap();
                        }
                        let initial: Vec<u32> = (0..WIDTH * HEIGHT * 4)
                            .map(|index| {
                                let index = index as u32;
                                [0x0000_0000_u32, 0x4000_0000, 0x8000_0000, 0xff00_0000]
                                    [index as usize % 4]
                                    | index.wrapping_mul(0x0005_374b) & 0x00ff_ffff
                            })
                            .collect();
                        machine
                            .graphics
                            .enhanced
                            .store(pixels, &initial, [0, 0, WIDTH as u32, HEIGHT as u32], false)
                            .unwrap();
                        let source = region_source(&mut machine, graphics, mutable, indexed);
                        set_graphics_fields(
                            &mut machine,
                            graphics,
                            &[
                                ("clipX", clip.0),
                                ("clipY", clip.1),
                                ("clipW", clip.2),
                                ("clipH", clip.3),
                            ],
                        );
                        machine
                            .graphics_draw_region(&region_arguments(
                                graphics, source, transform, origin,
                            ))
                            .unwrap();
                        let mut expected_samples = initial.clone();
                        for phase in 0..5 {
                            let mut reference_host = DefaultNativeContext;
                            let mut reference =
                                program.machine(Limits::default(), false, &mut reference_host);
                            let (reference_graphics, reference_pixels) =
                                direct_graphics_target(&mut reference, WIDTH, HEIGHT);
                            let original: Vec<i32> = if phase == 4 {
                                canonical.clone()
                            } else {
                                (0..WIDTH * HEIGHT)
                                    .map(|index| {
                                        let (x, y) = (index % WIDTH, index / WIDTH);
                                        initial[((y * 2 + phase / 2) * WIDTH * 2
                                            + x * 2
                                            + phase % 2)
                                            as usize]
                                            .cast_signed()
                                    })
                                    .collect()
                            };
                            for (slot, color) in reference
                                .graphics_int_array_mut(reference_pixels)
                                .unwrap()
                                .iter_mut()
                                .zip(original)
                            {
                                *slot = HeapValue::Int(color);
                            }
                            let source =
                                region_source(&mut reference, reference_graphics, mutable, indexed);
                            set_graphics_fields(
                                &mut reference,
                                reference_graphics,
                                &[
                                    ("clipX", clip.0),
                                    ("clipY", clip.1),
                                    ("clipW", clip.2),
                                    ("clipH", clip.3),
                                ],
                            );
                            reference
                                .graphics_draw_region(&region_arguments(
                                    reference_graphics,
                                    source,
                                    transform,
                                    origin,
                                ))
                                .unwrap();
                            let expected = reference
                                .graphics_int_array_snapshot(reference_pixels)
                                .unwrap();
                            if phase == 4 {
                                assert_eq!(
                                    machine.graphics_int_array_snapshot(pixels).unwrap(),
                                    expected
                                );
                            } else {
                                for (index, color) in expected.into_iter().enumerate() {
                                    let (x, y) = (index as i32 % WIDTH, index as i32 / WIDTH);
                                    expected_samples[((y * 2 + phase / 2) * WIDTH * 2
                                        + x * 2
                                        + phase % 2)
                                        as usize] = color.cast_unsigned();
                                }
                            }
                        }
                        assert_eq!(
                            machine.graphics.enhanced.pixels(pixels).unwrap().unwrap(),
                            expected_samples,
                            "mutable={mutable} indexed={indexed} promoted={promoted} transform={transform} origin={origin:?} clip={clip:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "manual transformed mixed-density image throughput measurement"]
fn enhanced_region_throughput() {
    let program = region_program();
    for (mutable, indexed) in [(false, false), (false, true), (true, false)] {
        for scale in [1, 2] {
            for transform in [0, 2, 4, 7] {
                let mut host = EnhancedHost {
                    original: scale == 1,
                    ..EnhancedHost::default()
                };
                let mut machine = program.machine(Limits::default(), false, &mut host);
                let (graphics, pixels) = direct_graphics_target(&mut machine, 240, 320);
                machine
                    .graphics_int_array_mut(pixels)
                    .unwrap()
                    .fill(HeapValue::Int(0xff10_2030_u32.cast_signed()));
                assert_eq!(
                    machine.ensure_enhanced_target(pixels, 240, 320).unwrap(),
                    scale == 2
                );
                let source_pixels = (0..128 * 96_u32)
                    .map(|index| {
                        let alpha = [0_u32, 128, 255][index as usize % 3];
                        let color = if indexed { index % 16 } else { index };
                        (alpha << 24 | color.wrapping_mul(0x0003_0711) & 0x00ff_ffff).cast_signed()
                    })
                    .collect();
                let source = machine
                    .allocate_image(
                        128,
                        96,
                        source_pixels,
                        mutable,
                        &[Value::Reference(Some(graphics))],
                    )
                    .unwrap();
                let (width, height) = if transform < 4 { (112, 80) } else { (80, 112) };
                let args = [
                    Value::Reference(Some(graphics)),
                    Value::Reference(Some(source)),
                    Value::Int(7),
                    Value::Int(8),
                    Value::Int(112),
                    Value::Int(80),
                    Value::Int(transform),
                    Value::Int(8),
                    Value::Int(8),
                    Value::Int(width),
                    Value::Int(height),
                ];
                machine.graphics_draw_region(&args).unwrap();
                let started = std::time::Instant::now();
                for _ in 0..64 {
                    machine
                        .graphics_draw_region(std::hint::black_box(&args))
                        .unwrap();
                }
                let elapsed = started.elapsed();
                let native = machine.graphics_int_array_snapshot(pixels).unwrap();
                let samples = machine
                    .graphics
                    .enhanced
                    .pixels(pixels)
                    .unwrap()
                    .unwrap_or_default();
                let checksum = native
                    .into_iter()
                    .map(i32::cast_unsigned)
                    .chain(samples)
                    .fold(0_u64, |sum, pixel| {
                        sum.wrapping_mul(31).wrapping_add(u64::from(pixel))
                    });
                eprintln!(
                    "region mutable={mutable} indexed={indexed} scale={scale} transform={transform} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}

#[test]
fn enhanced_regions_cancel_long_rows_and_columns_then_keep_samples_coherent() {
    use std::cell::Cell;
    use std::rc::Rc;

    let program = region_program();
    for indexed in [false, true] {
        for alpha in [128_u32, 255] {
            for transform in [0, 2, 4, 7] {
                let checks = Rc::new(Cell::new(0));
                let cancel_at = Rc::new(Cell::new(usize::MAX));
                let mut host = EnhancedHost {
                    cancellation: Some(CancellationContext {
                        checks: Rc::clone(&checks),
                        cancel_at: Rc::clone(&cancel_at),
                    }),
                    ..EnhancedHost::default()
                };
                let mut machine = program.machine(Limits::default(), false, &mut host);
                let (width, height) = if transform < 4 { (8192, 1) } else { (1, 8192) };
                let (graphics, pixels) = direct_graphics_target(&mut machine, width, height);
                assert!(
                    machine
                        .ensure_enhanced_target(pixels, width as u32, height as u32)
                        .unwrap()
                );
                let colors = (0..8192_u32)
                    .map(|index| {
                        let color = if indexed { index % 16 } else { index };
                        (alpha << 24 | color.wrapping_mul(9973) & 0x00ff_ffff).cast_signed()
                    })
                    .collect();
                let source = machine
                    .allocate_image(8192, 1, colors, false, &[Value::Reference(Some(graphics))])
                    .unwrap();
                let args = [
                    Value::Reference(Some(graphics)),
                    Value::Reference(Some(source)),
                    Value::Int(0),
                    Value::Int(0),
                    Value::Int(8192),
                    Value::Int(1),
                    Value::Int(transform),
                    Value::Int(0),
                    Value::Int(0),
                    Value::Int(width),
                    Value::Int(height),
                ];
                checks.set(0);
                cancel_at.set(2);
                assert_eq!(
                    machine.graphics_draw_region(&args).unwrap_err().code(),
                    "execution-cancelled"
                );
                assert_eq!(checks.get(), 2);
                cancel_at.set(usize::MAX);
                assert!(
                    machine
                        .ensure_enhanced_target(pixels, width as u32, height as u32)
                        .unwrap()
                );
                machine.graphics_draw_region(&args).unwrap();
                let logical = machine.graphics_int_array_snapshot(pixels).unwrap();
                let samples = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
                for y in 0..height {
                    for x in 0..width {
                        let color = logical[(y * width + x) as usize].cast_unsigned();
                        for phase in 0..4 {
                            assert_eq!(
                                samples[((y * 2 + phase / 2) * width * 2 + x * 2 + phase % 2)
                                    as usize],
                                color
                            );
                        }
                    }
                }
            }
        }
    }
}
