use super::*;
use crate::runtime::rendering::normalized_uv;

#[test]
fn owned_target_preserves_color_storage_and_matches_borrowed_depth_and_scissor() {
    for (width, height) in [(8_u32, 8_u32), (7, 3)] {
        for scissor in [[0, 0, width, height], [1, 1, 3, 2], [2, 1, 0, 1]] {
            let mut copied = Runtime::new(16, 1 << 20, 8, 8).unwrap();
            let mut owned = Runtime::new(16, 1 << 20, 8, 8).unwrap();
            let point = Vertex::new(Vec4::new(0.0, 0.0, 0.5, 1.0), 0xffff_0000);
            for runtime in [&mut copied, &mut owned] {
                runtime.renderer.draw_point(point).unwrap();
            }
            let colors: Vec<u32> = (0..width * height)
                .map(|index| index.wrapping_mul(0x1234_5679))
                .collect();
            copied
                .load_target_clipped(width, height, &colors, scissor)
                .unwrap();
            let storage = colors.as_ptr();
            owned
                .replace_target_clipped(width, height, colors, scissor)
                .unwrap();
            assert_eq!(owned.target_pixels().as_ptr(), storage);
            assert_eq!(owned.target_pixels(), copied.target_pixels());
            assert_eq!(owned.renderer.depth(), copied.renderer.depth());
            for runtime in [&mut copied, &mut owned] {
                runtime.renderer.draw_point(point).unwrap();
            }
            assert_eq!(owned.target_pixels(), copied.target_pixels());
            assert_eq!(owned.renderer.depth(), copied.renderer.depth());
            assert_eq!(owned.renderer.stats(), copied.renderer.stats());
        }
    }
}

#[test]
fn clipped_target_restricts_micro3d_color_writes() {
    let background = vec![0xff00_0000; 32 * 32];
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime
        .load_target_clipped(32, 32, &background, [8, 8, 16, 16])
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
                0x0300_0000,
                1,
                &[-20, -20, 0, 20, -20, 0, 0, 20, 0],
                &[],
                &[],
                &[0x00ff_0000],
            ),
        )
        .unwrap();

    for y in 0..32 {
        for x in 0..32 {
            if !(8..24).contains(&x) || !(8..24).contains(&y) {
                assert_eq!(runtime.target_pixels()[y * 32 + x], background[y * 32 + x]);
            }
        }
    }
    assert!(
        runtime.target_pixels()[8 * 32..24 * 32]
            .iter()
            .any(|pixel| *pixel != 0xff00_0000)
    );
}

#[test]
fn updated_target_scissor_restricts_later_micro3d_color_writes() {
    let background = vec![0xff00_0000; 32 * 32];
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    runtime.load_target(32, 32, &background).unwrap();
    runtime.set_target_scissor([12, 10, 8, 6]).unwrap();
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
                0x0300_0000,
                1,
                &[-20, -20, 0, 20, -20, 0, 0, 20, 0],
                &[],
                &[],
                &[0x00ff_0000],
            ),
        )
        .unwrap();

    for y in 0..32 {
        for x in 0..32 {
            if !(12..20).contains(&x) || !(10..16).contains(&y) {
                assert_eq!(runtime.target_pixels()[y * 32 + x], background[y * 32 + x]);
            }
        }
    }
    assert!(
        runtime.target_pixels()[10 * 32..16 * 32]
            .iter()
            .any(|pixel| *pixel != 0xff00_0000)
    );
}

#[test]
fn perspective_projection_does_not_apply_parallel_scale() {
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
                scale: [0, 0],
                center: [16, 16],
                projection: Projection::PerspectiveFov {
                    near: 1,
                    far: 1_000,
                    angle: 512,
                },
                ..FigureLayoutState::default()
            },
            AffineTrans::new([4096, 0, 0, 0, 0, 4096, 0, 0, 0, 0, 4096, 100]),
            EffectState::default(),
            PrimitiveData::new(
                0x0300_0000,
                1,
                &[-20, -20, 0, 20, -20, 0, 0, 20, 0],
                &[],
                &[],
                &[0x00ff_0000],
            ),
        )
        .unwrap();

    assert!(runtime.metrics().rasterized_triangles > 0);
    assert!(runtime.metrics().shaded_pixels > 0);
}

