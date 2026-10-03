use super::*;
use crc32fast::Hasher;

mod background;
mod checkpoint;
mod fragments;
mod throughput;

fn renderer() -> SoftwareRenderer {
    let mut renderer = SoftwareRenderer::new(32, 32, RenderLimits::default()).unwrap();
    renderer.clear(Some(0xff00_0000), true);
    renderer
}

#[test]
fn replacing_pixels_keeps_depth_and_state_and_rejects_wrong_lengths_atomically() {
    let mut renderer = renderer();
    renderer.depth.fill(123);
    renderer.set_scissor(1, 2, 3, 4).unwrap();
    let before = renderer.pixels().to_vec();
    for length in [1023, 1025] {
        assert_eq!(
            renderer.replace_pixels(vec![7; length]).unwrap_err().code(),
            "invalid-target"
        );
        assert_eq!(renderer.pixels(), before);
    }
    let expected: Vec<_> = (0..1024_u32)
        .map(|pixel| pixel.wrapping_mul(0x1234_5679))
        .collect();
    let source = expected.clone();
    let storage = source.as_ptr();
    renderer.replace_pixels(source).unwrap();
    assert_eq!(renderer.pixels().as_ptr(), storage);
    assert_eq!(renderer.pixels(), expected);
    assert_eq!(renderer.depth(), [123; 1024]);
    assert_eq!(renderer.scissor, [1, 2, 3, 4]);
    assert_eq!(renderer.stats(), RenderStats::default());
}

#[test]
fn adopted_pixels_preserve_storage_and_default_rendering_state() {
    let pixels: Vec<_> = (0..32).map(|index| 0x8040_2010 + index).collect();
    let storage = pixels.as_ptr();
    let mut copied = SoftwareRenderer::new(8, 4, RenderLimits::default()).unwrap();
    copied.load_pixels(&pixels).unwrap();
    let mut adopted = SoftwareRenderer::from_pixels(8, 4, pixels, RenderLimits::default()).unwrap();
    assert_eq!(adopted.pixels().as_ptr(), storage);
    assert_eq!(adopted.pixels(), copied.pixels());
    assert_eq!(adopted.depth(), copied.depth());
    for renderer in [&mut adopted, &mut copied] {
        renderer
            .draw_triangle([
                vertex(-0.8, -0.8, -0.5, 0xff20_40e0),
                vertex(0.8, -0.8, 0.5, 0xff20_40e0),
                vertex(0.0, 0.8, 0.0, 0xff20_40e0),
            ])
            .unwrap();
    }
    assert!(adopted.pixels().contains(&0xff20_40e0));
    assert_eq!(adopted.pixels(), copied.pixels());
    assert_eq!(adopted.depth(), copied.depth());
    assert_eq!(adopted.stats(), copied.stats());
}

#[test]
fn adopted_pixels_reject_invalid_dimensions_and_lengths_before_depth_allocation() {
    for (width, height, length, code) in [
        (0, 1, 0, "invalid-target"),
        (1, 0, 0, "invalid-target"),
        (u32::MAX, u32::MAX, 0, "resource-limit"),
        (2049, 2049, 0, "resource-limit"),
        (2048, 2048, 0, "invalid-target"),
        (2, 2, 3, "invalid-target"),
        (2, 2, 5, "invalid-target"),
    ] {
        assert_eq!(
            SoftwareRenderer::from_pixels(width, height, vec![0; length], RenderLimits::default())
                .unwrap_err()
                .code(),
            code,
        );
    }
}

#[test]
fn clearing_an_offscreen_viewport_preserves_both_attachments() {
    for (x, y) in [
        (33, 0),
        (33, 31),
        (i32::MAX, 0),
        (i32::MIN, 0),
        (0, i32::MAX),
        (0, i32::MIN),
        (-8, 0),
        (0, -8),
    ] {
        let mut renderer = renderer();
        renderer.depth.fill(7);
        renderer.set_viewport(x, y, 8, 8).unwrap();
        renderer.clear(Some(0xffff_0000), true);
        assert!(
            renderer.pixels().iter().all(|pixel| *pixel == 0xff00_0000),
            "{x},{y}"
        );
        assert!(renderer.depth().iter().all(|depth| *depth == 7), "{x},{y}");
    }
}

