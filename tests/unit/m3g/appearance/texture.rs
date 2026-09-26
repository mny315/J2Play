use super::*;

#[path = "texture/checkpoint.rs"]
mod checkpoint;

#[test]
fn white_modulation_and_replacement_preserve_format_alpha_rules() {
    for format in [
        ImageFormat::Alpha,
        ImageFormat::Luminance,
        ImageFormat::LuminanceAlpha,
        ImageFormat::Rgb,
        ImageFormat::Rgba,
    ] {
        for alpha in [0, 1, 127, 128, 254, 255] {
            for fragment_alpha in [0, 1, 127, 128, 254, 255] {
                for component in 0..=255 {
                    let texture = alpha << 24 | (component * 0x0001_0101);
                    let fragment = fragment_alpha << 24 | 0x00ff_ffff;
                    let color = if format == ImageFormat::Alpha {
                        0x00ff_ffff
                    } else {
                        texture & 0x00ff_ffff
                    };
                    let modulated_alpha = (alpha * fragment_alpha + 127) / 255;
                    assert_eq!(
                        blend_texture(BlendFunction::Modulate, format, texture, fragment, 0),
                        modulated_alpha << 24 | color
                    );
                    let replaced_alpha =
                        if matches!(format, ImageFormat::Luminance | ImageFormat::Rgb) {
                            fragment_alpha
                        } else {
                            alpha
                        };
                    assert_eq!(
                        blend_texture(BlendFunction::Replace, format, texture, fragment, 0),
                        replaced_alpha << 24 | color
                    );
                }
            }
        }
    }
}

#[test]
fn repeated_image_bindings_share_mip_levels_without_copying_sampling_state() {
    let image = Image2DState::mutable(ImageFormat::Rgba, 8, 8).unwrap();
    let mut previous = Texture2DState::new(image.clone());
    previous.set_wrapping(WrapMode::Clamp, WrapMode::Clamp);
    previous.set_transform(Mat4::translation(0.5, 0.0, 0.0).unwrap());
    previous.set_blend_function(BlendFunction::Replace);
    previous.set_mipmap_filter(true, true);

    let mut next = Texture2DState::new(image.clone());
    next.reuse_mipmaps(&previous);
    next.set_mipmap_filter(true, false);
    let mut fresh = Texture2DState::new(image);
    fresh.set_mipmap_filter(true, false);
    assert_eq!(next, fresh);
    assert_eq!(next.mipmaps.len(), 4);
    for (actual, cached) in next.mipmaps.iter().zip(&previous.mipmaps) {
        assert!(Arc::ptr_eq(&actual.pixels, &cached.pixels));
    }
}

#[test]
fn image_updates_and_sampling_identity_changes_reject_stale_mip_levels() {
    let mut image = Image2DState::mutable(ImageFormat::Rgba, 4, 4).unwrap();
    let mut previous = Texture2DState::new(image.clone());
    previous.set_mipmap_filter(true, true);
    let mut reshaped = image.clone();
    reshaped.width = 2;
    reshaped.height = 8;
    let mut reformatted = image.clone();
    reformatted.format = ImageFormat::Rgb;
    image.load_argb(&[0xff12_3456; 16]).unwrap();
    for changed in [reshaped, reformatted, image] {
        let mut next = Texture2DState::new(changed.clone());
        next.reuse_mipmaps(&previous);
        assert_eq!(next.mipmaps.len(), 1);
        next.set_mipmap_filter(true, true);
        let mut fresh = Texture2DState::new(changed);
        fresh.set_mipmap_filter(true, true);
        assert_eq!(next, fresh);
    }

    let image = Image2DState::mutable(ImageFormat::Rgba, 4, 4).unwrap();
    let mut keyed = Texture2DState::new(image.clone());
    keyed.set_color_key(Some(0x00ff_ffff));
    keyed.set_mipmap_filter(true, true);
    let mut plain = Texture2DState::new(image);
    plain.reuse_mipmaps(&keyed);
    assert_eq!(plain.mipmaps.len(), 1);
    plain.set_mipmap_filter(true, true);
    assert_eq!(
        plain.shade_coordinates_lod([0.5, 0.5, 0.0], u32::MAX, 2.0),
        u32::MAX
    );
    assert_eq!(
        keyed.shade_coordinates_lod([0.5, 0.5, 0.0], u32::MAX, 2.0),
        0
    );
}

