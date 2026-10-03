use super::graphics_support::direct_graphics_target;
use super::*;

#[test]
fn m3g_refresh_validates_all_managed_pixels_before_overwriting_the_renderer() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (target, pixels) = direct_graphics_target(&mut machine, 2, 2);
    machine.m3g.graphics.target = Some(target);
    machine.m3g.graphics.renderer =
        m3g::SoftwareRenderer::new(2, 2, machine.limits.m3g_render).unwrap();
    machine.m3g.graphics.renderer.load_pixels(&[7; 4]).unwrap();
    let Allocation::Array { elements, .. } = machine.heap.managed.get_mut(pixels).unwrap() else {
        unreachable!()
    };
    elements.fill(HeapValue::Int(-1));
    elements[3] = HeapValue::Float(2.0);
    assert!(machine.m3g_refresh_bound_target().is_err());
    assert_eq!(machine.m3g.graphics.renderer.pixels(), [7; 4]);
    machine
        .heap
        .managed
        .array_set(pixels, 3, HeapValue::Int(0x1234))
        .unwrap();
    machine.m3g_refresh_bound_target().unwrap();
    assert_eq!(
        machine.m3g.graphics.renderer.pixels(),
        [u32::MAX, u32::MAX, u32::MAX, 0x1234]
    );
}

#[test]
#[ignore = "manual M3G framebuffer transfer throughput measurement"]
fn m3g_target_refresh_throughput() {
    for (width, height) in [(1, 1), (8, 8), (240, 320), (640, 480)] {
        for midp in [false, true] {
            let program = Program::new();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let expected: Vec<u32> = (0..width * height)
                .map(|index: i32| index.cast_unsigned().wrapping_mul(0x1234_5679))
                .collect();
            let target = if midp {
                let (target, pixels) = direct_graphics_target(&mut machine, width, height);
                let Allocation::Array { elements, .. } =
                    machine.heap.managed.get_mut(pixels).unwrap()
                else {
                    unreachable!()
                };
                for (slot, pixel) in elements.iter_mut().zip(&expected) {
                    *slot = HeapValue::Int(pixel.cast_signed());
                }
                target
            } else {
                let mut image =
                    m3g::Image2DState::mutable(m3g::ImageFormat::Rgba, width as u32, height as u32)
                        .unwrap();
                image.load_argb(&expected).unwrap();
                m3g_support::object(
                    &mut machine,
                    "javax/microedition/m3g/Image2D",
                    m3g::ObjectKind::Image2D(image),
                )
                .0
            };
            machine.m3g.graphics.target = Some(target);
            machine.m3g.graphics.renderer =
                m3g::SoftwareRenderer::new(width as u32, height as u32, machine.limits.m3g_render)
                    .unwrap();
            let repetitions = (16_777_216 / expected.len()).clamp(128, 32_768);
            let started = std::time::Instant::now();
            for _ in 0..repetitions {
                machine.m3g_refresh_bound_target().unwrap();
                std::hint::black_box(machine.m3g.graphics.renderer.pixels());
            }
            let elapsed = started.elapsed();
            assert_eq!(machine.m3g.graphics.renderer.pixels(), expected);
            assert!(
                machine
                    .m3g
                    .graphics
                    .renderer
                    .depth()
                    .iter()
                    .all(|value| *value == u32::MAX)
            );
            let checksum = expected.iter().fold(0_u64, |sum, pixel| {
                sum.wrapping_mul(31).wrapping_add(u64::from(*pixel))
            });
            eprintln!(
                "refresh={width}x{height} midp={midp} elapsed={elapsed:?} checksum={checksum:016x}"
            );
        }
    }
}

