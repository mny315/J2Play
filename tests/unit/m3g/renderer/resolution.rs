use super::*;

#[test]
fn restored_damage_covers_the_whole_target_before_a_partial_draw() {
    for scale in [1, 2] {
        let mut original = renderer();
        original.set_resolution_scale(scale).unwrap();
        original.clear(Some(0x1234_5678), true);
        let bytes = save_state::encode(&original).unwrap();
        let mut restored: SoftwareRenderer = save_state::decode(&bytes).unwrap();
        assert_eq!(save_state::encode(&restored).unwrap(), bytes);
        assert!(restored.has_color_damage());
        restored.set_scissor(2, 3, 4, 5).unwrap();
        restored.clear(Some(0x2043_2100), false);
        assert_eq!(restored.take_rgb_damage(), Some([0, 0, 32, 32]));
        assert!(restored.pixels().iter().all(|pixel| pixel >> 24 == 255));
        assert!(!restored.has_color_damage());
    }
}

#[test]
fn double_density_rasterizes_geometry_and_keeps_logical_viewport_limits() {
    let mut renderer = renderer();
    renderer.set_resolution_scale(2).unwrap();
    assert_eq!((renderer.width(), renderer.height()), (32, 32));
    assert_eq!(renderer.pixels().len(), 64 * 64);
    renderer.set_viewport(0, 0, 32, 32).unwrap();
    renderer.set_scissor(2, 3, 4, 5).unwrap();
    renderer.clear(Some(0xff12_3456), true);
    assert_eq!(
        renderer
            .pixels()
            .iter()
            .filter(|p| **p == 0xff12_3456)
            .count(),
        4 * 5 * 4
    );
    assert!(renderer.set_viewport(0, 0, 1025, 1).is_err());
    assert!(renderer.set_scissor(31, 31, 2, 1).is_err());
    renderer
        .validate_checkpoint(RenderLimits::default())
        .unwrap();
    let before = renderer.pixels().to_vec();
    assert!(renderer.set_resolution_scale(4).is_err());
    assert_eq!(renderer.pixels(), before);
    renderer.set_resolution_scale(1).unwrap();
    assert_eq!(renderer.pixels().len(), 32 * 32);
    renderer
        .validate_checkpoint(RenderLimits::default())
        .unwrap();
}

#[test]
fn double_density_has_distinct_samples_and_resolves_to_guest_size() {
    let mut renderer = renderer();
    renderer.set_resolution_scale(2).unwrap();
    renderer
        .draw_triangle([
            vertex(-1.0, -1.0, 0.0, 0xffff_ffff),
            vertex(1.0, -1.0, 0.0, 0xffff_ffff),
            vertex(-1.0, 1.0, 0.0, 0xffff_ffff),
        ])
        .unwrap();
    let resolved = renderer.resolved_pixels();
    assert_eq!(resolved.len(), 32 * 32);
    assert!(
        resolved
            .iter()
            .any(|pixel| ![0xff00_0000, 0xffff_ffff].contains(pixel))
    );
}

#[test]
fn rgb_damage_covers_changed_samples_and_preserves_depth() {
    let mut renderer = renderer();
    renderer.set_resolution_scale(2).unwrap();
    assert_eq!(renderer.take_rgb_damage(), Some([0, 0, 32, 32]));
    renderer.clear(None, true);
    assert!(!renderer.has_color_damage());
    renderer.set_scissor(4, 5, 8, 9).unwrap();
    let before = renderer.pixels().to_vec();
    renderer
        .draw_triangle([
            vertex(-1.0, -1.0, 0.0, 0x8040_2050),
            vertex(1.0, -1.0, 0.0, 0x8040_2050),
            vertex(-1.0, 1.0, 0.0, 0x8040_2050),
        ])
        .unwrap();
    let depth = renderer.depth().to_vec();
    let [x, y, w, h] = renderer.take_rgb_damage().unwrap();
    assert!(x >= 4 && y >= 5 && x + w <= 12 && y + h <= 14);
    for (i, (&old, &new)) in before.iter().zip(renderer.pixels()).enumerate() {
        if old != new {
            assert!((x * 2..(x + w) * 2).contains(&(i as u32 % 64)));
            assert!((y * 2..(y + h) * 2).contains(&(i as u32 / 64)));
            assert_eq!(new >> 24, 255);
        }
    }
    assert_eq!(renderer.depth(), depth);
    assert!(!renderer.has_color_damage());
}

#[test]
fn scaled_attachment_adoption_validates_dimensions_and_preserves_samples() {
    let pixels: Vec<u32> = (0..24).collect();
    let renderer =
        SoftwareRenderer::from_scaled_pixels(3, 2, 2, pixels.clone(), RenderLimits::default())
            .unwrap();
    assert_eq!((renderer.width(), renderer.height()), (3, 2));
    assert_eq!(renderer.pixels(), pixels);
    renderer
        .validate_checkpoint(RenderLimits::default())
        .unwrap();
    for (width, scale, count) in [(3, 0, 24), (3, 3, 24), (u32::MAX, 2, 24), (3, 2, 6)] {
        assert!(
            SoftwareRenderer::from_scaled_pixels(
                width,
                2,
                scale,
                vec![0; count],
                RenderLimits::default()
            )
            .is_err()
        );
    }
}

#[test]
#[ignore = "manual original/double raster resolution throughput measurement"]
fn render_resolution_throughput() {
    for scale in [1, 2] {
        let (mut renderer, triangle) = textured_raster_fixture(true);
        renderer.set_resolution_scale(scale).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            renderer.reset_stats();
            renderer.clear(Some(0xff00_0000), true);
            renderer
                .draw_triangle(std::hint::black_box(triangle))
                .unwrap();
        }
        let elapsed = start.elapsed();
        eprintln!(
            "render scale={scale} elapsed={elapsed:?} pixels={} crc={:08x}",
            renderer.pixels().len(),
            raster_crc(&renderer)
        );
    }
}
