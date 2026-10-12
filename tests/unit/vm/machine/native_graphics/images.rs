use super::*;
use crate::machine::{DefaultNativeContext, Limits, Program};

#[path = "images/get_rgb.rs"]
mod get_rgb;

fn program() -> Program {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(2, 1) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    program
}

const RGB_PIXELS: [i32; 4] = [
    0xff11_2233_u32.cast_signed(),
    0x8044_5566_u32.cast_signed(),
    0x0077_8899,
    0xaabb_ccdd_u32.cast_signed(),
];

fn rgb_args(image: Handle, destination: Handle, values: [i32; 6]) -> Vec<Value> {
    let mut args = vec![
        Value::Reference(Some(image)),
        Value::Reference(Some(destination)),
    ];
    args.extend(values.map(Value::Int));
    args
}

#[test]
fn get_rgb_checks_source_rectangle_and_scanlength() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for mutable in [false, true] {
        let image = machine
            .allocate_image(2, 2, RGB_PIXELS.to_vec(), mutable, &[])
            .unwrap();
        let destination = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 8)
            .unwrap();
        machine
            .graphics_int_array_mut(destination)
            .unwrap()
            .fill(HeapValue::Int(77));
        for values in [
            [0, i32::MAX, 0, 0, i32::MAX, i32::MAX],
            [0, 2, -1, 1, 1, 1],
            [0, 2, 1, 0, 2, 1],
            [0, 2, 0, 1, 2, 2],
            [0, 1, 0, 0, 2, 2],
            [0, -1, 0, 0, 2, 2],
            [0, 0, 0, 0, 2, 2],
            [0, 2, 0, 0, 0, i32::MAX],
            [0, 2, 0, 0, i32::MAX, 0],
        ] {
            assert_eq!(
                machine
                    .image_get_rgb(&rgb_args(image, destination, values))
                    .unwrap_err()
                    .code(),
                "illegal-argument",
                "mutable={mutable} args={values:?}"
            );
            assert_eq!(
                machine.graphics_int_array_snapshot(destination).unwrap(),
                [77; 8]
            );
        }
    }
}

#[test]
fn get_rgb_destination_bounds_failure_does_not_write_partial_rows() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let image = machine
        .allocate_image(2, 2, RGB_PIXELS.to_vec(), false, &[])
        .unwrap();
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 3)
        .unwrap();
    for (offset, scanlength) in [(0, 2), (2, -2), (-1, 2), (i32::MAX, i32::MAX)] {
        machine
            .graphics_int_array_mut(destination)
            .unwrap()
            .fill(HeapValue::Int(77));
        assert_eq!(
            machine
                .image_get_rgb(&rgb_args(
                    image,
                    destination,
                    [offset, scanlength, 0, 0, 2, 2]
                ))
                .unwrap_err()
                .code(),
            "array-index-out-of-bounds-exception"
        );
        assert_eq!(
            machine.graphics_int_array_snapshot(destination).unwrap(),
            [77; 3]
        );
    }
}

#[test]
fn get_rgb_nonpositive_extents_do_not_copy_pixels() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let image = machine
        .allocate_image(2, 2, RGB_PIXELS.to_vec(), false, &[])
        .unwrap();
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    machine
        .graphics_int_array_mut(destination)
        .unwrap()
        .fill(HeapValue::Int(77));
    for [x, y, width, height] in [
        [0, 0, -1, 1],
        [0, 0, 1, -1],
        [2, 0, 0, 2],
        [0, 2, 2, 0],
        [i32::MAX, 0, i32::MIN, 1],
        [0, i32::MAX, 1, i32::MIN],
    ] {
        machine
            .image_get_rgb(&rgb_args(
                image,
                destination,
                [i32::MIN, i32::MIN, x, y, width, height],
            ))
            .unwrap();
        assert_eq!(
            machine.graphics_int_array_snapshot(destination).unwrap(),
            [77]
        );
    }
}

#[test]
fn get_rgb_preserves_argb_with_negative_scanlength() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for mutable in [false, true] {
        let image = machine
            .allocate_image(2, 2, RGB_PIXELS.to_vec(), mutable, &[])
            .unwrap();
        let destination = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 7)
            .unwrap();
        machine
            .graphics_int_array_mut(destination)
            .unwrap()
            .fill(HeapValue::Int(77));
        machine
            .image_get_rgb(&rgb_args(image, destination, [3, -3, 0, 0, 2, 2]))
            .unwrap();
        assert_eq!(
            machine.graphics_int_array_snapshot(destination).unwrap(),
            [
                RGB_PIXELS[2],
                RGB_PIXELS[3],
                77,
                RGB_PIXELS[0],
                RGB_PIXELS[1],
                77,
                77
            ]
        );
        machine
            .image_get_rgb(&rgb_args(image, destination, [0, i32::MIN, 1, 1, 1, 1]))
            .unwrap();
        assert_eq!(
            machine.graphics_int_array_snapshot(destination).unwrap()[0],
            RGB_PIXELS[3]
        );
    }
}

