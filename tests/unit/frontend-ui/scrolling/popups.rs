use super::*;

#[test]
fn dropdown_swipes_have_inertia_with_native_and_mouse_emulated_touch() {
    for native_touch in [false, true] {
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut paint = |events, dt| {
            time += dt;
            let mut owner = egui::Id::NULL;
            let mut top = 0.0;
            let mut viewport = Rect::NOTHING;
            let mut clicked = false;
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 700.0))),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    let combo = egui::ComboBox::from_id_salt("animated-dropdown")
                        .width(300.0)
                        .height(180.0)
                        .selected_text("Choose")
                        .show_ui(ui, |ui| {
                            top = ui.cursor().top();
                            viewport = ui.clip_rect();
                            for index in 0..30 {
                                clicked |= ui
                                    .add_sized(
                                        [270.0, 48.0],
                                        egui::Button::new(format!("Item {index}")),
                                    )
                                    .clicked();
                            }
                        });
                    owner = combo.response.id;
                },
            )
            .drop_without_applying_deltas();
            assert!(!clicked, "a swipe must not choose a dropdown row");
            (owner, top, viewport)
        };
        let (owner, _, _) = paint(vec![], 0.016);
        egui::Popup::open_id(&ctx, owner.with("popup"));
        for _ in 0..4 {
            paint(vec![], 0.016);
        }
        let (_, initial, viewport) = paint(vec![], 0.016);
        let mut pos = Pos2::new(viewport.center().x, viewport.bottom() - 15.0);
        let touch = |phase, pos| Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(1),
            phase,
            pos,
            force: None,
        };
        let mut events = vec![Event::PointerMoved(pos), pointer(pos, true)];
        if native_touch {
            events.insert(0, touch(egui::TouchPhase::Start, pos));
        }
        paint(events, 0.016);
        for _ in 0..6 {
            pos.y -= 15.0;
            let mut events = vec![Event::PointerMoved(pos)];
            if native_touch {
                events.insert(0, touch(egui::TouchPhase::Move, pos));
            }
            paint(events, 0.016);
        }
        let mut events = vec![pointer(pos, false)];
        if native_touch {
            events.insert(0, touch(egui::TouchPhase::End, pos));
            events.push(Event::PointerGone);
        }
        let (_, released, _) = paint(events, 0.016);
        assert!(released < initial - 50.0);
        let (_, moving, _) = paint(vec![], 0.032);
        assert!(
            moving < released - 10.0,
            "dropdown must keep moving after release"
        );
        let (_, cancelled, _) = paint(vec![Event::PointerCancelled], 0.016);
        for _ in 0..20 {
            assert!((paint(vec![], 0.016).1 - cancelled).abs() < 0.01);
        }
    }
}
