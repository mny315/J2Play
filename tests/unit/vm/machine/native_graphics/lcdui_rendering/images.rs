use super::*;

#[test]
fn nokia_direct_graphics_reuses_context_and_rotates_images() {
    assert_eq!(
        [0, 90, 180, 270].map(|rotation| nokia_direct_transform(rotation).unwrap()),
        [0, 6, 3, 5]
    );
    assert_eq!(nokia_direct_transform(0x5a | 0x2000), Some(7));
    assert_eq!(nokia_direct_transform(0x5a | 0x4000), Some(4));
    assert_eq!(nokia_direct_transform(45), None);

    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let source_pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 2)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(source_pixels, 0, HeapValue::Int(-65_536))
        .unwrap();
    machine
        .heap
        .managed
        .array_set(source_pixels, 1, HeapValue::Int(-16_711_936))
        .unwrap();
    let source_image = machine
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
                    HeapValue::Reference(Some(source_pixels)),
                ),
            ]),
        )
        .unwrap();
    let (graphics, target_pixels) = direct_graphics_target(&mut machine, 3, 3);
    let get_direct = runtime_method(
        "com/nokia/mid/ui/DirectUtils",
        "getDirectGraphics",
        "(Ljavax/microedition/lcdui/Graphics;)Lcom/nokia/mid/ui/DirectGraphics;",
        &[0xb0],
        1,
        1,
        Vec::new(),
        true,
    );
    assert!(matches!(
        machine
            .invoke_vm_native(
                &get_direct,
                &[Value::Reference(Some(graphics))],
                1
            )
            .unwrap(),
        Some(CallOutcome::Return(Some(Value::Reference(Some(handle))))) if handle == graphics
    ));

    machine
        .graphics_draw_nokia_image(&[
            Value::Reference(Some(graphics)),
            Value::Reference(Some(source_image)),
            Value::Int(1),
            Value::Int(0),
            Value::Int(20), // Graphics.TOP | Graphics.LEFT
            Value::Int(90),
        ])
        .unwrap();

    assert_eq!(machine.execution.counters.draw_region_calls, 1);
    assert_eq!(
        machine.graphics_int_array_snapshot(target_pixels).unwrap(),
        [0, -16_711_936, 0, 0, -65_536, 0, 0, 0, 0]
    );

    // Image anchors include VCENTER, applied to the rotated dimensions.
    // The 2x1 source becomes 1x2: centering it at (1,1) gives the same pixels.
    for index in 0..9 {
        machine
            .heap
            .managed
            .array_set(target_pixels, index, HeapValue::Int(0))
            .unwrap();
    }
    let centered = [
        Value::Reference(Some(graphics)),
        Value::Reference(Some(source_image)),
        Value::Int(1),
        Value::Int(1),
        Value::Int(3), // Graphics.HCENTER | Graphics.VCENTER
        Value::Int(90),
    ];
    machine.graphics_draw_nokia_image(&centered).unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(target_pixels).unwrap(),
        [0, -16_711_936, 0, 0, -65_536, 0, 0, 0, 0]
    );
    let mut conflicting = centered;
    conflicting[4] = Value::Int(18); // VCENTER | TOP remains invalid.
    assert_eq!(
        machine
            .graphics_draw_nokia_image(&conflicting)
            .unwrap_err()
            .code(),
        "illegal-argument"
    );

    machine
        .graphics_set_argb_color(&[
            Value::Reference(Some(graphics)),
            Value::Int(0x8040_2010_u32.cast_signed()),
        ])
        .unwrap();
    assert_eq!(
        machine
            .graphics_color_component(&[Value::Reference(Some(graphics))], 24)
            .unwrap(),
        128
    );
    machine
        .graphics_nokia_triangle(
            &[
                Value::Reference(Some(graphics)),
                Value::Int(0),
                Value::Int(0),
                Value::Int(2),
                Value::Int(0),
                Value::Int(0),
                Value::Int(2),
                Value::Int(0xffff_00ff_u32.cast_signed()),
            ],
            true,
        )
        .unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(target_pixels).unwrap()[0],
        0xffff_00ff_u32.cast_signed()
    );
}