#[test]
#[ignore = "manual M3G target binding throughput measurement"]
fn m3g_target_bind_throughput() {
    for (width, height) in [(1, 1), (8, 8), (240, 320), (640, 480)] {
        for depth in [false, true] {
            let program = Program::new();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let (target, pixels) = direct_graphics_target(&mut machine, width, height);
            let Allocation::Array { elements, .. } = machine.heap.managed.get_mut(pixels).unwrap()
            else {
                unreachable!()
            };
            elements.fill(HeapValue::Int(0x1234_5678));
            let graphics3d = machine
                .heap
                .managed
                .allocate_object("javax/microedition/m3g/Graphics3D", HashMap::new())
                .unwrap();
            let args = [
                Value::Reference(Some(graphics3d)),
                Value::Reference(Some(target)),
                Value::Int(i32::from(depth)),
                Value::Int(0),
            ];
            let repetitions = (16_777_216 / (width * height) as usize).clamp(128, 8192);
            let started = std::time::Instant::now();
            for _ in 0..repetitions {
                machine
                    .invoke_m3g_graphics3d_native(
                        "javax/microedition/m3g/Graphics3D",
                        "bindTarget",
                        "(Ljava/lang/Object;ZI)V",
                        std::hint::black_box(&args),
                    )
                    .unwrap();
                machine
                    .invoke_m3g_graphics3d_native(
                        "javax/microedition/m3g/Graphics3D",
                        "releaseTarget",
                        "()V",
                        &args[..1],
                    )
                    .unwrap();
            }
            let elapsed = started.elapsed();
            assert!(
                machine
                    .m3g
                    .graphics
                    .renderer
                    .pixels()
                    .iter()
                    .all(|pixel| *pixel == 0x1234_5678)
            );
            assert!(
                machine
                    .m3g
                    .graphics
                    .renderer
                    .depth()
                    .iter()
                    .all(|value| *value == u32::MAX)
            );
            eprintln!(
                "bind={width}x{height} depth={depth} elapsed={elapsed:?} pixels={} color=12345678",
                machine.m3g.graphics.renderer.pixels().len()
            );
        }
    }
}

#[test]
fn m3g_viewport_uses_the_graphics_origin_captured_at_bind() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 8, 8);
    let graphics3d = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Graphics3D", HashMap::new())
        .unwrap();
    let invoke = |machine: &mut Machine<'_, '_>, name, descriptor, tail: &[Value]| {
        let mut args = vec![Value::Reference(Some(graphics3d))];
        args.extend_from_slice(tail);
        machine
            .invoke_m3g_graphics3d_native(
                "javax/microedition/m3g/Graphics3D",
                name,
                descriptor,
                &args,
            )
            .unwrap()
    };
    for index in 0..64 {
        machine
            .heap
            .managed
            .array_set(pixels, index, HeapValue::Int(0xffff_ffff_u32.cast_signed()))
            .unwrap();
    }
    for (field, value) in [("tx", 2), ("ty", 1)] {
        machine
            .heap
            .managed
            .set_field(
                graphics,
                &format!("javax/microedition/lcdui/Graphics.{field}:I"),
                HeapValue::Int(value),
            )
            .unwrap();
    }
    machine
        .graphics_set_clip(&[
            Value::Reference(Some(graphics)),
            Value::Int(0),
            Value::Int(0),
            Value::Int(3),
            Value::Int(4),
        ])
        .unwrap();
    invoke(
        &mut machine,
        "bindTarget",
        "(Ljava/lang/Object;)V",
        &[Value::Reference(Some(graphics))],
    );
    for (name, expected) in [
        ("getViewportX", 0),
        ("getViewportY", 0),
        ("getViewportWidth", 3),
        ("getViewportHeight", 4),
    ] {
        assert!(
            matches!(invoke(&mut machine, name, "()I", &[]),
            CallOutcome::Return(Some(Value::Int(value))) if value == expected),
            "{name}"
        );
    }
    for field in ["tx", "ty"] {
        machine
            .heap
            .managed
            .set_field(
                graphics,
                &format!("javax/microedition/lcdui/Graphics.{field}:I"),
                HeapValue::Int(-2),
            )
            .unwrap();
    }
    invoke(
        &mut machine,
        "setViewport",
        "(IIII)V",
        &[Value::Int(1), Value::Int(1), Value::Int(2), Value::Int(2)],
    );
    invoke(
        &mut machine,
        "clear",
        "(Ljavax/microedition/m3g/Background;)V",
        &[Value::Reference(None)],
    );
    let actual = machine.graphics_int_array_snapshot(pixels).unwrap();
    for (index, pixel) in actual.iter().enumerate() {
        let expected = if (3..5).contains(&(index % 8)) && (2..4).contains(&(index / 8)) {
            0xff00_0000
        } else {
            0xffff_ffff
        };
        assert_eq!(pixel.cast_unsigned(), expected, "pixel {index}");
    }
    invoke(&mut machine, "releaseTarget", "()V", &[]);

    let image_guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Image2D", HashMap::new())
        .unwrap();
    let image = machine
        .m3g
        .runtime
        .create(
            Some(image_guest.to_raw()),
            m3g::ObjectKind::Image2D(
                m3g::Image2DState::mutable(m3g::ImageFormat::Rgb, 4, 4).unwrap(),
            ),
        )
        .unwrap();
    invoke(
        &mut machine,
        "bindTarget",
        "(Ljava/lang/Object;)V",
        &[Value::Reference(Some(image_guest))],
    );
    invoke(
        &mut machine,
        "setViewport",
        "(IIII)V",
        &[Value::Int(1), Value::Int(1), Value::Int(1), Value::Int(1)],
    );
    invoke(
        &mut machine,
        "clear",
        "(Ljavax/microedition/m3g/Background;)V",
        &[Value::Reference(None)],
    );
    let m3g::ObjectKind::Image2D(image) = machine.m3g.runtime.kind(image).unwrap() else {
        panic!("expected Image2D")
    };
    let mut expected = [0xffff_ffff; 16];
    expected[5] = 0xff00_0000;
    assert_eq!(image.pixels(), &expected);
}

