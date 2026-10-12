use super::graphics_support::direct_graphics_target;
use super::*;

#[test]
fn micro3d_prepare_validates_pixels_before_replacing_the_target_and_preserves_pending_work() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 2);
    machine.micro3d.target = Some(graphics);
    machine.micro3d.target_scissor = [0, 0, 3, 2];
    machine.micro3d.command_scissor = [1, 0, 2, 2];
    machine
        .micro3d
        .runtime
        .load_target(3, 2, &[1, 2, 3, 4, 5, 6])
        .unwrap();
    let Allocation::Array { elements, .. } = machine.heap.managed.get_mut(pixels).unwrap() else {
        unreachable!()
    };
    elements.fill(HeapValue::Int(-1));
    elements[5] = HeapValue::Float(2.0);
    assert!(machine.micro3d_prepare_render_target().is_err());
    assert_eq!(machine.micro3d.runtime.target_pixels(), [1, 2, 3, 4, 5, 6]);
    machine.micro3d.render_pending = true;
    machine.micro3d_prepare_render_target().unwrap();
    assert_eq!(machine.micro3d.runtime.target_pixels(), [1, 2, 3, 4, 5, 6]);
    assert!(machine.micro3d.render_pending);
    machine.micro3d.render_pending = false;
    machine
        .heap
        .managed
        .array_set(pixels, 5, HeapValue::Int(0x1234))
        .unwrap();
    machine.micro3d_prepare_render_target().unwrap();
    assert_eq!(
        machine.micro3d.runtime.target_pixels(),
        [u32::MAX, u32::MAX, u32::MAX, u32::MAX, u32::MAX, 0x1234]
    );
}

#[test]
#[ignore = "manual Micro3D target preparation throughput measurement"]
fn micro3d_target_prepare_throughput() {
    for (width, height) in [(1, 1), (8, 8), (240, 320), (640, 480)] {
        for clipped in [false, true] {
            let program = Program::new();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let (graphics, pixels) = direct_graphics_target(&mut machine, width, height);
            let expected: Vec<_> = (0..width * height)
                .map(|index| index.cast_unsigned().wrapping_mul(0x1234_5679))
                .collect();
            let Allocation::Array { elements, .. } = machine.heap.managed.get_mut(pixels).unwrap()
            else {
                unreachable!()
            };
            for (slot, pixel) in elements.iter_mut().zip(&expected) {
                *slot = HeapValue::Int(pixel.cast_signed());
            }
            machine.micro3d.target = Some(graphics);
            machine.micro3d.target_scissor = if clipped {
                [
                    width as u32 / 4,
                    height as u32 / 4,
                    width as u32 / 2,
                    height as u32 / 2,
                ]
            } else {
                [0, 0, width as u32, height as u32]
            };
            machine.micro3d.command_scissor = machine.micro3d.target_scissor;
            let repetitions = (16_777_216 / expected.len()).clamp(128, 32_768);
            let started = std::time::Instant::now();
            for _ in 0..repetitions {
                machine.micro3d_prepare_render_target().unwrap();
                std::hint::black_box(machine.micro3d.runtime.target_pixels());
            }
            let elapsed = started.elapsed();
            assert_eq!(machine.micro3d.runtime.target_pixels(), expected);
            let checksum = expected.iter().fold(0_u64, |sum, pixel| {
                sum.wrapping_mul(31).wrapping_add(u64::from(*pixel))
            });
            eprintln!(
                "micro_target={width}x{height} clipped={clipped} elapsed={elapsed:?} checksum={checksum:016x}"
            );
        }
    }
}

#[test]
fn micro3d_flush_validates_the_complete_target_before_writing() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 2);
    machine.micro3d.target = Some(graphics);
    machine
        .micro3d
        .runtime
        .load_target(3, 2, &[1, 2, 3, 4, 5, 6])
        .unwrap();

    for scissor in [[3, 0, 1, 1], [0, 1, 3, 2], [u32::MAX, 0, 2, 1]] {
        machine.micro3d.target_scissor = scissor;
        machine.micro3d.render_pending = true;
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(77));
        assert_eq!(
            machine.micro3d_flush_target().unwrap_err().code(),
            "framebuffer-size"
        );
        assert_eq!(
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            [77; 6]
        );
        assert!(machine.micro3d.render_pending);
    }

    machine.micro3d.target_scissor = [1, 0, 2, 2];
    machine.micro3d_flush_target().unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        [77, 2, 3, 77, 5, 6]
    );
    assert!(!machine.micro3d.render_pending);

    machine.micro3d.runtime.load_target(2, 2, &[0; 4]).unwrap();
    machine.micro3d.render_pending = true;
    assert_eq!(
        machine.micro3d_flush_target().unwrap_err().code(),
        "framebuffer-size"
    );
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        [77, 2, 3, 77, 5, 6]
    );

    for (width, height) in [(0, 2), (3, 0), (-1, 2), (3, -1)] {
        assert_eq!(
            machine
                .micro3d_graphics_scissor(graphics, width, height)
                .unwrap_err()
                .code(),
            "framebuffer-size"
        );
    }
}