#[test]
fn get_rgb_preserves_the_source_when_destination_aliases_it() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let image = machine
        .allocate_image(2, 2, RGB_PIXELS.to_vec(), true, &[])
        .unwrap();
    let pixels = machine
        .graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
        .unwrap();
    machine
        .image_get_rgb(&rgb_args(image, pixels, [2, -2, 0, 0, 2, 2]))
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        [RGB_PIXELS[2], RGB_PIXELS[3], RGB_PIXELS[0], RGB_PIXELS[1]]
    );
}

#[test]
fn region_snapshot_checks_backing_length_before_allocating() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    assert_eq!(
        machine
            .graphics_int_array_region_snapshot(pixels, i32::MAX, 0, 0, i32::MAX, i32::MAX)
            .unwrap_err()
            .code(),
        "array-index-out-of-bounds-exception"
    );
}

#[test]
fn pixel_range_copies_preserve_argb_across_storage_formats() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for pixels in [
        (0..32)
            .map(|index| RGB_PIXELS[index % 4])
            .collect::<Vec<_>>(),
        (0..32).map(|index| 0x1234_5600 | index).collect(),
    ] {
        for mutable in [false, true] {
            let image = machine
                .allocate_image(8, 4, pixels.clone(), mutable, &[])
                .unwrap();
            let source = machine
                .graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
                .unwrap();
            assert_eq!(
                machine
                    .graphics_int_array_region_snapshot(source, 8, 3, 1, 2, 2)
                    .unwrap(),
                [pixels[11], pixels[12], pixels[19], pixels[20]]
            );
            assert_eq!(
                machine
                    .graphics_int_array_region_snapshot(source, 8, 0, 1, 8, 2)
                    .unwrap(),
                pixels[8..24]
            );
            for process_alpha in [false, true] {
                for (offset, length) in [(0, 0), (0, 32), (11, 7), (32, 0)] {
                    let copy = machine
                        .image_copy_pixels(&[
                            Value::Reference(Some(source)),
                            Value::Int(offset),
                            Value::Int(length),
                            Value::Int(i32::from(process_alpha)),
                        ])
                        .unwrap();
                    let expected = pixels[offset as usize..(offset + length) as usize]
                        .iter()
                        .map(|pixel| {
                            if process_alpha {
                                *pixel
                            } else {
                                *pixel | 0xff00_0000_u32.cast_signed()
                            }
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(machine.graphics_int_array_snapshot(copy).unwrap(), expected);
                }
            }
            for (offset, length) in [(-1, 0), (0, -1), (31, 2), (33, 0), (1, i32::MAX)] {
                assert_eq!(
                    machine
                        .image_copy_pixels(&[
                            Value::Reference(Some(source)),
                            Value::Int(offset),
                            Value::Int(length),
                            Value::Int(1),
                        ])
                        .unwrap_err()
                        .code(),
                    "array-index-out-of-bounds-exception"
                );
            }
            assert_eq!(machine.graphics_int_array_snapshot(source).unwrap(), pixels);
        }
    }
}

#[test]
fn immutable_image_copy_reuses_the_source_without_another_heap_allocation() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 80,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let pixels = vec![0, 0x8012_3456_u32.cast_signed()];
    let source = machine
        .allocate_image(2, 1, pixels.clone(), false, &[])
        .unwrap();
    let bytes = machine.heap.managed.bytes();
    let allocations = machine.heap.managed.live_allocations().count();
    for _ in 0..8 {
        let copy = machine
            .image_create_copy(&[Value::Reference(Some(source))])
            .unwrap();
        assert_eq!(copy, source);
        assert_eq!(machine.heap.managed.bytes(), bytes);
        assert_eq!(machine.heap.managed.live_allocations().count(), allocations);
    }
    let backing = machine
        .graphics_reference_field(source, "javax/microedition/lcdui/Image.pixels:[I")
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(backing).unwrap(),
        pixels
    );
    assert_eq!(
        machine
            .image_create_copy(&[Value::Reference(None)])
            .unwrap_err()
            .code(),
        "null-pointer-exception"
    );
}

#[test]
fn mutable_image_copy_keeps_pixels_independent_from_later_source_edits() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = vec![0xff12_3456_u32.cast_signed(), -1];
    let source = machine
        .allocate_image(2, 1, pixels.clone(), true, &[])
        .unwrap();
    let copy = machine
        .image_create_copy(&[Value::Reference(Some(source))])
        .unwrap();
    assert_ne!(source, copy);
    assert_eq!(
        machine
            .graphics_int_field(copy, "javax/microedition/lcdui/Image.mutable:Z")
            .unwrap(),
        0
    );
    let original_backing = machine
        .graphics_reference_field(source, "javax/microedition/lcdui/Image.pixels:[I")
        .unwrap();
    let copy_backing = machine
        .graphics_reference_field(copy, "javax/microedition/lcdui/Image.pixels:[I")
        .unwrap();
    assert_ne!(original_backing, copy_backing);
    machine
        .heap
        .managed
        .array_set(original_backing, 0, HeapValue::Int(0))
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(copy_backing).unwrap(),
        pixels
    );
}