#[test]
fn perspective_angle_is_converted_to_a_screen_focal_length() {
    let layout = FigureLayoutState {
        projection: Projection::PerspectiveFov {
            near: 1,
            far: 1_000,
            angle: 512,
        },
        ..FigureLayoutState::default()
    };

    let position = project(Vector3D::new(100, 0, 1_000), &layout, 120, 160, 240, 320).unwrap();
    let [x, y] = screen_offset(position, 240, 320);
    assert!((28.0..=30.0).contains(&x));
    assert_eq!(y, 0.0);
}

#[test]
fn perspective_preserves_camera_space_vertical_orientation() {
    let layout = FigureLayoutState {
        projection: Projection::PerspectiveFov {
            near: 1,
            far: 1_000,
            angle: 512,
        },
        ..FigureLayoutState::default()
    };

    let position = project(Vector3D::new(0, -100, 1_000), &layout, 120, 160, 240, 320).unwrap();
    let [_, y] = screen_offset(position, 240, 320);
    assert!((-30.0..=-28.0).contains(&y));
}

#[test]
fn default_layout_matches_documented_parallel_scale() {
    let layout = FigureLayoutState::default();
    assert_eq!(layout.scale, [512, 512]);
    assert_eq!(layout.center, [0, 0]);
    assert_eq!(layout.projection, Projection::ParallelScale);

    let position = project(Vector3D::new(8, 8, 0), &layout, 120, 160, 240, 320).unwrap();
    assert_eq!(screen_offset(position, 240, 320), [1.0, 1.0]);
    assert_eq!((position.z, position.w), (0.995, 1.0));
}

#[test]
fn figure_uvs_use_bound_texture_texel_dimensions() {
    let texture = TextureData {
        width: 148,
        height: 120,
        pixels: vec![0; 148 * 120].into(),
        for_model: true,
        color_key: 0,
    };

    assert_eq!(normalized_uv([74, 60], Some(&texture)), (0.5, 0.5));
    assert_eq!(normalized_uv([148, 120], Some(&texture)), (1.0, 1.0));
    assert_eq!(
        normalized_uv([175, 175], Some(&texture)),
        (175.0 / 148.0, 175.0 / 120.0)
    );
    assert_eq!(normalized_uv([74, 60], None), (0.0, 0.0));
}

#[test]
fn projection_surface_sizes_use_their_documented_units() {
    let parallel = FigureLayoutState {
        projection: Projection::Parallel {
            width: 150,
            height: 200,
        },
        ..FigureLayoutState::default()
    };
    let position = project(Vector3D::new(75, 0, 0), &parallel, 120, 160, 240, 320).unwrap();
    assert_eq!(screen_offset(position, 240, 320), [120.0, 0.0]);

    let perspective = FigureLayoutState {
        projection: Projection::PerspectiveSize {
            near: 1_024,
            far: 4_096,
            width: 4096 * 150,
            height: 4096 * 200,
        },
        ..FigureLayoutState::default()
    };
    let position = project(
        Vector3D::new(75, 0, 1_024),
        &perspective,
        120,
        160,
        240,
        320,
    )
    .unwrap();
    assert_eq!(screen_offset(position, 240, 320), [120.0, 0.0]);
}

