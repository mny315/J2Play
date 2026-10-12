use super::*;

fn decode_renderer(renderer: &SoftwareRenderer) -> SoftwareRenderer {
    save_state::decode(&save_state::encode(renderer).unwrap()).unwrap()
}

#[test]
fn checkpoint_rejects_counters_that_can_overflow_during_drawing() {
    for corruption in 0..8 {
        let mut renderer = renderer();
        renderer.set_compositing(false, false, true, true);
        match corruption {
            0 => renderer.stats.submitted_triangles = u64::MAX,
            1 => renderer.stats.clipped_triangles = u64::MAX,
            2 => renderer.stats.culled_triangles = u64::MAX,
            3 => renderer.stats.rasterized_triangles = u64::MAX,
            4 => renderer.stats.tested_fragments = u64::MAX,
            5 => renderer.stats.shaded_fragments = u64::MAX,
            6 => renderer.stats.depth_rejected_fragments = u64::MAX,
            _ => renderer.stats.blended_fragments = u64::MAX,
        }
        assert_eq!(
            decode_renderer(&renderer)
                .validate_checkpoint(RenderLimits::default())
                .unwrap_err()
                .code(),
            "checkpoint-target",
            "counter {corruption}"
        );
    }
}

#[test]
fn checkpoint_rejects_viewports_and_offsets_unavailable_through_configuration() {
    for viewport in [
        [0, 0, 0, 32],
        [0, 0, 32, -1],
        [0, 0, 1_025, 32],
        [-8_000_000, -8_000_000, 16_000_000, 16_000_000],
    ] {
        let mut renderer = renderer();
        renderer.viewport = viewport;
        assert_eq!(
            decode_renderer(&renderer)
                .validate_checkpoint(RenderLimits::default())
                .unwrap_err()
                .code(),
            "checkpoint-target",
            "viewport {viewport:?}"
        );
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for component in 0..2 {
            let mut renderer = renderer();
            renderer.depth_offset[component] = value;
            assert!(
                decode_renderer(&renderer)
                    .validate_checkpoint(RenderLimits::default())
                    .is_err()
            );
        }
    }
}

#[test]
fn checkpoint_preserves_clipping_failed_draws_and_initial_target_viewports() {
    let mut renderer = renderer();
    renderer.set_compositing(false, false, true, true);
    renderer
        .draw_triangle([
            vertex(-2.0, -2.0, 0.0, 0x80ff_0000),
            vertex(2.0, -2.0, 0.0, 0x80ff_0000),
            vertex(0.0, 2.0, 0.0, 0x80ff_0000),
        ])
        .unwrap();
    assert!(renderer.stats.rasterized_triangles > renderer.stats.submitted_triangles);
    renderer
        .draw_triangle([vertex(f32::NAN, 0.0, 0.0, 0); 3])
        .unwrap_err();
    renderer.set_viewport(i32::MAX, i32::MIN, 32, 32).unwrap();
    renderer.draw_point(vertex(0.0, 0.0, 0.0, 0)).unwrap();
    let mut restored = decode_renderer(&renderer);
    restored
        .validate_checkpoint(RenderLimits::default())
        .unwrap();
    assert_eq!(restored.pixels(), renderer.pixels());
    assert_eq!(restored.stats(), renderer.stats());
    restored.set_viewport(0, 0, 32, 32).unwrap();
    restored
        .draw_point(vertex(0.0, 0.0, 0.0, 0x80ff_0000))
        .unwrap();

    // Construction starts with the full target even when it is larger than an
    // explicitly selectable viewport. That existing state remains restorable.
    let renderer = SoftwareRenderer::new(2_048, 1, RenderLimits::default()).unwrap();
    decode_renderer(&renderer)
        .validate_checkpoint(RenderLimits::default())
        .unwrap();
}
