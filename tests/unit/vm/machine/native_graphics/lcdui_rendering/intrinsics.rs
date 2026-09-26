use super::*;

#[test]
fn lcd_ui_arc_wrappers_share_native_rasterization_with_intrinsics() {
    const WIDTH: i32 = 16;
    const HEIGHT: i32 = 12;
    fn target(machine: &mut Machine<'_, '_>) -> (Handle, Handle) {
        let (graphics, pixels) = direct_graphics_target(machine, WIDTH, HEIGHT);
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(0xffff_ffff_u32.cast_signed()));
        set_graphics_fields(
            machine,
            graphics,
            &[
                ("tx", 2),
                ("ty", 1),
                ("clipX", 3),
                ("clipY", 2),
                ("clipW", 10),
                ("clipH", 8),
                ("color", 0xff00_00ff_u32.cast_signed()),
                ("stroke", 1),
            ],
        );
        (graphics, pixels)
    }

    fn draw_pair(
        machine: &mut Machine<'_, '_>,
        graphics: Handle,
        draw_arc: &Method,
        fill_arc: &Method,
    ) {
        assert!(matches!(
            machine
                .call(
                    draw_arc,
                    vec![
                        Value::Reference(Some(graphics)),
                        Value::Int(1),
                        Value::Int(1),
                        Value::Int(10),
                        Value::Int(8),
                        Value::Int(30),
                        Value::Int(240),
                    ],
                    1,
                )
                .unwrap(),
            CallOutcome::Return(None)
        ));
        machine
            .heap
            .managed
            .set_field(
                graphics,
                "javax/microedition/lcdui/Graphics.stroke:I",
                HeapValue::Int(0),
            )
            .unwrap();
        assert!(matches!(
            machine
                .call(
                    fill_arc,
                    vec![
                        Value::Reference(Some(graphics)),
                        Value::Int(7),
                        Value::Int(4),
                        Value::Int(8),
                        Value::Int(7),
                        Value::Int(-45),
                        Value::Int(180),
                    ],
                    1,
                )
                .unwrap(),
            CallOutcome::Return(None)
        ));
    }

    let limits = Limits::default();
    let mut program = Program::new();
    let graphics_classes = runtime_bootstrap::production_bootstrap_inventory_for_display(
        WIDTH.cast_unsigned(),
        HEIGHT.cast_unsigned(),
    )
    .into_iter()
    .filter(|entry| {
        matches!(
            entry.class.class_name(entry.class.this_class),
            Some("javax/microedition/lcdui/Graphics" | "javax/microedition/lcdui/Image")
        )
    })
    .map(|entry| entry.class)
    .collect::<Vec<_>>();
    assert_eq!(graphics_classes.len(), 2);
    for class in &graphics_classes {
        program.add_class(class, &limits).unwrap();
    }
    let draw_key = MethodKey {
        class: "javax/microedition/lcdui/Graphics".into(),
        name: "drawArc".into(),
        descriptor: "(IIIIII)V".into(),
    };
    let fill_key = MethodKey {
        class: "javax/microedition/lcdui/Graphics".into(),
        name: "fillArc".into(),
        descriptor: "(IIIIII)V".into(),
    };
    let draw_arc = program.methods.get(&draw_key).unwrap().clone();
    let fill_arc = program.methods.get(&fill_key).unwrap().clone();
    let mut interpreted_draw_arc = draw_arc.clone();
    interpreted_draw_arc.key.class = "test/InterpretedGraphics".into();
    interpreted_draw_arc.stack_key = Arc::new(interpreted_draw_arc.key.clone());
    let mut interpreted_fill_arc = fill_arc.clone();
    interpreted_fill_arc.key.class = "test/InterpretedGraphics".into();
    interpreted_fill_arc.stack_key = Arc::new(interpreted_fill_arc.key.clone());

    let mut interpreted_context = DefaultNativeContext;
    let mut interpreted = program.machine(limits.clone(), false, &mut interpreted_context);
    let (interpreted_graphics, interpreted_pixels) = target(&mut interpreted);
    draw_pair(
        &mut interpreted,
        interpreted_graphics,
        &interpreted_draw_arc,
        &interpreted_fill_arc,
    );
    assert!(interpreted.execution.instructions > 0);
    let expected = interpreted
        .graphics_int_array_snapshot(interpreted_pixels)
        .unwrap();

    let mut intrinsic_context = DefaultNativeContext;
    let mut intrinsic = program.machine(limits, false, &mut intrinsic_context);
    let (intrinsic_graphics, intrinsic_pixels) = target(&mut intrinsic);
    draw_pair(&mut intrinsic, intrinsic_graphics, &draw_arc, &fill_arc);
    assert_eq!(intrinsic.execution.instructions, 0);
    assert_eq!(
        intrinsic
            .graphics_int_array_snapshot(intrinsic_pixels)
            .unwrap(),
        expected
    );
}