#[test]
fn draw_image_streams_a_distinct_mutable_source_without_caching_it() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let source_pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 2)
        .unwrap();
    let source_values = [0xffff_0000_u32.cast_signed(), 0x8000_ff00_u32.cast_signed()];
    for (index, pixel) in source_values.into_iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(
                source_pixels,
                i32::try_from(index).unwrap(),
                HeapValue::Int(pixel),
            )
            .unwrap();
    }
    let source_image = machine
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
                    HeapValue::Int(1),
                ),
                (
                    "javax/microedition/lcdui/Image.pixels:[I".into(),
                    HeapValue::Reference(Some(source_pixels)),
                ),
            ]),
        )
        .unwrap();
    let (graphics, target_pixels) = direct_graphics_target(&mut machine, 4, 1);
    machine
        .graphics_int_array_mut(target_pixels)
        .unwrap()
        .fill(HeapValue::Int(0xff00_0000_u32.cast_signed()));

    machine
        .graphics_draw_image(&[
            Value::Reference(Some(graphics)),
            Value::Reference(Some(source_image)),
            Value::Int(1),
            Value::Int(0),
        ])
        .unwrap();

    assert_eq!(
        machine.graphics_int_array_snapshot(target_pixels).unwrap(),
        [
            0xff00_0000_u32.cast_signed(),
            source_values[0],
            0xff00_8000_u32.cast_signed(),
            0xff00_0000_u32.cast_signed(),
        ]
    );
    assert_eq!(
        machine.graphics_int_array_snapshot(source_pixels).unwrap(),
        source_values
    );
    assert!(
        !machine
            .heap
            .immutable_image_pixels
            .contains_key(&source_pixels)
    );
}

#[test]
fn tiled_layer_intrinsic_only_draws_tiles_intersecting_the_clip() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);

    let source_pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 4)
        .unwrap();
    for (index, pixel) in [-65_536, -16_711_936, -16_777_216, -1]
        .into_iter()
        .enumerate()
    {
        machine
            .heap
            .managed
            .array_set(
                source_pixels,
                i32::try_from(index).unwrap(),
                HeapValue::Int(pixel),
            )
            .unwrap();
    }
    let source_image = machine
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
                    HeapValue::Int(2),
                ),
                (
                    "javax/microedition/lcdui/Image.mutable:Z".into(),
                    HeapValue::Int(0),
                ),
                (
                    "javax/microedition/lcdui/Image.pixels:[I".into(),
                    HeapValue::Reference(Some(source_pixels)),
                ),
            ]),
        )
        .unwrap();

    let (graphics, target_pixels) = direct_graphics_target(&mut machine, 4, 4);
    set_graphics_fields(&mut machine, graphics, &[("tx", 3), ("ty", 5)]);

    let cells = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 100 * 100)
        .unwrap();
    for index in 0..100 * 100 {
        machine
            .heap
            .managed
            .array_set(cells, index, HeapValue::Int(1))
            .unwrap();
    }
    let animated = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    let layer = machine
        .heap.managed
        .allocate_object(
            "javax/microedition/lcdui/game/TiledLayer",
            HashMap::from([
                (
                    "javax/microedition/lcdui/game/Layer.visible:Z".into(),
                    HeapValue::Int(1),
                ),
                (
                    "javax/microedition/lcdui/game/Layer.x:I".into(),
                    HeapValue::Int(-3),
                ),
                (
                    "javax/microedition/lcdui/game/Layer.y:I".into(),
                    HeapValue::Int(-5),
                ),
                (
                    "javax/microedition/lcdui/game/TiledLayer.cols:I".into(),
                    HeapValue::Int(100),
                ),
                (
                    "javax/microedition/lcdui/game/TiledLayer.rows:I".into(),
                    HeapValue::Int(100),
                ),
                (
                    "javax/microedition/lcdui/game/TiledLayer.tw:I".into(),
                    HeapValue::Int(2),
                ),
                (
                    "javax/microedition/lcdui/game/TiledLayer.th:I".into(),
                    HeapValue::Int(2),
                ),
                (
                    "javax/microedition/lcdui/game/TiledLayer.image:Ljavax/microedition/lcdui/Image;"
                        .into(),
                    HeapValue::Reference(Some(source_image)),
                ),
                (
                    "javax/microedition/lcdui/game/TiledLayer.cells:[I".into(),
                    HeapValue::Reference(Some(cells)),
                ),
                (
                    "javax/microedition/lcdui/game/TiledLayer.animated:[I".into(),
                    HeapValue::Reference(Some(animated)),
                ),
            ]),
        )
        .unwrap();

    assert!(
        machine
            .graphics_tiled_layer_paint(&[
                Value::Reference(Some(layer)),
                Value::Reference(Some(graphics)),
            ])
            .unwrap()
    );
    assert_eq!(machine.execution.counters.draw_region_calls, 4);
    assert_eq!(
        machine.graphics_int_array_snapshot(target_pixels).unwrap(),
        [
            -65_536,
            -16_711_936,
            -65_536,
            -16_711_936,
            -16_777_216,
            -1,
            -16_777_216,
            -1,
            -65_536,
            -16_711_936,
            -65_536,
            -16_711_936,
            -16_777_216,
            -1,
            -16_777_216,
            -1,
        ]
    );
}

