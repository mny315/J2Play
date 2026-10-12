use super::*;

#[test]
#[ignore = "manual mipmap sampling throughput measurement"]
fn mipmapped_raster_throughput() {
    for linear in [false, true] {
        for linear_levels in [false, true] {
            let (mut renderer, triangle) = mipmapped_raster_fixture(linear, linear_levels);
            let started = std::time::Instant::now();
            for _ in 0..128 {
                renderer.reset_stats();
                renderer.clear(Some(0xff00_0000), true);
                renderer
                    .draw_triangle(std::hint::black_box(triangle))
                    .unwrap();
            }
            eprintln!(
                "linear={linear} linear_levels={linear_levels} elapsed={:?} crc={:08x}",
                started.elapsed(),
                raster_crc(&renderer),
            );
        }
    }
}

#[test]
#[ignore = "manual line interpolation throughput measurement"]
fn line_raster_throughput() {
    for size in [8, 256] {
        for depth in [false, true] {
            for (smooth, perspective) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                let mut renderer =
                    SoftwareRenderer::new(size, size, RenderLimits::default()).unwrap();
                renderer.set_compositing(depth, depth, true, true);
                renderer.set_depth_test_allows_equal(true);
                renderer.clear(Some(0xff12_3456), true);
                let mut line = [
                    vertex(-0.9, -0.75, -0.2, 0x8042_9bd7),
                    vertex(
                        0.9,
                        0.75,
                        0.2,
                        if smooth { 0xc0e8_2b64 } else { 0x8042_9bd7 },
                    ),
                ];
                if perspective {
                    line[1] = scaled_clip_vertex(line[1], 3.0);
                }
                let started = std::time::Instant::now();
                for _ in 0..16_384 {
                    renderer.reset_stats();
                    renderer.draw_line(std::hint::black_box(line)).unwrap();
                }
                eprintln!(
                    "line size={size} depth={depth} smooth={smooth} perspective={perspective} elapsed={:?} crc={:08x} stats={:?}",
                    started.elapsed(),
                    raster_crc(&renderer),
                    renderer.stats()
                );
            }
        }
    }
}

#[test]
#[ignore = "manual triangle clipping throughput measurement"]
fn triangle_clipping_throughput() {
    for offset in [
        [0.0, 0.0, 0.0],
        [-2.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, -2.0, 0.0],
        [0.0, 2.0, 0.0],
        [0.0, 0.0, -2.0],
        [0.0, 0.0, 2.0],
        [0.75, 0.0, 0.0],
    ] {
        let mut renderer = SoftwareRenderer::new(8, 8, RenderLimits::default()).unwrap();
        let triangle = [
            vertex(-0.5 + offset[0], -0.5 + offset[1], offset[2], 0xff88_aacc),
            vertex(0.5 + offset[0], -0.5 + offset[1], offset[2], 0xff88_aacc),
            vertex(offset[0], 0.5 + offset[1], offset[2], 0xff88_aacc),
        ];
        let started = std::time::Instant::now();
        for _ in 0..262_144 {
            renderer.reset_stats();
            renderer
                .draw_triangle(std::hint::black_box(triangle))
                .unwrap();
        }
        eprintln!(
            "offset={offset:?} elapsed={:?} crc={:08x} stats={:?}",
            started.elapsed(),
            raster_crc(&renderer),
            renderer.stats(),
        );
    }
}

#[test]
#[ignore = "manual triangle coverage throughput measurement"]
fn triangle_coverage_throughput() {
    for thin in [false, true] {
        let mut renderer = SoftwareRenderer::new(240, 320, RenderLimits::default()).unwrap();
        let triangle = [
            vertex(-0.95, -0.95, 0.0, 0xff88_aacc),
            vertex(0.95, 0.95, 0.0, 0xff88_aacc),
            vertex(if thin { 0.90 } else { -0.95 }, 0.95, 0.0, 0xff88_aacc),
        ];
        let started = std::time::Instant::now();
        for _ in 0..512 {
            renderer.reset_stats();
            renderer.clear(Some(0xff00_0000), true);
            renderer
                .draw_triangle(std::hint::black_box(triangle))
                .unwrap();
        }
        eprintln!(
            "thin={thin} elapsed={:?} crc={:08x} shaded={}",
            started.elapsed(),
            raster_crc(&renderer),
            renderer.stats().shaded_fragments,
        );
    }
}

