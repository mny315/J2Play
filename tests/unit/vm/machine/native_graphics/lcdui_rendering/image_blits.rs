use super::*;

fn pixels(width: i32, height: i32, indexed: bool) -> Vec<i32> {
    (0..width * height)
        .map(|index| {
            let alpha = [0_u32, 128, 255][index as usize % 3];
            let color = if indexed {
                index as u32 % 16
            } else {
                index as u32
            };
            (alpha << 24 | color.wrapping_mul(0x0003_0711) & 0x00ff_ffff).cast_signed()
        })
        .collect()
}

#[test]
fn draw_rgb_rows_match_framebuffer_with_clipping_and_signed_strides() {
    let program = Program::new();
    let source = pixels(6, 5, false);
    let reference_source = source
        .iter()
        .map(|pixel| pixel.cast_unsigned())
        .collect::<Vec<_>>();
    for (offset, stride) in [(0, 6), (24, -6), (3, 0)] {
        for process_alpha in [false, true] {
            for (x, y) in [(-2, -1), (0, 0), (2, 1), (5, 4)] {
                let mut reference = graphics::Framebuffer::new(6, 5).unwrap();
                let mut draw = reference.graphics();
                draw.set_color(0x0012_3456);
                draw.fill_rect(0, 0, 6, 5);
                draw.set_clip(1, 1, 4, 3);
                draw.draw_rgb(&reference_source, offset, stride, x, y, 3, 5, process_alpha)
                    .unwrap();

                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let (target, target_pixels) = direct_graphics_target(&mut machine, 6, 5);
                machine
                    .graphics_int_array_mut(target_pixels)
                    .unwrap()
                    .fill(HeapValue::Int(0xff12_3456_u32.cast_signed()));
                set_graphics_fields(
                    &mut machine,
                    target,
                    &[("clipX", 1), ("clipY", 1), ("clipW", 4), ("clipH", 3)],
                );
                let input = machine
                    .allocate_array(ArrayKind::Int, 30, &[], &[])
                    .unwrap();
                for (slot, &pixel) in machine
                    .graphics_int_array_mut(input)
                    .unwrap()
                    .iter_mut()
                    .zip(&source)
                {
                    *slot = HeapValue::Int(pixel);
                }
                let mut args = vec![
                    Value::Reference(Some(target)),
                    Value::Reference(Some(input)),
                ];
                args.extend([offset, stride, x, y, 3, 5, i32::from(process_alpha)].map(Value::Int));
                machine.graphics_draw_rgb(&args).unwrap();
                assert!(
                    machine
                        .graphics_int_array_snapshot(target_pixels)
                        .unwrap()
                        .iter()
                        .map(|pixel| pixel.cast_unsigned())
                        .eq(reference.pixels().iter().copied()),
                    "offset={offset} stride={stride} origin=({x},{y}) alpha={process_alpha}"
                );
            }
        }
    }
}

#[test]
#[ignore = "manual image row compositing throughput measurement"]
fn image_blit_throughput() {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(128, 96) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    for (name, mutable, indexed) in [
        ("argb", false, false),
        ("indexed", false, true),
        ("mutable", true, false),
    ] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let (graphics, output) = direct_graphics_target(&mut machine, 128, 96);
        let source = machine
            .allocate_image(
                128,
                96,
                pixels(128, 96, indexed),
                mutable,
                &[Value::Reference(Some(graphics))],
            )
            .unwrap();
        let args = [
            Value::Reference(Some(graphics)),
            Value::Reference(Some(source)),
            Value::Int(-8),
            Value::Int(-4),
        ];
        machine.graphics_draw_image(&args).unwrap();
        let started = std::time::Instant::now();
        for _ in 0..512 {
            machine
                .graphics_draw_image(std::hint::black_box(&args))
                .unwrap();
        }
        let elapsed = started.elapsed();
        let checksum = machine
            .graphics_int_array_snapshot(output)
            .unwrap()
            .iter()
            .fold(0_u64, |hash, pixel| {
                hash.wrapping_mul(31)
                    .wrapping_add(u64::from(pixel.cast_unsigned()))
            });
        eprintln!("image={name} elapsed={elapsed:?} checksum={checksum:016x}");
    }
}

