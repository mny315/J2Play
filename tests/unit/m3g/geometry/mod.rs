use super::*;

#[test]
fn checkpoint_checks_vertex_attributes_and_triangle_storage_before_use() {
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(
            Some(VertexArrayState::new(3, 3, VertexComponent::Short).unwrap()),
            1.0,
            [0.0; 3],
        )
        .unwrap();
    buffer
        .set_normals(Some(
            VertexArrayState::new(3, 3, VertexComponent::Short).unwrap(),
        ))
        .unwrap();
    buffer.validate_checkpoint().unwrap();
    for corruption in 0..4 {
        let mut invalid = buffer.clone();
        match corruption {
            0 => {
                invalid.normals =
                    Some(VertexArrayState::new(2, 3, VertexComponent::Short).unwrap());
            }
            1 => {
                invalid.normals =
                    Some(VertexArrayState::new(3, 2, VertexComponent::Short).unwrap());
            }
            2 => invalid.positions.as_mut().unwrap().1 = f32::NAN,
            _ => {
                invalid.colors = Some(VertexArrayState::new(3, 3, VertexComponent::Short).unwrap());
            }
        }
        assert!(invalid.validate_checkpoint().is_err());
    }
    for indices in [vec![0, 1, 2, 3], vec![0, 0, 1, 2]] {
        let valid = TriangleStripArrayState::new(indices, vec![4]).unwrap();
        let restored: TriangleStripArrayState =
            save_state::decode(&save_state::encode(&valid).unwrap()).unwrap();
        restored.validate_checkpoint().unwrap();
        assert_eq!(
            restored.triangles(4).unwrap().count(),
            restored.triangle_count
        );
        for corruption in 0..3 {
            let mut invalid = valid.clone();
            match corruption {
                0 => invalid.strips = Arc::from([5_usize]),
                1 => invalid.strips = Arc::from([usize::MAX]),
                _ => invalid.triangle_count += 1,
            }
            let limits = crate::ArenaLimits::default();
            let mut runtime = crate::Runtime::new_with_graph_depth(limits, 32);
            runtime
                .create(None, crate::ObjectKind::TriangleStripArray(invalid))
                .unwrap();
            assert!(
                runtime
                    .validate_checkpoint(limits, 32, 1024, |_| true)
                    .is_err()
            );
        }
    }
}

mod lighting;
mod morphing;
mod vertices;

fn set_test_attribute(
    buffer: &mut VertexBufferState,
    slot: usize,
    count: usize,
) -> Result<(), EmuError> {
    let array =
        (count != 0).then(|| VertexArrayState::new(count, 3, VertexComponent::Byte).unwrap());
    match slot {
        0 => buffer.set_positions(array, 1.0, [0.0; 3]),
        1 => buffer.set_normals(array),
        2 => buffer.set_colors(array),
        _ => buffer.set_texture_coordinates(slot - 3, array, 1.0, [0.0; 3]),
    }
}

#[test]
fn replacing_the_only_vertex_attribute_can_change_its_length() {
    for slot in 0..5 {
        let mut buffer = VertexBufferState::default();
        set_test_attribute(&mut buffer, slot, 1).unwrap();
        set_test_attribute(&mut buffer, slot, 2).unwrap();
        assert_eq!(buffer.vertex_count(), 2);
        let other = (slot + 1) % 5;
        set_test_attribute(&mut buffer, other, 2).unwrap();
        let before = buffer.clone();
        assert!(set_test_attribute(&mut buffer, slot, 3).is_err());
        assert_eq!(buffer, before);
        set_test_attribute(&mut buffer, other, 0).unwrap();
        set_test_attribute(&mut buffer, slot, 3).unwrap();
        assert_eq!(buffer.vertex_count(), 3);
        set_test_attribute(&mut buffer, slot, 0).unwrap();
        assert_eq!(buffer.vertex_count(), 0);
    }
}

#[test]
fn vertex_colors_require_bytes_and_failed_assignment_preserves_state() {
    let mut buffer = VertexBufferState::default();
    set_test_attribute(&mut buffer, 2, 1).unwrap();
    let before = buffer.clone();
    assert!(
        buffer
            .set_colors(Some(
                VertexArrayState::new(1, 3, VertexComponent::Short).unwrap()
            ))
            .is_err()
    );
    assert_eq!(buffer, before);
}

