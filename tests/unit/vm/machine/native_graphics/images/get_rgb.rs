use super::*;

#[test]
fn get_rgb_preserves_regions_padding_and_alpha_for_every_storage_format() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for storage in ["managed", "argb", "indexed"] {
        let pixels = (0..320_u32)
            .map(|index| {
                (if storage == "indexed" {
                    RGB_PIXELS[index as usize % 4].cast_unsigned()
                } else {
                    index.wrapping_mul(0x1020_3041)
                })
                .cast_signed()
            })
            .collect::<Vec<_>>();
        let image = machine
            .allocate_image(16, 20, pixels.clone(), storage == "managed", &[])
            .unwrap();
        for [x, y, width, height] in [[0, 0, 16, 20], [3, 2, 7, 11], [15, 19, 1, 1]] {
            for negative in [false, true] {
                let stride = width + 3;
                let offset = 2 + if negative { (height - 1) * stride } else { 0 };
                let stride = if negative { -stride } else { stride };
                let destination = machine
                    .heap
                    .managed
                    .allocate_array(ArrayKind::Int, 400)
                    .unwrap();
                machine
                    .graphics_int_array_mut(destination)
                    .unwrap()
                    .fill(Value::Int(77));
                machine
                    .image_get_rgb(&rgb_args(
                        image,
                        destination,
                        [offset, stride, x, y, width, height],
                    ))
                    .unwrap();
                let mut expected = vec![77; 400];
                for row in 0..height {
                    for column in 0..width {
                        expected[(offset + row * stride + column) as usize] =
                            pixels[((y + row) * 16 + x + column) as usize];
                    }
                }
                assert_eq!(
                    machine.graphics_int_array_snapshot(destination).unwrap(),
                    expected,
                    "{storage}, negative={negative}"
                );
            }
        }
    }
}

#[test]
fn get_rgb_rejects_short_backing_before_writing_the_destination() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for mutable in [false, true] {
        let image = machine
            .allocate_image(2, 2, RGB_PIXELS.to_vec(), mutable, &[])
            .unwrap();
        machine
            .heap
            .managed
            .set_field(
                image,
                "javax/microedition/lcdui/Image.height:I",
                Value::Int(3),
            )
            .unwrap();
        let destination = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 6)
            .unwrap();
        machine
            .graphics_int_array_mut(destination)
            .unwrap()
            .fill(Value::Int(77));
        let error = machine
            .image_get_rgb(&rgb_args(image, destination, [0, 2, 0, 0, 2, 3]))
            .unwrap_err();
        assert_eq!(error.code(), "array-index-out-of-bounds-exception");
        assert_eq!(
            machine.graphics_int_array_snapshot(destination).unwrap(),
            [77; 6]
        );
    }
}

#[test]
fn get_rgb_validates_all_requested_source_values_before_writing() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for alias in [false, true] {
        let image = machine
            .allocate_image(2, 2, RGB_PIXELS.to_vec(), true, &[])
            .unwrap();
        let source = machine
            .graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
            .unwrap();
        machine.graphics_int_array_mut(source).unwrap()[3] = HeapValue::Reference(None);
        let destination = if alias {
            source
        } else {
            machine
                .heap
                .managed
                .allocate_array(ArrayKind::Int, 4)
                .unwrap()
        };
        let before = machine
            .graphics_int_array_mut(destination)
            .unwrap()
            .to_vec();
        assert_eq!(
            machine
                .image_get_rgb(&rgb_args(image, destination, [2, -2, 0, 0, 2, 2]))
                .unwrap_err()
                .code(),
            "type-mismatch"
        );
        assert_eq!(machine.graphics_int_array_mut(destination).unwrap(), before);
        // An invalid value outside the requested region must not reject it.
        machine
            .image_get_rgb(&rgb_args(image, destination, [0, 1, 1, 0, 1, 1]))
            .unwrap();
        assert_eq!(
            machine.graphics_int_array_mut(destination).unwrap()[0],
            HeapValue::Int(RGB_PIXELS[1])
        );
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn get_rgb_throughput() {
    let program = program();
    for (width, height, iterations) in [(8, 8, 4096), (240, 320, 128)] {
        for storage in ["managed", "argb", "indexed"] {
            for partial in [false, true] {
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let pixels = (0..width * height)
                    .map(|index| {
                        (if storage == "indexed" {
                            RGB_PIXELS[index as usize % 4].cast_unsigned()
                        } else {
                            (index as u32).wrapping_mul(0x1020_3041)
                        })
                        .cast_signed()
                    })
                    .collect();
                let image = machine
                    .allocate_image(width, height, pixels, storage == "managed", &[])
                    .unwrap();
                let destination = machine
                    .heap
                    .managed
                    .allocate_array(ArrayKind::Int, width * height)
                    .unwrap();
                let region = if partial {
                    [width / 4, height / 4, width / 2, height / 2]
                } else {
                    [0, 0, width, height]
                };
                let [x, y, copy_width, copy_height] = region;
                let args = rgb_args(
                    image,
                    destination,
                    [
                        (copy_height - 1) * width,
                        -width,
                        x,
                        y,
                        copy_width,
                        copy_height,
                    ],
                );
                let started = std::time::Instant::now();
                for _ in 0..iterations {
                    machine.image_get_rgb(std::hint::black_box(&args)).unwrap();
                }
                let elapsed = started.elapsed();
                let checksum = machine
                    .graphics_int_array_snapshot(destination)
                    .unwrap()
                    .into_iter()
                    .fold(0_u64, |hash, pixel| {
                        hash.wrapping_mul(31)
                            .wrapping_add(u64::from(pixel.cast_unsigned()))
                    });
                eprintln!(
                    "get-rgb storage={storage} size={width}x{height} partial={partial} elapsed={elapsed:?} checksum={checksum:x}"
                );
            }
        }
    }
}