#[test]
fn image_regions_match_framebuffer_for_all_transforms_and_storage_types() {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(6, 5) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    for (mutable, indexed) in [(false, false), (false, true), (true, false)] {
        let source = pixels(20, 18, indexed);
        let reference_source = graphics::Image::from_argb(
            &source
                .iter()
                .map(|pixel| pixel.cast_unsigned())
                .collect::<Vec<_>>(),
            20,
            18,
            true,
        )
        .unwrap();
        for (code, transform) in [
            graphics::Transform::None,
            graphics::Transform::MirrorRot180,
            graphics::Transform::Mirror,
            graphics::Transform::Rot180,
            graphics::Transform::MirrorRot270,
            graphics::Transform::Rot90,
            graphics::Transform::Rot270,
            graphics::Transform::MirrorRot90,
        ]
        .into_iter()
        .enumerate()
        {
            for (x, y) in [(-2, -1), (0, 0), (2, 1), (5, 4)] {
                let mut reference = graphics::Framebuffer::new(6, 5).unwrap();
                let mut draw = reference.graphics();
                draw.set_color(0x0012_3456);
                draw.fill_rect(0, 0, 6, 5);
                draw.set_clip(1, 1, 4, 3);
                draw.draw_region(&reference_source, 4, 5, 5, 3, transform, x, y, 20)
                    .unwrap();

                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let (target, output) = direct_graphics_target(&mut machine, 6, 5);
                machine
                    .graphics_int_array_mut(output)
                    .unwrap()
                    .fill(HeapValue::Int(0xff12_3456_u32.cast_signed()));
                set_graphics_fields(
                    &mut machine,
                    target,
                    &[("clipX", 1), ("clipY", 1), ("clipW", 4), ("clipH", 3)],
                );
                let input = machine
                    .allocate_image(
                        20,
                        18,
                        source.clone(),
                        mutable,
                        &[Value::Reference(Some(target))],
                    )
                    .unwrap();
                let mut args = vec![
                    Value::Reference(Some(target)),
                    Value::Reference(Some(input)),
                ];
                let (width, height) = if code < 4 { (5, 3) } else { (3, 5) };
                args.extend([4, 5, 5, 3, code as i32, x, y, width, height].map(Value::Int));
                machine.graphics_draw_region(&args).unwrap();
                assert!(
                    machine
                        .graphics_int_array_snapshot(output)
                        .unwrap()
                        .iter()
                        .map(|pixel| pixel.cast_unsigned())
                        .eq(reference.pixels().iter().copied()),
                    "transform={code} origin=({x},{y}) mutable={mutable} indexed={indexed}"
                );
            }
        }
    }
}

#[test]
#[ignore = "manual RGB row compositing throughput measurement"]
fn rgb_blit_throughput() {
    let program = Program::new();
    for aliased in [false, true] {
        for stride in [128, -128, 0] {
            for alpha in [false, true] {
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let (graphics, output) = direct_graphics_target(&mut machine, 128, 96);
                let source = if aliased {
                    output
                } else {
                    machine
                        .allocate_array(ArrayKind::Int, 128 * 96, &[], &[])
                        .unwrap()
                };
                for (slot, pixel) in machine
                    .graphics_int_array_mut(source)
                    .unwrap()
                    .iter_mut()
                    .zip(pixels(128, 96, false))
                {
                    *slot = HeapValue::Int(pixel);
                }
                let offset = if stride < 0 { 95 * 128 } else { 0 };
                let mut args = vec![
                    Value::Reference(Some(graphics)),
                    Value::Reference(Some(source)),
                ];
                args.extend([offset, stride, -8, -4, 128, 96, i32::from(alpha)].map(Value::Int));
                let started = std::time::Instant::now();
                for _ in 0..512 {
                    machine
                        .graphics_draw_rgb(std::hint::black_box(&args))
                        .unwrap();
                }
                let elapsed = started.elapsed();
                let checksum = machine
                    .graphics_int_array_snapshot(output)
                    .unwrap()
                    .iter()
                    .fold(0_u64, |hash, pixel| {
                        hash.wrapping_mul(31)
                            .wrapping_add(u64::from(pixel.cast_unsigned()))
                    });
                eprintln!(
                    "rgb aliased={aliased} stride={stride} alpha={alpha} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}

#[test]
#[ignore = "manual transformed image compositing throughput measurement"]
fn region_blit_throughput() {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(128, 96) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    for (name, mutable, indexed) in [
        ("argb", false, false),
        ("indexed", false, true),
        ("mutable", true, false),
    ] {
        for transform in 0..8 {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let (graphics, output) = direct_graphics_target(&mut machine, 128, 96);
            let source = machine
                .allocate_image(
                    128,
                    96,
                    pixels(128, 96, indexed),
                    mutable,
                    &[Value::Reference(Some(graphics))],
                )
                .unwrap();
            let (width, height) = if transform < 4 { (120, 88) } else { (88, 120) };
            let mut args = vec![
                Value::Reference(Some(graphics)),
                Value::Reference(Some(source)),
            ];
            args.extend([4, 5, 120, 88, transform, -8, -4, width, height].map(Value::Int));
            machine.graphics_draw_region(&args).unwrap();
            let started = std::time::Instant::now();
            for _ in 0..512 {
                machine
                    .graphics_draw_region(std::hint::black_box(&args))
                    .unwrap();
            }
            let elapsed = started.elapsed();
            let checksum = machine
                .graphics_int_array_snapshot(output)
                .unwrap()
                .iter()
                .fold(0_u64, |hash, pixel| {
                    hash.wrapping_mul(31)
                        .wrapping_add(u64::from(pixel.cast_unsigned()))
                });
            eprintln!(
                "region={name} transform={transform} elapsed={elapsed:?} checksum={checksum:016x}"
            );
        }
    }
}