#[test]
fn m3g_empty_midp_clips_stay_empty_after_bind_and_clear() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 8, 8);
    let graphics3d = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Graphics3D", HashMap::new())
        .unwrap();
    for clip in [
        [1, 1, 0, 2],
        [1, 1, 2, 0],
        [8, 0, 1, 1],
        [0, 8, 1, 1],
        [-2, 0, 1, 1],
    ] {
        for index in 0..64 {
            machine
                .heap
                .managed
                .array_set(pixels, index, HeapValue::Int(0xffff_ffff_u32.cast_signed()))
                .unwrap();
        }
        let mut clip_args = vec![Value::Reference(Some(graphics))];
        clip_args.extend(clip.map(Value::Int));
        machine.graphics_set_clip(&clip_args).unwrap();
        for (name, descriptor, value) in [
            ("bindTarget", "(Ljava/lang/Object;)V", Some(graphics)),
            ("clear", "(Ljavax/microedition/m3g/Background;)V", None),
        ] {
            machine
                .invoke_m3g_graphics3d_native(
                    "javax/microedition/m3g/Graphics3D",
                    name,
                    descriptor,
                    &[Value::Reference(Some(graphics3d)), Value::Reference(value)],
                )
                .unwrap();
        }
        assert!(
            machine
                .graphics_int_array_snapshot(pixels)
                .unwrap()
                .iter()
                .all(|pixel| pixel.cast_unsigned() == 0xffff_ffff),
            "{clip:?}"
        );
        machine
            .invoke_m3g_graphics3d_native(
                "javax/microedition/m3g/Graphics3D",
                "releaseTarget",
                "()V",
                &[Value::Reference(Some(graphics3d))],
            )
            .unwrap();
    }
}

#[test]
fn m3g_midp_target_rejects_invalid_dimensions_before_snapshotting() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, _) = direct_graphics_target(&mut machine, 2, 2);
    let image = machine
        .graphics_reference_field(
            graphics,
            "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;",
        )
        .unwrap();
    for (width, height) in [(0, 2), (2, 0), (-1, 2), (2, -1), (i32::MAX, 2), (2, 3)] {
        for (field, value) in [("width", width), ("height", height)] {
            machine
                .heap
                .managed
                .set_field(
                    image,
                    &format!("javax/microedition/lcdui/Image.{field}:I"),
                    HeapValue::Int(value),
                )
                .unwrap();
        }
        assert!(
            machine.m3g_target_snapshot(graphics).is_err(),
            "{width}x{height}"
        );
    }
}

