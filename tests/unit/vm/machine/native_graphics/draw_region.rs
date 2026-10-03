use super::*;

const GRAPHICS: &str = "javax/microedition/lcdui/Graphics";
const IMAGE: &str = "javax/microedition/lcdui/Image";
const DESCRIPTOR: &str = "(Ljavax/microedition/lcdui/Image;IIIIIIII)V";

fn program() -> Program {
    let mut program = Program::new();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(7, 6) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    program
}

fn method(program: &Program, interpreted: bool) -> Method {
    let mut method = program.methods[&MethodKey {
        class: GRAPHICS.into(),
        name: "drawRegion".into(),
        descriptor: DESCRIPTOR.into(),
    }]
        .clone();
    if interpreted {
        method.key.class = "test/InterpretedGraphics".into();
        method.stack_key = Arc::new(method.key.clone());
    }
    method
}

fn fixture(machine: &mut Machine<'_, '_>, mutable: bool) -> (Handle, Handle, Handle, Handle) {
    let target = machine
        .allocate_image(7, 6, vec![0xff11_2233_u32.cast_signed(); 42], true, &[])
        .unwrap();
    let fields = machine
        .instance_fields(GRAPHICS)
        .unwrap()
        .into_iter()
        .map(|field| (field.key.to_string(), field.initial))
        .collect();
    let graphics = machine
        .allocate_object(GRAPHICS, fields, &[], &[Value::Reference(Some(target))])
        .unwrap();
    machine
        .retarget_canvas_graphics(graphics, target, 7, 6)
        .unwrap();
    for (name, value) in [
        ("tx", 1),
        ("ty", -1),
        ("clipX", 1),
        ("clipY", 1),
        ("clipW", 5),
        ("clipH", 4),
    ] {
        machine
            .heap
            .managed
            .set_field(
                graphics,
                &format!("{GRAPHICS}.{name}:I"),
                HeapValue::Int(value),
            )
            .unwrap();
    }
    let pixels = (0..12)
        .map(|i| [0xffff_0000_u32, 0x8000_ff00, 0x0000_00ff, 0xff00_ffff][i % 4].cast_signed())
        .collect();
    let source = machine
        .allocate_image(4, 3, pixels, mutable, &[Value::Reference(Some(graphics))])
        .unwrap();
    let target_pixels = machine
        .graphics_reference_field(target, &format!("{IMAGE}.pixels:[I"))
        .unwrap();
    (graphics, source, target, target_pixels)
}

fn draw_args(graphics: Handle, source: Option<Handle>, values: [i32; 8]) -> Vec<Value> {
    let mut args = vec![Value::Reference(Some(graphics)), Value::Reference(source)];
    args.extend(values.map(Value::Int));
    args
}

fn outcome(machine: &mut Machine<'_, '_>, method: &Method, args: &[Value]) -> String {
    match machine.call(method, args, 1) {
        Ok(CallOutcome::Return(None)) => "ok".into(),
        Ok(CallOutcome::Return(Some(Value::Int(value)))) => format!("int:{value}"),
        Ok(CallOutcome::Throw(exception)) => machine.object_class(exception).unwrap().into_owned(),
        Err(error) => error.code().into(),
        _ => panic!("unexpected drawRegion outcome"),
    }
}

#[test]
fn draw_region_public_matches_bootstrap_transforms_anchors_clipping_and_errors() {
    let program = program();
    let mut cases = Vec::new();
    for transform in 0..8 {
        for anchor in [0, 3, 20, 24, 36, 40] {
            cases.push([1, 1, 3, 2, transform, 3, 3, anchor]);
        }
    }
    cases.extend([
        [0, 0, 1, 1, 8, 100, 100, 0], // Clipping never bypasses public validation.
        [0, 0, 1, 1, 0, 100, 100, 128],
        [-1, 0, 1, 1, 0, 100, 100, 0],
        [0, 0, 0, 3, 0, 3, 3, 128], // Empty region skips anchor validation.
        [0, 0, 4, 0, 7, 3, 3, 128],
        [0, 0, -1, 1, 0, 0, 0, 0],
        [0, 0, 1, -1, 0, 0, 0, 0],
        [-1, 0, 1, 1, 0, 0, 0, 0],
        [0, -1, 1, 1, 0, 0, 0, 0],
        [2, 0, 3, 1, 0, 0, 0, 0],
        [0, 2, 1, 2, 0, 0, 0, 0],
        [i32::MAX, 0, 1, 1, 0, 0, 0, 0],
        [0, 0, i32::MAX, 1, 0, 0, 0, 0],
        [0, 0, 1, 1, -1, 0, 0, 0],
        [0, 0, 1, 1, 8, 0, 0, 0],
        [0, 0, 1, 1, 0, 0, 0, 12],
        [0, 0, 1, 1, 0, 0, 0, 128],
        [0, 0, 1, 1, 0, 0, 0, 256],
        [0, 0, 1, 1, 0, 0, 0, i32::MIN],
        [0, 0, 4, 3, 7, i32::MAX, i32::MIN, 40],
    ]);
    for mutable in [false, true] {
        for values in &cases {
            let mut results = Vec::new();
            for interpreted in [true, false] {
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let (graphics, source, _, pixels) = fixture(&mut machine, mutable);
                let result = outcome(
                    &mut machine,
                    &method(&program, interpreted),
                    &draw_args(graphics, Some(source), *values),
                );
                results.push((result, machine.graphics_int_array_snapshot(pixels).unwrap()));
            }
            assert_eq!(results[0], results[1], "mutable={mutable}, args={values:?}");
        }
    }
    for interpreted in [true, false] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let (graphics, _, target, _) = fixture(&mut machine, true);
        let method = method(&program, interpreted);
        for (source, expected) in [
            (None, "java/lang/NullPointerException"),
            (Some(target), "java/lang/IllegalArgumentException"),
        ] {
            assert_eq!(
                outcome(
                    &mut machine,
                    &method,
                    &draw_args(graphics, source, [0, 0, 1, 1, 0, 0, 0, 0])
                ),
                expected
            );
        }
    }
}