fn textured_raster_fixture(smooth: bool) -> (SoftwareRenderer, [Vertex; 3]) {
    let pixels: Vec<u32> = (0..64 * 64)
        .map(|index| 0xff00_0000 | ((index * 67) & 0x00ff_ffff))
        .collect();
    let image = crate::Image2DState::from_argb(crate::ImageFormat::Rgba, 64, 64, &pixels).unwrap();
    let mut texture = Texture2DState::new(image);
    texture.set_wrapping(crate::WrapMode::Clamp, crate::WrapMode::Clamp);
    let mut renderer = SoftwareRenderer::new(240, 320, RenderLimits::default()).unwrap();
    renderer.set_texture(Some(texture));
    let colors = if smooth {
        [0xffcc_8844, 0xff33_99ee, 0xff77_dd99]
    } else {
        [0xffcc_8844; 3]
    };
    let triangle = [
        Vertex::textured(Vec4::new(-1.0, -1.0, 0.2, 1.0), colors[0], -0.1, 0.0),
        Vertex::textured(Vec4::new(2.0, -2.0, 0.8, 2.0), colors[1], 1.1, 0.0),
        Vertex::textured(Vec4::new(0.0, 0.7, 0.0, 0.5), colors[2], 0.5, 1.0),
    ];
    (renderer, triangle)
}

fn raster_crc(renderer: &SoftwareRenderer) -> u32 {
    let mut hash = Hasher::new();
    for pixel in renderer.pixels() {
        hash.update(&pixel.to_le_bytes());
    }
    hash.finalize()
}

fn mipmapped_raster_fixture(linear: bool, linear_levels: bool) -> (SoftwareRenderer, [Vertex; 3]) {
    let (mut renderer, mut triangle) = textured_raster_fixture(true);
    let texture = renderer.textures[0].as_mut().unwrap();
    texture.set_wrapping(crate::WrapMode::Repeat, crate::WrapMode::Clamp);
    texture.set_image_filters(linear, !linear);
    texture.set_mipmap_filter(true, linear_levels);
    for vertex in &mut triangle {
        for coordinate in &mut vertex.texture[0][..2] {
            *coordinate = *coordinate * 8.3 - 4.2;
        }
    }
    (renderer, triangle)
}

#[test]
fn mipmapped_triangles_preserve_filtering_and_perspective_interpolation() {
    for (linear, linear_levels, expected) in [
        (false, false, 0x39a2_a78c),
        (false, true, 0x220a_4759),
        (true, false, 0x7b24_9737),
        (true, true, 0xd723_ea4c),
    ] {
        let (mut renderer, triangle) = mipmapped_raster_fixture(linear, linear_levels);
        renderer.clear(Some(0xff00_0000), true);
        renderer.draw_triangle(triangle).unwrap();
        assert_eq!(raster_crc(&renderer), expected);
    }
}

fn scaled_clip_vertex(mut vertex: Vertex, scale: f32) -> Vertex {
    let position = vertex.position;
    vertex.position = Vec4::new(
        position.x * scale,
        position.y * scale,
        position.z * scale,
        position.w * scale,
    );
    vertex
}

fn assert_homogeneous_scale_invariant(draw: impl Fn(f32) -> SoftwareRenderer) {
    let reference = draw(1.0);
    assert!(reference.stats().shaded_fragments > 0);
    assert!(reference.pixels().iter().any(|pixel| *pixel != 0xff00_0000));
    // Powers of two preserve every mantissa bit while changing clip-space units.
    for scale in [2.0_f32.powi(-60), 2.0_f32.powi(60)] {
        let scaled = draw(scale);
        assert!(
            scaled.pixels() == reference.pixels(),
            "color at scale {scale}"
        );
        assert!(
            scaled.depth() == reference.depth(),
            "depth at scale {scale}"
        );
    }
}

