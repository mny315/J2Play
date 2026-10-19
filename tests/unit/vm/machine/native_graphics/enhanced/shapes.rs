use super::super::graphics_support::set_graphics_fields;
use super::*;

#[derive(Clone, Copy, Debug)]
enum Shape<'a> {
    Arc([i32; 6]),
    Polygon(&'a [(i32, i32)]),
}

impl Shape<'_> {
    fn draw(
        self,
        machine: &mut Machine<'_, '_>,
        graphics: Handle,
        color: i32,
        fill: bool,
    ) -> Result<(), EmuError> {
        match self {
            Self::Arc([x, y, width, height, start, sweep]) => machine.graphics_draw_arc(
                &[
                    Value::Reference(Some(graphics)),
                    Value::Int(x),
                    Value::Int(y),
                    Value::Int(width),
                    Value::Int(height),
                    Value::Int(start),
                    Value::Int(sweep),
                ],
                fill,
            ),
            Self::Polygon(points) => {
                machine.graphics_render_nokia_polygon(graphics, points, color, fill)
            }
        }
    }
}

#[test]
fn enhanced_shapes_match_independent_sample_passes_with_shared_coverage() {
    let program = Program::new();
    let mut polygons = vec![
        vec![],
        vec![(1, 1)],
        vec![(0, 0), (6, 5)],
        vec![(0, 0), (6, 0), (6, 5), (0, 5)],
        vec![(0, 0), (6, 5), (6, 0), (0, 5)],
        vec![(i32::MIN, 1), (3, i32::MAX), (i32::MAX, -2), (0, i32::MIN)],
    ];
    for count in [15, 16, 17, 32, 65] {
        polygons.push(
            (0..count)
                .map(|index| [(0, 0), (6, 5), (6, 0), (0, 5)][index % 4])
                .collect(),
        );
    }
    let mut shapes: Vec<_> = polygons
        .iter()
        .map(|points| Shape::Polygon(points))
        .collect();
    shapes.extend(
        [
            [0, 0, 6, 5, 0, 360],
            [-1, 2, 5, 3, -45, 180],
            [0, 0, 4, 0, 0, 360],
            [5, 4, -1, 3, 0, 360],
            [0, 0, 5, 5, i32::MIN, i32::MAX],
            [-1, 0, 9, 7, 53, -270],
        ]
        .map(Shape::Arc),
    );
    for shape in shapes {
        for fill in [false, true] {
            for promoted in [false, true] {
                for color in [0x0012_3456_u32, 0x8047_294b, 0xff98_7654] {
                    for dotted in [false, true] {
                        for (tx, ty, clip) in [
                            (0, 0, (0, 0, 7, 6)),
                            (-1, 2, (1, 1, 4, 3)),
                            (0, 0, (3, 2, 0, 0)),
                            (i32::MAX, i32::MIN, (0, 0, 7, 6)),
                        ] {
                            let render = |reference| {
                                let mut host = EnhancedHost::default();
                                let mut machine =
                                    program.machine(Limits::default(), false, &mut host);
                                let (graphics, pixels) = direct_graphics_target(&mut machine, 7, 6);
                                for (index, slot) in machine
                                    .graphics_int_array_mut(pixels)
                                    .unwrap()
                                    .iter_mut()
                                    .enumerate()
                                {
                                    *slot = HeapValue::Int(
                                        ([0_u32, 0x4000_0000, 0x8000_0000, 0xff00_0000][index % 4]
                                            | (index as u32).wrapping_mul(0x0005_374b)
                                                & 0x00ff_ffff)
                                            .cast_signed(),
                                    );
                                }
                                machine.ensure_enhanced_target(pixels, 7, 6).unwrap();
                                if promoted {
                                    machine.enhanced_2d(graphics, None, 0, |_| Ok(())).unwrap();
                                }
                                let samples: Vec<u32> = (0..7 * 6 * 4_u32)
                                    .map(|index| {
                                        [0_u32, 0x4000_0000, 0x8000_0000, 0xff00_0000]
                                            [index as usize % 4]
                                            | index.wrapping_mul(0x0003_751b) & 0x00ff_ffff
                                    })
                                    .collect();
                                machine
                                    .graphics
                                    .enhanced
                                    .store(pixels, &samples, [0, 0, 7, 6], false)
                                    .unwrap();
                                set_graphics_fields(
                                    &mut machine,
                                    graphics,
                                    &[
                                        ("color", color.cast_signed()),
                                        ("stroke", i32::from(dotted)),
                                        ("tx", tx),
                                        ("ty", ty),
                                        ("clipX", clip.0),
                                        ("clipY", clip.1),
                                        ("clipW", clip.2),
                                        ("clipH", clip.3),
                                    ],
                                );
                                let draw = |machine: &mut Machine<'_, '_>| {
                                    shape.draw(machine, graphics, color.cast_signed(), fill)
                                };
                                if reference {
                                    machine.enhanced_2d(graphics, None, 0, draw).unwrap();
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
                                "{shape:?} fill={fill} promoted={promoted} color={color:08x} dotted={dotted} translate={tx},{ty} clip={clip:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "manual mixed-density arc and polygon throughput measurement"]
fn enhanced_shape_throughput() {
    let program = Program::new();
    for size in [9, 257] {
        for alpha in [128_u32, 255] {
            for arc in [false, true] {
                for fill in [false, true] {
                    for (scale, promoted) in [(1, false), (2, false), (2, true)] {
                        let mut host = EnhancedHost {
                            original: scale == 1,
                            ..EnhancedHost::default()
                        };
                        let mut machine = program.machine(Limits::default(), false, &mut host);
                        let (graphics, pixels) = direct_graphics_target(&mut machine, size, size);
                        machine
                            .graphics_int_array_mut(pixels)
                            .unwrap()
                            .fill(HeapValue::Int(0xff10_2030_u32.cast_signed()));
                        assert_eq!(
                            machine
                                .ensure_enhanced_target(pixels, size as u32, size as u32)
                                .unwrap(),
                            scale == 2
                        );
                        if promoted {
                            machine.enhanced_2d(graphics, None, 0, |_| Ok(())).unwrap();
                        }
                        let color = (alpha << 24 | 0x12_3456).cast_signed();
                        set_graphics_fields(&mut machine, graphics, &[("color", color)]);
                        let points = [(0, 0), (size - 1, size - 1), (0, size - 1), (size - 1, 0)];
                        let shape = if arc {
                            Shape::Arc([0, 0, size - 1, size - 1, 0, 360])
                        } else {
                            Shape::Polygon(&points)
                        };
                        shape.draw(&mut machine, graphics, color, fill).unwrap();
                        let started = std::time::Instant::now();
                        for _ in 0..if size == 9 { 1024 } else { 16 } {
                            std::hint::black_box(shape)
                                .draw(&mut machine, graphics, color, fill)
                                .unwrap();
                        }
                        let elapsed = started.elapsed();
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
                            "enhanced-shape size={size} alpha={alpha} arc={arc} fill={fill} scale={scale} promoted={promoted} elapsed={elapsed:?} checksum={checksum:016x}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn enhanced_shapes_cancel_long_rows_and_can_draw_again() {
    use std::cell::Cell;
    use std::rc::Rc;
    let program = Program::new();
    for arc in [false, true] {
        for fill in [false, true] {
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
            let (graphics, pixels) = direct_graphics_target(&mut machine, 8192, 2);
            machine.ensure_enhanced_target(pixels, 8192, 2).unwrap();
            machine.enhanced_2d(graphics, None, 0, |_| Ok(())).unwrap();
            let color = 0x8047_294b_u32.cast_signed();
            set_graphics_fields(&mut machine, graphics, &[("color", color)]);
            let points = [(0, 0), (8191, 0), (8191, 1), (0, 1)];
            let shape = if arc {
                Shape::Arc([0, 0, 8191, 1, 0, 360])
            } else {
                Shape::Polygon(&points)
            };
            checks.set(0);
            cancel_at.set(5);
            assert_eq!(
                shape
                    .draw(&mut machine, graphics, color, fill)
                    .unwrap_err()
                    .code(),
                "execution-cancelled"
            );
            assert_eq!(checks.get(), 5);
            if arc {
                // A cancelled single-pass arc must publish the same prefix
                // to the canonical buffer and every retained sample plane.
                let logical = machine.graphics_int_array_snapshot(pixels).unwrap();
                let samples = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
                for (index, color) in logical.into_iter().enumerate() {
                    let (x, y) = (index % 8192, index / 8192);
                    for phase in 0..4 {
                        assert_eq!(
                            samples[(y * 2 + phase / 2) * 8192 * 2 + x * 2 + phase % 2],
                            color.cast_unsigned()
                        );
                    }
                }
            }
            cancel_at.set(usize::MAX);
            machine.ensure_enhanced_target(pixels, 8192, 2).unwrap();
            shape.draw(&mut machine, graphics, color, fill).unwrap();
            let logical = machine.graphics_int_array_snapshot(pixels).unwrap();
            let samples = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
            for (index, color) in logical.into_iter().enumerate() {
                let (x, y) = (index % 8192, index / 8192);
                for phase in 0..4 {
                    assert_eq!(
                        samples[(y * 2 + phase / 2) * 8192 * 2 + x * 2 + phase % 2],
                        color.cast_unsigned()
                    );
                }
            }
        }
    }
}