#[test]
fn bind_pose_applies_bone_transform_without_action() {
    let figure = FigureData {
        format_version: 5,
        uv_bits: 8,
        vertices: vec![Vector3D::new(7, -11, 13)],
        normals: Vec::new(),
        faces: Vec::new(),
        bones: vec![Bone {
            vertex_count: 1,
            parent: -1,
            transform: AffineTrans {
                values: [4096, 0, 0, 0, 0, 0, 4096, 0, 0, -4096, 0, 0],
            },
        }],
        pattern_count: 1,
        material_count: 1,
    };

    assert_eq!(
        figure_geometry(&figure, None, false).unwrap().0,
        [Vector3D::new(7, 13, 11)]
    );
}

#[test]
fn action_with_trailing_segments_animates_a_figure_skeleton_prefix() {
    let figure = FigureData {
        format_version: 5,
        uv_bits: 8,
        vertices: vec![Vector3D::new(1, 2, 3)],
        normals: Vec::new(),
        faces: Vec::new(),
        bones: vec![Bone {
            vertex_count: 1,
            parent: -1,
            transform: AffineTrans::IDENTITY,
        }],
        pattern_count: 1,
        material_count: 1,
    };
    let action = ActionData {
        frame_count: 1,
        segments: vec![
            crate::ActionSegmentData::Affine(AffineTrans::new([
                4096, 0, 0, 5, 0, 4096, 0, 0, 0, 0, 4096, 0,
            ])),
            crate::ActionSegmentData::Affine(AffineTrans::ZERO),
        ],
        pattern_keys: Vec::new(),
    };

    assert_eq!(
        figure_geometry(&figure, Some((&action, 0)), false)
            .unwrap()
            .0,
        [Vector3D::new(6, 2, 3)]
    );
}

#[test]
fn parallel_size_coordinates_reach_the_rasterizer() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    let background = vec![0xff00_0000; 32 * 32];
    runtime.load_target(32, 32, &background).unwrap();
    runtime
        .render_primitives(
            None,
            0,
            0,
            FigureLayoutState {
                center: [16, 16],
                projection: Projection::Parallel {
                    width: 32,
                    height: 32,
                },
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

    assert!(runtime.metrics().rasterized_triangles > 0);
    assert_ne!(runtime.target_pixels(), background);
}

#[test]
fn line_primitives_reach_the_rasterizer() {
    let mut runtime = Runtime::new(16, 1 << 20, 32, 32).unwrap();
    let background = vec![0xff00_0000; 32 * 32];
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
                0x0200_0400,
                1,
                &[-12, 0, 0, 12, 0, 0],
                &[],
                &[],
                &[0x00ff_ffff],
            ),
        )
        .unwrap();

    assert!(runtime.metrics().rasterized_triangles > 0);
    assert_ne!(runtime.target_pixels(), background);
}

#[test]
fn perspective_rejects_geometry_behind_the_near_plane() {
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
                    near: 10,
                    far: 1_000,
                    angle: 512,
                },
                ..FigureLayoutState::default()
            },
            AffineTrans::IDENTITY,
            EffectState::default(),
            PrimitiveData::new(
                0x0300_0000,
                1,
                &[-20, -20, 0, 20, -20, 0, 0, 20, 0],
                &[],
                &[],
                &[0x00ff_0000],
            ),
        )
        .unwrap();

    assert_eq!(runtime.metrics().rasterized_triangles, 0);
    assert_eq!(runtime.metrics().shaded_pixels, 0);
}

fn screen_offset(position: Vec4, width: u32, height: u32) -> [f32; 2] {
    let inverse_w = position.w.recip();
    [
        (position.x * inverse_w + 1.0) * 0.5 * width as f32 - width as f32 * 0.5,
        (1.0 - position.y * inverse_w) * 0.5 * height as f32 - height as f32 * 0.5,
    ]
}

fn project(
    vector: Vector3D,
    layout: &FigureLayoutState,
    center_x: i32,
    center_y: i32,
    target_width: u32,
    target_height: u32,
) -> Result<Vec4, EmuError> {
    Ok(
        PreparedProjection::new(layout, center_x, center_y, target_width, target_height)?
            .project(vector),
    )
}
