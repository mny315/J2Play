use super::*;

#[derive(Clone, Copy, Debug)]
enum CopyKind {
    Argb,
    Indexed,
    Mutable,
    Rgb { alpha: bool, negative: bool },
}

fn copy_program() -> Program {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(7, 6) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    program
}

fn copy_source(
    machine: &mut Machine<'_, '_>,
    graphics: Handle,
    kind: CopyKind,
    width: i32,
    height: i32,
    opaque: bool,
) -> (Handle, Handle) {
    let colors = (0..width * height)
        .map(|index| {
            let index = index as u32;
            let alpha = if opaque {
                255
            } else {
                [0, 128, 255][index as usize % 3]
            };
            let index = if matches!(kind, CopyKind::Indexed) {
                index % 16
            } else {
                index
            };
            (alpha << 24 | index.wrapping_mul(0x0003_0711) & 0x00ff_ffff).cast_signed()
        })
        .collect();
    let image = machine
        .allocate_image(
            width,
            height,
            colors,
            matches!(kind, CopyKind::Mutable | CopyKind::Rgb { .. }),
            &[Value::Reference(Some(graphics))],
        )
        .unwrap();
    let pixels = machine
        .graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
        .unwrap();
    (image, pixels)
}

fn copy_draw(
    machine: &mut Machine<'_, '_>,
    graphics: Handle,
    source: (Handle, Handle),
    kind: CopyKind,
    size: (i32, i32),
    origin: (i32, i32),
) -> Result<(), EmuError> {
    match kind {
        CopyKind::Rgb { alpha, negative } => machine.graphics_draw_rgb(&[
            Value::Reference(Some(graphics)),
            Value::Reference(Some(source.1)),
            Value::Int(if negative { (size.1 - 1) * size.0 } else { 0 }),
            Value::Int(if negative { -size.0 } else { size.0 }),
            Value::Int(origin.0),
            Value::Int(origin.1),
            Value::Int(size.0),
            Value::Int(size.1),
            Value::Int(i32::from(alpha)),
        ]),
        _ => machine.graphics_draw_image(&[
            Value::Reference(Some(graphics)),
            Value::Reference(Some(source.0)),
            Value::Int(origin.0),
            Value::Int(origin.1),
        ]),
    }
}