#[test]
fn point_rasterization_is_invariant_to_homogeneous_scale() {
    assert_homogeneous_scale_invariant(|scale| {
        let mut renderer = renderer();
        renderer
            .draw_point(scaled_clip_vertex(
                vertex(0.25, -0.5, 0.25, 0xff33_aacc),
                scale,
            ))
            .unwrap();
        renderer
    });
}

#[test]
fn clipped_line_rasterization_is_invariant_to_homogeneous_scale() {
    assert_homogeneous_scale_invariant(|scale| {
        let mut renderer = renderer();
        renderer
            .draw_line(
                [
                    vertex(-2.0, -0.5, 0.25, 0xffcc_7733),
                    Vertex::new(Vec4::new(0.75, 0.75, -0.25, 0.5), 0xff33_77cc),
                ]
                .map(|vertex| scaled_clip_vertex(vertex, scale)),
            )
            .unwrap();
        renderer
    });
}

#[test]
fn a_line_blends_each_covered_pixel_only_once() {
    for (start, end) in [
        ((0.0, 0.02), (0.02, 0.02)),
        ((0.0, 0.0), (0.0, 0.0)),
        ((-0.8, -0.2), (0.77, 0.33)),
        ((0.77, 0.33), (-0.8, -0.2)),
    ] {
        let mut renderer = renderer();
        renderer.set_compositing(false, false, true, true);
        renderer
            .draw_line([
                vertex(start.0, start.1, 0.0, 0x80ff_0000),
                vertex(end.0, end.1, 0.0, 0x80ff_0000),
            ])
            .unwrap();
        let covered = renderer
            .pixels()
            .iter()
            .filter(|pixel| **pixel != 0xff00_0000)
            .count();
        assert!(covered > 0);
        assert!(
            renderer
                .pixels()
                .iter()
                .all(|pixel| matches!(*pixel, 0xff00_0000 | 0xbf80_0000)),
            "repeated blending for {start:?} -> {end:?}"
        );
        assert_eq!(renderer.stats().blended_fragments, covered as u64);
    }
}

#[test]
fn constant_line_colors_survive_clipping_and_homogeneous_scaling() {
    for scale in [f32::MIN_POSITIVE, 1.0e-20, 1.0, 1.0e20] {
        for source in [0x0042_9bd7, 0x8042_9bd7, 0xff42_9bd7] {
            for clipped in [false, true] {
                let mut renderer = renderer();
                renderer
                    .draw_line(
                        [
                            vertex(if clipped { -2.0 } else { -0.8 }, -0.4, -0.5, source),
                            vertex(0.8, 0.4, 0.5, source),
                        ]
                        .map(|vertex| scaled_clip_vertex(vertex, scale)),
                    )
                    .unwrap();
                assert!(renderer.pixels().contains(&source));
                assert!(
                    renderer
                        .pixels()
                        .iter()
                        .all(|pixel| *pixel == source || *pixel == 0xff00_0000)
                );
            }
        }
    }
}

#[test]
fn textured_triangle_rasterization_is_invariant_to_homogeneous_scale() {
    for smooth in [false, true] {
        assert_homogeneous_scale_invariant(|scale| {
            let (mut renderer, triangle) = textured_raster_fixture(smooth);
            renderer.clear(Some(0xff00_0000), true);
            renderer
                .draw_triangle(triangle.map(|vertex| scaled_clip_vertex(vertex, scale)))
                .unwrap();
            renderer
        });
    }
}

#[test]
fn zero_homogeneous_vertices_remain_culled() {
    let mut renderer = renderer();
    let vertex = Vertex::new(Vec4::new(0.0, 0.0, 0.0, 0.0), 0xffff_ffff);
    renderer.draw_point(vertex).unwrap();
    renderer.draw_line([vertex; 2]).unwrap();
    renderer.draw_triangle([vertex; 3]).unwrap();
    assert_eq!(renderer.stats().submitted_triangles, 3);
    assert_eq!(renderer.stats().culled_triangles, 3);
    assert_eq!(renderer.stats().tested_fragments, 0);
}