#[test]
#[ignore = "manual tiled texture filtering throughput measurement"]
fn tiled_texture_throughput() {
    for linear in [false, true] {
        for wrap in [crate::WrapMode::Clamp, crate::WrapMode::Repeat] {
            let (mut renderer, mut triangle) = textured_raster_fixture(false);
            let texture = renderer.textures[0].as_mut().unwrap();
            texture.set_wrapping(wrap, wrap);
            texture.set_linear_filter(linear);
            for vertex in &mut triangle {
                for coordinate in &mut vertex.texture[0][..2] {
                    *coordinate = *coordinate * 16.0 - 8.0;
                }
            }
            let started = std::time::Instant::now();
            for _ in 0..128 {
                renderer.reset_stats();
                renderer.clear(Some(0xff00_0000), true);
                renderer
                    .draw_triangle(std::hint::black_box(triangle))
                    .unwrap();
            }
            eprintln!(
                "linear={linear} wrap={wrap:?} elapsed={:?} crc={:08x}",
                started.elapsed(),
                raster_crc(&renderer),
            );
        }
    }
}

#[test]
#[ignore = "manual opaque fragment compositing throughput measurement"]
fn opaque_fragment_throughput() {
    for mode in [
        FrameBlend::Replace,
        FrameBlend::Alpha,
        FrameBlend::SourceOver,
    ] {
        for blend in [
            crate::BlendFunction::Modulate,
            crate::BlendFunction::Replace,
        ] {
            let (mut renderer, mut triangle) = textured_raster_fixture(false);
            for vertex in &mut triangle {
                vertex.color = u32::MAX;
            }
            renderer.textures[0]
                .as_mut()
                .unwrap()
                .set_blend_function(blend);
            renderer
                .set_compositing_mode(false, false, true, true, mode, 0.0, 0.0)
                .unwrap();
            let started = std::time::Instant::now();
            for _ in 0..128 {
                renderer.reset_stats();
                renderer.clear(Some(0x4012_3456), true);
                renderer
                    .draw_triangle(std::hint::black_box(triangle))
                    .unwrap();
            }
            eprintln!(
                "mode={mode:?} texture={blend:?} elapsed={:?} crc={:08x}",
                started.elapsed(),
                raster_crc(&renderer)
            );
        }
    }
}

#[test]
#[ignore = "manual translucent fragment compositing throughput measurement"]
fn translucent_fragment_throughput() {
    for mode in [
        FrameBlend::Replace,
        FrameBlend::Alpha,
        FrameBlend::SourceOver,
        FrameBlend::AlphaAdd,
        FrameBlend::Add,
        FrameBlend::Subtract,
        FrameBlend::Half,
        FrameBlend::Modulate,
        FrameBlend::ModulateX2,
    ] {
        for textured in [false, true] {
            let (mut renderer, mut triangle) = textured_raster_fixture(false);
            if !textured {
                renderer.set_texture(None);
            }
            for vertex in &mut triangle {
                vertex.color = 0x8042_9bd7;
            }
            renderer
                .set_compositing_mode(false, false, true, true, mode, 0.0, 0.0)
                .unwrap();
            let started = std::time::Instant::now();
            for _ in 0..128 {
                renderer.reset_stats();
                renderer.clear(Some(0x4056_3412), true);
                renderer
                    .draw_triangle(std::hint::black_box(triangle))
                    .unwrap();
            }
            eprintln!(
                "mode={mode:?} textured={textured}: elapsed={:?} crc={:08x}",
                started.elapsed(),
                raster_crc(&renderer)
            );
        }
    }
}

#[test]
#[ignore = "manual software rasterizer throughput measurement"]
fn textured_raster_throughput() {
    for wrap in [crate::WrapMode::Clamp, crate::WrapMode::Repeat] {
        for smooth in [false, true] {
            let (mut renderer, triangle) = textured_raster_fixture(smooth);
            renderer.textures[0]
                .as_mut()
                .unwrap()
                .set_wrapping(wrap, wrap);
            let started = std::time::Instant::now();
            for _ in 0..128 {
                renderer.reset_stats();
                renderer.clear(Some(0xff00_0000), true);
                renderer
                    .draw_triangle(std::hint::black_box(triangle))
                    .unwrap();
            }
            let elapsed = started.elapsed();
            eprintln!(
                "wrap={wrap:?} smooth={smooth} elapsed={elapsed:?} crc={:08x}",
                raster_crc(&renderer)
            );
        }
    }
}