#[test]
fn m3g_midp_publication_validates_storage_before_writing_and_forces_opaque_alpha() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (graphics, _) = direct_graphics_target(&mut machine, 2, 2);
    let image = machine
        .graphics_reference_field(
            graphics,
            "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;",
        )
        .unwrap();
    machine.m3g.graphics.renderer =
        m3g::SoftwareRenderer::new(2, 2, machine.limits.m3g_render).unwrap();
    machine.m3g.graphics.target = Some(graphics);
    let colors = [0x0012_3456, 0x7f98_7654, 0xffff_0000, 0];
    machine.m3g.graphics.renderer.load_pixels(&colors).unwrap();
    for length in [3, 5, 4] {
        let pixels = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, length)
            .unwrap();
        for index in 0..length {
            machine
                .heap
                .managed
                .array_set(pixels, index, HeapValue::Int(7))
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
        let result = machine.m3g_publish_bound_target();
        let actual = machine.graphics_int_array_snapshot(pixels).unwrap();
        if length == 4 {
            result.unwrap();
            assert_eq!(
                actual,
                colors.map(|color| (color | 0xff00_0000).cast_signed())
            );
        } else {
            assert!(result.is_err());
            assert!(actual.iter().all(|pixel| *pixel == 7));
        }
    }
}

#[test]
pub(crate) fn m3g_target_rebind_keeps_depth_only_for_same_enabled_surface() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let first_target = machine
        .heap
        .managed
        .allocate_object("m3g-depth-target-a", HashMap::new())
        .unwrap();
    let second_target = machine
        .heap
        .managed
        .allocate_object("m3g-depth-target-b", HashMap::new())
        .unwrap();
    let limits = machine.limits.m3g_render;
    let triangle = |z, color| {
        [
            m3g::Vertex::new(m3g::Vec4::new(-0.8, -0.8, z, 1.0), color),
            m3g::Vertex::new(m3g::Vec4::new(0.8, -0.8, z, 1.0), color),
            m3g::Vertex::new(m3g::Vec4::new(0.0, 0.8, z, 1.0), color),
        ]
    };

    machine
        .m3g
        .graphics
        .bind_target_surface(
            first_target,
            8,
            8,
            vec![0xff00_0000; 64],
            [0, 0, 8, 8],
            true,
            limits,
        )
        .unwrap();
    machine
        .m3g
        .graphics
        .renderer
        .draw_triangle(triangle(-0.5, 0xffff_0000))
        .unwrap();
    let first_frame = machine.m3g.graphics.renderer.pixels().to_vec();

    machine
        .m3g
        .graphics
        .bind_target_surface(first_target, 8, 8, first_frame, [0, 0, 8, 8], true, limits)
        .unwrap();
    machine
        .m3g
        .graphics
        .renderer
        .draw_triangle(triangle(0.5, 0xff00_ff00))
        .unwrap();
    assert_eq!(
        machine.m3g.graphics.renderer.pixels()[4 * 8 + 4],
        0xffff_0000
    );
    machine
        .m3g
        .graphics
        .bind_target_surface(
            second_target,
            8,
            8,
            vec![0xff00_0000; 64],
            [0, 0, 8, 8],
            true,
            limits,
        )
        .unwrap();
    machine
        .m3g
        .graphics
        .renderer
        .draw_triangle(triangle(0.5, 0xff00_ff00))
        .unwrap();
    assert_eq!(
        machine.m3g.graphics.renderer.pixels()[4 * 8 + 4],
        0xff00_ff00
    );
}

#[test]
pub(crate) fn m3g_depth_test_accepts_fragments_at_equal_depth() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let target = machine
        .heap
        .managed
        .allocate_object("m3g-equal-depth-target", HashMap::new())
        .unwrap();
    let triangle = |color| {
        [
            m3g::Vertex::new(m3g::Vec4::new(-0.8, -0.8, 0.0, 1.0), color),
            m3g::Vertex::new(m3g::Vec4::new(0.8, -0.8, 0.0, 1.0), color),
            m3g::Vertex::new(m3g::Vec4::new(0.0, 0.8, 0.0, 1.0), color),
        ]
    };

    machine
        .m3g
        .graphics
        .bind_target_surface(
            target,
            8,
            8,
            vec![0xff00_0000; 64],
            [0, 0, 8, 8],
            true,
            machine.limits.m3g_render,
        )
        .unwrap();
    machine
        .m3g
        .graphics
        .renderer
        .draw_triangle(triangle(0xffff_0000))
        .unwrap();
    machine
        .m3g
        .graphics
        .renderer
        .draw_triangle(triangle(0xff00_ff00))
        .unwrap();

    assert_eq!(
        machine.m3g.graphics.renderer.pixels()[4 * 8 + 4],
        0xff00_ff00
    );
}
