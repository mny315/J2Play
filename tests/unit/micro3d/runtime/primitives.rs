use super::*;
use crate::runtime::point_sprites::projected_screen_position;
use crate::runtime::projection::parallel_depth;

#[test]
fn point_primitives_render_one_pixel_per_visible_vertex() {
    let background = vec![0xff00_0000; 32 * 32];
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_primitives(
            None,
            0,
            0,
            FigureLayoutState {
                scale: [4096, 4096],
                center: [16, 16],
                projection: Projection::ParallelScale,
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(
                0x0100_0800,
                2,
                &[-4, 0, 0, 4, 0, 0],
                &[],
                &[],
                &[0x00ff_0000, 0x0000_ff00],
            ),
        )
        .unwrap();

    let changed: Vec<_> = runtime
        .target_pixels()
        .iter()
        .enumerate()
        .filter(|(_, pixel)| **pixel != 0xff00_0000)
        .map(|(index, pixel)| (index, *pixel))
        .collect();
    assert_eq!(
        changed,
        [(16 * 32 + 12, 0xffff_0000), (16 * 32 + 20, 0xff00_ff00)]
    );
    assert_eq!(runtime.metrics().rasterized_triangles, 2);
    assert_eq!(runtime.metrics().shaded_pixels, 2);
}

#[test]
fn micro3d_v3_preserves_submission_order_for_coplanar_primitives() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .load_target(32, 32, &vec![0xff00_0000; 32 * 32])
        .unwrap();
    let layout = FigureLayoutState {
        scale: [4096, 4096],
        center: [16, 16],
        projection: Projection::ParallelScale,
        ..FigureLayoutState::default()
    };
    let coordinates = [-10, -10, 0, 10, -10, 0, 0, 10, 0];

    for color in [0x00ff_0000, 0x0000_ff00] {
        runtime
            .render_primitives(
                None,
                0,
                0,
                layout.clone(),
                AffineTrans::IDENTITY,
                EffectState::default(),
                PrimitiveData::new(0x0300_0400, 1, &coordinates, &[], &[], &[color]),
            )
            .unwrap();
    }

    assert_eq!(runtime.target_pixels()[16 * 32 + 16], 0xff00_ff00);
}

