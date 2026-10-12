use super::*;

fn construct(machine: &mut Machine<'_, '_>, class: &str) -> Handle {
    let guest = machine
        .heap
        .managed
        .allocate_object(class, HashMap::new())
        .unwrap();
    let method = runtime_method(class, "<init>", "()V", &[0xb1], 1, 1, Vec::new(), false);
    assert!(matches!(
        machine
            .invoke_m3g_native(&method, &[Value::Reference(Some(guest))], 1)
            .unwrap(),
        CallOutcome::Return(None)
    ));
    guest
}

#[test]
fn default_light_is_directional_and_illuminates_only_front_facing_normals() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let guest = construct(&mut machine, "javax/microedition/m3g/Light");
    assert!(matches!(
        machine
            .invoke_m3g_world_native(
                "javax/microedition/m3g/Light",
                "getMode",
                "()I",
                &[Value::Reference(Some(guest))]
            )
            .unwrap(),
        CallOutcome::Return(Some(Value::Int(129)))
    ));
    machine
        .m3g
        .graphics
        .lights
        .push((guest, m3g::Mat4::IDENTITY));
    let lights = machine.m3g_light_sources(u32::MAX).unwrap();
    for (normal_z, expected) in [(1.0, 0xffcc_cccc), (-1.0, 0xff00_0000)] {
        let color = m3g::shade_lit_vertex(
            m3g::MaterialState::default(),
            u32::MAX,
            false,
            m3g::Vec3::new(0.0, 0.0, normal_z),
            m3g::Vec3::default(),
            m3g::Vec3::new(0.0, 0.0, 1.0),
            &lights,
        )
        .unwrap();
        assert_eq!(color, expected);
    }
}

#[test]
fn default_fog_uses_linear_distance() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let guest = construct(&mut machine, "javax/microedition/m3g/Fog");
    assert!(matches!(
        machine
            .invoke_m3g_appearance_native(
                "javax/microedition/m3g/Fog",
                "getMode",
                "()I",
                &[Value::Reference(Some(guest))]
            )
            .unwrap(),
        CallOutcome::Return(Some(Value::Int(81)))
    ));
    let native = machine.m3g_handle(guest).unwrap();
    let m3g::ObjectKind::Fog(fog) = machine.m3g.runtime.kind(native).unwrap() else {
        unreachable!()
    };
    for (distance, expected) in [(0.0, u32::MAX), (0.5, 0xff80_8080), (1.0, 0xff00_0000)] {
        assert_eq!(fog.apply(u32::MAX, distance), expected);
    }
}

#[test]
fn fog_linear_setter_preserves_reversed_equal_and_negative_distances() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let guest = construct(&mut machine, "javax/microedition/m3g/Fog");
    for [near, far] in [[3.0_f32, 1.0], [1.0, 1.0], [-2.0, 3.0], [3.0, -2.0]] {
        assert!(matches!(
            machine
                .invoke_m3g_appearance_native(
                    "javax/microedition/m3g/Fog",
                    "setLinear",
                    "(FF)V",
                    &[
                        Value::Reference(Some(guest)),
                        Value::Float(near),
                        Value::Float(far),
                    ]
                )
                .unwrap(),
            CallOutcome::Return(None)
        ));
        for (method, expected) in [("getNearDistance", near), ("getFarDistance", far)] {
            let outcome = machine
                .invoke_m3g_appearance_native(
                    "javax/microedition/m3g/Fog",
                    method,
                    "()F",
                    &[Value::Reference(Some(guest))],
                )
                .unwrap();
            assert!(
                matches!(outcome, CallOutcome::Return(Some(Value::Float(value))) if value.to_bits() == expected.to_bits())
            );
        }
    }
}