#[test]
fn lcd_ui_hot_public_intrinsics_match_the_interpreted_bootstrap() {
    const WIDTH: i32 = 6;
    const HEIGHT: i32 = 5;

    fn target(machine: &mut Machine<'_, '_>) -> (Handle, Handle, Handle, Handle) {
        let (graphics, target_pixels) = direct_graphics_target(machine, WIDTH, HEIGHT);
        machine
            .graphics_int_array_mut(target_pixels)
            .unwrap()
            .fill(HeapValue::Int(0xff10_1010_u32.cast_signed()));
        set_graphics_fields(
            machine,
            graphics,
            &[
                ("clipX", 1),
                ("clipY", 1),
                ("clipW", 4),
                ("clipH", 3),
                ("color", 0xff00_0000_u32.cast_signed()),
            ],
        );
        let rgb = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 4)
            .unwrap();
        for (index, pixel) in [0xffff_0000_u32, 0x8000_ff00, 0xff00_00ff, 0xffff_ffff]
            .into_iter()
            .enumerate()
        {
            machine
                .heap
                .managed
                .array_set(
                    rgb,
                    i32::try_from(index).unwrap(),
                    HeapValue::Int(pixel.cast_signed()),
                )
                .unwrap();
        }
        let image_pixels = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 2)
            .unwrap();
        machine
            .heap
            .managed
            .array_set(
                image_pixels,
                0,
                HeapValue::Int(0xffff_ff00_u32.cast_signed()),
            )
            .unwrap();
        machine
            .heap
            .managed
            .array_set(
                image_pixels,
                1,
                HeapValue::Int(0xffff_00ff_u32.cast_signed()),
            )
            .unwrap();
        let image = machine
            .heap
            .managed
            .allocate_object(
                "javax/microedition/lcdui/Image",
                HashMap::from([
                    (
                        "javax/microedition/lcdui/Image.width:I".into(),
                        HeapValue::Int(2),
                    ),
                    (
                        "javax/microedition/lcdui/Image.height:I".into(),
                        HeapValue::Int(1),
                    ),
                    (
                        "javax/microedition/lcdui/Image.mutable:Z".into(),
                        HeapValue::Int(0),
                    ),
                    (
                        "javax/microedition/lcdui/Image.pixels:[I".into(),
                        HeapValue::Reference(Some(image_pixels)),
                    ),
                ]),
            )
            .unwrap();
        (graphics, target_pixels, rgb, image)
    }

    fn run(
        machine: &mut Machine<'_, '_>,
        methods: &[Method],
        graphics: Handle,
        rgb: Handle,
        image: Handle,
    ) {
        let calls = [
            vec![
                Value::Reference(Some(graphics)),
                Value::Int(18),
                Value::Int(52),
                Value::Int(86),
            ],
            vec![
                Value::Reference(Some(graphics)),
                Value::Int(1),
                Value::Int(-1),
            ],
            vec![
                Value::Reference(Some(graphics)),
                Value::Int(-1),
                Value::Int(2),
                Value::Int(5),
                Value::Int(3),
            ],
            vec![
                Value::Reference(Some(graphics)),
                Value::Reference(Some(rgb)),
                Value::Int(0),
                Value::Int(2),
                Value::Int(0),
                Value::Int(1),
                Value::Int(2),
                Value::Int(2),
                Value::Int(1),
            ],
            vec![
                Value::Reference(Some(graphics)),
                Value::Reference(Some(image)),
                Value::Int(5),
                Value::Int(4),
                Value::Int(40),
            ],
        ];
        for (method, arguments) in methods.iter().zip(calls) {
            assert!(matches!(
                machine.call(method, arguments, 1).unwrap(),
                CallOutcome::Return(None)
            ));
        }
        assert!(matches!(
            machine
                .call(
                    &methods[4],
                    [
                        Value::Reference(Some(graphics)),
                        Value::Reference(Some(image)),
                        Value::Int(3),
                        Value::Int(3),
                        Value::Int(3), // HCENTER | VCENTER
                    ],
                    1,
                )
                .unwrap(),
            CallOutcome::Return(None)
        ));
    }

    let limits = Limits::default();
    let mut program = Program::new();
    for class in runtime_bootstrap::production_bootstrap_inventory_for_display(
        WIDTH.cast_unsigned(),
        HEIGHT.cast_unsigned(),
    )
    .into_iter()
    .filter(|entry| {
        matches!(
            entry.class.class_name(entry.class.this_class),
            Some("javax/microedition/lcdui/Graphics" | "javax/microedition/lcdui/Image")
        )
    })
    .map(|entry| entry.class)
    {
        program.add_class(&class, &limits).unwrap();
    }
    let signatures = [
        ("setColor", "(III)V"),
        ("translate", "(II)V"),
        ("fillRect", "(IIII)V"),
        ("drawRGB", "([IIIIIIIZ)V"),
        ("drawImage", "(Ljavax/microedition/lcdui/Image;III)V"),
    ];
    let intrinsic_methods = signatures.map(|(name, descriptor)| {
        program
            .methods
            .get(&MethodKey {
                class: "javax/microedition/lcdui/Graphics".into(),
                name: name.into(),
                descriptor: descriptor.into(),
            })
            .unwrap()
            .clone()
    });
    let interpreted_methods = intrinsic_methods.clone().map(|mut method| {
        method.key.class = "test/InterpretedGraphics".into();
        method.stack_key = Arc::new(method.key.clone());
        method
    });

    let mut interpreted_context = DefaultNativeContext;
    let mut interpreted = program.machine(limits.clone(), false, &mut interpreted_context);
    let (graphics, expected_pixels, rgb, image) = target(&mut interpreted);
    run(&mut interpreted, &interpreted_methods, graphics, rgb, image);
    assert!(interpreted.execution.instructions > 0);
    let expected = interpreted
        .graphics_int_array_snapshot(expected_pixels)
        .unwrap();
    let expected_state = [
        interpreted
            .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")
            .unwrap(),
        interpreted
            .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")
            .unwrap(),
        interpreted
            .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.color:I")
            .unwrap(),
    ];

    let mut intrinsic_context = DefaultNativeContext;
    let mut intrinsic = program.machine(limits, false, &mut intrinsic_context);
    let (graphics, actual_pixels, rgb, image) = target(&mut intrinsic);
    run(&mut intrinsic, &intrinsic_methods, graphics, rgb, image);
    assert_eq!(intrinsic.execution.instructions, 0);
    assert_eq!(
        intrinsic
            .graphics_int_array_snapshot(actual_pixels)
            .unwrap(),
        expected
    );
    assert_eq!(
        [
            intrinsic
                .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.tx:I")
                .unwrap(),
            intrinsic
                .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.ty:I")
                .unwrap(),
            intrinsic
                .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.color:I")
                .unwrap(),
        ],
        expected_state
    );

    let draw_rgb = &intrinsic_methods[3];
    let arguments = |source, offset, scanlength, width, height| {
        [
            Value::Reference(Some(graphics)),
            Value::Reference(source),
            Value::Int(offset),
            Value::Int(scanlength),
            Value::Int(0),
            Value::Int(0),
            Value::Int(width),
            Value::Int(height),
            Value::Int(1),
        ]
    };
    let Err(error) = intrinsic
        .invoke_lcd_ui_graphics_public_intrinsic(draw_rgb, &arguments(Some(rgb), -1, 1, 1, 1))
    else {
        panic!("negative drawRGB offset must fail");
    };
    assert_eq!(error.code(), "array-index-out-of-bounds-exception");
    let Err(error) = intrinsic
        .invoke_lcd_ui_graphics_public_intrinsic(draw_rgb, &arguments(Some(rgb), 0, 1, -1, 1))
    else {
        panic!("negative drawRGB width must fail");
    };
    assert_eq!(error.code(), "illegal-argument");
    assert!(matches!(
        intrinsic
            .invoke_lcd_ui_graphics_public_intrinsic(draw_rgb, &arguments(Some(rgb), 0, 8, 1, 1),)
            .unwrap(),
        Some(CallOutcome::Return(None))
    ));
}
