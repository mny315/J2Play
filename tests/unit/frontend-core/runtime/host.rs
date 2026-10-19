use super::*;

#[test]
fn scaled_frame_preparation_preserves_pixels_canvas_mapping_and_save_samples() {
    for width in 1_u32..=7 {
        for height in 1_u32..=9 {
            for normal in [false, true] {
                let canvas = (
                    width - if normal && width > 2 { 2 } else { 0 },
                    height - if normal && height > 2 { 2 } else { 0 },
                );
                let mut source: Vec<_> = (0..canvas.0 * canvas.1 * 4)
                    .map(|pixel| pixel.wrapping_mul(0x6a21_8341))
                    .collect();
                let mut scratch = vec![u32::MAX; 64];
                let (frame, saved) = prepare_scaled_lcdui_frame(
                    SessionId(3),
                    AttemptId(7),
                    (width, height),
                    canvas,
                    &source,
                    &mut scratch,
                )
                .unwrap();
                assert_eq!(frame.session_id, SessionId(3));
                assert_eq!(frame.attempt_id, AttemptId(7));
                assert_eq!(
                    (frame.width, frame.height, frame.pixel_scale()),
                    (width * 2, height * 2, 2)
                );
                let region = frame.canvas_region;
                assert_eq!((region.x, region.y), (width - canvas.0, height - canvas.1));
                for row in 0..height * 2 {
                    for column in 0..width * 2 {
                        let inside = column >= region.x
                            && column < region.x + region.width
                            && row >= region.y
                            && row < region.y + region.height;
                        let expected = if inside {
                            source[((row - region.y) * canvas.0 * 2 + column - region.x) as usize]
                        } else {
                            ::runtime::LCDUI_SYSTEM_CHROME
                        };
                        assert_eq!(frame.pixels[(row * width * 2 + column) as usize], expected);
                    }
                }
                assert_eq!(
                    saved.canvas_region,
                    [region.x / 2, region.y / 2, canvas.0, canvas.1]
                );
                for row in 0..height {
                    for column in 0..width {
                        assert_eq!(
                            saved.pixels[(row * width + column) as usize],
                            frame.pixels[((row * 2 + 1) * width * 2 + column * 2 + 1) as usize]
                        );
                    }
                }
                assert_eq!(
                    frame.canvas_point(
                        i32::try_from(region.x).unwrap(),
                        i32::try_from(region.y).unwrap()
                    ),
                    Some((0, 0))
                );
                let previous = frame.pixels.to_vec();
                source.fill(u32::MAX);
                assert_eq!(frame.pixels.as_ref(), previous);
            }
        }
    }
}

#[test]
fn scaled_frame_preparation_rejects_invalid_pixels_without_changing_scratch() {
    let mut scratch = vec![7; 8];
    assert!(
        prepare_scaled_lcdui_frame(
            SessionId(1),
            AttemptId(1),
            (8, 8),
            (8, 8),
            &[0; 3],
            &mut scratch,
        )
        .is_err()
    );
    assert_eq!(scratch, [7; 8]);
}

#[test]
#[ignore = "manual frontend frame and checkpoint thumbnail throughput measurement"]
fn scaled_frame_preparation_throughput() {
    for (width, height) in [(240_u32, 320_u32), (480, 640)] {
        for normal in [false, true] {
            let canvas = (width, height - if normal { 24 } else { 0 });
            let source: Vec<_> = (0..canvas.0 * canvas.1 * 4)
                .map(|pixel| 0xff00_0000 | (pixel.wrapping_mul(137) & 0x00ff_ffff))
                .collect();
            let mut scratch = Vec::new();
            let mut previous = None;
            let started = Instant::now();
            for _ in 0..64 {
                previous = Some(
                    prepare_scaled_lcdui_frame(
                        SessionId(1),
                        AttemptId(1),
                        std::hint::black_box((width, height)),
                        canvas,
                        std::hint::black_box(&source),
                        &mut scratch,
                    )
                    .unwrap(),
                );
                std::hint::black_box(&previous);
            }
            let elapsed = started.elapsed();
            let (frame, saved) = previous.unwrap();
            let checksum = frame
                .pixels
                .iter()
                .chain(saved.pixels.iter())
                .fold(0_u64, |sum, pixel| {
                    sum.wrapping_mul(31).wrapping_add(u64::from(*pixel))
                });
            eprintln!(
                "frontend {width}x{height} normal={normal} elapsed={elapsed:?} checksum={checksum:016x}"
            );
        }
    }
}
