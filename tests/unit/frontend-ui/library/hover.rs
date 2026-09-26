use super::*;

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn touch_scroll_and_release_do_not_flash_the_library_hover_outline() {
    for native_touch in [false, true] {
        let ctx = egui::Context::default();
        let theme = MaterialTheme::fallback(PlatformThemeMode::Light);
        apply_material_theme(&ctx, &theme);
        let mut time = 0.0;
        let mut paint = |events| {
            time += 0.016;
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 250.0))),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    let response = ui.interact(
                        Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(300.0, 200.0)),
                        egui::Id::new("hover-row"),
                        egui::Sense::click(),
                    );
                    crate::library_view::track_launch(ui, &response, &theme);
                },
            );
            let outlined = output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(rect) if rect.stroke.color == theme.primary && (rect.stroke.width - 1.5).abs() < f32::EPSILON
            ));
            output.drop_without_applying_deltas();
            outlined
        };
        let mut pos = Pos2::new(100.0, 180.0);
        paint(vec![]);
        assert!(
            paint(vec![Event::PointerMoved(pos)]),
            "mouse hover is still visible"
        );
        let touch = |phase, pos| Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(1),
            phase,
            pos,
            force: None,
        };
        let mut events = vec![button(pos, true)];
        if native_touch {
            events.insert(0, touch(egui::TouchPhase::Start, pos));
        }
        assert!(!paint(events), "touch-down must not flash an outline");
        for _ in 0..5 {
            pos.y -= 15.0;
            let mut events = vec![Event::PointerMoved(pos)];
            if native_touch {
                events.insert(0, touch(egui::TouchPhase::Move, pos));
            }
            assert!(!paint(events));
        }
        let mut events = vec![button(pos, false)];
        if native_touch {
            events.insert(0, touch(egui::TouchPhase::End, pos));
            events.push(Event::PointerGone);
        }
        assert!(!paint(events));
        for _ in 0..40 {
            assert!(!paint(vec![]), "inertia must not restore stationary hover");
        }
        pos.x += 10.0;
        paint(vec![Event::PointerMoved(pos)]);
        assert!(
            paint(vec![]),
            "a real mouse can hover again after touch: native_touch={native_touch}"
        );
    }
}