#[test]
fn parallel_screen_layer_does_not_occlude_perspective_scene() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .load_target(32, 32, &vec![0xff00_0000; 32 * 32])
        .unwrap();
    runtime
        .render_primitives(
            None,
            0,
            0,
            FigureLayoutState {
                center: [16, 16],
                projection: Projection::PerspectiveFov {
                    near: 32,
                    far: 32_764,
                    angle: 512,
                },
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(
                0x0300_0400,
                1,
                &[-400, -400, 1000, 400, -400, 1000, 0, 400, 1000],
                &[],
                &[],
                &[0x0000_ff00],
            ),
        )
        .unwrap();
    runtime
        .render_primitives(
            None,
            0,
            0,
            FigureLayoutState {
                scale: [4096, 4096],
                center: [16, 16],
                projection: Projection::ParallelScale,
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(
                0x0300_0400,
                1,
                &[-10, -10, 0, 10, -10, 0, 0, 10, 0],
                &[],
                &[],
                &[0x00ff_0000],
            ),
        )
        .unwrap();

    assert_eq!(runtime.target_pixels()[16 * 32 + 16], 0xff00_ff00);
}

#[test]
fn parallel_screen_layers_preserve_relative_depth() {
    assert!(parallel_depth(-100) < parallel_depth(0));
    assert!(parallel_depth(0) < parallel_depth(100));
    assert!((0.99..=1.0).contains(&parallel_depth(i32::MIN)));
    assert!((0.99..=1.0).contains(&parallel_depth(i32::MAX)));
}

#[test]
fn point_sprite_parameter_layouts_select_documented_blocks() {
    let shared = [8, 6, 0, 0, 0, 4, 4, 1];
    assert_eq!(
        point_sprite_parameters(0x0500_1000, 2, &shared)
            .unwrap()
            .collect::<Vec<_>>(),
        [shared, shared]
    );

    let per_primitive = [8, 6, 0, 0, 0, 4, 4, 1, 12, 10, 1024, 4, 4, 8, 8, 0];
    for command in [0x0500_2000, 0x0500_3000] {
        assert_eq!(
            point_sprite_parameters(command, 2, &per_primitive)
                .unwrap()
                .collect::<Vec<_>>(),
            [shared, [12, 10, 1024, 4, 4, 8, 8, 0]]
        );
    }

    // Callers may retain a larger scratch array. Only the submitted blocks
    // are validated and yielded, including for a shared-parameter batch.
    let mut trailing = shared.to_vec();
    trailing.extend([-1; 8]);
    for (command, count) in [(0x0500_1000, 2), (0x0500_2000, 1), (0x0500_3000, 1)] {
        assert_eq!(
            point_sprite_parameters(command, count, &trailing)
                .unwrap()
                .collect::<Vec<_>>(),
            vec![shared; count]
        );
    }
}

#[test]
fn point_sprite_rotation_uses_the_public_trig_scale_without_magnifying_billboards() {
    let layout = FigureLayoutState {
        center: [16, 16],
        projection: Projection::ParallelScale,
        ..FigureLayoutState::default()
    };
    let screen_bounds = |angle| {
        let vertices = point_sprite_vertices(
            Vector3D::new(0, 0, 0),
            [8, 6, angle, 0, 0, 1, 1, 1],
            &PreparedProjection::new(&layout, 16, 16, 32, 32).unwrap(),
            AffineTrans::IDENTITY,
            None,
            32,
            32,
        )
        .unwrap();
        let positions =
            vertices.map(|vertex| projected_screen_position(vertex.position, 32, 32).unwrap());
        let xs = positions.map(|position| position[0]);
        let ys = positions.map(|position| position[1]);
        [
            xs.into_iter().fold(f64::INFINITY, f64::min),
            xs.into_iter().fold(f64::NEG_INFINITY, f64::max),
            ys.into_iter().fold(f64::INFINITY, f64::min),
            ys.into_iter().fold(f64::NEG_INFINITY, f64::max),
        ]
    };

    let unrotated = screen_bounds(0);
    assert!((unrotated[1] - unrotated[0] - 8.0).abs() < 1.0e-5);
    assert!((unrotated[3] - unrotated[2] - 6.0).abs() < 1.0e-5);

    let quarter_turn = screen_bounds(1024);
    assert!((quarter_turn[1] - quarter_turn[0] - 6.0).abs() < 1.0e-5);
    assert!((quarter_turn[3] - quarter_turn[2] - 8.0).abs() < 1.0e-5);
}

#[test]
fn point_sprites_render_textured_screen_aligned_quads() {
    let background = vec![0xff00_0000; 32 * 32];
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Texture(TextureData {
                width: 2,
                height: 2,
                pixels: vec![0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff].into(),
                for_model: true,
                color_key: 0,
            }),
        )
        .unwrap();
    runtime
        .render_primitives(
            Some(1),
            0,
            0,
            FigureLayoutState {
                center: [16, 16],
                projection: Projection::ParallelScale,
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(
                0x0500_1000,
                1,
                &[0, 0, 0],
                &[],
                &[8, 6, 0, 0, 0, 2, 2, 1],
                &[],
            ),
        )
        .unwrap();

    for color in [0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff] {
        assert_eq!(
            runtime
                .target_pixels()
                .iter()
                .filter(|&&pixel| pixel == color)
                .count(),
            12
        );
    }
    assert_eq!(runtime.metrics().rasterized_triangles, 2);
    assert_eq!(runtime.metrics().shaded_pixels, 48);
}

#[test]
fn zero_sized_point_sprite_slots_are_invisible() {
    let background = vec![0xff00_0000; 32 * 32];
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Texture(TextureData {
                width: 2,
                height: 2,
                pixels: vec![0xffff_ffff; 4].into(),
                for_model: true,
                color_key: 0,
            }),
        )
        .unwrap();

    runtime
        .render_primitives(
            Some(1),
            0,
            0,
            FigureLayoutState {
                center: [16, 16],
                projection: Projection::ParallelScale,
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(
                0x0500_2000,
                2,
                &[0, 0, 0, 0, 0, 0],
                &[],
                &[0, 0, 0, 0, 0, 0, 0, 2, 8, 6, 0, 0, 0, 2, 2, 1],
                &[],
            ),
        )
        .unwrap();

    assert_ne!(runtime.target_pixels(), background);
    assert_eq!(runtime.metrics().rasterized_triangles, 2);
    assert!(runtime.metrics().shaded_pixels > 0);
}

#[test]
fn negative_point_sprite_dimensions_remain_invalid() {
    let error = point_sprite_parameters(0x0500_1000, 1, &[-1, 8, 0, 0, 0, 1, 1, 1])
        .err()
        .unwrap();

    assert_eq!(error.code(), "point-sprite-size");
}

#[test]
fn point_sprite_validation_is_bounded_and_precedes_rendering() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    let background = vec![0xff00_0000; 32 * 32];
    runtime.load_target(32, 32, &background).unwrap();
    let error = runtime
        .render_primitives(
            None,
            0,
            0,
            FigureLayoutState::default(),
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(
                0x0500_2000,
                2,
                &[0, 0, 0, 1, 1, 1],
                &[],
                &[8, 8, 0, 0, 0, 1, 1, 1],
                &[],
            ),
        )
        .unwrap_err();

    assert_eq!(error.code(), "point-sprite-parameters");
    assert_eq!(runtime.target_pixels(), background);
    assert_eq!(runtime.metrics().render_calls, 0);

    let error = runtime
        .render_primitives(
            None,
            0,
            0,
            FigureLayoutState::default(),
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(
                0x0500_2000,
                2,
                &[0, 0, 0, 1, 1, 1],
                &[],
                &[8, 8, 0, 0, 0, 1, 1, 1, 8, 8, 0, 0, 0, 1, 1, 4],
                &[],
            ),
        )
        .unwrap_err();
    assert_eq!(error.code(), "point-sprite-flags");
    assert_eq!(runtime.target_pixels(), background);
    assert_eq!(runtime.metrics().render_calls, 0);
}