#[test]
fn image_anchor_intrinsic_matches_signed_bootstrap_mask_for_all_anchor_bits() {
    let program = program();
    let original = program.methods[&MethodKey {
        class: GRAPHICS.into(),
        name: "origin".into(),
        descriptor: "(IIIZ)I".into(),
    }]
        .clone();
    let mut interpreted = original.clone();
    interpreted.key.class = "test/InterpretedGraphics".into();
    interpreted.stack_key = Arc::new(interpreted.key.clone());
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for anchor in [0, 3, 20, 24, 36, 40, 12, 128, 256, 65_536, i32::MIN, -1] {
        for horizontal in [0, 1] {
            let args = [10, 3, anchor, horizontal].map(Value::Int);
            let expected = outcome(&mut machine, &interpreted, &args);
            assert_eq!(
                outcome(&mut machine, &original, &args),
                expected,
                "anchor={anchor}, horizontal={horizontal}"
            );
            if anchor & !0x3f != 0 {
                assert_eq!(expected, "java/lang/IllegalArgumentException");
            }
        }
    }
}

#[test]
#[ignore = "manual drawRegion wrapper throughput measurement"]
fn draw_region_public_throughput() {
    let program = program();
    let mut reference_pixels = None;
    for interpreted in [true, false] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                max_instructions: 100_000_000,
                ..Limits::default()
            },
            false,
            &mut context,
        );
        let (graphics, source, _, pixels) = fixture(&mut machine, false);
        let method = method(&program, interpreted);
        let args = draw_args(graphics, Some(source), [1, 1, 3, 2, 2, 3, 3, 20]);
        let started = std::time::Instant::now();
        for _ in 0..20_000 {
            assert_eq!(outcome(&mut machine, &method, &args), "ok");
        }
        eprintln!(
            "interpreted={interpreted} elapsed={:?} instructions={}",
            started.elapsed(),
            machine.execution.instructions,
        );
        let pixels = machine.graphics_int_array_snapshot(pixels).unwrap();
        if let Some(expected) = &reference_pixels {
            assert_eq!(&pixels, expected);
        } else {
            reference_pixels = Some(pixels);
        }
    }
}

#[test]
fn clipped_region_pixels_match_independent_software_renderer_for_every_transform() {
    let program = program();
    let pixels = (0..12_u32)
        .map(|index| 0xff10_2030 | (index * 0x0003_0711))
        .collect::<Vec<_>>();
    let reference_source = graphics::Image::from_argb(&pixels, 4, 3, true).unwrap();
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
        for x in [-3, -1, 0, 2, 5, 7] {
            for y in [-3, -1, 1, 4, 6] {
                let mut reference = graphics::Framebuffer::new(7, 6).unwrap();
                let mut reference_graphics = reference.graphics();
                reference_graphics.set_color(0x0011_2233);
                reference_graphics.fill_rect(0, 0, 7, 6);
                reference_graphics.set_clip(1, 1, 5, 4);
                reference_graphics
                    .draw_region(&reference_source, 0, 0, 4, 3, transform, x + 1, y - 1, 20)
                    .unwrap();
                for immutable in [false, true] {
                    let mut context = DefaultNativeContext;
                    let mut machine = program.machine(Limits::default(), false, &mut context);
                    let (graphics, source, _, target_pixels) = fixture(&mut machine, true);
                    let source_pixels = machine
                        .graphics_reference_field(source, &format!("{IMAGE}.pixels:[I"))
                        .unwrap();
                    for (index, pixel) in pixels.iter().enumerate() {
                        machine
                            .heap
                            .managed
                            .array_set(
                                source_pixels,
                                i32::try_from(index).unwrap(),
                                HeapValue::Int(pixel.cast_signed()),
                            )
                            .unwrap();
                    }
                    machine
                        .heap
                        .managed
                        .set_field(
                            source,
                            &format!("{IMAGE}.mutable:Z"),
                            HeapValue::Int(i32::from(!immutable)),
                        )
                        .unwrap();
                    assert_eq!(
                        outcome(
                            &mut machine,
                            &method(&program, false),
                            &draw_args(
                                graphics,
                                Some(source),
                                [0, 0, 4, 3, i32::try_from(code).unwrap(), x, y, 20]
                            ),
                        ),
                        "ok"
                    );
                    let actual = machine.graphics_int_array_snapshot(target_pixels).unwrap();
                    assert!(
                        actual
                            .iter()
                            .map(|pixel| pixel.cast_unsigned())
                            .eq(reference.pixels().iter().copied()),
                        "transform={code}, origin=({x},{y}), immutable={immutable}"
                    );
                }
            }
        }
    }
}

