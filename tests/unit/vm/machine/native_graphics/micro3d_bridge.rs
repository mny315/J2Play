use super::*;

#[test]
fn micro3d_bridge_executes_fixed_point_utility_and_frame_contracts() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for (name, input, expected) in [("sin", 1024, 4096), ("cos", 2048, -4096), ("sqrt", -81, 9)] {
        let method = runtime_method(
            "com/mascotcapsule/micro3d/v3/Util3D",
            name,
            "(I)I",
            &[0xac],
            1,
            1,
            Vec::new(),
            true,
        );
        let outcome = machine
            .invoke_micro3d_native(&method, &[Value::Int(input)])
            .unwrap();
        assert!(
            matches!(outcome, CallOutcome::Return(Some(Value::Int(value))) if value == expected)
        );
    }

    let _reserved_zero = machine
        .heap
        .managed
        .allocate_object("sentinel", HashMap::new())
        .unwrap();
    let action_table = machine
        .heap
        .managed
        .allocate_object("com/mascotcapsule/micro3d/v3/ActionTable", HashMap::new())
        .unwrap();
    machine
        .micro3d
        .runtime
        .create(
            action_table.to_raw(),
            micro3d::ObjectKind::Action(micro3d::ActionTableData {
                format_version: 5,
                frame_counts: vec![14],
                actions: vec![micro3d::ActionData {
                    frame_count: 14,
                    segments: Vec::new(),
                    pattern_keys: Vec::new(),
                }],
            }),
        )
        .unwrap();
    for name in ["getNumFrame", "getNumFrames"] {
        let method = runtime_method(
            "com/mascotcapsule/micro3d/v3/ActionTable",
            name,
            "(I)I",
            &[0xac],
            1,
            2,
            Vec::new(),
            false,
        );
        assert!(matches!(
            machine
                .invoke_micro3d_native(
                    &method,
                    &[Value::Reference(Some(action_table)), Value::Int(0)]
                )
                .unwrap(),
            CallOutcome::Return(Some(Value::Int(917_504)))
        ));
    }
}

#[test]
fn micro3d_rotation_methods_preserve_the_affine_translation_column() {
    let original = micro3d::AffineTrans::new([1, 2, 3, 111, 4, 5, 6, -222, 7, 8, 9, 333]);

    for rotation in [
        micro3d::AffineTrans::rotation_x(731),
        micro3d::AffineTrans::rotation_y(731),
        micro3d::AffineTrans::rotation_z(731),
        micro3d::AffineTrans::rotation(micro3d::Vector3D::new(1, 2, 3), 731),
    ] {
        let result = micro3d_preserve_translation(rotation, original);
        assert_eq!(result.values[3], 111);
        assert_eq!(result.values[7], -222);
        assert_eq!(result.values[11], 333);
        for index in [0, 1, 2, 4, 5, 6, 8, 9, 10] {
            assert_eq!(result.values[index], rotation.values[index]);
        }
    }
}

#[test]
fn micro3d_command_lists_account_for_point_sprite_parameter_layouts() {
    assert_eq!(
        micro3d_primitive_payload_counts(0x0500_1000, 7).unwrap(),
        (7, 0, 8, 0)
    );
    assert_eq!(
        micro3d_primitive_payload_counts(0x0500_2000, 7).unwrap(),
        (7, 0, 56, 0)
    );
    assert_eq!(
        micro3d_primitive_payload_counts(0x0500_3000, 7).unwrap(),
        (7, 0, 56, 0)
    );
    assert_eq!(
        micro3d_primitive_payload_counts(0x0500_0000, 1)
            .unwrap_err()
            .code(),
        "illegal-argument-exception"
    );
}

#[test]
fn micro3d_command_lists_account_for_point_vertices_and_colors() {
    assert_eq!(
        micro3d_primitive_payload_counts(0x0100_0400, 7).unwrap(),
        (7, 0, 0, 1)
    );
    assert_eq!(
        micro3d_primitive_payload_counts(0x0100_0800, 7).unwrap(),
        (7, 0, 0, 7)
    );
    assert_eq!(
        micro3d_primitive_payload_counts(0x0200_0400, 2).unwrap(),
        (4, 0, 0, 1)
    );
    assert_eq!(
        micro3d_primitive_payload_counts(0x0300_0100, 1)
            .unwrap_err()
            .code(),
        "illegal-argument-exception"
    );
}

#[test]
fn micro3d_command_clip_intersects_lcdui_clip_and_target_bounds() {
    assert_eq!(
        micro3d_command_scissor(240, 320, [10, 20, 100, 200], [0, 40, 80, 400]).unwrap(),
        [10, 40, 70, 180]
    );
    assert_eq!(
        micro3d_command_scissor(240, 320, [0, 0, 240, 320], [-10, -20, 300, 400]).unwrap(),
        [0, 0, 240, 320]
    );
    assert_eq!(
        micro3d_command_scissor(240, 320, [0, 0, 240, 320], [80, 40, 20, 60])
            .unwrap_err()
            .code(),
        "illegal-argument-exception"
    );
}