#[test]
fn point_sprite_on_perspective_eye_plane_is_clipped_without_failing_frame() {
    let background = vec![0xff00_0000; 32 * 32];
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime.load_target(32, 32, &background).unwrap();

    runtime
        .render_primitives(
            None,
            0,
            0,
            FigureLayoutState {
                center: [16, 16],
                projection: Projection::PerspectiveFov {
                    near: 32,
                    far: 32_764,
                    angle: 512,
                },
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(
                0x0500_1000,
                1,
                &[0, 0, 0],
                &[],
                &[8, 8, 0, 0, 0, 1, 1, 1],
                &[],
            ),
        )
        .unwrap();

    assert_eq!(runtime.target_pixels(), background);
    assert_eq!(runtime.metrics().render_calls, 1);
    assert_eq!(runtime.metrics().rasterized_triangles, 0);
}

#[test]
#[ignore = "manual point sprite projection throughput measurement"]
fn point_sprite_batch_throughput() {
    for projection in [
        Projection::ParallelScale,
        Projection::PerspectiveFov {
            near: 1,
            far: 4096,
            angle: 512,
        },
    ] {
        for (flags, shared) in [(0, true), (1, true), (0, false), (1, false)] {
            let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
            let background = vec![0xff00_0000; 32 * 32];
            let layout = FigureLayoutState {
                scale: [4096, 4096],
                center: [16, 16],
                projection,
                ..FigureLayoutState::default()
            };
            let coordinates: Vec<_> = (0..128)
                .flat_map(|index| [index % 16 * 2 - 16, index / 16 * 4 - 16, 64 + index])
                .collect();
            let parameters: Vec<_> = (0..if shared { 1 } else { 128 })
                .flat_map(|index| [3, 5, 257 + index, 0, 0, 1, 1, flags])
                .collect();
            let command = if shared { 0x0500_1000 } else { 0x0500_3000 };
            let started = std::time::Instant::now();
            for _ in 0..256 {
                runtime.load_target(32, 32, &background).unwrap();
                runtime
                    .render_primitives(
                        None,
                        0,
                        0,
                        layout.clone(),
                        AffineTrans::IDENTITY,
                        EffectState::default(),
                        PrimitiveData::new(command, 128, &coordinates, &[], &parameters, &[]),
                    )
                    .unwrap();
                std::hint::black_box(runtime.target_pixels());
            }
            let checksum = runtime
                .target_pixels()
                .iter()
                .chain(runtime.renderer.depth())
                .fold(0_u64, |sum, value| {
                    sum.wrapping_mul(31).wrapping_add(u64::from(*value))
                });
            eprintln!(
                "sprites projection={projection:?} flags={flags} shared={shared} elapsed={:?} checksum={checksum:016x}",
                started.elapsed()
            );
        }
    }
}

#[test]
fn inactive_point_sprite_batches_do_not_require_a_valid_projection() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    let background = vec![0xff12_3456; 32 * 32];
    runtime.load_target(32, 32, &background).unwrap();
    let layout = FigureLayoutState {
        projection: Projection::Parallel {
            width: 0,
            height: 0,
        },
        ..FigureLayoutState::default()
    };
    runtime
        .render_primitives(
            None,
            0,
            0,
            layout.clone(),
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(0x0500_1000, 2, &[0; 6], &[], &[0, 0, 0, 0, 0, 0, 0, 1], &[]),
        )
        .unwrap();
    assert_eq!(runtime.target_pixels(), background);
    assert_eq!(runtime.metrics().rasterized_triangles, 0);
    let error = runtime
        .render_primitives(
            None,
            0,
            0,
            layout,
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(0x0500_1000, 1, &[0; 3], &[], &[1, 1, 0, 0, 0, 0, 0, 1], &[]),
        )
        .unwrap_err();
    assert_eq!(error.code(), "projection");
    assert_eq!(runtime.target_pixels(), background);
}