#[test]
fn tiled_layer_empty_cells_remain_cancellable_across_rows_and_columns() {
    let program = Program::new();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (source_graphics, _) = direct_graphics_target(&mut machine, 1, 1);
    let image = machine
        .graphics_reference_field(
            source_graphics,
            "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;",
        )
        .unwrap();
    for (width, height) in [(8192, 1), (1, 8192), (128, 128)] {
        let (graphics, pixels) = direct_graphics_target(&mut machine, width, height);
        let cells = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, (width + 1) * (height + 1))
            .unwrap();
        let mut fields = HashMap::new();
        for (name, value) in [("visible:Z", 1), ("x:I", -1), ("y:I", -1)] {
            fields.insert(
                format!("javax/microedition/lcdui/game/Layer.{name}"),
                HeapValue::Int(value),
            );
        }
        for (name, value) in [
            ("cols:I", width + 1),
            ("rows:I", height + 1),
            ("tw:I", 1),
            ("th:I", 1),
        ] {
            fields.insert(
                format!("javax/microedition/lcdui/game/TiledLayer.{name}"),
                HeapValue::Int(value),
            );
        }
        for (name, handle) in [
            ("image:Ljavax/microedition/lcdui/Image;", image),
            ("cells:[I", cells),
            ("animated:[I", cells),
        ] {
            fields.insert(
                format!("javax/microedition/lcdui/game/TiledLayer.{name}"),
                HeapValue::Reference(Some(handle)),
            );
        }
        let layer = machine
            .heap
            .managed
            .allocate_object("javax/microedition/lcdui/game/TiledLayer", fields)
            .unwrap();
        let args = [
            Value::Reference(Some(layer)),
            Value::Reference(Some(graphics)),
        ];
        for point in [1, 2] {
            checks.set(0);
            cancel_at.set(point);
            assert_eq!(
                machine
                    .graphics_tiled_layer_paint(&args)
                    .unwrap_err()
                    .code(),
                "execution-cancelled"
            );
        }
        checks.set(0);
        cancel_at.set(usize::MAX);
        assert!(machine.graphics_tiled_layer_paint(&args).unwrap());
        assert!(checks.get() >= 8);
        assert_eq!(machine.execution.counters.draw_region_calls, 0);
        assert!(
            machine
                .graphics_int_array_snapshot(pixels)
                .unwrap()
                .iter()
                .all(|pixel| *pixel == 0)
        );
    }
}