#[test]
fn clipping_large_finite_lines_preserves_color_and_depth() {
    let draw = |scale| {
        let mut renderer = renderer();
        renderer
            .draw_line(
                [
                    vertex(-1.5, -0.5, 0.25, 0xffff_0000),
                    vertex(1.5, 0.5, -0.25, 0xff00_00ff),
                ]
                .map(|vertex| scaled_clip_vertex(vertex, scale)),
            )
            .unwrap();
        renderer
    };
    let reference = draw(1.0);
    assert!(reference.stats().shaded_fragments > 0);
    let large = draw(2.0_f32.powi(127));
    assert_eq!(large.pixels(), reference.pixels());
    assert_eq!(large.depth(), reference.depth());
}

#[test]
fn clipping_large_finite_triangles_preserves_color_and_depth() {
    let draw = |scale| {
        let mut renderer = renderer();
        renderer
            .draw_triangle(
                [
                    vertex(-1.5, -0.5, 0.25, 0xffff_0000),
                    vertex(1.5, -0.5, 0.25, 0xff00_ff00),
                    vertex(0.0, 1.5, -0.25, 0xff00_00ff),
                ]
                .map(|vertex| scaled_clip_vertex(vertex, scale)),
            )
            .unwrap();
        renderer
    };
    let reference = draw(1.0);
    assert!(reference.stats().shaded_fragments > 0);
    let large = draw(2.0_f32.powi(127));
    assert_eq!(large.pixels(), reference.pixels());
    assert_eq!(large.depth(), reference.depth());
}

#[test]
fn textured_clipped_perspective_triangles_preserve_reference_pixels() {
    for (smooth, expected) in [(false, 0xf019_d0ca), (true, 0x8354_7be3)] {
        let (mut renderer, triangle) = textured_raster_fixture(smooth);
        renderer.clear(Some(0xff00_0000), true);
        renderer.draw_triangle(triangle).unwrap();
        assert_eq!(raster_crc(&renderer), expected);
    }
}

#[test]
fn texture_transform_uses_constant_and_interpolated_third_coordinates() {
    for constant in [false, true] {
        let (mut reference, mut triangle) = textured_raster_fixture(true);
        if constant {
            for vertex in &mut triangle {
                vertex.texture[0][0] = 0.25;
            }
        }
        reference.clear(Some(0xff00_0000), true);
        reference.draw_triangle(triangle).unwrap();
        assert!(reference.stats().shaded_fragments > 0);

        let (mut transformed, _) = textured_raster_fixture(true);
        for vertex in &mut triangle {
            vertex.texture[0].swap(0, 2);
        }
        transformed.textures[0].as_mut().unwrap().set_transform(
            Mat4::from_row_major([
                0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ])
            .unwrap(),
        );
        transformed.clear(Some(0xff00_0000), true);
        transformed.draw_triangle(triangle).unwrap();
        assert_eq!(transformed.pixels(), reference.pixels());
    }
}

#[test]
fn background_crop_is_scaled_across_the_full_viewport() {
    let image = crate::Image2DState::from_argb(
        crate::ImageFormat::Rgb,
        2,
        2,
        &[0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff],
    )
    .unwrap();
    let mut renderer = SoftwareRenderer::new(4, 4, RenderLimits::default()).unwrap();
    renderer.clear(Some(0xff00_0000), true);
    renderer.set_scissor(1, 1, 2, 2).unwrap();
    renderer
        .draw_background(&image, [0, 0, 2, 2], false, false)
        .unwrap();

    assert_eq!(renderer.pixels()[5], 0xffff_0000);
    assert_eq!(renderer.pixels()[6], 0xff00_ff00);
    assert_eq!(renderer.pixels()[2 * 4 + 1], 0xff00_00ff);
    assert_eq!(renderer.pixels()[2 * 4 + 2], 0xffff_ffff);
    assert_eq!(renderer.pixels()[3 * 4 + 3], 0xff00_0000);
}

#[test]
fn empty_background_crop_leaves_the_clear_color_untouched() {
    let image =
        crate::Image2DState::from_argb(crate::ImageFormat::Rgb, 1, 1, &[0xffff_ffff]).unwrap();
    let mut renderer = renderer();
    renderer
        .draw_background(&image, [0, 0, 0, 1], false, false)
        .unwrap();
    assert!(renderer.pixels().iter().all(|pixel| *pixel == 0xff00_0000));
}

