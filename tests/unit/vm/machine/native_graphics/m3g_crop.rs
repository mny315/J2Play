use super::*;

fn image(machine: &mut Machine<'_, '_>, width: u32, height: u32) -> (Handle, m3g::Handle) {
    m3g_support::object(
        machine,
        "javax/microedition/m3g/Image2D",
        m3g::ObjectKind::Image2D(
            m3g::Image2DState::mutable(m3g::ImageFormat::Rgba, width, height).unwrap(),
        ),
    )
}

fn crop(machine: &Machine<'_, '_>, object: m3g::Handle) -> [i32; 4] {
    match machine.m3g.runtime.kind(object).unwrap() {
        m3g::ObjectKind::Background(state) => state.crop,
        m3g::ObjectKind::Sprite3D(state) => state.crop,
        _ => panic!("expected a cropped image"),
    }
}

#[test]
fn sprite_constructor_and_image_changes_reset_crop_to_the_profile_limit() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            m3g_max_sprite_crop_dimension: 7,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let (first, _) = image(&mut machine, 8, 4);
    let (second, second_native) = image(&mut machine, 2, 16);
    let sprite = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Sprite3D", HashMap::new())
        .unwrap();
    let constructor = runtime_method(
        "javax/microedition/m3g/Sprite3D",
        "<init>",
        "(ZLjavax/microedition/m3g/Image2D;Ljavax/microedition/m3g/Appearance;)V",
        &[0xb1],
        0,
        4,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &constructor,
            &[
                Value::Reference(Some(sprite)),
                Value::Int(1),
                Value::Reference(Some(first)),
                Value::Reference(None),
            ],
            1,
        )
        .unwrap();
    let native = machine.m3g_handle(sprite).unwrap();
    assert_eq!(crop(&machine, native), [0, 0, 7, 4]);
    for _ in 0..2 {
        let m3g::ObjectKind::Sprite3D(state) = machine.m3g.runtime.kind_mut(native).unwrap() else {
            unreachable!()
        };
        state.crop = [3, -2, -4, 5];
        machine
            .invoke_m3g_mesh_native(
                "javax/microedition/m3g/Sprite3D",
                "setImage",
                "(Ljavax/microedition/m3g/Image2D;)V",
                &[
                    Value::Reference(Some(sprite)),
                    Value::Reference(Some(second)),
                ],
            )
            .unwrap();
        assert_eq!(crop(&machine, native), [0, 0, 2, 7]);
        let m3g::ObjectKind::Sprite3D(state) = machine.m3g.runtime.kind(native).unwrap() else {
            unreachable!()
        };
        assert_eq!(state.image, second_native);
    }
    let error = machine
        .invoke_m3g_mesh_native(
            "javax/microedition/m3g/Sprite3D",
            "setImage",
            "(Ljavax/microedition/m3g/Image2D;)V",
            &[Value::Reference(Some(sprite)), Value::Reference(None)],
        )
        .err()
        .expect("a null sprite image must fail");
    assert_eq!(
        java_error_class(&error),
        Some("java/lang/NullPointerException")
    );
    assert_eq!(crop(&machine, native), [0, 0, 2, 7]);
}

#[test]
fn crop_setters_preserve_state_on_invalid_sizes_and_accept_boundary_coordinates() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            m3g_max_sprite_crop_dimension: 7,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let (_, image) = image(&mut machine, 2, 2);
    let (sprite, native_sprite) = m3g_support::object(
        &mut machine,
        "javax/microedition/m3g/Sprite3D",
        m3g::ObjectKind::Sprite3D(m3g::SpriteState {
            node: m3g::NodeState::default(),
            scaled: false,
            image,
            appearance: None,
            crop: [0, 0, 2, 2],
        }),
    );
    let (background, native_background) = m3g_support::object(
        &mut machine,
        "javax/microedition/m3g/Background",
        m3g::ObjectKind::Background(m3g::BackgroundState::default()),
    );
    for (guest, native, sprite) in [
        (sprite, native_sprite, true),
        (background, native_background, false),
    ] {
        for ([width, height], sprite_valid, background_valid) in [
            ([7, 7], true, true),
            ([-7, 7], true, false),
            ([7, -7], true, false),
            ([0, 0], true, true),
            ([8, 7], false, true),
            ([7, -8], false, false),
            ([i32::MAX, i32::MAX], false, true),
            ([i32::MIN, 1], false, false),
            ([1, i32::MIN], false, false),
        ] {
            let before = crop(&machine, native);
            let wanted = [i32::MIN, i32::MAX, width, height];
            let args = [
                Value::Reference(Some(guest)),
                Value::Int(wanted[0]),
                Value::Int(wanted[1]),
                Value::Int(width),
                Value::Int(height),
            ];
            let result = if sprite {
                machine.invoke_m3g_mesh_native(
                    "javax/microedition/m3g/Sprite3D",
                    "setCrop",
                    "(IIII)V",
                    &args,
                )
            } else {
                machine.invoke_m3g_world_native(
                    "javax/microedition/m3g/Background",
                    "setCrop",
                    "(IIII)V",
                    &args,
                )
            };
            let valid = if sprite {
                sprite_valid
            } else {
                background_valid
            };
            if valid {
                assert!(matches!(result.unwrap(), CallOutcome::Return(None)));
                assert_eq!(crop(&machine, native), wanted);
            } else {
                let error = result.err().expect("invalid crop must be rejected");
                assert_eq!(
                    java_error_class(&error),
                    Some("java/lang/IllegalArgumentException")
                );
                assert_eq!(crop(&machine, native), before);
            }
        }
    }
}

#[test]
pub(crate) fn m3g_background_set_image_resets_crop_to_the_complete_image() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let background = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Background", HashMap::new())
        .unwrap();
    let image = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Image2D", HashMap::new())
        .unwrap();
    for (class, descriptor, receiver, tail) in [
        (
            "javax/microedition/m3g/Background",
            "()V",
            background,
            vec![],
        ),
        (
            "javax/microedition/m3g/Image2D",
            "(III)V",
            image,
            vec![Value::Int(99), Value::Int(240), Value::Int(320)],
        ),
    ] {
        let constructor = runtime_method(
            class,
            "<init>",
            descriptor,
            &[0xb1],
            0,
            4,
            Vec::new(),
            false,
        );
        let mut arguments = vec![Value::Reference(Some(receiver))];
        arguments.extend(tail);
        machine
            .invoke_m3g_native(&constructor, &arguments, 1)
            .unwrap();
    }
    let set_image = runtime_method(
        "javax/microedition/m3g/Background",
        "setImage",
        "(Ljavax/microedition/m3g/Image2D;)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &set_image,
            &[
                Value::Reference(Some(background)),
                Value::Reference(Some(image)),
            ],
            1,
        )
        .unwrap();
    let native = machine.m3g_handle(background).unwrap();
    let m3g::ObjectKind::Background(state) = machine.m3g.runtime.kind(native).unwrap() else {
        panic!("background changed native type");
    };
    assert_eq!(state.crop, [0, 0, 240, 320]);

    machine
        .invoke_m3g_native(
            &set_image,
            &[Value::Reference(Some(background)), Value::Reference(None)],
            1,
        )
        .unwrap();
    let m3g::ObjectKind::Background(state) = machine.m3g.runtime.kind(native).unwrap() else {
        panic!("background changed native type");
    };
    assert_eq!(state.crop, [0; 4]);
}