#[test]
fn contiguous_image_copies_match_independent_sample_passes() {
    let program = copy_program();
    let kinds = [
        CopyKind::Argb,
        CopyKind::Indexed,
        CopyKind::Mutable,
        CopyKind::Rgb {
            alpha: false,
            negative: false,
        },
        CopyKind::Rgb {
            alpha: true,
            negative: false,
        },
        CopyKind::Rgb {
            alpha: false,
            negative: true,
        },
        CopyKind::Rgb {
            alpha: true,
            negative: true,
        },
    ];
    for kind in kinds {
        for promoted in [false, true] {
            for origin in [(-2, -1), (1, 1), (5, 4)] {
                for clip in [(0, 0, 7, 6), (1, 1, 4, 3), (3, 2, 0, 0)] {
                    let render = |reference: bool| {
                        let mut host = EnhancedHost::default();
                        let mut machine = program.machine(Limits::default(), false, &mut host);
                        let (graphics, pixels) = direct_graphics_target(&mut machine, 7, 6);
                        for (index, slot) in machine
                            .graphics_int_array_mut(pixels)
                            .unwrap()
                            .iter_mut()
                            .enumerate()
                        {
                            *slot = HeapValue::Int((0x4010_2030_u32 | index as u32).cast_signed());
                        }
                        machine.ensure_enhanced_target(pixels, 7, 6).unwrap();
                        if promoted {
                            machine.enhanced_2d(graphics, None, 0, |_| Ok(())).unwrap();
                        }
                        let samples: Vec<u32> = (0..7 * 6 * 4_u32)
                            .map(|index| {
                                [0, 0x4000_0000, 0x8000_0000, 0xff00_0000][index as usize % 4]
                                    | index.wrapping_mul(0x0005_374b) & 0x00ff_ffff
                            })
                            .collect();
                        machine
                            .graphics
                            .enhanced
                            .store(pixels, &samples, [0, 0, 7, 6], false)
                            .unwrap();
                        let source = copy_source(&mut machine, graphics, kind, 4, 3, false);
                        super::super::graphics_support::set_graphics_fields(
                            &mut machine,
                            graphics,
                            &[
                                ("clipX", clip.0),
                                ("clipY", clip.1),
                                ("clipW", clip.2),
                                ("clipH", clip.3),
                            ],
                        );
                        let draw = |machine: &mut Machine<'_, '_>| {
                            copy_draw(machine, graphics, source, kind, (4, 3), origin)
                        };
                        if reference {
                            let image = (!matches!(kind, CopyKind::Rgb { .. })).then_some(source.0);
                            machine.enhanced_2d(graphics, image, 0, draw).unwrap();
                        } else {
                            draw(&mut machine).unwrap();
                        }
                        (
                            machine.graphics_int_array_snapshot(pixels).unwrap(),
                            machine.graphics.enhanced.pixels(pixels).unwrap().unwrap(),
                        )
                    };
                    assert_eq!(
                        render(false),
                        render(true),
                        "{kind:?} promoted={promoted} origin={origin:?} clip={clip:?}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "manual contiguous mixed-density image throughput measurement"]
fn enhanced_image_copy_throughput() {
    let program = copy_program();
    for kind in [
        CopyKind::Argb,
        CopyKind::Indexed,
        CopyKind::Mutable,
        CopyKind::Rgb {
            alpha: false,
            negative: false,
        },
        CopyKind::Rgb {
            alpha: true,
            negative: true,
        },
    ] {
        for opaque in [false, true] {
            if opaque && matches!(kind, CopyKind::Rgb { .. }) {
                continue;
            }
            for (scale, promoted) in [(1, false), (2, false), (2, true)] {
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
                if promoted {
                    machine.enhanced_2d(graphics, None, 0, |_| Ok(())).unwrap();
                }
                let source = copy_source(&mut machine, graphics, kind, 128, 96, opaque);
                copy_draw(&mut machine, graphics, source, kind, (128, 96), (8, 8)).unwrap();
                let start = std::time::Instant::now();
                for _ in 0..64 {
                    copy_draw(
                        &mut machine,
                        graphics,
                        std::hint::black_box(source),
                        kind,
                        (128, 96),
                        (8, 8),
                    )
                    .unwrap();
                }
                let elapsed = start.elapsed();
                let checksum = machine
                    .graphics_int_array_snapshot(pixels)
                    .unwrap()
                    .into_iter()
                    .map(i32::cast_unsigned)
                    .chain(
                        machine
                            .graphics
                            .enhanced
                            .pixels(pixels)
                            .unwrap()
                            .unwrap_or_default(),
                    )
                    .fold(0_u64, |sum, pixel| {
                        sum.wrapping_mul(31).wrapping_add(u64::from(pixel))
                    });
                eprintln!(
                    "copy kind={kind:?} opaque={opaque} scale={scale} promoted={promoted} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}

#[test]
#[ignore = "manual small clipped image throughput measurement"]
fn enhanced_small_image_copy_throughput() {
    for (side, repeats) in [(1, 32768), (16, 4096)] {
        let program = copy_program();
        for kind in [
            CopyKind::Argb,
            CopyKind::Indexed,
            CopyKind::Mutable,
            CopyKind::Rgb {
                alpha: false,
                negative: false,
            },
            CopyKind::Rgb {
                alpha: true,
                negative: true,
            },
        ] {
            for opaque in [false, true] {
                if opaque && matches!(kind, CopyKind::Rgb { .. }) {
                    continue;
                }
                for (scale, promoted) in [(1, false), (2, false), (2, true)] {
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
                    if promoted {
                        machine.enhanced_2d(graphics, None, 0, |_| Ok(())).unwrap();
                    }
                    super::super::graphics_support::set_graphics_fields(
                        &mut machine,
                        graphics,
                        &[("clipX", 8), ("clipY", 8), ("clipW", side), ("clipH", side)],
                    );
                    let source = copy_source(&mut machine, graphics, kind, 128, 96, opaque);
                    copy_draw(&mut machine, graphics, source, kind, (128, 96), (8, 8)).unwrap();
                    let start = std::time::Instant::now();
                    for _ in 0..repeats {
                        copy_draw(
                            &mut machine,
                            graphics,
                            std::hint::black_box(source),
                            kind,
                            (128, 96),
                            (8, 8),
                        )
                        .unwrap();
                    }
                    let elapsed = start.elapsed();
                    let checksum = machine
                        .graphics_int_array_snapshot(pixels)
                        .unwrap()
                        .into_iter()
                        .map(i32::cast_unsigned)
                        .chain(
                            machine
                                .graphics
                                .enhanced
                                .pixels(pixels)
                                .unwrap()
                                .unwrap_or_default(),
                        )
                        .fold(0_u64, |sum, pixel| {
                            sum.wrapping_mul(31).wrapping_add(u64::from(pixel))
                        });
                    eprintln!(
                        "small-copy side={side} kind={kind:?} opaque={opaque} scale={scale} promoted={promoted} elapsed={elapsed:?} checksum={checksum:016x}"
                    );
                }
            }
        }
    }
}

#[test]
fn contiguous_image_copies_keep_completed_samples_on_cancel_or_bad_source() {
    use std::cell::Cell;
    use std::rc::Rc;
    let program = copy_program();
    for kind in [
        CopyKind::Argb,
        CopyKind::Indexed,
        CopyKind::Mutable,
        CopyKind::Rgb {
            alpha: false,
            negative: false,
        },
        CopyKind::Rgb {
            alpha: true,
            negative: false,
        },
    ] {
        for promoted in [false, true] {
            for bad_source in [false, true] {
                if bad_source && matches!(kind, CopyKind::Argb | CopyKind::Indexed) {
                    continue;
                }
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
                let (graphics, pixels) = direct_graphics_target(&mut machine, 8192, 1);
                machine.ensure_enhanced_target(pixels, 8192, 1).unwrap();
                if promoted {
                    machine.enhanced_2d(graphics, None, 0, |_| Ok(())).unwrap();
                }
                let source = copy_source(&mut machine, graphics, kind, 8192, 1, false);
                if bad_source {
                    machine.graphics_int_array_mut(source.1).unwrap()[107] =
                        HeapValue::Reference(None);
                }
                checks.set(0);
                if !bad_source {
                    cancel_at.set(2);
                }
                let error =
                    copy_draw(&mut machine, graphics, source, kind, (8192, 1), (0, 0)).unwrap_err();
                assert_eq!(
                    error.code(),
                    if bad_source {
                        "type-mismatch"
                    } else {
                        "execution-cancelled"
                    }
                );
                if !bad_source {
                    assert_eq!(checks.get(), 2);
                }
                let logical = machine.graphics_int_array_snapshot(pixels).unwrap();
                let samples = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
                for (index, color) in logical.into_iter().enumerate() {
                    for phase in 0..4 {
                        assert_eq!(
                            samples[(phase / 2) * 8192 * 2 + index * 2 + phase % 2],
                            color.cast_unsigned(),
                            "{kind:?} promoted={promoted} bad_source={bad_source} index={index} phase={phase}"
                        );
                    }
                }
            }
        }
    }
}