#[test]
pub(crate) fn micro3d_release_preserves_lcdui_drawing_performed_after_bind() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let graphics3d = machine
        .heap
        .managed
        .allocate_object("com/mascotcapsule/micro3d/v3/Graphics3D", HashMap::new())
        .unwrap();
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 4)
        .unwrap();
    for index in 0..4 {
        machine
            .heap
            .managed
            .array_set(pixels, index, HeapValue::Int(0xffff_ffff_u32.cast_signed()))
            .unwrap();
    }
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
                    HeapValue::Int(2),
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
            HashMap::from([
                (
                    "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;"
                        .into(),
                    HeapValue::Reference(Some(image)),
                ),
                (
                    "javax/microedition/lcdui/Graphics.clipX:I".into(),
                    HeapValue::Int(1),
                ),
                (
                    "javax/microedition/lcdui/Graphics.clipY:I".into(),
                    HeapValue::Int(0),
                ),
                (
                    "javax/microedition/lcdui/Graphics.clipW:I".into(),
                    HeapValue::Int(1),
                ),
                (
                    "javax/microedition/lcdui/Graphics.clipH:I".into(),
                    HeapValue::Int(2),
                ),
            ]),
        )
        .unwrap();
    let call = |machine: &mut Machine<'_, '_>, name, descriptor, tail| {
        let method = runtime_method(
            "com/mascotcapsule/micro3d/v3/Graphics3D",
            name,
            descriptor,
            &[0xb1],
            0,
            2,
            Vec::new(),
            false,
        );
        let mut arguments = vec![Value::Reference(Some(graphics3d))];
        arguments.extend(tail);
        machine.invoke_micro3d_native(&method, &arguments).unwrap();
    };

    call(
        &mut machine,
        "bind",
        "(Ljavax/microedition/lcdui/Graphics;)V",
        vec![Value::Reference(Some(graphics))],
    );
    assert_eq!(machine.micro3d.target_scissor, [1, 0, 1, 2]);
    machine
        .heap
        .managed
        .array_set(pixels, 0, HeapValue::Int(0xff00_0000_u32.cast_signed()))
        .unwrap();
    machine
        .heap
        .managed
        .array_set(pixels, 1, HeapValue::Int(0xffff_0000_u32.cast_signed()))
        .unwrap();

    // With no pending 3D commands, neither flush nor release may restore
    // the framebuffer snapshot that existed when bind was called.
    call(&mut machine, "flush", "()V", vec![]);
    call(
        &mut machine,
        "release",
        "(Ljavax/microedition/lcdui/Graphics;)V",
        vec![Value::Reference(Some(graphics))],
    );
    assert_eq!(
        machine.heap.managed.array_get(pixels, 0).unwrap(),
        HeapValue::Int(0xff00_0000_u32.cast_signed())
    );
    assert_eq!(
        machine.heap.managed.array_get(pixels, 1).unwrap(),
        HeapValue::Int(0xffff_0000_u32.cast_signed())
    );

    call(
        &mut machine,
        "bind",
        "(Ljavax/microedition/lcdui/Graphics;)V",
        vec![Value::Reference(Some(graphics))],
    );
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
            micro3d::ObjectKind::Layout(micro3d::FigureLayoutState {
                scale: [4096, 4096],
                center: [1, 1],
                projection: micro3d::Projection::ParallelScale,
                ..micro3d::FigureLayoutState::default()
            }),
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
    let create_figure = |machine: &mut Machine<'_, '_>, color| {
        let figure = machine
            .heap
            .managed
            .allocate_object("com/mascotcapsule/micro3d/v3/Figure", HashMap::new())
            .unwrap();
        machine
            .micro3d
            .runtime
            .create(
                figure.to_raw(),
                micro3d::ObjectKind::Figure(micro3d::FigureState {
                    data: micro3d::FigureData {
                        format_version: 5,
                        uv_bits: 8,
                        vertices: vec![
                            micro3d::Vector3D::new(-2, -2, 0),
                            micro3d::Vector3D::new(2, -2, 0),
                            micro3d::Vector3D::new(0, 2, 0),
                        ],
                        normals: Vec::new(),
                        faces: vec![micro3d::Face {
                            indices: [0, 1, 2],
                            uv: [[0, 0]; 3],
                            attributes: 0x10,
                            color: Some(color),
                            pattern: 0,
                            material: None,
                        }],
                        bones: vec![micro3d::Bone {
                            vertex_count: 3,
                            parent: -1,
                            transform: micro3d::AffineTrans::IDENTITY,
                        }],
                        pattern_count: 1,
                        material_count: 0,
                    },
                    textures: Vec::new(),
                    selected_texture: 0,
                    pattern: 0,
                    posture: None,
                }),
            )
            .unwrap();
        figure
    };
    let immediate = create_figure(&mut machine, 0xff00_ff00);
    let queued = create_figure(&mut machine, 0xffff_0000);
    let figure_arguments = |figure| {
        vec![
            Value::Reference(Some(figure)),
            Value::Int(0),
            Value::Int(0),
            Value::Reference(Some(layout)),
            Value::Reference(Some(effect)),
        ]
    };
    call(
        &mut machine,
        "drawFigure",
        "(Lcom/mascotcapsule/micro3d/v3/Figure;IILcom/mascotcapsule/micro3d/v3/FigureLayout;Lcom/mascotcapsule/micro3d/v3/Effect3D;)V",
        figure_arguments(immediate),
    );
    assert!((0..4).any(|index| {
        machine.heap.managed.array_get(pixels, index).unwrap()
            == HeapValue::Int(0xff00_ff00_u32.cast_signed())
    }));
    call(
        &mut machine,
        "renderFigure",
        "(Lcom/mascotcapsule/micro3d/v3/Figure;IILcom/mascotcapsule/micro3d/v3/FigureLayout;Lcom/mascotcapsule/micro3d/v3/Effect3D;)V",
        figure_arguments(queued),
    );
    call(
        &mut machine,
        "release",
        "(Ljavax/microedition/lcdui/Graphics;)V",
        vec![Value::Reference(Some(graphics))],
    );
    assert!(!(0..4).any(|index| {
        machine.heap.managed.array_get(pixels, index).unwrap()
            == HeapValue::Int(0xffff_0000_u32.cast_signed())
    }));
}
