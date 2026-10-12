use super::*;

#[test]
fn gameplay_title_preserves_the_animated_toolbar_clip() {
    let ctx = egui::Context::default();
    let parent_clip = Rect::from_min_max(Pos2::new(80.0, 80.0), Pos2::new(300.0, 100.0));
    let title_rect = Rect::from_min_max(Pos2::new(20.0, 64.0), Pos2::new(320.0, 100.0));
    let title = "Clipped toolbar title";
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.set_clip_rect(parent_clip);
        paint_gameplay_title(ui, title_rect, title);
    });
    output.textures_delta.clear();
    let text = output
        .shapes
        .iter()
        .find(
            |shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == title),
        )
        .expect("toolbar title must be painted");
    assert_eq!(text.clip_rect, parent_clip.intersect(title_rect));
}

#[test]
fn gameplay_toolbar_reserves_one_right_aligned_row_of_icon_actions() {
    let available = 453.0;
    for (rotate, expected_title_width) in [(false, 261.0), (true, 213.0)] {
        let spacing = gameplay_toolbar_spacing(available, rotate);
        let title_width = gameplay_toolbar_title_width(available, spacing, rotate);
        assert!((title_width - expected_title_width).abs() < f32::EPSILON);
        assert!(gameplay_toolbar_title_width(160.0, spacing, rotate).abs() < f32::EPSILON);
        for width in [453.0, 260.0, 240.0] {
            assert!((gameplay_toolbar_spacing(width, rotate) - 4.0).abs() < f32::EPSILON);
        }

        let row = Rect::from_min_size(Pos2::new(20.0, 30.0), Vec2::new(available, 36.0));
        let geometry = gameplay_toolbar_geometry(row, spacing, rotate);
        assert!((geometry.title.width() - title_width).abs() < f32::EPSILON);
        assert!((geometry.title.left() - row.left()).abs() < f32::EPSILON);
        assert!((geometry.stop.right() - row.right()).abs() < f32::EPSILON);
        assert_eq!(geometry.rotate.is_some(), rotate);
        let actions: Vec<_> = [geometry.fullscreen, geometry.fast_forward, geometry.debug]
            .into_iter()
            .chain(geometry.rotate)
            .chain([geometry.stop])
            .collect();
        assert!(
            actions
                .iter()
                .all(|action| (action.center().y - row.center().y).abs() < f32::EPSILON)
        );
        assert!(
            actions
                .windows(2)
                .all(|pair| { (pair[1].left() - pair[0].right() - spacing).abs() < f32::EPSILON })
        );
    }
}

#[test]
fn debug_icon_visible_bounds_are_centered_in_both_themes() {
    for visuals in [egui::Visuals::dark(), egui::Visuals::light()] {
        let ctx = egui::Context::default();
        ctx.set_visuals(visuals);
        for height in [24.0, 36.0, 48.0] {
            let rect = Rect::from_min_size(Pos2::new(40.0, 20.0), Vec2::new(48.0, height));
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint_emulator_icon(
                    ui.painter(),
                    rect,
                    EmulatorIcon::Debug,
                    ui.visuals().text_color(),
                );
            });
            let bounds = output.shapes.iter().fold(Rect::NOTHING, |bounds, shape| {
                bounds.union(shape.shape.visual_bounding_rect())
            });
            output.drop_without_applying_deltas();
            assert!(rect.contains_rect(bounds));
            assert!((bounds.center().x - rect.center().x).abs() < 0.01);
            assert!((bounds.center().y - rect.center().y).abs() < 0.01);
        }
    }
}

#[test]
fn gameplay_icons_stay_inside_touch_targets() {
    let rect = Rect::from_center_size(Pos2::ZERO, Vec2::splat(46.0));
    for point in fullscreen_icon_segments(rect).into_iter().flatten() {
        assert!(rect.contains(point));
    }
    let radius = emulator_icon_radius(rect);
    for point in speedometer_segments(rect).into_iter().flatten() {
        assert!(point.distance(rect.center()) <= radius);
    }

    for point in debug_icon_segments(rect).into_iter().flatten() {
        assert!(rect.contains(point));
    }
    for point in rotate_icon_segments(rect).into_iter().flatten() {
        assert!(rect.contains(point));
    }
    assert!(rect.contains_rect(stop_icon_rect(rect)));
}
