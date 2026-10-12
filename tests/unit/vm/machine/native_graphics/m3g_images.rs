use super::*;

const IMAGE: &str = "javax/microedition/m3g/Image2D";

fn byte_array(machine: &mut Machine<'_, '_>, length: i32, prefix: &[u8]) -> Handle {
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, length)
        .unwrap();
    machine
        .m3g_write_primitive_int_array(array, &ArrayKind::Byte, prefix)
        .unwrap();
    array
}

fn pixels<'a>(machine: &'a Machine<'_, '_>, guest: Handle) -> &'a [u32] {
    let m3g::ObjectKind::Image2D(image) = machine
        .m3g
        .runtime
        .kind(machine.m3g_handle(guest).unwrap())
        .unwrap()
    else {
        panic!("expected Image2D")
    };
    image.pixels()
}

#[test]
fn image_constructors_copy_unsigned_pixel_prefixes_and_ignore_long_array_tails() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let colors = byte_array(&mut machine, 65_536, &[1, 2, 3, 128, 129, 130]);
    let indices = byte_array(&mut machine, 65_536, &[1, 0]);
    for (descriptor, sources, expected) in [
        ("(III[B)V", vec![colors], [0xff01_0203, 0xff80_8182]),
        (
            "(III[B[B)V",
            vec![indices, colors],
            [0xff80_8182, 0xff01_0203],
        ),
    ] {
        let constructor = runtime_method(
            IMAGE,
            "<init>",
            descriptor,
            &[0xb1],
            0,
            6,
            Vec::new(),
            false,
        );
        let guest = machine
            .heap
            .managed
            .allocate_object(IMAGE, HashMap::new())
            .unwrap();
        let mut args = vec![
            Value::Reference(Some(guest)),
            Value::Int(99),
            Value::Int(2),
            Value::Int(1),
        ];
        args.extend(
            sources
                .into_iter()
                .map(|source| Value::Reference(Some(source))),
        );
        machine.invoke_m3g_native(&constructor, &args, 1).unwrap();
        assert_eq!(pixels(&machine, guest), expected);
    }
    // A short direct source must not leave a half-created native owner.
    let short = byte_array(&mut machine, 5, &[1, 2, 3, 4, 5]);
    let constructor = runtime_method(
        IMAGE,
        "<init>",
        "(III[B)V",
        &[0xb1],
        0,
        5,
        Vec::new(),
        false,
    );
    let guest = machine
        .heap
        .managed
        .allocate_object(IMAGE, HashMap::new())
        .unwrap();
    let before = machine.m3g.runtime.counters();
    let error = machine
        .invoke_m3g_native(
            &constructor,
            &[
                Value::Reference(Some(guest)),
                Value::Int(99),
                Value::Int(2),
                Value::Int(1),
                Value::Reference(Some(short)),
            ],
            1,
        )
        .err()
        .expect("short pixels");
    assert_eq!(
        java_error_class(&error),
        Some("java/lang/IllegalArgumentException")
    );
    assert_eq!(machine.m3g.runtime.counters(), before);
    assert!(machine.m3g_handle(guest).is_err());
}

#[test]
fn image_region_updates_copy_the_required_prefix_and_preserve_pixels_on_failure() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (guest, native) = m3g_support::object(
        &mut machine,
        IMAGE,
        m3g::ObjectKind::Image2D(m3g::Image2DState::mutable(m3g::ImageFormat::Rgb, 2, 2).unwrap()),
    );
    let colors = byte_array(&mut machine, 65_536, &[1, 2, 3, 128, 129, 130]);
    let short = byte_array(&mut machine, 5, &[1, 2, 3, 4, 5]);
    for (width, height, source, valid) in [
        (2, 1, colors, true),
        (2, 1, short, false),
        (0, 1, colors, false),
        (2, 2, colors, false),
        (i32::MAX, 1, colors, false),
    ] {
        let before = machine.m3g.runtime.kind(native).unwrap().clone();
        let result = machine.invoke_m3g_object_native(
            IMAGE,
            "set",
            "(IIII[B)V",
            &[
                Value::Reference(Some(guest)),
                Value::Int(0),
                Value::Int(1),
                Value::Int(width),
                Value::Int(height),
                Value::Reference(Some(source)),
            ],
        );
        if valid {
            assert!(matches!(result.unwrap(), CallOutcome::Return(None)));
            assert_eq!(
                pixels(&machine, guest),
                [0xffff_ffff, 0xffff_ffff, 0xff01_0203, 0xff80_8182]
            );
            let m3g::ObjectKind::Image2D(snapshot) = before else {
                unreachable!()
            };
            assert_eq!(snapshot.pixels(), [0xffff_ffff; 4]);
        } else {
            assert_eq!(
                java_error_class(&result.err().expect("invalid update")),
                Some("java/lang/IllegalArgumentException")
            );
            let m3g::ObjectKind::Image2D(snapshot) = before else {
                unreachable!()
            };
            assert_eq!(pixels(&machine, guest), snapshot.pixels());
        }
    }
}

#[test]
fn image_constructors_report_invalid_dimensions_without_allocating_native_state() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let source = byte_array(&mut machine, 4, &[1, 2, 3, 4]);
    for descriptor in ["(III)V", "(III[B)V", "(III[B[B)V"] {
        let constructor = runtime_method(
            IMAGE,
            "<init>",
            descriptor,
            &[0xb1],
            0,
            6,
            Vec::new(),
            false,
        );
        for (width, height) in [(0, 1), (1, 0), (0, 0), (-1, 1), (1, -1)] {
            let guest = machine
                .heap
                .managed
                .allocate_object(IMAGE, HashMap::new())
                .unwrap();
            let mut args = vec![
                Value::Reference(Some(guest)),
                Value::Int(100),
                Value::Int(width),
                Value::Int(height),
            ];
            if descriptor.contains("[B") {
                args.push(Value::Reference(Some(source)));
            }
            if descriptor.ends_with("[B[B)V") {
                args.push(Value::Reference(Some(source)));
            }
            let before = machine.m3g.runtime.counters();
            let error = machine
                .invoke_m3g_native(&constructor, &args, 1)
                .err()
                .expect("invalid dimensions");
            assert_eq!(
                java_error_class(&error),
                Some("java/lang/IllegalArgumentException"),
                "{descriptor} {width}x{height}"
            );
            assert_eq!(machine.m3g.runtime.counters(), before);
            assert!(machine.m3g_handle(guest).is_err());
        }
    }
}