#[test]
fn zero_normal_components_decode_to_a_nonzero_direction_before_lighting() {
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(
            Some(VertexArrayState::new(1, 3, VertexComponent::Byte).unwrap()),
            1.0,
            [0.0; 3],
        )
        .unwrap();
    for component_type in [VertexComponent::Byte, VertexComponent::Short] {
        buffer
            .set_normals(Some(VertexArrayState::new(1, 3, component_type).unwrap()))
            .unwrap();
        let vertices = buffer
            .transformed_lit_vertices(
                Mat4::IDENTITY,
                Mat4::IDENTITY,
                Vec3::new(0.0, 0.0, 1.0),
                MaterialState {
                    diffuse: u32::MAX,
                    ..Default::default()
                },
                false,
                &[LightSource::Directional {
                    direction: Vec3::new(0.0, 0.0, 1.0),
                    color: 0x00ff_ffff,
                    intensity: 1.0,
                }],
                1.0,
            )
            .unwrap();
        // Integer zero decodes to a small positive component on each axis;
        // its normalized direction is (1,1,1), not a zero vector.
        assert_eq!(vertices[0].color, 0xff93_9393);
    }
}

#[test]
fn vertex_attributes_validate_counts_and_transform() {
    let mut positions = VertexArrayState::new(3, 3, VertexComponent::Short).unwrap();
    positions
        .set_shorts(0, 3, &[-1, -1, 0, 1, -1, 0, 0, 1, 0])
        .unwrap();
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(Some(positions), 1.0, [0.0; 3])
        .unwrap();
    let vertices = buffer.transformed_vertices(Mat4::IDENTITY).unwrap();
    assert_eq!(vertices.len(), 3);
    assert_eq!(vertices[2].position, Vec4::new(0.0, 1.0, 0.0, 1.0));

    let mismatch = VertexArrayState::new(4, 3, VertexComponent::Byte).unwrap();
    assert_eq!(
        buffer.set_normals(Some(mismatch)).unwrap_err().code(),
        "attribute-count"
    );

    let mut oversized_source = VertexArrayState::new(1, 3, VertexComponent::Short).unwrap();
    oversized_source
        .set_shorts(0, 1, &[4, 5, 6, 7, 8, 9])
        .unwrap();
    assert_eq!(oversized_source.component(0, 2).unwrap(), 6);
}

#[test]
fn strips_expand_with_alternating_winding_and_bounds_checks() {
    let strips = TriangleStripArrayState::new(vec![0, 1, 2, 3], vec![4]).unwrap();
    assert_eq!(
        strips.triangles(4).unwrap().collect::<Vec<_>>(),
        [[0, 1, 2], [2, 1, 3]]
    );
    assert_eq!(strips.triangles(3).unwrap_err().code(), "index-bounds");
    let oversized_source = TriangleStripArrayState::new(vec![0, 1, 2, 2], vec![3]).unwrap();
    assert_eq!(oversized_source.indices(), [0, 1, 2]);
    assert_eq!(
        oversized_source.triangles(3).unwrap().collect::<Vec<_>>(),
        [[0, 1, 2]]
    );
    assert_eq!(
        TriangleStripArrayState::new(vec![0, 1], vec![2])
            .unwrap_err()
            .code(),
        "invalid-strips"
    );
}

#[test]
fn implicit_strips_validate_size_before_expanding_indices() {
    for lengths in [
        vec![],
        vec![0],
        vec![1],
        vec![2],
        vec![MAX_TRIANGLE_STRIP_INDICES + 1],
        vec![MAX_TRIANGLE_STRIP_INDICES, 3],
        vec![i32::MAX as usize],
        vec![usize::MAX],
        vec![usize::MAX, 3],
    ] {
        assert_eq!(
            TriangleStripArrayState::implicit(0, lengths)
                .unwrap_err()
                .code(),
            "invalid-strips"
        );
    }
    for first in [0, 7, 65_535] {
        for lengths in [vec![3], vec![4], vec![3, 4], vec![4, 5]] {
            let count = lengths.iter().sum::<usize>();
            let explicit = TriangleStripArrayState::new(
                (first..first + count as u32).collect(),
                lengths.clone(),
            )
            .unwrap();
            assert_eq!(
                TriangleStripArrayState::implicit(first, lengths).unwrap(),
                explicit
            );
        }
    }
    let maximum = TriangleStripArrayState::implicit(0, vec![MAX_TRIANGLE_STRIP_INDICES]).unwrap();
    assert_eq!(maximum.index_count(), MAX_TRIANGLE_STRIP_INDICES);
    assert_eq!(
        maximum.triangles(MAX_TRIANGLE_STRIP_INDICES).unwrap().len(),
        MAX_TRIANGLE_STRIP_INDICES - 2
    );
    assert!(TriangleStripArrayState::implicit(u32::MAX - 1, vec![3]).is_err());
}

