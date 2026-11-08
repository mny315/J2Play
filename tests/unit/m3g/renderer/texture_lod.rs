use super::*;

const SIZE: u32 = 256;

fn perspective_surface() -> ([Vertex; 4], Texture2DState) {
    let pixels: Vec<_> = (0..64)
        .map(|index| {
            if (index / 8 + index % 8) % 2 == 0 {
                0xffff_0000
            } else {
                0xff00_ff00
            }
        })
        .collect();
    let image = crate::Image2DState::from_argb(crate::ImageFormat::Rgba, 8, 8, &pixels).unwrap();
    let texture = Texture2DState::new(image);
    let vertices = [
        Vertex::textured(Vec4::new(-1.0, -1.0, 0.0, 1.0), u32::MAX, 0.0, 0.0),
        Vertex::textured(Vec4::new(1.0, -1.0, 0.0, 1.0), u32::MAX, 8.0, 0.0),
        Vertex::textured(Vec4::new(32.0, 32.0, 0.0, 32.0), u32::MAX, 256.0, 256.0),
        Vertex::textured(Vec4::new(-32.0, 32.0, 0.0, 32.0), u32::MAX, 0.0, 256.0),
    ];
    (vertices, texture)
}

fn draw_surface(
    vertices: [Vertex; 4],
    texture: Texture2DState,
    unit: usize,
    scale: u32,
    diagonal: bool,
) -> SoftwareRenderer {
    let mut renderer = SoftwareRenderer::new(SIZE, SIZE, RenderLimits::default()).unwrap();
    renderer.set_resolution_scale(scale).unwrap();
    renderer.set_texture_unit(unit, Some(texture)).unwrap();
    renderer.set_compositing(false, false, true, true);
    let vertices = vertices.map(|mut vertex| {
        vertex.texture[unit] = vertex.texture[0];
        vertex
    });
    let indices = if diagonal {
        [[0, 1, 3], [1, 2, 3]]
    } else {
        [[0, 1, 2], [0, 2, 3]]
    };
    for indices in indices {
        renderer
            .draw_triangle(indices.map(|index| vertices[index]))
            .unwrap();
    }
    renderer
}

#[test]
fn perspective_mipmaps_preserve_near_detail_and_filter_distant_samples() {
    for scale in [1, 2] {
        for unit in [0, 1] {
            for linear_levels in [false, true] {
                let (vertices, mut texture) = perspective_surface();
                texture.set_mipmap_filter(true, linear_levels);
                let renderer = draw_surface(vertices, texture.clone(), unit, scale, false);
                let size = f64::from(SIZE * scale);
                // Independent analytic mapping of this projective plane:
                // s = (8*x/size)/q, t = (8*(1-y/size))/q.
                let q_dy = (31.0 / 32.0) / size;
                for y in [0, 3, 17, 53, 111, 189, 251] {
                    for x in [7, 37, 79, 131, 199, 247] {
                        let (x, y) = (x * scale, y * scale);
                        let screen_u = (f64::from(x) + 0.5) / size;
                        let screen_v = (f64::from(y) + 0.5) / size;
                        let inverse_w = (31.0_f64 / 32.0).mul_add(screen_v, 1.0 / 32.0);
                        let s = 8.0 * screen_u / inverse_w;
                        let t = 8.0 * (1.0 - screen_v) / inverse_w;
                        let ds_dx = 8.0 / (size * inverse_w);
                        let ds_dy = -s * q_dy / inverse_w;
                        let dt_dy = (-8.0 / size - t * q_dy) / inverse_w;
                        let rho = (8.0 * ds_dx).max((8.0 * ds_dy).hypot(8.0 * dt_dy));
                        let lod = rho.max(1.0).log2();
                        assert_eq!(
                            renderer.pixels()[(y * SIZE * scale + x) as usize],
                            texture.shade_coordinates_lod([s, t, 0.0], u32::MAX, lod),
                            "scale={scale}, unit={unit}, linear_levels={linear_levels}, ({x}, {y}), lod={lod}"
                        );
                    }
                }
                assert!(renderer.pixels().contains(&0xffff_0000));
                assert!(renderer.pixels().contains(&0xff00_ff00));
                assert!(renderer.pixels().contains(&0xff80_8000));
            }
        }
    }
}

#[test]
fn mip_selection_tracks_texture_coordinate_transforms() {
    for perspective in [false, true] {
        let (mut vertices, mut texture) = perspective_surface();
        if !perspective {
            for vertex in &mut vertices {
                vertex.position.x /= vertex.position.w;
                vertex.position.y /= vertex.position.w;
                vertex.position.w = 1.0;
            }
        }
        for linear_levels in [false, true] {
            texture.set_mipmap_filter(true, linear_levels);
            let reference = draw_surface(vertices, texture.clone(), 0, 1, false);
            let rescaled = vertices.map(|mut vertex| {
                vertex.texture[0][0] *= 16.0;
                vertex.texture[0][1] *= 16.0;
                vertex
            });
            let mut transformed = texture.clone();
            transformed.set_transform(Mat4::scale(1.0 / 16.0, 1.0 / 16.0, 1.0).unwrap());
            let actual = draw_surface(rescaled, transformed, 0, 1, false);
            assert_eq!(actual.pixels(), reference.pixels());
        }
    }
}