#[test]
fn mipmaps_do_not_treat_filtered_colors_as_source_color_keys() {
    // No source pixel has the key color. Downsampling creates that RGB value,
    // which must retain the opaque alpha of its source pixels at every level.
    let pixels: Vec<_> = (0..16)
        .map(|index| {
            if index % 2 == 0 {
                0xff00_0000
            } else {
                0xff02_0202
            }
        })
        .collect();
    let image = Image2DState::from_argb(ImageFormat::Rgba, 4, 4, &pixels).unwrap();
    let mut texture = Texture2DState::new(image);
    texture.set_blend_function(BlendFunction::Replace);
    texture.set_color_key(Some(0x0001_0101));
    for linear in [false, true] {
        texture.set_linear_filter(linear);
        texture.set_mipmap_filter(true, linear);
        for lod in [1.0, 1.5, 2.0] {
            assert_eq!(
                texture.shade_coordinates_lod([0.5, 0.5, 0.0], 0, lod),
                0xff01_0101,
                "linear={linear}, lod={lod}"
            );
        }
    }
}

#[test]
fn nearest_addressing_preserves_boundary_and_extreme_coordinates() {
    let reference = |value: f64, size: u32, wrap| {
        let addressed = match wrap {
            WrapMode::Clamp => value.clamp(0.0, 1.0 - f64::EPSILON),
            WrapMode::Repeat => value.rem_euclid(1.0),
        };
        let texel = (addressed * f64::from(size)).floor() as i64;
        match wrap {
            WrapMode::Clamp => texel.clamp(0, i64::from(size) - 1) as usize,
            WrapMode::Repeat => texel.rem_euclid(i64::from(size)) as usize,
        }
    };
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let random = (0..10_000).map(|_| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        f64::from_bits(seed)
    });
    let boundaries = [
        -f64::MAX,
        -1.0,
        -f64::MIN_POSITIVE,
        -f64::from_bits(1),
        -0.0,
        0.0,
        0.5,
        1.0 - f64::EPSILON,
        1.0,
        1.0 + f64::EPSILON,
        f64::MAX,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NAN,
    ];
    for value in boundaries.into_iter().chain(random) {
        for size in [1, 2, 3, 63, 64, 127, 256, 1023, 1024] {
            for wrap in [WrapMode::Clamp, WrapMode::Repeat] {
                assert_eq!(
                    texture_index(value, size, wrap),
                    reference(value, size, wrap),
                    "value={value:?}, size={size}, wrap={wrap:?}"
                );
            }
        }
    }
}

#[test]
fn linear_addressing_matches_full_modulo_at_boundaries_and_extreme_coordinates() {
    let mut seed = 0x184_2026_u64;
    let random = (0..10_000).map(|_| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        f64::from_bits(seed)
    });
    let boundaries = [
        -f64::MAX,
        -1.0,
        -f64::from_bits(1),
        -0.0,
        0.0,
        0.5,
        1.0 - f64::EPSILON,
        1.0,
        1.0 + f64::EPSILON,
        f64::MAX,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NAN,
    ];
    for value in boundaries.into_iter().chain(random) {
        for size in [1, 2, 3, 63, 64, 127, 256, 1023, 1024] {
            for wrap in [WrapMode::Clamp, WrapMode::Repeat] {
                let reference = match wrap {
                    WrapMode::Clamp => value.clamp(0.0, 1.0 - f64::EPSILON),
                    WrapMode::Repeat => value.rem_euclid(1.0),
                } * f64::from(size)
                    - 0.5;
                let actual = addressed(value, wrap) * f64::from(size) - 0.5;
                for offset in [0.0, 1.0] {
                    let raw = (reference.floor() + offset) as i64;
                    let expected = match wrap {
                        WrapMode::Clamp => raw.clamp(0, i64::from(size) - 1),
                        WrapMode::Repeat => raw.rem_euclid(i64::from(size)),
                    } as usize;
                    assert_eq!(
                        texel_index((actual.floor() + offset) as i64, size, wrap),
                        expected,
                        "value={value:?} size={size} wrap={wrap:?} offset={offset}",
                    );
                }
                let expected_fraction = reference - reference.floor();
                let actual_fraction = actual - actual.floor();
                assert!(
                    expected_fraction.to_bits() == actual_fraction.to_bits()
                        || expected_fraction.is_nan() && actual_fraction.is_nan(),
                    "filter weights changed for value={value:?} size={size} wrap={wrap:?}",
                );
            }
        }
    }
}

