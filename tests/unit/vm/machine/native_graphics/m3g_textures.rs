use super::*;

fn texture_appearance(
    machine: &mut Machine<'_, '_>,
    size: u32,
) -> (m3g::Handle, m3g::Handle, m3g::Handle) {
    let (_, image) = m3g_support::object(
        machine,
        "javax/microedition/m3g/Image2D",
        m3g::ObjectKind::Image2D(
            m3g::Image2DState::mutable(m3g::ImageFormat::Rgba, size, size).unwrap(),
        ),
    );
    let (_, texture) = m3g_support::object(
        machine,
        "javax/microedition/m3g/Texture2D",
        m3g::ObjectKind::Texture2D(m3g::TextureObjectState {
            transformable: m3g::TransformableState::default(),
            image,
            level_filter: 209,
            image_filter: 210,
            wrap_s: 241,
            wrap_t: 241,
            blending: 228,
            blend_color: 0,
        }),
    );
    let (_, appearance) = m3g_support::object(
        machine,
        "javax/microedition/m3g/Appearance",
        m3g::ObjectKind::Appearance(m3g::AppearanceState {
            textures: [Some(texture), None],
            ..m3g::AppearanceState::default()
        }),
    );
    (appearance, image, texture)
}

#[test]
fn rebinding_textures_observes_image_updates_and_disabled_mipmaps() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (appearance, image, texture) = texture_appearance(&mut machine, 4);
    let sample = |machine: &Machine<'_, '_>| {
        let texture = machine.m3g.graphics.renderer.texture_unit(0).unwrap();
        (
            texture.shade_coordinates_lod([0.5, 0.5, 0.0], 0, 2.0),
            texture.allocated_bytes(),
        )
    };
    machine.m3g_apply_appearance(appearance).unwrap();
    assert_eq!(sample(&machine), (0xffff_ffff, 84));
    machine.m3g_apply_appearance(appearance).unwrap();
    assert_eq!(sample(&machine), (0xffff_ffff, 84));
    let m3g::ObjectKind::Image2D(image) = machine.m3g.runtime.kind_mut(image).unwrap() else {
        unreachable!()
    };
    image.load_argb(&[0xff12_3456; 16]).unwrap();
    // The bound texture still owns its old snapshot until the next draw bind.
    assert_eq!(sample(&machine), (0xffff_ffff, 84));
    machine.m3g_apply_appearance(appearance).unwrap();
    assert_eq!(sample(&machine), (0xff12_3456, 84));
    let m3g::ObjectKind::Texture2D(texture) = machine.m3g.runtime.kind_mut(texture).unwrap() else {
        unreachable!()
    };
    texture.level_filter = 208;
    machine.m3g_apply_appearance(appearance).unwrap();
    assert_eq!(sample(&machine), (0xff12_3456, 64));
}

#[test]
#[ignore = "manual repeated M3G texture binding throughput measurement"]
fn repeated_mipmap_binding_throughput() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (appearance, _, _) = texture_appearance(&mut machine, 256);
    machine.m3g_apply_appearance(appearance).unwrap();
    let started = std::time::Instant::now();
    for _ in 0..1024 {
        machine
            .m3g_apply_appearance(std::hint::black_box(appearance))
            .unwrap();
        std::hint::black_box(&machine.m3g.graphics.renderer);
    }
    eprintln!("1024 repeated mipmap binds: {:?}", started.elapsed());
}