#[test]
fn projective_texture_q_controls_perspective_mip_selection() {
    let (mut vertices, mut texture) = perspective_surface();
    vertices[2].texture[0][2] = 4.0;
    vertices[3].texture[0][2] = 4.0;
    texture.set_mipmap_filter(true, false);
    texture.set_transform(
        Mat4::from_row_major([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.25, 1.0,
        ])
        .unwrap(),
    );
    let actual = draw_surface(vertices, texture.clone(), 0, 1, false);
    for y in [0, 3, 17, 53, 111, 189, 251] {
        for x in [7, 37, 79, 131, 199, 247] {
            let screen_u = (f64::from(x) + 0.5) / f64::from(SIZE);
            let screen_v = (f64::from(y) + 0.5) / f64::from(SIZE);
            let w_inverse = (31.0_f64 / 32.0).mul_add(screen_v, 1.0 / 32.0);
            // Texture Q adds (1-v)/32 to the interpolated reciprocal W.
            let q = (15.0_f64 / 16.0).mul_add(screen_v, 1.0 / 16.0);
            let s = 8.0 * screen_u / q;
            let t = 8.0 * (1.0 - screen_v) / q;
            let ds_dx = 8.0 / (f64::from(SIZE) * q);
            let ds_dy = -s * (15.0 / 16.0) / (f64::from(SIZE) * q);
            let dt_dy = (-8.0 - t * (15.0 / 16.0)) / (f64::from(SIZE) * q);
            let lod = (8.0 * ds_dx)
                .max((8.0 * ds_dy).hypot(8.0 * dt_dy))
                .max(1.0)
                .log2();
            let coordinates = [
                8.0 * screen_u,
                8.0 * (1.0 - screen_v),
                (1.0 - screen_v) / 8.0,
            ]
            .map(|value| value / w_inverse);
            assert_eq!(
                actual.pixels()[(y * SIZE + x) as usize],
                texture.shade_coordinates_lod(coordinates, u32::MAX, lod),
                "({x}, {y}), lod={lod}"
            );
        }
    }
}

#[test]
fn perspective_mip_selection_is_independent_of_the_quad_diagonal() {
    let (vertices, mut texture) = perspective_surface();
    texture.set_mipmap_filter(true, true);
    let first = draw_surface(vertices, texture.clone(), 0, 1, false);
    let second = draw_surface(vertices, texture, 0, 1, true);
    assert_eq!(first.pixels(), second.pixels());
    assert_eq!(first.stats().shaded_fragments, u64::from(SIZE * SIZE));
    assert_eq!(first.stats(), second.stats());
}

#[test]
fn clipping_preserves_the_projective_texture_gradient() {
    let (vertices, mut texture) = perspective_surface();
    texture.set_mipmap_filter(true, false);
    let reference = draw_surface(vertices, texture.clone(), 0, 2, false);
    let enlarged = vertices.map(|mut vertex| {
        vertex.position.x *= 2.0;
        vertex.position.y *= 2.0;
        vertex
    });
    for diagonal in [false, true] {
        let actual = draw_surface(enlarged, texture.clone(), 0, 1, diagonal);
        for y in [3, 31, 73, 129, 189, 251] {
            for x in [7, 37, 79, 131, 199, 247] {
                assert_eq!(
                    actual.pixels()[(y * SIZE + x) as usize],
                    reference.pixels()[((y + SIZE / 2) * SIZE * 2 + x + SIZE / 2) as usize],
                    "diagonal={diagonal}, ({x}, {y})"
                );
            }
        }
        assert_eq!(actual.stats().clipped_triangles, 2);
        assert_eq!(actual.stats().shaded_fragments, u64::from(SIZE * SIZE));
    }
}

#[test]
#[ignore = "manual perspective mipmap throughput measurement"]
fn perspective_mipmap_throughput() {
    let (vertices, mut texture) = perspective_surface();
    texture.set_mipmap_filter(true, true);
    texture.set_linear_filter(true);
    let mut renderer = draw_surface(vertices, texture, 0, 1, false);
    let started = std::time::Instant::now();
    for _ in 0..128 {
        renderer.reset_stats();
        for indices in [[0, 1, 2], [0, 2, 3]] {
            renderer
                .draw_triangle(indices.map(|index| vertices[index]))
                .unwrap();
        }
    }
    eprintln!(
        "perspective mipmap elapsed={:?}, crc={:08x}, stats={:?}",
        started.elapsed(),
        raster_crc(&renderer),
        renderer.stats()
    );
}
