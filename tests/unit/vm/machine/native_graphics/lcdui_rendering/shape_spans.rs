use super::*;

#[test]
#[ignore = "manual release throughput measurement"]
fn shape_rasterization_throughput() {
    let program = Program::new();
    for size in [9, 257] {
        for alpha in [128_u32, 255] {
            for arc in [false, true] {
                for fill in [false, true] {
                    let mut context = DefaultNativeContext;
                    let mut machine = program.machine(Limits::default(), false, &mut context);
                    let (graphics, pixels) = direct_graphics_target(&mut machine, size, size);
                    let color = ((alpha << 24) | 0x12_3456).cast_signed();
                    set_graphics_fields(&mut machine, graphics, &[("color", color)]);
                    let points = [(0, 0), (size - 1, size - 1), (0, size - 1), (size - 1, 0)];
                    let mut args = vec![Value::Reference(Some(graphics))];
                    args.extend([0, 0, size - 1, size - 1, 0, 360].map(Value::Int));
                    let started = std::time::Instant::now();
                    for _ in 0..128 {
                        machine
                            .graphics_int_array_mut(pixels)
                            .unwrap()
                            .fill(HeapValue::Int(-1));
                        if arc {
                            machine
                                .graphics_draw_arc(std::hint::black_box(&args), fill)
                                .unwrap();
                        } else {
                            machine
                                .graphics_render_nokia_polygon(
                                    graphics,
                                    std::hint::black_box(&points),
                                    color,
                                    fill,
                                )
                                .unwrap();
                        }
                    }
                    let elapsed = started.elapsed();
                    let checksum = machine
                        .graphics_int_array_snapshot(pixels)
                        .unwrap()
                        .into_iter()
                        .fold(0_u64, |sum, pixel| {
                            sum.rotate_left(1) ^ u64::from(pixel.cast_unsigned())
                        });
                    eprintln!(
                        "shape size={size} alpha={alpha} arc={arc} fill={fill} elapsed={elapsed:?} checksum={checksum:016x}"
                    );
                }
            }
        }
    }
}
