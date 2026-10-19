use super::*;

#[derive(Clone, Copy, Debug)]
enum FieldLayout {
    Unlinked,
    Linked,
    Reordered,
}

fn native_fields_program() -> Program {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(7, 6) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    program
}

fn field_target(
    machine: &mut Machine<'_, '_>,
    width: i32,
    height: i32,
    layout: FieldLayout,
) -> (Handle, Handle) {
    if matches!(layout, FieldLayout::Unlinked) {
        return direct_graphics_target(machine, width, height);
    }
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, width * height)
        .unwrap();
    let mut image_fields = machine
        .initial_instance_fields("javax/microedition/lcdui/Image")
        .unwrap();
    if matches!(layout, FieldLayout::Reordered) {
        image_fields.reverse();
    }
    let image = machine
        .heap
        .managed
        .allocate_object_linked("javax/microedition/lcdui/Image", &mut image_fields)
        .unwrap();
    for (name, value) in [("width", width), ("height", height)] {
        machine
            .heap
            .managed
            .set_field(
                image,
                &format!("javax/microedition/lcdui/Image.{name}:I"),
                HeapValue::Int(value),
            )
            .unwrap();
    }
    machine
        .heap
        .managed
        .set_field(
            image,
            "javax/microedition/lcdui/Image.pixels:[I",
            HeapValue::Reference(Some(pixels)),
        )
        .unwrap();
    let mut graphics_fields = machine
        .initial_instance_fields("javax/microedition/lcdui/Graphics")
        .unwrap();
    if matches!(layout, FieldLayout::Reordered) {
        graphics_fields.reverse();
    }
    let graphics = machine
        .heap
        .managed
        .allocate_object_linked("javax/microedition/lcdui/Graphics", &mut graphics_fields)
        .unwrap();
    machine
        .heap
        .managed
        .set_field(
            graphics,
            "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;",
            HeapValue::Reference(Some(image)),
        )
        .unwrap();
    super::super::graphics_support::set_graphics_fields(
        machine,
        graphics,
        &[
            ("tx", 0),
            ("ty", 0),
            ("clipX", 0),
            ("clipY", 0),
            ("clipW", width),
            ("clipH", height),
            ("stroke", 0),
            ("color", 0xff12_3456_u32.cast_signed()),
        ],
    );
    (graphics, pixels)
}

#[test]
fn linked_reordered_and_synthetic_native_fields_produce_the_same_raster() {
    let program = native_fields_program();
    for original in [false, true] {
        let mut expected = None;
        for layout in [
            FieldLayout::Unlinked,
            FieldLayout::Linked,
            FieldLayout::Reordered,
        ] {
            let mut host = EnhancedHost {
                original,
                ..EnhancedHost::default()
            };
            let mut machine = program.machine(Limits::default(), false, &mut host);
            let (graphics, pixels) = field_target(&mut machine, 7, 6, layout);
            machine
                .graphics_int_array_mut(pixels)
                .unwrap()
                .fill(HeapValue::Int(0x4010_2030));
            machine.ensure_enhanced_target(pixels, 7, 6).unwrap();
            super::super::graphics_support::set_graphics_fields(
                &mut machine,
                graphics,
                &[
                    ("tx", 1),
                    ("ty", -1),
                    ("clipX", 1),
                    ("clipY", 2),
                    ("clipW", 5),
                    ("clipH", 3),
                    ("stroke", 1),
                    ("color", 0x8047_294b_u32.cast_signed()),
                ],
            );
            assert_eq!(machine.graphics_target(graphics).unwrap(), (pixels, 7, 6));
            machine
                .graphics_draw_arc(
                    &[
                        Value::Reference(Some(graphics)),
                        Value::Int(0),
                        Value::Int(0),
                        Value::Int(6),
                        Value::Int(5),
                        Value::Int(-45),
                        Value::Int(270),
                    ],
                    true,
                )
                .unwrap();
            let source = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Int, 6)
                .unwrap();
            for (index, pixel) in machine
                .graphics_int_array_mut(source)
                .unwrap()
                .iter_mut()
                .enumerate()
            {
                *pixel = HeapValue::Int((0x8012_3400_u32 | index as u32).cast_signed());
            }
            machine
                .graphics_draw_rgb(&[
                    Value::Reference(Some(graphics)),
                    Value::Reference(Some(source)),
                    Value::Int(3),
                    Value::Int(-3),
                    Value::Int(0),
                    Value::Int(3),
                    Value::Int(3),
                    Value::Int(2),
                    Value::Int(0),
                ])
                .unwrap();
            let actual = (
                machine.graphics_int_array_snapshot(pixels).unwrap(),
                machine
                    .graphics
                    .enhanced
                    .pixels(pixels)
                    .unwrap()
                    .unwrap_or_default(),
            );
            assert!(
                actual
                    .0
                    .iter()
                    .any(|pixel| pixel.cast_unsigned() >> 24 == 255)
            );
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected, "layout={layout:?} original={original}");
            } else {
                expected = Some(actual);
            }
        }
    }
}

