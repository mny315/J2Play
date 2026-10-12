use super::*;

#[test]
#[ignore = "manual DirectGraphics pixel transfer throughput measurement"]
fn nokia_pixel_transfer_throughput() {
    let program = Program::new();
    let width = 240;
    let height = 320;
    for (kind, format, sample) in [
        (ArrayKind::Int, 8888, 0x8040_80c0_u32.cast_signed()),
        (ArrayKind::Short, 4444, 0x848c),
        (ArrayKind::Short, 565, 0x4418),
    ] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let (graphics, pixels) = direct_graphics_target(&mut machine, width, height);
        let source = machine
            .heap
            .managed
            .allocate_array(kind.clone(), width * height)
            .unwrap();
        let destination = machine
            .heap
            .managed
            .allocate_array(kind, width * height)
            .unwrap();
        for index in 0..width * height {
            machine
                .heap
                .managed
                .array_set(source, index, HeapValue::Int(sample ^ (index & 15)))
                .unwrap();
        }
        for transform in [0, 90] {
            let (source_width, source_height) = if transform == 0 {
                (width, height)
            } else {
                (height, width)
            };
            let draw = direct_pixel_args(
                graphics,
                source,
                [
                    1,
                    0,
                    source_width,
                    0,
                    0,
                    source_width,
                    source_height,
                    transform,
                    format,
                ],
            );
            let get = get_pixels_args(
                graphics,
                destination,
                [0, width, 0, 0, width, height, format],
            );
            for drawing in [true, false] {
                let started = std::time::Instant::now();
                for _ in 0..128 {
                    if drawing {
                        machine
                            .graphics_nokia_draw_pixels(std::hint::black_box(&draw))
                            .unwrap();
                    } else {
                        machine
                            .graphics_nokia_get_pixels(std::hint::black_box(&get))
                            .unwrap();
                    }
                }
                let elapsed = started.elapsed();
                let output = if drawing { pixels } else { destination };
                let checksum = (0..width * height).fold(0_u64, |hash, index| {
                    let HeapValue::Int(pixel) =
                        machine.heap.managed.array_get(output, index).unwrap()
                    else {
                        panic!("pixel array contained a non-integer");
                    };
                    hash.wrapping_mul(31)
                        .wrapping_add(u64::from(pixel.cast_unsigned()))
                });
                eprintln!(
                    "nokia format={format} transform={transform} draw={drawing} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}
