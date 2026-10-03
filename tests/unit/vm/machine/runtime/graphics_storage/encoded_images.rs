use super::*;

fn encoded_image_test_program() -> Program {
    let mut image_class = test_class_definition(None);
    image_class.fields = [
        ("width", "I", ValueKind::Int, Value::Int(0)),
        ("height", "I", ValueKind::Int, Value::Int(0)),
        ("pixels", "[I", ValueKind::Reference, Value::Reference(None)),
        ("mutable", "Z", ValueKind::Int, Value::Int(0)),
    ]
    .into_iter()
    .map(|(name, descriptor, kind, initial)| Field {
        key: format!("javax/microedition/lcdui/Image.{name}:{descriptor}").into(),
        declaring_class: "javax/microedition/lcdui/Image".into(),
        kind,
        is_static: false,
        field_token: FieldToken::new(),
        instance_slot: None,
        initial,
        constant_string: None,
    })
    .collect();
    let mut program = Program::new();
    program
        .classes
        .insert("javax/microedition/lcdui/Image".into(), image_class);
    program
}

fn one_pixel_bmp() -> Vec<u8> {
    let mut bmp = vec![0_u8; 58];
    bmp[..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&58_u32.to_le_bytes());
    bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&1_i32.to_le_bytes());
    bmp[22..26].copy_from_slice(&1_i32.to_le_bytes());
    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
    bmp[34..38].copy_from_slice(&4_u32.to_le_bytes());
    bmp[54..58].copy_from_slice(&[0x33, 0x22, 0x11, 0]);
    bmp
}

#[test]
fn nokia_encoded_image_can_be_drawn_into_after_creation() {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(1, 1) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    let method = |class: &str, name: &str, descriptor: &str| {
        program.methods[&MethodKey {
            class: class.into(),
            name: name.into(),
            descriptor: descriptor.into(),
        }]
            .clone()
    };
    let create = method(
        "com/nokia/mid/ui/DirectUtils",
        "createImage",
        "([BII)Ljavax/microedition/lcdui/Image;",
    );
    let get_graphics = method(
        "javax/microedition/lcdui/Image",
        "getGraphics",
        "()Ljavax/microedition/lcdui/Graphics;",
    );
    let fill = method("javax/microedition/lcdui/Graphics", "fillRect", "(IIII)V");
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let encoded = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 58)
        .unwrap();
    for (index, byte) in (0..).zip(one_pixel_bmp()) {
        machine
            .heap
            .managed
            .array_set(encoded, index, HeapValue::Int(i32::from(byte)))
            .unwrap();
    }
    let CallOutcome::Return(Some(Value::Reference(Some(image)))) = machine
        .call(
            &create,
            [
                Value::Reference(Some(encoded)),
                Value::Int(0),
                Value::Int(58),
            ],
            1,
        )
        .unwrap()
    else {
        panic!("DirectUtils.createImage did not return an image");
    };
    assert_eq!(
        machine
            .graphics_int_field(image, "javax/microedition/lcdui/Image.mutable:Z")
            .unwrap(),
        1
    );
    let pixels = machine
        .graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        [0xff11_2233_u32.cast_signed()]
    );
    let CallOutcome::Return(Some(Value::Reference(Some(graphics)))) = machine
        .call(&get_graphics, [Value::Reference(Some(image))], 1)
        .unwrap()
    else {
        panic!("Image.getGraphics did not return a graphics context");
    };
    for (color, expected) in [
        (0x00ff_0000_u32, 0xff11_2233_u32),
        (0x80ff_0000, 0xff88_1119),
        (0xffab_cdef, 0xffab_cdef),
    ] {
        machine
            .graphics_set_argb_color(&[
                Value::Reference(Some(graphics)),
                Value::Int(color.cast_signed()),
            ])
            .unwrap();
        assert!(matches!(
            machine
                .call(
                    &fill,
                    [
                        Value::Reference(Some(graphics)),
                        Value::Int(0),
                        Value::Int(0),
                        Value::Int(1),
                        Value::Int(1)
                    ],
                    1
                )
                .unwrap(),
            CallOutcome::Return(None)
        ));
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            [expected.cast_signed()]
        );
    }
}

#[test]
fn encoded_image_decode_does_not_duplicate_pixels_in_the_managed_heap() {
    for mutable in [false, true] {
        let program = encoded_image_test_program();
        let mut context = DefaultNativeContext;
        // byte[58] costs 82 bytes. One pixel needs 28 bytes as either a
        // mutable int[1] or an immutable token/payload, plus 24 for Image:
        // 134 total. An intermediate array or Image would exceed this heap.
        let limits = Limits {
            max_heap_bytes: 145,
            ..Limits::default()
        };
        let mut machine = program.machine(limits, false, &mut context);
        let bmp = one_pixel_bmp();
        let encoded = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Byte, 58)
            .unwrap();
        for (index, byte) in bmp.into_iter().enumerate() {
            machine
                .heap
                .managed
                .array_set(encoded, index as i32, HeapValue::Int(i32::from(byte)))
                .unwrap();
        }

        let image = machine
            .image_create_from_encoded(
                &[
                    Value::Reference(Some(encoded)),
                    Value::Int(0),
                    Value::Int(58),
                ],
                mutable,
            )
            .unwrap();

        let pixels = machine
            .graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
            .unwrap();
        assert_eq!(
            machine.heap.managed.array_length(pixels).unwrap(),
            usize::from(mutable)
        );
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            [0xff11_2233_u32.cast_signed()]
        );
        assert_eq!(machine.heap.managed.bytes(), 134);
    }
}

