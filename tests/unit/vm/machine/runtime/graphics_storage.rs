use super::*;
use std::cell::Cell;
use std::rc::Rc;

mod egl;
mod encoded_images;
mod jsr239_checkpoint;
mod jsr239_rendering;
mod jsr239_storage;
mod nio;

fn nio_reference_call(
    machine: &mut Machine<'_, '_>,
    class: &str,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Handle {
    let method = machine.program.methods[&MethodKey {
        class: class.into(),
        name: name.into(),
        descriptor: descriptor.into(),
    }]
        .clone();
    match machine.invoke_nio_native(&method, args).unwrap() {
        CallOutcome::Return(Some(Value::Reference(Some(handle)))) => handle,
        _ => panic!("NIO call did not return its buffer"),
    }
}

fn nio_float_values(machine: &Machine<'_, '_>, buffer: Handle) -> Result<Vec<f32>, EmuError> {
    let pointer = machine.jsr239_float_pointer(buffer, 2, 0)?;
    nio_float_words(machine.jsr239_float_pointer_words(&pointer, 0..pointer.components())?)
}

fn nio_float_words(words: &[[HeapValue; 4]]) -> Result<Vec<f32>, EmuError> {
    words
        .iter()
        .map(|word| {
            let [
                HeapValue::Int(a),
                HeapValue::Int(b),
                HeapValue::Int(c),
                HeapValue::Int(d),
            ] = *word
            else {
                return Err(type_error());
            };
            Ok(f32::from_ne_bytes([a as u8, b as u8, c as u8, d as u8]))
        })
        .collect()
}

fn jsr239_call(
    machine: &mut Machine<'_, '_>,
    name: &str,
    descriptor: &str,
    arguments: &[Value],
) -> Result<CallOutcome, EmuError> {
    let method = machine.program.methods[&MethodKey {
        class: "javax/microedition/khronos/opengles/GLImpl".into(),
        name: name.into(),
        descriptor: descriptor.into(),
    }]
        .clone();
    machine.call(&method, arguments, 1)
}

fn gl_call(machine: &mut Machine<'_, '_>, name: &str, values: &[i32]) -> CallOutcome {
    let mut arguments = vec![Value::Reference(machine.jsr239.gl)];
    arguments.extend(values.iter().copied().map(Value::Int));
    jsr239_call(
        machine,
        name,
        &format!("({})V", "I".repeat(values.len())),
        &arguments,
    )
    .unwrap()
}

fn gl_set(machine: &mut Machine<'_, '_>, name: &str, values: &[i32]) {
    assert!(matches!(
        gl_call(machine, name, values),
        CallOutcome::Return(None)
    ));
}

fn jsr239_texture<'a>(machine: &'a Machine<'_, '_>, name: i32) -> &'a m3g::Texture2DState {
    machine.jsr239.textures[&name].image.as_ref().unwrap()
}

fn jsr239_canvas_target(machine: &mut Machine<'_, '_>) -> Handle {
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 16)
        .unwrap();
    let image = machine
        .heap
        .managed
        .allocate_object(
            "javax/microedition/lcdui/Image",
            HashMap::from([
                (
                    "javax/microedition/lcdui/Image.width:I".into(),
                    HeapValue::Int(4),
                ),
                (
                    "javax/microedition/lcdui/Image.height:I".into(),
                    HeapValue::Int(4),
                ),
                (
                    "javax/microedition/lcdui/Image.pixels:[I".into(),
                    HeapValue::Reference(Some(pixels)),
                ),
            ]),
        )
        .unwrap();
    let canvas = machine
        .heap
        .managed
        .allocate_object(
            "javax/microedition/lcdui/Canvas",
            HashMap::from([(
                "javax/microedition/lcdui/Canvas.framebuffer:Ljavax/microedition/lcdui/Image;"
                    .into(),
                HeapValue::Reference(Some(image)),
            )]),
        )
        .unwrap();
    machine.jsr239.target = Some(canvas);
    pixels
}

#[test]
pub(crate) fn image_region_transforms_match_midp_sprite_coordinates() {
    let source = [1, 2, 3, 4, 5, 6];
    let expected = [
        (2, 3, vec![1, 2, 3, 4, 5, 6]),
        (2, 3, vec![5, 6, 3, 4, 1, 2]),
        (2, 3, vec![2, 1, 4, 3, 6, 5]),
        (2, 3, vec![6, 5, 4, 3, 2, 1]),
        (3, 2, vec![1, 3, 5, 2, 4, 6]),
        (3, 2, vec![5, 3, 1, 6, 4, 2]),
        (3, 2, vec![2, 4, 6, 1, 3, 5]),
        (3, 2, vec![6, 4, 2, 5, 3, 1]),
    ];
    for (transform, (width, height, pixels)) in expected.into_iter().enumerate() {
        assert_eq!(
            transform_image_pixels(source.to_vec(), 2, 3, transform as i32).unwrap(),
            (width, height, pixels),
            "transform {transform}"
        );
    }
    assert_eq!(
        transform_image_pixels(source.to_vec(), 2, 3, 8)
            .unwrap_err()
            .code(),
        "illegal-argument"
    );
    assert_eq!(
        transform_image_pixels(source[..5].to_vec(), 2, 3, 0)
            .unwrap_err()
            .code(),
        "array-index-out-of-bounds-exception"
    );
}

