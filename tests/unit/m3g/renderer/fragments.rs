use super::*;
use crate::renderer::fragments::blend_frame;

#[test]
fn minified_triangles_keep_independent_image_filters_and_mip_selection() {
    let pixels: Vec<_> = (0..64)
        .map(|index| {
            if (index / 8 + index % 8) % 2 == 0 {
                0xffff_0000
            } else {
                0xff00_ff00
            }
        })
        .collect();
    let image = crate::Image2DState::from_argb(crate::ImageFormat::Rgba, 8, 8, &pixels).unwrap();
    let draw = |min_linear, mag_linear, mipmaps| {
        let mut texture = crate::Texture2DState::new(image.clone());
        texture.set_image_filters(min_linear, mag_linear);
        texture.set_mipmap_filter(mipmaps, false);
        let mut renderer = renderer();
        renderer.set_texture(Some(texture));
        renderer
            .draw_triangle([
                Vertex::textured(Vec4::new(-0.8, -0.8, 0.0, 1.0), u32::MAX, 0.0, 0.0),
                Vertex::textured(Vec4::new(0.8, -0.8, 0.0, 1.0), u32::MAX, 7.3, 0.0),
                Vertex::textured(Vec4::new(0.0, 0.8, 0.0, 1.0), u32::MAX, 3.65, 7.3),
            ])
            .unwrap();
        renderer.pixels().to_vec()
    };
    let nearest = draw(false, false, false);
    let linear = draw(true, true, false);
    assert_ne!(nearest, linear);
    assert_eq!(draw(false, true, false), nearest);
    assert_eq!(draw(true, false, false), linear);
    let mipmapped = draw(false, false, true);
    assert_ne!(mipmapped, nearest);
    assert!(mipmapped.contains(&0xff80_8000));
    assert!(
        mipmapped
            .iter()
            .all(|pixel| matches!(*pixel, 0xff00_0000 | 0xff80_8000))
    );
}

#[test]
fn alpha_blending_preserves_integer_channel_equations_at_alpha_boundaries() {
    for mode in [FrameBlend::Alpha, FrameBlend::SourceOver] {
        for alpha in [0, 1, 127, 128, 254, 255] {
            for source_component in 0..=255 {
                for destination_component in 0..=255 {
                    let source = alpha << 24 | (source_component * 0x0001_0101);
                    let destination = destination_component * 0x0101_0101;
                    let color =
                        (source_component * alpha + destination_component * (255 - alpha) + 127)
                            / 255;
                    let output_alpha = if mode == FrameBlend::SourceOver {
                        alpha + (destination_component * (255 - alpha) + 127) / 255
                    } else {
                        (alpha * alpha + destination_component * (255 - alpha) + 127) / 255
                    };
                    assert_eq!(
                        blend_frame(mode, source, destination),
                        output_alpha << 24 | (color * 0x0001_0101)
                    );
                }
            }
        }
    }
}

#[test]
fn depth_and_source_alpha_blending_are_observable() {
    let mut renderer = renderer();
    let triangle = |z, color| {
        [
            vertex(-0.8, -0.8, z, color),
            vertex(0.8, -0.8, z, color),
            vertex(0.0, 0.8, z, color),
        ]
    };
    renderer.draw_triangle(triangle(0.5, 0xffff_0000)).unwrap();
    renderer.draw_triangle(triangle(-0.5, 0xff00_ff00)).unwrap();
    let center = renderer.pixels()[16 * 32 + 16];
    assert_eq!(center, 0xff00_ff00);
    renderer.draw_triangle(triangle(0.75, 0xff00_00ff)).unwrap();
    assert_eq!(renderer.pixels()[16 * 32 + 16], center);
    assert!(renderer.stats().depth_rejected_fragments > 0);

    renderer.set_compositing(false, false, true, true);
    renderer.draw_triangle(triangle(0.0, 0x80ff_0000)).unwrap();
    assert_eq!(renderer.pixels()[16 * 32 + 16], 0xbf80_7f00);
    assert!(renderer.stats().blended_fragments > 0);
}

