use super::*;

#[test]
fn checkpoint_rejects_image_dimensions_that_do_not_match_pixels() {
    let valid = Image2DState::mutable(ImageFormat::Rgba, 2, 2).unwrap();
    let restored: Image2DState = save_state::decode(&save_state::encode(&valid).unwrap()).unwrap();
    restored.validate_checkpoint().unwrap();
    for (width, height) in [(0, 2), (2, 3), (u32::MAX, u32::MAX)] {
        let mut invalid = valid.clone();
        invalid.width = width;
        invalid.height = height;
        let limits = crate::ArenaLimits::default();
        let mut runtime = crate::Runtime::new_with_graph_depth(limits, 32);
        runtime
            .create(None, crate::ObjectKind::Image2D(invalid))
            .unwrap();
        assert!(
            runtime
                .validate_checkpoint(limits, 32, 1024, |_| true)
                .is_err()
        );
    }
}

#[test]
fn image_formats_decode_and_mutable_updates_are_atomic() {
    let image = Image2DState::from_bytes(ImageFormat::Rgba, 1, 1, &[1, 2, 3, 4]).unwrap();
    assert_eq!(image.pixels.as_ref(), [0x0401_0203]);
    let mut mutable = Image2DState::mutable(ImageFormat::Rgb, 2, 2).unwrap();
    mutable.set(1, 1, 1, 1, &[10, 20, 30, 99, 98, 97]).unwrap();
    assert_eq!(mutable.pixels[3], 0xff0a_141e);
    let before = mutable.clone();
    mutable.set(0, 0, 1, 1, &[40, 50, 60]).unwrap();
    assert_eq!(before.pixels[0], 0xffff_ffff);
    assert_eq!(mutable.pixels[0], 0xff28_323c);
    let before = mutable.clone();
    assert!(mutable.set(0, 0, 2, 0, &[]).is_err());
    assert!(mutable.set(0, 0, 0, 2, &[]).is_err());
    assert_eq!(mutable, before);
    assert!(mutable.set(1, 1, 2, 1, &[0; 6]).is_err());
    assert_eq!(mutable, before);
}

#[test]
fn mutable_images_start_opaque_white_in_every_format() {
    for format in [
        ImageFormat::Alpha,
        ImageFormat::Luminance,
        ImageFormat::LuminanceAlpha,
        ImageFormat::Rgb,
        ImageFormat::Rgba,
    ] {
        let image = Image2DState::mutable(format, 3, 2).unwrap();
        assert_eq!(image.pixels(), &[0xffff_ffff; 6], "{format:?}");
    }
}

#[test]
fn rendering_into_rgb_discards_alpha_and_keeps_existing_snapshots_independent() {
    let source = [0x0012_3456, 0x7f98_7654, 0xffff_0000, 0];
    for (format, expected) in [
        (ImageFormat::Rgb, source.map(|pixel| pixel | 0xff00_0000)),
        (ImageFormat::Rgba, source),
    ] {
        let mut image = Image2DState::mutable(format, 2, 2).unwrap();
        let before = image.clone();
        image.load_argb(&source).unwrap();
        assert_eq!(image.pixels(), &expected);
        assert_eq!(before.pixels(), &[0xffff_ffff; 4]);
        assert!(image.load_argb(&source[..3]).is_err());
        assert_eq!(image.pixels(), &expected);
    }
}

#[test]
fn rectangular_image_updates_decode_rows_without_touching_the_surrounding_pixels() {
    let mut image = Image2DState::mutable(ImageFormat::Rgba, 4, 3).unwrap();
    image
        .set(
            1,
            1,
            2,
            2,
            &[
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 99, 99, 99, 99,
            ],
        )
        .unwrap();
    assert_eq!(
        image.pixels(),
        &[
            0xffff_ffff,
            0xffff_ffff,
            0xffff_ffff,
            0xffff_ffff,
            0xffff_ffff,
            0x0401_0203,
            0x0805_0607,
            0xffff_ffff,
            0xffff_ffff,
            0x0c09_0a0b,
            0x100d_0e0f,
            0xffff_ffff,
        ]
    );
    let before = image.clone();
    assert!(image.set(1, 1, 2, 2, &[0; 15]).is_err());
    assert_eq!(image, before);
}

#[test]
fn immutable_images_copy_only_the_required_source_prefix() {
    let direct =
        Image2DState::from_bytes(ImageFormat::Rgb, 1, 1, &[10, 20, 30, 99, 98, 97]).unwrap();
    assert_eq!(direct.pixels.as_ref(), [0xff0a_141e]);

    let palette = Image2DState::from_palette(
        ImageFormat::Rgba,
        2,
        2,
        &[0, 1, 0, 1, 9, 9, 9, 9],
        &[255, 0, 0, 255, 0, 255, 0, 128],
    )
    .unwrap();
    assert_eq!(
        palette.pixels.as_ref(),
        [0xffff_0000, 0x8000_ff00, 0xffff_0000, 0x8000_ff00]
    );

    let shared: Arc<[u32]> = vec![0xff01_0203].into();
    let image = Image2DState::from_shared_argb(1, 1, Arc::clone(&shared)).unwrap();
    assert!(Arc::ptr_eq(&image.pixels, &shared));
    assert!(Image2DState::from_shared_argb(2, 1, shared).is_err());
}

#[test]
fn short_palettes_leave_unspecified_entries_deterministic() {
    let image = Image2DState::from_palette(ImageFormat::Rgb, 2, 1, &[0, 1], &[10, 20, 30]).unwrap();
    assert_eq!(image.pixels.as_ref(), [0xff0a_141e, 0xff00_0000]);

    assert!(Image2DState::from_palette(ImageFormat::Rgb, 1, 1, &[0], &[10, 20]).is_err());
}

#[test]
fn linear_fog_supports_reversed_tiny_and_wide_distance_ranges() {
    for (near, far, midpoint) in [
        (1.0, 3.0, 2.0),
        (3.0, 1.0, 2.0),
        (0.0, 1.0e-20, 0.5e-20_f32),
        (-f32::MAX, f32::MAX, 0.0),
    ] {
        let fog = FogState {
            color: 0x0000_00ff,
            mode: FogMode::Linear,
            near,
            far,
            density: 1.0,
        };
        assert_eq!(fog.apply(0x80ff_0000, f64::from(near)), 0x80ff_0000);
        assert_eq!(fog.apply(0x80ff_0000, f64::from(far)), 0x8000_00ff);
        assert_eq!(fog.apply(0x80ff_0000, f64::from(midpoint)), 0x8080_0080);
    }
}