#[test]
fn encoded_image_reads_only_its_resource_region_and_checks_bounds() {
    let program = encoded_image_test_program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let bmp = one_pixel_bmp();
    let encoded = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 256)
        .unwrap();
    for (index, byte) in bmp.iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(
                encoded,
                99 + index as i32,
                HeapValue::Int(i32::from(*byte as i8)),
            )
            .unwrap();
    }
    let image = machine
        .image_create_from_encoded(
            &[
                Value::Reference(Some(encoded)),
                Value::Int(99),
                Value::Int(58),
            ],
            false,
        )
        .unwrap();
    let pixels = machine
        .graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        [0xff11_2233_u32.cast_signed()]
    );
    for (offset, length) in [
        (-1, 58),
        (99, -1),
        (255, 58),
        (257, 0),
        (i32::MAX, i32::MAX),
    ] {
        let error = machine
            .image_create_from_encoded(
                &[
                    Value::Reference(Some(encoded)),
                    Value::Int(offset),
                    Value::Int(length),
                ],
                false,
            )
            .unwrap_err();
        assert_eq!(error.code(), "array-index-out-of-bounds-exception");
    }
    for (offset, length) in [(256, 0), (98, 58), (99, 10)] {
        let error = machine
            .image_create_from_encoded(
                &[
                    Value::Reference(Some(encoded)),
                    Value::Int(offset),
                    Value::Int(length),
                ],
                false,
            )
            .unwrap_err();
        assert_eq!(error.code(), "illegal-argument");
    }
}

#[test]
#[ignore = "manual resource-slice decoding throughput measurement"]
fn encoded_image_resource_slice_throughput() {
    for length in [1_024, 1_048_576] {
        let program = encoded_image_test_program();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let encoded = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Byte, length)
            .unwrap();
        let offset = length / 2;
        for (index, byte) in one_pixel_bmp().iter().enumerate() {
            machine
                .heap
                .managed
                .array_set(
                    encoded,
                    offset + index as i32,
                    HeapValue::Int(i32::from(*byte as i8)),
                )
                .unwrap();
        }
        let started = std::time::Instant::now();
        for _ in 0..300 {
            let image = machine
                .image_create_from_encoded(
                    &[
                        Value::Reference(Some(encoded)),
                        Value::Int(offset),
                        Value::Int(58),
                    ],
                    false,
                )
                .unwrap();
            std::hint::black_box(image);
            machine.collect_heap(vec![encoded]);
        }
        eprintln!(
            "resource bytes={length} decodes=300 elapsed={:?}",
            started.elapsed()
        );
    }
}

#[test]
fn encoded_image_accepts_a_tall_sprite_sheet_with_bounded_area() {
    const WIDTH: i32 = 20;
    const HEIGHT: i32 = 4_160;
    const PIXEL_OFFSET: usize = 62;
    const ROW_BYTES: usize = 4;

    let data_length = ROW_BYTES * usize::try_from(HEIGHT).unwrap();
    let file_size = PIXEL_OFFSET + data_length;
    let mut bmp = vec![0_u8; file_size];
    bmp[..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&u32::try_from(file_size).unwrap().to_le_bytes());
    bmp[10..14].copy_from_slice(&u32::try_from(PIXEL_OFFSET).unwrap().to_le_bytes());
    bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&WIDTH.to_le_bytes());
    bmp[22..26].copy_from_slice(&HEIGHT.to_le_bytes());
    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&1_u16.to_le_bytes());
    bmp[34..38].copy_from_slice(&u32::try_from(data_length).unwrap().to_le_bytes());
    bmp[46..50].copy_from_slice(&2_u32.to_le_bytes());
    bmp[58..62].copy_from_slice(&[0xff, 0xff, 0xff, 0]);

    let program = encoded_image_test_program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let encoded = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, i32::try_from(bmp.len()).unwrap())
        .unwrap();
    for (index, byte) in bmp.into_iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(encoded, index as i32, HeapValue::Int(i32::from(byte)))
            .unwrap();
    }

    let image = machine
        .image_create_from_encoded(
            &[
                Value::Reference(Some(encoded)),
                Value::Int(0),
                Value::Int(i32::try_from(file_size).unwrap()),
            ],
            false,
        )
        .unwrap();

    assert_eq!(
        machine
            .graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")
            .unwrap(),
        WIDTH
    );
    assert_eq!(
        machine
            .graphics_int_field(image, "javax/microedition/lcdui/Image.height:I")
            .unwrap(),
        HEIGHT
    );
    let pixels = machine
        .graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
        .unwrap();
    assert_eq!(
        machine
            .heap
            .immutable_image_pixels
            .get(&pixels)
            .unwrap()
            .len(),
        usize::try_from(WIDTH * HEIGHT).unwrap()
    );
}