#[test]
fn fully_clipped_images_do_not_create_pixel_caches_or_collect_the_heap() {
    let program = program();
    for region in [false, true] {
        for transform in 0..if region { 8 } else { 1 } {
            for (x, y) in [(-100, -100), (100, 100), (i32::MIN, 0), (0, i32::MAX)] {
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(
                    Limits {
                        max_heap_bytes: 4096,
                        ..Limits::default()
                    },
                    false,
                    &mut context,
                );
                let (graphics, source, _, target_pixels) = fixture(&mut machine, true);
                machine
                    .heap
                    .managed
                    .set_field(source, &format!("{IMAGE}.mutable:Z"), HeapValue::Int(0))
                    .unwrap();
                let remaining = 4096 - machine.heap.managed.bytes() - 24 - 8;
                let garbage = machine
                    .heap
                    .managed
                    .allocate_array(ArrayKind::Byte, i32::try_from(remaining).unwrap())
                    .unwrap();
                let before = machine.heap.managed.bytes();
                let cached = machine.heap.immutable_image_pixels.len();
                let pixels = machine.graphics_int_array_snapshot(target_pixels).unwrap();
                let (method, args) = if region {
                    (
                        method(&program, false),
                        draw_args(graphics, Some(source), [0, 0, 4, 3, transform, x, y, 20]),
                    )
                } else {
                    (
                        program.methods[&MethodKey {
                            class: GRAPHICS.into(),
                            name: "drawImage".into(),
                            descriptor: "(Ljavax/microedition/lcdui/Image;III)V".into(),
                        }]
                            .clone(),
                        vec![
                            Value::Reference(Some(graphics)),
                            Value::Reference(Some(source)),
                            Value::Int(x),
                            Value::Int(y),
                            Value::Int(20),
                        ],
                    )
                };
                assert_eq!(outcome(&mut machine, &method, &args), "ok");
                assert_eq!(machine.heap.managed.bytes(), before);
                assert!(machine.heap.managed.get(garbage).is_ok());
                assert_eq!(machine.heap.immutable_image_pixels.len(), cached);
                assert_eq!(
                    machine.graphics_int_array_snapshot(target_pixels).unwrap(),
                    pixels
                );
            }
        }
    }
}

#[test]
fn draw_region_public_keeps_destination_alive_while_caching_source_pixels() {
    image_cache_keeps_destination(true);
}

#[test]
fn draw_image_public_keeps_destination_alive_while_caching_source_pixels() {
    image_cache_keeps_destination(false);
}

fn image_cache_keeps_destination(region: bool) {
    const HEAP_BYTES: usize = 4096;
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: HEAP_BYTES,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let (graphics, source, target, pixels) = fixture(&mut machine, true);
    // Legacy heap-backed immutable image whose first draw creates its cache.
    machine
        .heap
        .managed
        .set_field(source, &format!("{IMAGE}.mutable:Z"), HeapValue::Int(0))
        .unwrap();
    let garbage_bytes = HEAP_BYTES - machine.heap.managed.bytes() - 24 - 8;
    let garbage = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, i32::try_from(garbage_bytes).unwrap())
        .unwrap();
    let (method, args) = if region {
        (
            method(&program, false),
            draw_args(graphics, Some(source), [1, 1, 3, 2, 0, 2, 2, 20]),
        )
    } else {
        (
            program.methods[&MethodKey {
                class: GRAPHICS.into(),
                name: "drawImage".into(),
                descriptor: "(Ljavax/microedition/lcdui/Image;III)V".into(),
            }]
                .clone(),
            vec![
                Value::Reference(Some(graphics)),
                Value::Reference(Some(source)),
                Value::Int(2),
                Value::Int(2),
                Value::Int(20),
            ],
        )
    };
    assert_eq!(outcome(&mut machine, &method, &args), "ok");
    assert!(machine.heap.managed.get(garbage).is_err());
    for handle in [graphics, source, target, pixels] {
        assert!(machine.heap.managed.get(handle).is_ok());
    }
    assert!(machine.heap.temporary_roots.is_empty());
}