#[test]
fn linear_filter_preserves_texel_centers_across_image_sizes() {
    for size in 1..=64 {
        let pixels: Vec<u32> = (0..size)
            .map(|index| 0xff00_0000 | (index * 3) << 16 | (index * 2) << 8 | index)
            .collect();
        for horizontal in [true, false] {
            let (width, height) = if horizontal { (size, 1) } else { (1, size) };
            let image = Image2DState::from_argb(ImageFormat::Rgba, width, height, &pixels).unwrap();
            let mut texture = Texture2DState::new(image);
            texture.set_blend_function(BlendFunction::Replace);
            texture.set_linear_filter(true);
            for wrap in [WrapMode::Clamp, WrapMode::Repeat] {
                texture.set_wrapping(wrap, wrap);
                for (index, expected) in pixels.iter().copied().enumerate() {
                    for offset in [-1.0, 0.0, 1.0] {
                        if wrap == WrapMode::Clamp && offset != 0.0 {
                            continue;
                        }
                        let coordinate = (index as f64 + 0.5) / f64::from(size) + offset;
                        let (s, t) = if horizontal {
                            (coordinate, 0.5)
                        } else {
                            (0.5, coordinate)
                        };
                        assert_eq!(
                            texture.shade(s, t, 0),
                            expected,
                            "size={size}, horizontal={horizontal}, wrap={wrap:?}, index={index}, offset={offset}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn linear_filter_wraps_or_clamps_neighbors_at_texture_edges() {
    let image = Image2DState::from_argb(
        ImageFormat::Rgba,
        3,
        1,
        &[0xffff_0000, 0xff00_ff00, 0xff00_00ff],
    )
    .unwrap();
    let mut texture = Texture2DState::new(image);
    texture.set_blend_function(BlendFunction::Replace);
    texture.set_linear_filter(true);
    for coordinate in [-1.0, 0.0, 1.0, 2.0] {
        assert_eq!(texture.shade(coordinate, 0.5, 0), 0xff80_0080);
    }
    texture.set_wrapping(WrapMode::Clamp, WrapMode::Clamp);
    for (coordinate, expected) in [
        (-1.0, 0xffff_0000),
        (0.0, 0xffff_0000),
        (1.0, 0xff00_00ff),
        (2.0, 0xff00_00ff),
    ] {
        assert_eq!(texture.shade(coordinate, 0.5, 0), expected);
    }
}

#[test]
fn nearest_repeat_and_linear_clamp_have_stable_samples() {
    let image = Image2DState::from_bytes(
        ImageFormat::Rgb,
        2,
        2,
        &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
    )
    .unwrap();
    let mut texture = Texture2DState::new(image);
    assert_eq!(texture.mipmaps.len(), 1);
    texture.set_blend_function(BlendFunction::Replace);
    assert_eq!(texture.shade(1.25, 0.25, u32::MAX), 0xffff_0000);
    texture.set_wrapping(WrapMode::Clamp, WrapMode::Clamp);
    texture.set_linear_filter(true);
    assert_eq!(texture.shade(0.5, 0.5, u32::MAX), 0xff80_8080);
    texture.set_linear_filter(false);
    texture.set_mipmap_filter(true, false);
    assert_eq!(texture.mipmaps.len(), 2);
    assert_eq!(
        texture.shade_coordinates_lod([0.1, 0.1, 0.0], u32::MAX, 1.0),
        0xff80_8080
    );
}

#[test]
fn mip_filter_selects_and_interpolates_levels_at_lod_boundaries() {
    let image = Image2DState::from_argb(
        ImageFormat::Rgba,
        2,
        2,
        &[0xff00_0000, 0xffff_0000, 0xff00_ff00, 0xff00_00ff],
    )
    .unwrap();
    let mut texture = Texture2DState::new(image);
    texture.set_blend_function(BlendFunction::Replace);
    for keyed in [false, true] {
        texture.set_color_key(keyed.then_some(0));
        for linear_levels in [false, true] {
            texture.set_mipmap_filter(true, linear_levels);
            let base = if keyed { 0 } else { 0xff00_0000 };
            let last = if keyed { 0xbf40_4040 } else { 0xff40_4040 };
            for lod in [f64::NEG_INFINITY, -1.0, -0.0, 0.0, f64::NAN] {
                assert_eq!(texture.shade_coordinates_lod([0.25; 3], 0, lod), base);
            }
            for lod in [1.0, 2.0, f64::INFINITY] {
                assert_eq!(texture.shade_coordinates_lod([0.25; 3], 0, lod), last);
            }
            for (lod, nearest, mixed) in [
                (0.25, base, if keyed { 0x3010_1010 } else { 0xff10_1010 }),
                (0.5, last, if keyed { 0x6020_2020 } else { 0xff20_2020 }),
                (0.75, last, if keyed { 0x8f30_3030 } else { 0xff30_3030 }),
            ] {
                assert_eq!(
                    texture.shade_coordinates_lod([0.25; 3], 0, lod),
                    if linear_levels { mixed } else { nearest },
                    "keyed={keyed} linear_levels={linear_levels} lod={lod}",
                );
            }
        }
    }
}

#[test]
fn texture_projection_is_invariant_under_nonzero_homogeneous_rescaling() {
    let image = Image2DState::from_argb(
        ImageFormat::Rgba,
        2,
        2,
        &[0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff],
    )
    .unwrap();
    let mut texture = Texture2DState::new(image);
    texture.set_blend_function(BlendFunction::Replace);
    for scale in [1.0, 1.0e-8, -1.0e-8, 1.0e-18, 1.0e18] {
        texture.set_transform(
            Mat4::from_array(Mat4::IDENTITY.as_array().map(|value| value * scale)).unwrap(),
        );
        assert_eq!(texture.shade(0.75, 0.75, 0), 0xffff_ffff, "{scale}");
    }
}

#[test]
fn texture_blending_preserves_channels_absent_from_the_image_format() {
    for (format, bytes, expected) in [
        (
            ImageFormat::Alpha,
            vec![64],
            [0x4040_2010, 0x2040_2010, 0x2040_2010, 0x2040_2010],
        ),
        (
            ImageFormat::Luminance,
            vec![128],
            [0x8080_8080, 0x8020_1008, 0x8030_3038, 0x80c0_a090],
        ),
        (
            ImageFormat::LuminanceAlpha,
            vec![128, 64],
            [0x4080_8080, 0x2020_1008, 0x2030_3038, 0x20c0_a090],
        ),
        (
            ImageFormat::Rgb,
            vec![128; 3],
            [0x8080_8080, 0x8020_1008, 0x8030_3038, 0x80c0_a090],
        ),
        (
            ImageFormat::Rgba,
            vec![128, 128, 128, 64],
            [0x4080_8080, 0x2020_1008, 0x2030_3038, 0x20c0_a090],
        ),
    ] {
        let mut texture =
            Texture2DState::new(Image2DState::from_bytes(format, 1, 1, &bytes).unwrap());
        texture.set_blend_color(0x0020_4060);
        for (blend, expected) in [
            BlendFunction::Replace,
            BlendFunction::Modulate,
            BlendFunction::Blend,
            BlendFunction::Add,
        ]
        .into_iter()
        .zip(expected)
        {
            texture.set_blend_function(blend);
            assert_eq!(
                texture.shade(0.5, 0.5, 0x8040_2010),
                expected,
                "{format:?} {blend:?}"
            );
        }
        if matches!(format, ImageFormat::Rgb | ImageFormat::Rgba) {
            texture.set_blend_function(BlendFunction::Decal);
            assert_eq!(
                texture.shade(0.5, 0.5, 0x8040_2010),
                if format == ImageFormat::Rgb {
                    0x8080_8080
                } else {
                    0x8050_382c
                }
            );
        }
    }
}

#[test]
fn identity_sampling_matches_the_matrix_path_and_tracks_transform_changes() {
    let image = Image2DState::from_argb(
        ImageFormat::Rgba,
        2,
        2,
        &[0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff],
    )
    .unwrap();
    let mut texture = Texture2DState::new(image);
    texture.set_blend_function(BlendFunction::Replace);
    for wrap in [WrapMode::Clamp, WrapMode::Repeat] {
        texture.set_wrapping(wrap, wrap);
        let mut reference = texture.clone();
        reference.identity_transform = false;
        for s in [
            -1.0,
            -0.0,
            0.0,
            0.25,
            0.5 - f64::EPSILON,
            0.5,
            1.0,
            2.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            for z in [0.0, 1.0, f64::MAX, f64::NAN] {
                assert_eq!(
                    texture.shade_coordinates([s, 0.25, z], 0),
                    reference.shade_coordinates([s, 0.25, z], 0)
                );
            }
        }
    }
    texture.set_wrapping(WrapMode::Clamp, WrapMode::Clamp);
    texture.set_transform(Mat4::translation(0.5, 0.0, 0.0).unwrap());
    assert_eq!(texture.shade(0.0, 0.0, 0), 0xff00_ff00);
    texture.set_transform(Mat4::IDENTITY);
    assert_eq!(texture.shade(0.0, 0.0, 0), 0xffff_0000);
}

#[test]
fn color_key_sampling_reuses_source_pixels_and_keys_before_filtering() {
    let shared: Arc<[u32]> = vec![0xffff_0000, 0xff00_ff00].into();
    let image = Image2DState::from_shared_argb(2, 1, Arc::clone(&shared)).unwrap();
    let mut texture = Texture2DState::new(image);
    texture.set_blend_function(BlendFunction::Replace);
    texture.set_wrapping(WrapMode::Clamp, WrapMode::Clamp);
    texture.set_color_key(Some(0x12ff_0000));

    assert!(Arc::ptr_eq(&texture.mipmaps[0].pixels, &shared));
    assert_eq!(texture.shade(0.25, 0.5, 0), 0);
    assert_eq!(texture.shade(0.75, 0.5, 0), 0xff00_ff00);

    texture.set_linear_filter(true);
    assert_eq!(texture.shade(0.5, 0.5, 0), 0x8000_8000);
}
