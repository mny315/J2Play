use super::*;

#[test]
fn draw_rgb_null_source_throws_null_pointer_in_public_and_interpreted_paths() {
    let mut program = Program::new();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(4, 3) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    let original = program.methods[&MethodKey {
        class: "javax/microedition/lcdui/Graphics".into(),
        name: "drawRGB".into(),
        descriptor: "([IIIIIIIZ)V".into(),
    }]
        .clone();
    for interpreted in [true, false] {
        let mut method = original.clone();
        if interpreted {
            method.key.class = "test/InterpretedGraphics".into();
            method.stack_key = Arc::new(method.key.clone());
        }
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let (graphics, _) = direct_graphics_target(&mut machine, 4, 3);
        for (width, height) in [(1, 1), (0, 0), (-1, -1)] {
            let CallOutcome::Throw(exception) = machine
                .call(
                    &method,
                    [
                        Value::Reference(Some(graphics)),
                        Value::Reference(None),
                        Value::Int(0),
                        Value::Int(1),
                        Value::Int(0),
                        Value::Int(0),
                        Value::Int(width),
                        Value::Int(height),
                        Value::Int(1),
                    ],
                    1,
                )
                .unwrap()
            else {
                panic!("null drawRGB source must throw");
            };
            assert_eq!(
                machine.object_class(exception).unwrap(),
                "java/lang/NullPointerException"
            );
        }
    }
}

#[test]
fn draw_rgb_overlap_snapshots_only_visible_rows_and_preserves_negative_stride() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 4, 3);
    let capacity = machine.graphics.draw_rgb_scratch.capacity();
    let original = (0..12)
        .map(|value| 0xff00_0000_u32.cast_signed() | value)
        .collect::<Vec<_>>();
    for (offset, stride, x, y, width, height, expected) in [
        (
            0,
            0,
            0,
            0,
            4,
            i32::MAX,
            vec![0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3],
        ),
        (
            8,
            -4,
            -1,
            1,
            4,
            3,
            vec![0, 1, 2, 3, 9, 10, 11, 7, 5, 6, 7, 11],
        ),
    ] {
        for (slot, &pixel) in machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .iter_mut()
            .zip(&original)
        {
            *slot = HeapValue::Int(pixel);
        }
        machine
            .graphics_draw_rgb(&[
                Value::Reference(Some(graphics)),
                Value::Reference(Some(pixels)),
                Value::Int(offset),
                Value::Int(stride),
                Value::Int(x),
                Value::Int(y),
                Value::Int(width),
                Value::Int(height),
                Value::Int(1),
            ])
            .unwrap();
        assert_eq!(machine.graphics.draw_rgb_scratch.capacity(), capacity);
        assert!(machine.graphics.draw_rgb_scratch.len() <= 12);
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            expected
                .into_iter()
                .map(|value| 0xff00_0000_u32.cast_signed() | value)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn draw_rgb_rejects_invalid_source_before_growing_scratch() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let capacity = machine.graphics.draw_rgb_scratch.capacity();
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 1)
        .unwrap();
    for (offset, stride, width, height) in [(0, 512, 512, 512), (-1, 0, 1, 1024)] {
        assert_eq!(
            machine
                .graphics_int_array_strided_snapshot_into_scratch(
                    array, offset, stride, width, height
                )
                .unwrap_err()
                .code(),
            "array-index-out-of-bounds-exception"
        );
        assert_eq!(machine.graphics.draw_rgb_scratch.capacity(), capacity);
    }
    assert_eq!(
        machine
            .graphics_int_array_strided_snapshot_into_scratch(array, 0, 0, 1, i32::MAX)
            .unwrap_err()
            .code(),
        "out-of-memory-error"
    );
    assert_eq!(machine.graphics.draw_rgb_scratch.capacity(), capacity);
}

#[test]
fn draw_rgb_reuses_compact_source_scratch() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 12)
        .unwrap();
    for index in 0..12 {
        machine
            .heap
            .managed
            .array_set(array, index, HeapValue::Int(index))
            .unwrap();
    }
    machine
        .graphics_int_array_strided_snapshot_into_scratch(array, 2, 4, 4, 2)
        .unwrap();
    assert_eq!(machine.graphics.draw_rgb_scratch, [2, 3, 4, 5, 6, 7, 8, 9]);
    let capacity = machine.graphics.draw_rgb_scratch.capacity();
    machine
        .graphics_int_array_strided_snapshot_into_scratch(array, 4, 2, 2, 2)
        .unwrap();
    assert_eq!(machine.graphics.draw_rgb_scratch, [4, 5, 6, 7]);
    assert_eq!(machine.graphics.draw_rgb_scratch.capacity(), capacity);
}
