use super::*;

#[test]
fn physical_insets_are_scaled_and_clamped_to_the_viewport() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0));
    let inset = inset_content_rect(
        rect,
        PlatformInsets {
            left: 20,
            top: 40,
            right: 60,
            bottom: 80,
        },
        2.0,
    );
    assert_eq!(
        inset,
        Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(470.0, 360.0))
    );

    let clamped = inset_content_rect(
        rect,
        PlatformInsets {
            left: u32::MAX,
            top: u32::MAX,
            right: u32::MAX,
            bottom: u32::MAX,
        },
        1.0,
    );
    assert!(clamped.width() >= 0.0 && clamped.height() >= 0.0);
}

#[test]
fn automatic_fit_uses_the_full_available_scale() {
    assert!((frame_scale_for_fit(GameScale::AutomaticFit, 4.75) - 4.75).abs() < f32::EPSILON);
    assert!((frame_scale_for_fit(GameScale::AutomaticFit, 1.875) - 1.875).abs() < f32::EPSILON);
    assert!((frame_scale_for_fit(GameScale::AutomaticFit, 0.75) - 0.75).abs() < f32::EPSILON);
}

#[test]
fn manual_scale_is_still_capped_to_the_available_area() {
    let below_fit = frame_scale_for_fit(GameScale::Manual { percent: 150 }, 1.875);
    assert!((below_fit - 1.5).abs() < f32::EPSILON);

    let above_fit = frame_scale_for_fit(GameScale::Manual { percent: 200 }, 1.875);
    assert!((above_fit - 1.875).abs() < f32::EPSILON);
}

#[test]
fn collapsed_or_tiny_viewports_never_force_the_canvas_outside_its_bounds() {
    for size in [
        Vec2::ZERO,
        Vec2::new(0.0, 100.0),
        Vec2::new(100.0, 0.0),
        Vec2::splat(0.5),
    ] {
        let available = Rect::from_min_size(Pos2::new(12.0, 20.0), size);
        for scale in [GameScale::AutomaticFit, GameScale::Manual { percent: 800 }] {
            let canvas = crate::layout::game_canvas_rect(available, (240, 320), scale);
            assert!(
                available.contains_rect(canvas),
                "{canvas:?} outside {available:?}"
            );
        }
    }
}

#[test]
fn small_keypad_canvases_use_the_240_by_320_presentation_slot() {
    let available = Vec2::new(500.0, 920.0);
    let small = gameplay_layout(available, (128, 160), false, false);
    let square = gameplay_layout(available, (130, 130), false, false);
    let reference = gameplay_layout(available, KEYPAD_REFERENCE_PORTRAIT, false, false);

    assert_eq!(keypad_slot_dimensions((128, 160)), (240, 320));
    assert_eq!(keypad_slot_dimensions((130, 130)), (240, 320));
    assert!((small.game_height - reference.game_height).abs() < f32::EPSILON);
    assert!((square.game_height - reference.game_height).abs() < f32::EPSILON);
    assert!((small.controls_height - reference.controls_height).abs() < f32::EPSILON);
    let fitted_reference_height = available.x * 320.0 / 240.0;
    assert!(small.game_height <= fitted_reference_height);
    assert!(small.controls_height >= GAMEPLAY_MIN_CONTROLS_HEIGHT);

    for (width, height) in [(128.0, 160.0), (130.0, 130.0)] {
        let fit = (available.x / width).min(small.game_height / height);
        let scale = frame_scale_for_fit(GameScale::AutomaticFit, fit);
        assert!(width * scale <= available.x);
        assert!(height * scale <= small.game_height);
    }
}

#[test]
fn keypad_reference_slot_follows_orientation_and_does_not_shrink_large_frames() {
    assert_eq!(keypad_slot_dimensions((160, 128)), (320, 240));
    assert_eq!(keypad_slot_dimensions((240, 320)), (240, 320));
    assert_eq!(keypad_slot_dimensions((480, 800)), (480, 800));
    assert_eq!(keypad_slot_dimensions((0, 0)), (0, 0));
}

#[test]
fn portrait_keypad_layout_reserves_the_automatic_fit_slot() {
    let available = Vec2::new(453.0, 920.0);
    let layout = gameplay_layout(available, (240, 320), false, false);

    let width_limited_height = available.x * 320.0 / 240.0;
    assert!(layout.game_height <= width_limited_height);
    assert!(layout.controls_height >= GAMEPLAY_MIN_CONTROLS_HEIGHT);
    assert!((layout.game_height + layout.gap + layout.controls_height - available.y).abs() < 0.01);
}

#[test]
fn keypad_layout_keeps_minimum_controls_when_height_limits_the_frame() {
    let available = Vec2::new(320.0, 500.0);
    let layout = gameplay_layout(available, (240, 320), false, false);
    let expected_controls = GAMEPLAY_MIN_CONTROLS_HEIGHT;

    assert!((layout.controls_height - expected_controls).abs() < 0.01);
    assert!((layout.game_height - (492.0 - expected_controls)).abs() < 0.01);
    assert!((layout.game_height + layout.gap + layout.controls_height - available.y).abs() < 0.01);
}

#[test]
fn touch_layout_still_gives_the_whole_area_to_the_canvas() {
    let available = Vec2::new(320.0, 640.0);
    let layout = gameplay_layout(available, (240, 320), true, false);

    assert!((layout.game_height - available.y).abs() < f32::EPSILON);
    assert!(layout.controls_height.abs() < f32::EPSILON);
    assert!(layout.gap.abs() < f32::EPSILON);
}
