use super::*;

#[test]
fn drag_offsets_are_normalized_and_bounded() {
    let bounds = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(400.0, 200.0));
    let full_gesture = transform_after_drag(
        ControlTransform::default(),
        egui::Pos2::ZERO,
        egui::Pos2::ZERO,
        Vec2::new(160.0, 0.0),
        bounds,
        true,
    );
    assert_eq!(full_gesture.offset_x, 4_000);

    let moved = transform_after_drag(
        ControlTransform::default(),
        egui::Pos2::ZERO,
        egui::Pos2::ZERO,
        Vec2::new(40.0, -20.0),
        bounds,
        true,
    );
    assert_eq!(moved.offset_x, 1_000);
    assert_eq!(moved.offset_y, -1_000);
    assert_eq!(moved.size_percent, 100);

    let snapped = transform_after_drag(
        ControlTransform::default(),
        egui::Pos2::ZERO,
        egui::Pos2::ZERO,
        Vec2::new(37.0, 4.0),
        bounds,
        true,
    );
    assert_eq!(snapped.offset_x, 1_000);
    assert_eq!(snapped.offset_y, 250);

    let unsnapped = transform_after_drag(
        ControlTransform::default(),
        egui::Pos2::ZERO,
        egui::Pos2::ZERO,
        Vec2::new(37.0, 4.0),
        bounds,
        false,
    );
    assert_eq!(unsnapped.offset_x, 925);
    assert_eq!(unsnapped.offset_y, 200);

    let bounded = transform_after_drag(
        ControlTransform::default(),
        egui::Pos2::ZERO,
        egui::Pos2::ZERO,
        Vec2::new(10_000.0, -10_000.0),
        bounds,
        true,
    );
    assert_eq!(bounded.offset_x, CONTROL_POSITION_UNITS);
    assert_eq!(bounded.offset_y, -CONTROL_POSITION_UNITS);

    let from_clamped_edge = transform_after_drag(
        ControlTransform {
            offset_x: CONTROL_POSITION_UNITS,
            ..ControlTransform::default()
        },
        egui::Pos2::new(380.0, 100.0),
        egui::Pos2::new(300.0, 100.0),
        Vec2::new(-20.0, 0.0),
        bounds,
        true,
    );
    assert_eq!(from_clamped_edge.offset_x, 1_500);
}