#[test]
#[ignore = "release throughput fixture"]
fn immutable_palette_creation_throughput() {
    for (count, iterations) in [(1, 32_768), (64, 8192), (1024, 1024), (76_800, 16)] {
        let colors = [1, 4, 16, 256, 257, count]
            .into_iter()
            .filter(|colors| *colors <= count)
            .collect::<std::collections::BTreeSet<_>>();
        for colors in colors {
            let pixels = (0..count)
                .map(|index| ((index % colors) as u32).wrapping_mul(0x9e37_79b9) as i32)
                .collect::<Vec<_>>();
            let expected = ImmutableImagePixels::from_argb(pixels.clone());
            assert_eq!(expected.len(), count);
            for (index, pixel) in pixels.iter().enumerate() {
                assert_eq!(expected.get(index), Some(*pixel));
            }
            let inputs = vec![pixels; iterations];
            let mut checksum = 0_usize;
            let start = std::time::Instant::now();
            for input in inputs {
                let result = std::hint::black_box(ImmutableImagePixels::from_argb(input));
                checksum = checksum.wrapping_add(result.storage_bytes().unwrap());
            }
            println!(
                "palette-create pixels={count} colors={colors} ns={} indexed={} bytes={} checksum={checksum:016x}",
                start.elapsed().as_nanos(),
                expected.is_indexed(),
                expected.storage_bytes().unwrap(),
            );
        }
    }
}

#[test]
pub(crate) fn palette_backed_immutable_pixels_are_lossless_and_fit_a_tight_heap() {
    let original = (0..1_024)
        .map(|index| {
            [
                0x0000_0000,
                0xff11_2233_u32.cast_signed(),
                0x8044_5566_u32.cast_signed(),
                0xffff_ffff_u32.cast_signed(),
            ][index % 4]
        })
        .collect::<Vec<_>>();
    let pixels = Arc::new(ImmutableImagePixels::from_argb(original.clone()));
    assert!(pixels.is_indexed());
    assert_eq!(pixels.storage_bytes(), Some(1_024 + 4 * 4));

    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        // int[0] token (24) + four ARGB palette entries (16) + 1024
        // byte indices. A direct ARGB backing would need 4,096 bytes.
        max_heap_bytes: 1_064,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    let token = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 0)
        .unwrap();
    machine
        .store_immutable_image_pixels(token, pixels, &[], &[])
        .unwrap();
    assert_eq!(machine.heap.managed.bytes(), 1_064);
    assert_eq!(
        machine.graphics_int_array_snapshot(token).unwrap(),
        original
    );
}

#[test]
pub(crate) fn immutable_image_copy_uses_native_storage_and_detaches_from_source() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let source = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 3)
        .unwrap();
    for (index, pixel) in [0x0011_2233, 0x8044_5566_u32.cast_signed(), 0x0077_8899]
        .into_iter()
        .enumerate()
    {
        machine
            .heap
            .managed
            .array_set(source, index as i32, HeapValue::Int(pixel))
            .unwrap();
    }

    let token = machine
        .image_copy_pixels(&[
            Value::Reference(Some(source)),
            Value::Int(1),
            Value::Int(2),
            Value::Int(0),
        ])
        .unwrap();
    assert_eq!(machine.heap.managed.array_length(token).unwrap(), 0);
    assert_eq!(
        machine.graphics_int_array_snapshot(token).unwrap(),
        vec![0xff44_5566_u32.cast_signed(), 0xff77_8899_u32.cast_signed()]
    );

    machine
        .heap
        .managed
        .array_set(source, 1, HeapValue::Int(0))
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(token).unwrap(),
        vec![0xff44_5566_u32.cast_signed(), 0xff77_8899_u32.cast_signed()]
    );

    machine.collect_heap(Vec::new());
    assert!(!machine.heap.immutable_image_pixels.contains_key(&token));
}

#[test]
pub(crate) fn immutable_array_snapshot_cache_is_charged_once() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 3)
        .unwrap();
    for (index, value) in [1, 2, 3].into_iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(pixels, index as i32, HeapValue::Int(value))
            .unwrap();
    }
    let image = machine
        .heap
        .managed
        .allocate_object(
            "javax/microedition/lcdui/Image",
            HashMap::from([(
                "javax/microedition/lcdui/Image.mutable:Z".to_owned(),
                HeapValue::Int(0),
            )]),
        )
        .unwrap();
    let before = machine.heap.managed.bytes();

    let first = machine
        .graphics_cached_image_pixels(image, pixels, &[])
        .unwrap()
        .unwrap();
    assert_eq!(first.len(), 3);
    assert_eq!(
        [first.get(0), first.get(1), first.get(2)],
        [Some(1), Some(2), Some(3)]
    );
    assert_eq!(
        machine.heap.managed.bytes(),
        before + 3 * std::mem::size_of::<i32>()
    );
    machine
        .graphics_cached_image_pixels(image, pixels, &[])
        .unwrap();
    assert_eq!(
        machine.heap.managed.bytes(),
        before + 3 * std::mem::size_of::<i32>()
    );
}

#[test]
pub(crate) fn graphics_set_clip_preserves_the_clipped_far_edge() {
    assert_eq!(
        graphics_clip_rectangle(-100, -30, 120, 50, 0, 0, 240, 320),
        [0, 0, 20, 20]
    );
    assert_eq!(
        graphics_clip_rectangle(-100, -30, 120, 50, 5, 10, 240, 320),
        [0, 0, 25, 30]
    );
    assert_eq!(
        graphics_clip_rectangle(230, 310, 30, 30, 0, 0, 240, 320),
        [230, 310, 10, 10]
    );
    assert_eq!(
        graphics_clip_rectangle(i32::MAX, i32::MIN, i32::MAX, 40, 0, 0, 240, 320),
        [240, 0, 0, 0]
    );
    assert_eq!(
        graphics_clip_rectangle(10, 20, -1, 0, 0, 0, 240, 320),
        [10, 20, 0, 0]
    );
}