#[test]
fn flat_fragments_match_white_texturing_across_clip_scales_and_depth_modes() {
    let white = crate::Texture2DState::new(
        crate::Image2DState::from_argb(crate::ImageFormat::Rgba, 1, 1, &[u32::MAX]).unwrap(),
    );
    for scale in [f32::MIN_POSITIVE, 1.0e-20, 1.0, 1.0e20] {
        for clipped in [false, true] {
            for source in [0x0042_9bd7, 0x8042_9bd7, 0xff42_9bd7] {
                for (depth_test, depth_write) in
                    [(false, false), (false, true), (true, false), (true, true)]
                {
                    let triangle = [
                        [-1.0, -1.0, -0.5, 1.0],
                        [2.0, -2.0, 1.0, 2.0],
                        [0.0, if clipped { 8.0 } else { 4.0 }, 0.0, 4.0],
                    ]
                    .map(|position| {
                        let [x, y, z, w] = position.map(|value| value * scale);
                        Vertex::new(Vec4::new(x, y, z, w), source)
                    });
                    let outputs = [false, true].map(|textured| {
                        let mut renderer =
                            SoftwareRenderer::new(16, 12, RenderLimits::default()).unwrap();
                        renderer.clear(Some(0x4056_3412), true);
                        renderer
                            .set_compositing_mode(
                                depth_test,
                                depth_write,
                                true,
                                true,
                                FrameBlend::SourceOver,
                                0.5,
                                2.0,
                            )
                            .unwrap();
                        if textured {
                            renderer.set_texture(Some(white.clone()));
                        }
                        renderer.draw_triangle(triangle).unwrap();
                        (
                            renderer.pixels().to_vec(),
                            renderer.depth().to_vec(),
                            renderer.stats(),
                        )
                    });
                    assert_eq!(
                        outputs[0], outputs[1],
                        "scale={scale} clipped={clipped} source={source:08x} test={depth_test} write={depth_write}"
                    );
                    assert!(outputs[0].2.shaded_fragments > 0);
                }
            }
        }
    }
}

#[test]
fn primitive_depth_testing_and_writing_remain_independent() {
    for primitive in 0..3 {
        let draw = |renderer: &mut SoftwareRenderer, depth, color| match primitive {
            0 => renderer.draw_triangle([
                vertex(-0.8, -0.8, depth, color),
                vertex(0.8, -0.8, depth, color),
                vertex(0.0, 0.8, depth, color),
            ]),
            1 => renderer.draw_line([
                vertex(-0.8, 0.0, depth, color),
                vertex(0.8, 0.0, depth, color),
            ]),
            _ => renderer.draw_point(vertex(0.0, 0.0, depth, color)),
        };
        for (depth_test, depth_write) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let mut renderer = renderer();
            draw(&mut renderer, -0.5, 0xffff_0000).unwrap();
            let previous_depth = renderer.depth().to_vec();
            renderer.reset_stats();
            renderer
                .set_compositing_mode(
                    depth_test,
                    depth_write,
                    true,
                    true,
                    FrameBlend::Replace,
                    0.0,
                    0.0,
                )
                .unwrap();
            draw(&mut renderer, 0.5, 0xff00_00ff).unwrap();
            let center = 16 * 32 + 16;
            assert_eq!(
                renderer.pixels()[center],
                if depth_test { 0xffff_0000 } else { 0xff00_00ff }
            );
            assert_eq!(renderer.stats().depth_rejected_fragments > 0, depth_test);
            if depth_write && !depth_test {
                assert!(renderer.depth()[center] > previous_depth[center]);
            } else {
                assert_eq!(renderer.depth(), previous_depth);
            }
        }
    }
}

