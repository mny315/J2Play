use super::{Color32, frame_colors};
use std::hint::black_box;
use std::time::Instant;

fn pixels(count: usize, alpha: &str) -> Vec<u32> {
    let mut pixels: Vec<_> = (0..count)
        .map(|index| {
            let value = u32::try_from(index).unwrap().wrapping_mul(0x0012_375b);
            let alpha = if alpha == "mixed" { value >> 24 } else { 255 };
            value & 0x00ff_ffff | alpha << 24
        })
        .collect();
    match alpha {
        "first" => pixels[0] &= 0x00ff_ffff,
        "last" => pixels[count - 1] &= 0x00ff_ffff,
        "middle" => pixels[count / 2] = pixels[count / 2] & 0x00ff_ffff | 0x8000_0000,
        _ => {}
    }
    pixels
}

fn expected(pixels: &[u32]) -> Vec<Color32> {
    pixels
        .iter()
        .map(|pixel| {
            let [alpha, red, green, blue] = pixel.to_be_bytes();
            Color32::from_rgba_unmultiplied(red, green, blue, alpha)
        })
        .collect()
}

#[test]
fn frame_conversion_preserves_colors_and_alpha_at_buffer_boundaries() {
    assert!(frame_colors(&[]).is_empty());
    for count in [1, 7, 8, 9, 63, 64, 65, 4096, 4097] {
        for alpha in ["opaque", "first", "middle", "last", "mixed"] {
            let pixels = pixels(count, alpha);
            assert_eq!(frame_colors(&pixels), expected(&pixels), "{count} {alpha}");
        }
    }
    let pixels: Vec<_> = (0..=255)
        .flat_map(|alpha| {
            (0..=255).map(move |value| alpha << 24 | value << 16 | (255 - value) << 8 | value)
        })
        .collect();
    assert_eq!(frame_colors(&pixels), expected(&pixels));
}

#[test]
#[ignore = "manual release throughput measurement"]
fn frame_color_throughput() {
    for (width, height) in [(8, 8), (240, 320), (480, 800)] {
        let iterations = if width == 8 { 65_536 } else { 128 };
        for alpha in ["opaque", "first", "last", "mixed"] {
            let pixels = pixels(width * height, alpha);
            let expected = expected(&pixels);
            let started = Instant::now();
            let mut last = Vec::new();
            for _ in 0..iterations {
                last = black_box(frame_colors(black_box(&pixels)));
            }
            let elapsed = started.elapsed();
            assert_eq!(last, expected);
            let checksum: u64 = last
                .iter()
                .map(|pixel| u64::from(u32::from_le_bytes(pixel.to_array())))
                .sum();
            eprintln!(
                "frame-color {width}x{height} {alpha}: elapsed={elapsed:?} checksum={checksum}"
            );
        }
    }
}