#[test]
fn ray_intersections_preserve_barycentric_coordinates_across_scene_scales() {
    for scale in [1.0, 1.0e-6, 1.0e-18, 1.0e18] {
        let hit = Ray::new(Vec3::new(0.0, 0.0, 2.0 * scale), Vec3::new(0.0, 0.0, -1.0))
            .unwrap()
            .intersect_triangle([
                Vec3::new(-scale, -scale, 0.0),
                Vec3::new(scale, -scale, 0.0),
                Vec3::new(0.0, scale, 0.0),
            ])
            .unwrap()
            .unwrap_or_else(|| panic!("missed triangle at scale {scale}"));
        assert!((hit.distance / scale - 2.0).abs() < 1.0e-6, "{scale}");
        assert!((hit.u - 0.25).abs() < 1.0e-6, "{scale}");
        assert!((hit.v - 0.5).abs() < 1.0e-6, "{scale}");
    }
}

#[test]
fn ray_hits_report_original_direction_units_and_triangle_facing() {
    let triangle = [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    for length in [0.25, 1.0, 4.0] {
        let ray = Ray::new(Vec3::new(0.0, 0.0, 2.0), Vec3::new(0.0, 0.0, -length)).unwrap();
        for (vertices, front) in [
            (triangle, true),
            ([triangle[0], triangle[2], triangle[1]], false),
        ] {
            let hit = ray.intersect_triangle(vertices).unwrap().unwrap();
            assert_eq!(hit.distance, 2.0 / length);
            assert_eq!(hit.front_facing, front);
        }
        assert!(ray.intersect_triangle([triangle[0]; 3]).unwrap().is_none());
    }
    for direction in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
        assert!(
            Ray::new(Vec3::new(0.0, 0.0, 2.0), direction)
                .unwrap()
                .intersect_triangle(triangle)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn ray_queries_reject_zero_directions_and_non_finite_geometry() {
    assert_eq!(
        Ray::new(Vec3::default(), Vec3::default())
            .unwrap_err()
            .code(),
        "zero-length"
    );
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let point = Vec3::new(invalid, 0.0, 0.0);
        let direction = Vec3::new(0.0, 0.0, -1.0);
        assert_eq!(Ray::new(point, direction).unwrap_err().code(), "non-finite");
        assert_eq!(
            Ray::new(Vec3::default(), point).unwrap_err().code(),
            "non-finite"
        );
        assert_eq!(
            Ray::new(Vec3::default(), direction)
                .unwrap()
                .intersect_triangle([point; 3])
                .unwrap_err()
                .code(),
            "non-finite"
        );
    }
}

#[test]
fn singular_model_only_needs_an_inverse_for_directional_lighting() {
    let mut positions = VertexArrayState::new(1, 3, VertexComponent::Short).unwrap();
    positions.set_shorts(0, 1, &[1, 2, 3]).unwrap();
    let mut normals = VertexArrayState::new(1, 3, VertexComponent::Short).unwrap();
    normals.set_shorts(0, 1, &[0, 0, i16::MAX]).unwrap();
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(Some(positions), 1.0, [0.0; 3])
        .unwrap();
    buffer.set_normals(Some(normals)).unwrap();
    let flattened = Mat4::scale(2.0, 3.0, 0.0).unwrap();
    let material = MaterialState {
        emissive: 0x0012_3456,
        ..MaterialState::default()
    };

    let vertices = buffer
        .transformed_lit_vertices(
            flattened,
            Mat4::IDENTITY,
            Vec3::new(0.0, 0.0, 1.0),
            material,
            false,
            &[],
            1.0,
        )
        .unwrap();
    assert_eq!(vertices[0].position, Vec4::new(2.0, 6.0, 0.0, 1.0));
    assert_eq!(vertices[0].color, 0xff12_3456);

    let directional = [LightSource::Directional {
        direction: Vec3::new(0.0, 0.0, 1.0),
        color: 0x00ff_ffff,
        intensity: 1.0,
    }];
    assert_eq!(
        buffer
            .transformed_lit_vertices(
                flattened,
                Mat4::IDENTITY,
                Vec3::new(0.0, 0.0, 1.0),
                material,
                false,
                &directional,
                1.0,
            )
            .unwrap_err()
            .code(),
        "singular-transform"
    );
}
