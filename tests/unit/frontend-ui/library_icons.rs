use super::*;

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([255, 127, 0, 128]));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[test]
fn thumbnails_preserve_small_images_aspect_and_alpha_with_decode_limits() {
    for (width, height, edge, expected) in [
        (32, 16, 80, [32, 16]),
        (256, 128, 80, [80, 40]),
        (128, 256, 80, [40, 80]),
        (256, 256, 256, [256, 256]),
        (1, 256, 32, [1, 32]),
    ] {
        let thumbnail = decode_icon(&png(width, height), edge).unwrap();
        assert_eq!(thumbnail.size, expected);
        assert!(
            thumbnail
                .pixels
                .iter()
                .all(|pixel| *pixel == egui::Color32::from_rgba_unmultiplied(255, 127, 0, 128))
        );
    }
    // Resizing must not bypass the original source-image bounds.
    assert!(decode_icon(&png(257, 1), 80).is_err());
    assert!(decode_icon(&png(1, 257), 80).is_err());
    assert!(decode_icon(b"not an image", 80).is_err());
}

#[test]
fn viewport_thumbnails_fit_the_pixel_budget_at_every_library_size_and_scale() {
    for visible in 0..=frontend_core::MAX_LIBRARY_ENTRIES {
        for scale in [0.5, 1.0, 1.25, 2.0, 4.0, 16.0] {
            for points in [LIST_ICON_POINTS, TILE_ICON_POINTS] {
                let edge = thumbnail_edge(visible, points, scale) as usize;
                assert!((1..=256).contains(&edge));
                assert!(visible * edge.pow(2) <= MAX_ICON_PIXELS);
            }
        }
    }
    assert_eq!(thumbnail_edge(200, TILE_ICON_POINTS, 1.0), 80);
    assert_eq!(thumbnail_edge(200, TILE_ICON_POINTS, 2.0), 80);
    assert_eq!(thumbnail_edge(40, TILE_ICON_POINTS, 2.0), 160);
    assert_eq!(thumbnail_edge(200, LIST_ICON_POINTS, 1.0), 56);
    for invalid_scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let edge = thumbnail_edge(usize::MAX, TILE_ICON_POINTS, invalid_scale) as usize;
        assert!((1..=256).contains(&edge));
        assert!(frontend_core::MAX_LIBRARY_ENTRIES * edge.pow(2) <= MAX_ICON_PIXELS);
    }
}