#[test]
#[ignore = "manual linked and defensive native field lookup throughput measurement"]
fn native_field_lookup_throughput() {
    let program = native_fields_program();
    for layout in [
        FieldLayout::Unlinked,
        FieldLayout::Linked,
        FieldLayout::Reordered,
    ] {
        let mut host = EnhancedHost::default();
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let (graphics, _) = field_target(&mut machine, 7, 6, layout);
        let mut checksum = 0_u64;
        let started = std::time::Instant::now();
        for _ in 0..65_536 {
            let graphics = std::hint::black_box(graphics);
            for name in [
                "javax/microedition/lcdui/Graphics.tx:I",
                "javax/microedition/lcdui/Graphics.ty:I",
                "javax/microedition/lcdui/Graphics.clipX:I",
                "javax/microedition/lcdui/Graphics.clipY:I",
                "javax/microedition/lcdui/Graphics.clipW:I",
                "javax/microedition/lcdui/Graphics.clipH:I",
                "javax/microedition/lcdui/Graphics.stroke:I",
                "javax/microedition/lcdui/Graphics.color:I",
            ] {
                checksum = checksum.wrapping_add(u64::from(
                    machine
                        .graphics_int_field(graphics, name)
                        .unwrap()
                        .cast_unsigned(),
                ));
            }
            let (_, width, height) = machine.graphics_target(graphics).unwrap();
            checksum = checksum.wrapping_add(width as u64 + height as u64);
        }
        let elapsed = started.elapsed();
        eprintln!("native-fields layout={layout:?} elapsed={elapsed:?} checksum={checksum:016x}");
    }
}

#[test]
#[ignore = "manual native drawing with linked and defensive field layouts"]
fn native_field_blit_throughput() {
    let program = native_fields_program();
    for layout in [
        FieldLayout::Unlinked,
        FieldLayout::Linked,
        FieldLayout::Reordered,
    ] {
        for size in [1, 16] {
            for original in [true, false] {
                let mut host = EnhancedHost {
                    original,
                    ..EnhancedHost::default()
                };
                let mut machine = program.machine(Limits::default(), false, &mut host);
                let (graphics, pixels) = field_target(&mut machine, size, size, layout);
                machine
                    .ensure_enhanced_target(pixels, size as u32, size as u32)
                    .unwrap();
                let source = machine
                    .heap
                    .managed
                    .allocate_array(ArrayKind::Int, size * size)
                    .unwrap();
                machine
                    .graphics_int_array_mut(source)
                    .unwrap()
                    .fill(HeapValue::Int(0x8047_294b_u32.cast_signed()));
                let args = [
                    Value::Reference(Some(graphics)),
                    Value::Reference(Some(source)),
                    Value::Int(0),
                    Value::Int(size),
                    Value::Int(0),
                    Value::Int(0),
                    Value::Int(size),
                    Value::Int(size),
                    Value::Int(1),
                ];
                machine.graphics_draw_rgb(&args).unwrap();
                let started = std::time::Instant::now();
                for _ in 0..if size == 1 { 16_384 } else { 1024 } {
                    machine
                        .graphics_draw_rgb(std::hint::black_box(&args))
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
                    "native-blit layout={layout:?} size={size} scale={} elapsed={elapsed:?} checksum={checksum:016x}",
                    if original { 1 } else { 2 }
                );
            }
        }
    }
}