fn vertex(x: f32, y: f32, z: f32, color: u32) -> Vertex {
    let mut vertex = Vertex::new(Vec4::new(x, y, z, 1.0), color);
    vertex.project(Mat4::IDENTITY);
    vertex
}

#[test]
fn real_triangle_has_stable_crc_and_interpolated_color() {
    let mut renderer = renderer();
    renderer
        .draw_triangle([
            vertex(-0.75, -0.75, 0.0, 0xffff_0000),
            vertex(0.75, -0.75, 0.0, 0xff00_ff00),
            vertex(0.0, 0.75, 0.0, 0xff00_00ff),
        ])
        .unwrap();
    assert!(renderer.pixels().iter().any(|pixel| *pixel != 0xff00_0000));
    assert_eq!(renderer.stats().rasterized_triangles, 1);
    let mut hasher = Hasher::new();
    for pixel in renderer.pixels() {
        hasher.update(&pixel.to_be_bytes());
    }
    assert_eq!(hasher.finalize(), 0x273c_fdca);
}

#[test]
fn clipped_line_is_bounded_and_interpolates_color() {
    let mut renderer = renderer();
    renderer
        .draw_line([
            vertex(-2.0, 0.0, 0.0, 0xffff_0000),
            vertex(2.0, 0.0, 0.0, 0xff00_00ff),
        ])
        .unwrap();

    assert_eq!(renderer.stats().submitted_triangles, 1);
    assert_eq!(renderer.stats().clipped_triangles, 1);
    assert_eq!(renderer.stats().rasterized_triangles, 1);
    assert!(renderer.pixels()[16 * 32] & 0x00ff_0000 != 0);
    assert!(renderer.pixels()[16 * 32 + 31] & 0x0000_00ff != 0);
    assert!(
        renderer.pixels()[15 * 32..16 * 32]
            .iter()
            .all(|pixel| *pixel == 0xff00_0000)
    );
    assert!(
        renderer.pixels()[17 * 32..18 * 32]
            .iter()
            .all(|pixel| *pixel == 0xff00_0000)
    );
}

#[test]
fn flat_shading_keeps_the_last_vertex_color_when_that_vertex_is_clipped() {
    for line in [false, true] {
        let mut renderer = renderer();
        renderer.set_smooth_shading(false);
        if line {
            renderer
                .draw_line([
                    vertex(-2.0, 0.0, 0.0, 0xffff_0000),
                    vertex(2.0, 0.0, 0.0, 0xff00_00ff),
                ])
                .unwrap();
        } else {
            renderer
                .draw_triangle([
                    vertex(-0.8, -0.8, 0.0, 0xffff_0000),
                    vertex(0.8, -0.8, 0.0, 0xff00_ff00),
                    vertex(0.0, 2.0, 0.0, 0xff00_00ff),
                ])
                .unwrap();
        }
        assert_eq!(renderer.stats().clipped_triangles, 1);
        assert!(renderer.pixels().contains(&0xff00_00ff));
        assert!(
            renderer
                .pixels()
                .iter()
                .all(|pixel| matches!(*pixel, 0xff00_0000 | 0xff00_00ff)),
            "line={line}"
        );
    }
}

#[test]
fn indexed_strip_draws_real_geometry_and_validates_before_mutation() {
    let mut positions = crate::VertexArrayState::new(4, 3, crate::VertexComponent::Short).unwrap();
    positions
        .set_shorts(0, 4, &[-1, -1, 0, 1, -1, 0, -1, 1, 0, 1, 1, 0])
        .unwrap();
    let mut vertices = crate::VertexBufferState::default();
    vertices
        .set_positions(Some(positions), 0.75, [0.0; 3])
        .unwrap();
    vertices.set_default_color(0xffff_00ff);
    let strips = crate::TriangleStripArrayState::new(vec![0, 1, 2, 3], vec![4]).unwrap();
    let mut renderer = renderer();
    renderer
        .draw_indexed(&vertices, &strips, crate::Mat4::IDENTITY)
        .unwrap();
    assert_eq!(renderer.stats().submitted_triangles, 2);
    assert!(renderer.pixels().contains(&0xffff_00ff));

    let invalid = crate::TriangleStripArrayState::new(vec![0, 1, 9], vec![3]).unwrap();
    let before = renderer.stats();
    let pixels_before = renderer.pixels().to_vec();
    let depth_before = renderer.depth().to_vec();
    assert_eq!(
        renderer
            .draw_indexed(&vertices, &invalid, crate::Mat4::IDENTITY)
            .unwrap_err()
            .code(),
        "index-bounds"
    );
    assert_eq!(renderer.stats(), before);
    assert_eq!(renderer.pixels(), pixels_before);
    assert_eq!(renderer.depth(), depth_before);
}