#[test]
fn micro3d_command_lists_apply_state_lighting_lines_and_persistent_clip() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 32 * 32)
        .unwrap();
    let image = machine
        .heap
        .managed
        .allocate_object(
            "javax/microedition/lcdui/Image",
            HashMap::from([
                (
                    "javax/microedition/lcdui/Image.width:I".into(),
                    HeapValue::Int(32),
                ),
                (
                    "javax/microedition/lcdui/Image.height:I".into(),
                    HeapValue::Int(32),
                ),
                (
                    "javax/microedition/lcdui/Image.pixels:[I".into(),
                    HeapValue::Reference(Some(pixels)),
                ),
            ]),
        )
        .unwrap();
    let graphics = machine
        .heap
        .managed
        .allocate_object(
            "javax/microedition/lcdui/Graphics",
            HashMap::from([(
                "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;".into(),
                HeapValue::Reference(Some(image)),
            )]),
        )
        .unwrap();
    let layout = machine
        .heap
        .managed
        .allocate_object("com/mascotcapsule/micro3d/v3/FigureLayout", HashMap::new())
        .unwrap();
    let effect = machine
        .heap
        .managed
        .allocate_object("com/mascotcapsule/micro3d/v3/Effect3D", HashMap::new())
        .unwrap();
    machine
        .micro3d
        .runtime
        .create(
            layout.to_raw(),
            micro3d::ObjectKind::Layout(micro3d::FigureLayoutState::default()),
        )
        .unwrap();
    machine
        .micro3d
        .runtime
        .create(
            effect.to_raw(),
            micro3d::ObjectKind::Effect(micro3d::EffectState::default()),
        )
        .unwrap();
    machine.micro3d.target = Some(graphics);
    machine.micro3d.target_scissor = [0, 0, 32, 32];
    machine.micro3d.command_scissor = machine.micro3d.target_scissor;

    let commands = |machine: &mut Machine<'_, '_>, values: &[i32]| {
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, i32::try_from(values.len()).unwrap())
            .unwrap();
        for (index, value) in values.iter().copied().enumerate() {
            machine
                .heap
                .managed
                .array_set(array, i32::try_from(index).unwrap(), HeapValue::Int(value))
                .unwrap();
        }
        array
    };
    let clipped = commands(
        &mut machine,
        &[
            -33_554_431,
            0x8400_0000_u32.cast_signed(),
            8,
            10,
            24,
            28,
            i32::MIN,
        ],
    );
    let args = |commands| {
        [
            Value::Reference(None),
            Value::Reference(None),
            Value::Int(0),
            Value::Int(0),
            Value::Reference(Some(layout)),
            Value::Reference(Some(effect)),
            Value::Reference(Some(commands)),
        ]
    };
    machine.micro3d_draw_command_list(&args(clipped)).unwrap();
    assert_eq!(machine.micro3d.command_scissor, [8, 10, 16, 18]);

    let unchanged = commands(&mut machine, &[-33_554_431, i32::MIN]);
    machine.micro3d_draw_command_list(&args(unchanged)).unwrap();
    assert_eq!(machine.micro3d.command_scissor, [8, 10, 16, 18]);

    let rendered = commands(
        &mut machine,
        &[
            -33_554_431,
            0x8300_0001_u32.cast_signed(),
            0x8500_0000_u32.cast_signed(),
            16,
            16,
            0x9000_0000_u32.cast_signed(),
            4096,
            4096,
            0xa000_0000_u32.cast_signed(),
            2048,
            0xa100_0000_u32.cast_signed(),
            0,
            0,
            4096,
            0,
            0x0301_0a00,
            -8,
            -8,
            0,
            8,
            -8,
            0,
            0,
            8,
            0,
            0,
            0,
            4096,
            0x00ff_0000,
            0x0201_0400,
            -6,
            4,
            0,
            6,
            4,
            0,
            0x0000_ff00,
            0x8200_0000_u32.cast_signed(),
            i32::MIN,
        ],
    );
    machine.micro3d_draw_command_list(&args(rendered)).unwrap();
    assert_eq!(
        machine
            .heap
            .managed
            .array_get(pixels, 16 * 32 + 16)
            .unwrap(),
        HeapValue::Int(0xff80_0000_u32.cast_signed())
    );
    assert!((0..32 * 32).any(|index| {
        machine.heap.managed.array_get(pixels, index).unwrap()
            == HeapValue::Int(0xff00_ff00_u32.cast_signed())
    }));
}

#[test]
fn default_micro3d_figure_layout_owns_an_affine_transform() {
    let limits = Limits::default();
    let mut program = Program::new();
    for class in micro3d::bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(limits, false, &mut context);
    let _reserved_zero = machine
        .heap
        .managed
        .allocate_object("sentinel", HashMap::new())
        .unwrap();
    let class = "com/mascotcapsule/micro3d/v3/FigureLayout";
    let fields = machine
        .instance_fields(class)
        .unwrap()
        .into_iter()
        .map(|field| (field.key.to_string(), field.initial))
        .collect();
    let layout = machine.heap.managed.allocate_object(class, fields).unwrap();
    let constructor = runtime_method(class, "<init>", "()V", &[0xb1], 0, 1, Vec::new(), false);
    machine
        .invoke_micro3d_native(&constructor, &[Value::Reference(Some(layout))])
        .unwrap();
    let getter = runtime_method(
        class,
        "getAffineTrans",
        "()Lcom/mascotcapsule/micro3d/v3/AffineTrans;",
        &[0xb0],
        1,
        1,
        Vec::new(),
        false,
    );

    let CallOutcome::Return(Some(Value::Reference(Some(affine)))) = machine
        .invoke_micro3d_native(&getter, &[Value::Reference(Some(layout))])
        .unwrap()
    else {
        panic!("default FigureLayout must return its AffineTrans");
    };
    assert_eq!(
        machine.micro3d_affine(affine).unwrap(),
        micro3d::AffineTrans::IDENTITY
    );
    assert_eq!(
        machine.micro3d.runtime.kind(layout.to_raw()).unwrap(),
        &micro3d::ObjectKind::Layout(micro3d::FigureLayoutState {
            affines: vec![affine.to_raw()],
            ..micro3d::FigureLayoutState::default()
        })
    );

    machine.collect_heap(vec![layout]);
    assert_eq!(
        machine.micro3d_affine(affine).unwrap(),
        micro3d::AffineTrans::IDENTITY
    );
}
