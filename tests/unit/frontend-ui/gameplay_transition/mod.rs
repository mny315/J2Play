use super::*;
use crate::{Pos2, VirtualControlLayout, controls::gameplay_control_geometry};

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height))
}

#[test]
fn fullscreen_reverses_from_the_visible_frame_and_finishes_exactly() {
    let now = Instant::now();
    let viewport = rect(0.0, 0.0, 453.0, 960.0);
    let normal = rect(66.0, 70.0, 300.0, 400.0);
    let full = rect(45.0, 0.0, 360.0, 480.0);
    let mut animation = GameplayTransition::default();
    animation.begin(viewport, false, false, now);
    assert_eq!(animation.canvas(normal), normal);
    animation.request();
    assert!(animation.busy());
    assert!(animation.begin(viewport, true, false, now));
    assert_eq!(animation.canvas(full), normal);
    assert_eq!(animation.toolbar().to_bits(), 1.0_f32.to_bits());
    animation.begin(viewport, true, false, now + DURATION / 2);
    let middle = animation.canvas(full);
    assert!((middle.height() - 440.0).abs() < 0.01);
    assert!((middle.aspect_ratio() - 0.75).abs() < 0.001);
    assert!((animation.toolbar() - 0.5).abs() < 0.001);
    animation.request();
    assert!(animation.begin(viewport, false, false, now + DURATION / 2));
    assert_eq!(animation.canvas(normal), middle);
    animation.begin(viewport, false, false, now + DURATION * 2);
    assert_eq!(animation.canvas(normal), normal);
    assert_eq!(animation.toolbar().to_bits(), 1.0_f32.to_bits());
    assert!(!animation.busy());
    animation.request();
    animation.begin(viewport, true, false, now + DURATION * 2);
    animation.canvas(full);
    animation.begin(viewport, true, false, now + DURATION * 3);
    assert_eq!(animation.canvas(full), full);
    assert_eq!(animation.toolbar().to_bits(), 0.0_f32.to_bits());
    assert!(!animation.busy());
}

#[test]
fn both_rotation_directions_keep_canvas_and_controls_inside_the_new_viewport() {
    let portrait = rect(12.0, 24.0, 453.0, 960.0);
    let landscape = rect(30.0, 12.0, 960.0, 453.0);
    let layout = VirtualControlLayout::default();
    for (old, new) in [(portrait, landscape), (landscape, portrait)] {
        let now = Instant::now();
        let mut animation = GameplayTransition::default();
        let geometry = |viewport: Rect| {
            let area = rect(
                viewport.left(),
                viewport.bottom() - 350.0,
                viewport.width(),
                350.0,
            );
            gameplay_control_geometry(area, &layout, None).unwrap()
        };
        let old_controls = geometry(old);
        let new_controls = geometry(new);
        let canvas =
            |viewport: Rect| Rect::from_center_size(viewport.center(), Vec2::new(180.0, 240.0));
        animation.begin(old, false, old.width() > old.height(), now);
        animation.canvas(canvas(old));
        animation.controls(old_controls);
        animation.request();
        for elapsed in [Duration::ZERO, DURATION / 2, DURATION] {
            animation.begin(new, false, new.width() > new.height(), now + elapsed);
            let current = animation.canvas(canvas(new));
            assert!(new.contains_rect(current));
            assert!((current.aspect_ratio() - 0.75).abs() < 0.001);
            let controls = animation.controls(new_controls);
            for (center, size) in controls.number_keys {
                assert!(new.contains_rect(Rect::from_center_size(center, Vec2::splat(size))));
            }
            assert!(new.contains_rect(Rect::from_center_size(
                controls.direction_pad_center,
                Vec2::splat(controls.direction_button_size * 3.0)
            )));
            if elapsed == DURATION {
                assert_eq!(controls.fire_center, new_controls.fire_center);
                assert_eq!(current, canvas(new));
                assert!(!animation.busy());
            }
        }
    }
}

#[test]
fn ignored_rotation_and_suspension_do_not_leave_an_animation_running() {
    let now = Instant::now();
    let viewport = rect(0.0, 0.0, 453.0, 960.0);
    let mut animation = GameplayTransition::default();
    animation.begin(viewport, false, false, now);
    animation.request();
    let requested = animation.requested.unwrap();
    animation.begin(viewport, false, false, requested + REQUEST_TIMEOUT);
    assert!(!animation.busy());
    animation.request();
    animation.begin(viewport, true, false, now);
    assert!(animation.busy());
    animation = GameplayTransition::default();
    animation.begin(viewport, true, false, now);
    assert!(!animation.busy());
    assert_eq!(animation.toolbar().to_bits(), 0.0_f32.to_bits());
    assert_eq!(fit_rect(rect(0.0, 0.0, 1.0, 1.0), Rect::ZERO), Rect::ZERO);
}