#[test]
fn homogeneous_clipping_never_writes_outside_viewport() {
    let mut renderer = renderer();
    renderer.set_viewport(8, 8, 16, 16).unwrap();
    renderer
        .draw_triangle([
            vertex(-4.0, -0.5, 0.0, 0xffff_ffff),
            vertex(4.0, -0.5, 0.0, 0xffff_ffff),
            vertex(0.0, 4.0, 0.0, 0xffff_ffff),
        ])
        .unwrap();
    assert!(renderer.pixels().contains(&0xffff_ffff));
    for y in 0..32 {
        for x in 0..32 {
            if !(8..24).contains(&x) || !(8..24).contains(&y) {
                assert_eq!(renderer.pixels()[y * 32 + x], 0xff00_0000);
            }
        }
    }
    assert_eq!(renderer.stats().clipped_triangles, 1);
}

#[test]
fn native_work_limits_fail_before_unbounded_rasterization() {
    let mut renderer = SoftwareRenderer::new(
        32,
        32,
        RenderLimits {
            triangles: 1,
            fragments: 16,
            ..RenderLimits::default()
        },
    )
    .unwrap();
    let triangle = [
        vertex(-1.0, -1.0, 0.0, 0xffff_ffff),
        vertex(1.0, -1.0, 0.0, 0xffff_ffff),
        vertex(0.0, 1.0, 0.0, 0xffff_ffff),
    ];
    assert_eq!(
        renderer.draw_triangle(triangle).unwrap_err().code(),
        "render-budget"
    );
    renderer.reset_stats();
    renderer.limits.fragments = 2_000;
    renderer.draw_triangle(triangle).unwrap();
    assert_eq!(
        renderer.draw_triangle(triangle).unwrap_err().code(),
        "render-budget"
    );
}

#[test]
fn oversized_viewport_coordinates_fail_before_edge_arithmetic() {
    let mut renderer = SoftwareRenderer::new(
        1,
        1,
        RenderLimits {
            max_viewport_width: 16_000_000,
            max_viewport_height: 16_000_000,
            ..RenderLimits::default()
        },
    )
    .unwrap();
    renderer
        .set_viewport(-8_000_000, -8_000_000, 16_000_000, 16_000_000)
        .unwrap();
    assert_eq!(
        renderer
            .draw_triangle([
                vertex(-1.0, -1.0, 0.0, 0xffff_ffff),
                vertex(1.0, -1.0, 0.0, 0xffff_ffff),
                vertex(-1.0, 1.0, 0.0, 0xffff_ffff),
            ])
            .unwrap_err()
            .code(),
        "coordinate-overflow",
    );
    assert_eq!(renderer.pixels(), &[0]);
    assert_eq!(renderer.depth(), &[u32::MAX]);
}

#[test]
fn rejects_non_finite_vertices_and_invalid_viewport() {
    let mut renderer = renderer();
    assert_eq!(
        renderer.set_viewport(0, 0, 0, 1).unwrap_err().code(),
        "invalid-viewport"
    );
    assert_eq!(
        renderer
            .draw_triangle([
                vertex(f32::NAN, 0.0, 0.0, 0),
                vertex(0.0, 0.0, 0.0, 0),
                vertex(1.0, 0.0, 0.0, 0),
            ])
            .unwrap_err()
            .code(),
        "non-finite-vertex"
    );
}