#[test]
fn transparent_fragments_obey_depth_write_and_alpha_test_independently_of_color_masks() {
    for mode in [FrameBlend::Alpha, FrameBlend::AlphaAdd] {
        for depth_test in [false, true] {
            for depth_write in [false, true] {
                for threshold in [0, 1] {
                    for (color_write, alpha_write) in
                        [(false, false), (true, false), (false, true), (true, true)]
                    {
                        let mut renderer = renderer();
                        renderer
                            .set_compositing_mode(
                                depth_test,
                                depth_write,
                                color_write,
                                alpha_write,
                                mode,
                                0.0,
                                0.0,
                            )
                            .unwrap();
                        renderer.set_alpha_threshold(threshold);
                        renderer
                            .draw_point(vertex(0.0, 0.0, -0.5, 0x00ff_ffff))
                            .unwrap();
                        assert_eq!(renderer.pixels()[16 * 32 + 16], 0xff00_0000);

                        renderer.set_alpha_threshold(0);
                        renderer.set_compositing(true, true, true, false);
                        renderer
                            .draw_point(vertex(0.0, 0.0, 0.5, 0xffff_0000))
                            .unwrap();
                        let occluded = depth_write && threshold == 0;
                        assert_eq!(
                            renderer.pixels()[16 * 32 + 16],
                            if occluded { 0xff00_0000 } else { 0xffff_0000 },
                            "{mode:?} test={depth_test} write={depth_write} threshold={threshold} color={color_write} alpha={alpha_write}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn m3g_alpha_blending_uses_the_same_factors_for_all_four_channels() {
    assert_eq!(
        blend_frame(FrameBlend::Alpha, 0x8080_8080, 0x4040_4040),
        0x6060_6060
    );
    assert_eq!(
        blend_frame(FrameBlend::AlphaAdd, 0x8080_8080, 0x4040_4040),
        0x8080_8080
    );
    assert_eq!(
        blend_frame(FrameBlend::SourceOver, 0x8080_8080, 0x4040_4040),
        0xa060_6060
    );
}

#[test]
fn all_frame_blends_channel_masks_and_flat_shading_are_observable() {
    assert_eq!(
        blend_frame(FrameBlend::Replace, 0x8040_80ff, 0xff20_4020),
        0x8040_80ff
    );
    assert_eq!(
        blend_frame(FrameBlend::Modulate, 0xffff_0000, 0xff80_8080),
        0xff80_0000
    );
    assert_eq!(
        blend_frame(FrameBlend::ModulateX2, 0xff80_8000, 0xff80_8080),
        0xff81_8100
    );
    assert_eq!(
        blend_frame(FrameBlend::AlphaAdd, 0x80ff_0000, 0xff00_1000),
        0xff80_1000
    );
    assert_eq!(
        blend_frame(FrameBlend::Add, 0xff20_3040, 0xfff0_e0d0),
        0xffff_ffff
    );
    assert_eq!(
        blend_frame(FrameBlend::Half, 0xff20_3040, 0xff40_6080),
        0xff30_4860
    );
    assert_eq!(
        blend_frame(FrameBlend::Subtract, 0xff20_3040, 0xff50_6070),
        0xff30_3030
    );

    let mut renderer = renderer();
    renderer.clear(Some(0x7f11_2233), true);
    renderer
        .set_compositing_mode(false, false, true, false, FrameBlend::Replace, 0.0, 0.0)
        .unwrap();
    renderer.set_smooth_shading(false);
    renderer
        .draw_triangle([
            vertex(-0.8, -0.8, 0.0, 0xffff_0000),
            vertex(0.8, -0.8, 0.0, 0xff00_ff00),
            vertex(0.0, 0.8, 0.0, 0xff00_00ff),
        ])
        .unwrap();
    assert_eq!(renderer.pixels()[16 * 32 + 16], 0x7f00_00ff);
}

#[test]
fn texture_and_fog_are_applied_in_fragment_order() {
    let image =
        crate::Image2DState::from_bytes(crate::ImageFormat::Rgb, 1, 1, &[255, 0, 0]).unwrap();
    let mut texture = crate::Texture2DState::new(image);
    texture.set_blend_function(crate::BlendFunction::Replace);
    let mut renderer = renderer();
    renderer.set_texture(Some(texture));
    renderer.set_fog(Some(crate::FogState {
        color: 0x0000_00ff,
        mode: crate::FogMode::Linear,
        near: 0.0,
        far: 1.0,
        density: 1.0,
    }));
    renderer
        .draw_triangle(
            [
                Vertex::textured(Vec4::new(-0.8, -0.8, -0.5, 1.0), 0xffff_ffff, 0.0, 0.0),
                Vertex::textured(Vec4::new(0.8, -0.8, -0.5, 1.0), 0xffff_ffff, 1.0, 0.0),
                Vertex::textured(Vec4::new(0.0, 0.8, -0.5, 1.0), 0xffff_ffff, 0.5, 1.0),
            ]
            .map(|mut vertex| {
                vertex.project(Mat4::IDENTITY);
                vertex
            }),
        )
        .unwrap();
    assert_eq!(renderer.pixels()[16 * 32 + 16], 0xff80_0080);
}

#[test]
fn fog_distance_survives_clipping_and_is_independent_of_depth_mapping() {
    let triangle = [
        vertex(-2.0, -0.8, 0.0, 0xffff_0000),
        vertex(0.8, -0.8, 0.0, 0xffff_0000),
        vertex(0.0, 0.8, 0.0, 0xffff_0000),
    ]
    .map(|mut vertex| {
        vertex.eye_z = -4.0;
        vertex
    });
    for (near, far, offset) in [(0.0, 1.0, 0.0), (0.0, 0.25, 0.0), (1.0, 0.5, 1024.0)] {
        for primitive in 0..3 {
            let mut renderer = renderer();
            renderer.set_fog(Some(crate::FogState {
                color: 0x0000_00ff,
                mode: crate::FogMode::Linear,
                near: 2.0,
                far: 6.0,
                density: 1.0,
            }));
            renderer.set_depth_range(near, far).unwrap();
            renderer
                .set_compositing_mode(false, true, true, true, FrameBlend::Replace, 1.0, offset)
                .unwrap();
            match primitive {
                0 => renderer.draw_triangle(triangle).unwrap(),
                1 => renderer.draw_line([triangle[0], triangle[1]]).unwrap(),
                _ => renderer.draw_point(triangle[2]).unwrap(),
            }
            let pixels: Vec<_> = renderer
                .pixels()
                .iter()
                .copied()
                .filter(|pixel| *pixel != 0xff00_0000)
                .collect();
            assert!(!pixels.is_empty());
            assert!(
                pixels.iter().all(|pixel| *pixel == 0xff80_0080),
                "{near}, {far}, {offset}, {primitive}"
            );
        }
    }
}

#[test]
fn fog_distance_is_perspective_correct_at_fragment_centers() {
    // At this one-pixel viewport's center the barycentric weights are 1/4,
    // 1/4, 1/2. The projected Z values are all zero, while eye distances
    // 2, 4, 8 interpolate with clip W 1, 2, 4 to distance 4.
    let mut triangle = [
        Vertex::new(Vec4::new(-1.0, -1.0, 0.0, 1.0), 0xffff_0000),
        Vertex::new(Vec4::new(2.0, -2.0, 0.0, 2.0), 0xffff_0000),
        Vertex::new(Vec4::new(0.0, 4.0, 0.0, 4.0), 0xffff_0000),
    ];
    for (vertex, distance) in triangle.iter_mut().zip([2.0, 4.0, 8.0]) {
        vertex.eye_z = -distance;
    }
    let mut renderer = SoftwareRenderer::new(1, 1, RenderLimits::default()).unwrap();
    renderer.set_fog(Some(crate::FogState {
        color: 0x0000_00ff,
        mode: crate::FogMode::Linear,
        near: 2.0,
        far: 6.0,
        density: 1.0,
    }));
    renderer.draw_triangle(triangle).unwrap();
    assert_eq!(renderer.pixels(), &[0xff80_0080]);
}

#[test]
fn fragment_alpha_override_runs_after_alpha_testing() {
    let image =
        crate::Image2DState::from_argb(crate::ImageFormat::Rgba, 1, 1, &[0x00ff_0000]).unwrap();
    let mut texture = crate::Texture2DState::new(image);
    texture.set_blend_function(crate::BlendFunction::Replace);
    let triangle = [
        Vertex::textured(Vec4::new(-0.8, -0.8, 0.0, 1.0), 0xffff_ffff, 0.0, 0.0),
        Vertex::textured(Vec4::new(0.8, -0.8, 0.0, 1.0), 0xffff_ffff, 1.0, 0.0),
        Vertex::textured(Vec4::new(0.0, 0.8, 0.0, 1.0), 0xffff_ffff, 0.5, 1.0),
    ];
    let mut renderer = renderer();
    renderer.set_texture(Some(texture));
    renderer.set_fragment_alpha_override(Some(u8::MAX));
    renderer.clear(Some(0xff00_0000), true);
    renderer.draw_triangle(triangle).unwrap();
    assert_eq!(renderer.pixels()[16 * 32 + 16], 0xffff_0000);

    renderer.clear(Some(0xff00_0000), true);
    renderer.set_alpha_threshold(1);
    renderer.draw_triangle(triangle).unwrap();
    assert_eq!(renderer.pixels()[16 * 32 + 16], 0xff00_0000);
}

#[test]
fn points_lines_and_triangles_share_fragment_compositing() {
    for mode in [
        FrameBlend::Add,
        FrameBlend::Alpha,
        FrameBlend::AlphaAdd,
        FrameBlend::Half,
        FrameBlend::Modulate,
        FrameBlend::ModulateX2,
        FrameBlend::Replace,
        FrameBlend::SourceOver,
        FrameBlend::Subtract,
    ] {
        for (color_write, alpha_write) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            for (source, threshold, alpha_override) in [
                (0xff40_80c0, 0, None),
                (0x8040_80c0, 0, None),
                (0x0040_80c0, 0, None),
                (0x8040_80c0, 129, Some(255)),
                (0x0040_80c0, 0, Some(255)),
            ] {
                for fog in [
                    None,
                    Some(crate::FogState {
                        color: 0x00c0_8040,
                        mode: crate::FogMode::Linear,
                        near: 0.0,
                        far: 1.0,
                        density: 1.0,
                    }),
                ] {
                    let outputs = [0, 1, 2].map(|primitive| {
                        let mut renderer =
                            SoftwareRenderer::new(4, 4, RenderLimits::default()).unwrap();
                        renderer.clear(Some(0x4020_4060), true);
                        renderer
                            .set_compositing_mode(
                                true,
                                true,
                                color_write,
                                alpha_write,
                                mode,
                                0.0,
                                0.0,
                            )
                            .unwrap();
                        renderer.set_alpha_threshold(threshold);
                        renderer.set_fragment_alpha_override(alpha_override);
                        renderer.set_fog(fog);
                        match primitive {
                            0 => renderer.draw_triangle([
                                vertex(-1.0, -1.0, -0.5, source),
                                vertex(1.0, -1.0, -0.5, source),
                                vertex(0.0, 1.0, -0.5, source),
                            ]),
                            1 => renderer.draw_line([
                                vertex(-0.75, 0.0, -0.5, source),
                                vertex(0.75, 0.0, -0.5, source),
                            ]),
                            _ => renderer.draw_point(vertex(0.0, 0.0, -0.5, source)),
                        }
                        .unwrap();
                        (renderer.pixels()[10], renderer.depth()[10])
                    });
                    assert_eq!(
                        outputs, [outputs[0]; 3],
                        "{mode:?} color={color_write} alpha={alpha_write} source={source:08x} threshold={threshold} override={alpha_override:?} fog={fog:?}"
                    );
                }
            }
        }
    }
}
