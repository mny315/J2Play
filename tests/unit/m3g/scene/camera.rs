use super::*;

#[test]
fn parallel_projection_avoids_overflow_in_finite_width_and_depth_ranges() {
    let projection = CameraProjection::Parallel {
        height: f32::MAX,
        aspect_ratio: 2.0,
        near: f32::MAX / 2.0,
        far: f32::MAX,
    }
    .render_matrix()
    .unwrap();
    assert!(projection.as_array().iter().all(|value| value.is_finite()));
    assert!(projection.as_array()[0] > 0.0);
    assert!((projection.as_array()[14] + 3.0).abs() < 1.0e-6);
}

#[test]
fn perspective_accepts_maximum_far_plane_without_intermediate_overflow() {
    let projection = CameraProjection::Perspective {
        field_of_view: 90.0,
        aspect_ratio: 0.75,
        near: 0.5,
        far: f32::MAX,
    }
    .render_matrix()
    .unwrap();

    assert!(projection.as_array().iter().all(|value| value.is_finite()));
    assert_eq!(projection.as_array()[10], -1.0);
    assert_eq!(projection.as_array()[14], -1.0);
}
