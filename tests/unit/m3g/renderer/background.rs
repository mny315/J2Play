use super::*;

#[test]
fn background_sampling_matches_scalar_coordinates_at_viewport_and_crop_edges() {
    let pixels: Vec<_> = (0..15)
        .map(|index| 0x8040_2010 + index * 0x0004_0608)
        .collect();
    let image = crate::Image2DState::from_argb(crate::ImageFormat::Rgba, 3, 5, &pixels).unwrap();
    for viewport in [
        [0, 0, 32, 24],
        [-9, -7, 37, 31],
        [11, 9, 9, 7],
        [33, 0, 8, 8],
        [i32::MIN, 0, 8, 8],
        [i32::MAX, 0, 8, 8],
    ] {
        for crop in [
            [0, 0, 3, 5],
            [-7, -9, 19, 21],
            [1, 2, 0, 5],
            [1, 2, 3, 0],
            [i32::MIN, i32::MAX, i32::MAX, i32::MAX],
            [i32::MAX, i32::MIN, 7, 13],
        ] {
            for (repeat_x, repeat_y) in [(false, false), (false, true), (true, false), (true, true)]
            {
                for scissor in [[0, 0, 32, 24], [3, 4, 19, 13], [32, 24, 0, 0]] {
                    let mut renderer =
                        SoftwareRenderer::new(32, 24, RenderLimits::default()).unwrap();
                    renderer.clear(Some(0xff12_3456), true);
                    renderer
                        .set_viewport(
                            viewport[0],
                            viewport[1],
                            viewport[2] as u32,
                            viewport[3] as u32,
                        )
                        .unwrap();
                    renderer
                        .set_scissor(scissor[0], scissor[1], scissor[2], scissor[3])
                        .unwrap();
                    let mut expected = renderer.pixels().to_vec();
                    let depth = renderer.depth().to_vec();
                    if crop[2] != 0 && crop[3] != 0 {
                        for y in 0..24_u32 {
                            for x in 0..32_u32 {
                                let dx = i64::from(x) - i64::from(viewport[0]);
                                let dy = i64::from(y) - i64::from(viewport[1]);
                                if dx < 0
                                    || dx >= i64::from(viewport[2])
                                    || dy < 0
                                    || dy >= i64::from(viewport[3])
                                    || x < scissor[0]
                                    || x >= scissor[0] + scissor[2]
                                    || y < scissor[1]
                                    || y >= scissor[1] + scissor[3]
                                {
                                    continue;
                                }
                                let mut sx = i64::from(crop[0])
                                    + dx * i64::from(crop[2]) / i64::from(viewport[2]);
                                let mut sy = i64::from(crop[1])
                                    + dy * i64::from(crop[3]) / i64::from(viewport[3]);
                                if repeat_x {
                                    sx = sx.rem_euclid(3);
                                }
                                if repeat_y {
                                    sy = sy.rem_euclid(5);
                                }
                                if (0..3).contains(&sx) && (0..5).contains(&sy) {
                                    expected[(y * 32 + x) as usize] =
                                        pixels[(sy * 3 + sx) as usize];
                                }
                            }
                        }
                    }
                    renderer
                        .draw_background(&image, crop, repeat_x, repeat_y)
                        .unwrap();
                    assert_eq!(
                        renderer.pixels(),
                        expected,
                        "viewport={viewport:?} crop={crop:?} repeat={repeat_x}/{repeat_y} scissor={scissor:?}"
                    );
                    assert_eq!(renderer.depth(), depth);
                    assert_eq!(renderer.stats(), RenderStats::default());
                }
            }
        }
    }
}

#[test]
#[ignore = "manual background image scaling throughput measurement"]
fn background_sampling_throughput() {
    let pixels: Vec<_> = (0..256)
        .map(|index| 0xff00_0000 | (index * 0x0001_0307))
        .collect();
    let image = crate::Image2DState::from_argb(crate::ImageFormat::Rgb, 16, 16, &pixels).unwrap();
    for (width, height) in [(8, 10), (240, 320)] {
        for (crop, repeat) in [
            ([0, 0, 16, 16], false),
            ([-17, -9, 96, 80], true),
            ([-2, -3, 20, 24], false),
            ([i32::MAX, i32::MAX, i32::MAX, i32::MAX], true),
        ] {
            let mut renderer =
                SoftwareRenderer::new(width, height, RenderLimits::default()).unwrap();
            renderer.clear(Some(0xff12_3456), true);
            let started = std::time::Instant::now();
            for _ in 0..512 {
                renderer
                    .draw_background(std::hint::black_box(&image), crop, repeat, repeat)
                    .unwrap();
                std::hint::black_box(renderer.pixels());
            }
            eprintln!(
                "background size={width}x{height} crop={crop:?} repeat={repeat} elapsed={:?} crc={:08x}",
                started.elapsed(),
                raster_crc(&renderer)
            );
        }
    }
}
