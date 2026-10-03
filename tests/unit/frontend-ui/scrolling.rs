use super::*;

mod editors;
#[path = "scrolling/motion.rs"]
mod motion;
mod popups;

fn pointer(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn paint(ctx: &egui::Context, events: Vec<Event>, slider: Option<&mut f32>) -> (f32, bool) {
    paint_at(ctx, events, slider, None)
}

fn paint_at(
    ctx: &egui::Context,
    events: Vec<Event>,
    slider: Option<&mut f32>,
    time: Option<f64>,
) -> (f32, bool) {
    let mut offset = 0.0;
    let mut clicked = false;
    let mut slider = slider;
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 250.0))),
            events,
            time,
            ..Default::default()
        },
        |ui| {
            let response = crate::scrolling::vertical().show(ui, |ui| {
                if let Some(value) = slider.as_deref_mut() {
                    ui.add(egui::Slider::new(value, 0.0..=100.0).show_value(false));
                }
                for index in 0..30 {
                    clicked |= ui
                        .add_sized([340.0, 48.0], egui::Button::new(format!("Item {index}")))
                        .clicked();
                }
            });
            offset = response.state.offset.y;
        },
    )
    .drop_without_applying_deltas();
    (offset, clicked)
}

#[test]
fn mouse_emulated_touch_swipes_scroll_buttons_without_activating_them() {
    let ctx = egui::Context::default();
    for _ in 0..3 {
        paint(&ctx, vec![], None);
    }
    let mut pos = Pos2::new(120.0, 190.0);
    paint(
        &ctx,
        vec![Event::PointerMoved(pos), pointer(pos, true)],
        None,
    );
    let mut offset = 0.0;
    for _ in 0..5 {
        pos.y -= 25.0;
        let result = paint(&ctx, vec![Event::PointerMoved(pos)], None);
        offset = result.0;
        assert!(!result.1);
    }
    assert!(offset > 50.0, "swipe must move the list: {offset}");
    assert!(!ctx.input(egui::InputState::has_touch_screen));
    assert!(!paint(&ctx, vec![pointer(pos, false)], None).1);
    paint(&ctx, vec![Event::PointerCancelled], None);
    // A later stationary tap still activates the same ordinary button type.
    paint(
        &ctx,
        vec![Event::PointerMoved(pos), pointer(pos, true)],
        None,
    );
    assert!(paint(&ctx, vec![pointer(pos, false)], None).1);
}

#[test]
fn draggable_child_keeps_its_gesture_in_a_scrollable_page() {
    let ctx = egui::Context::default();
    let mut value = 0.0;
    for _ in 0..3 {
        paint(&ctx, vec![], Some(&mut value));
    }
    let mut pos = Pos2::new(15.0, 18.0);
    paint(
        &ctx,
        vec![Event::PointerMoved(pos), pointer(pos, true)],
        Some(&mut value),
    );
    for _ in 0..4 {
        pos += Vec2::new(15.0, -4.0);
        let (offset, clicked) = paint(&ctx, vec![Event::PointerMoved(pos)], Some(&mut value));
        assert!(offset.abs() < f32::EPSILON);
        assert!(!clicked);
    }
    assert!(value > 10.0);
    paint(&ctx, vec![Event::PointerCancelled], Some(&mut value));
    assert!(ctx.dragged_id().is_none());
}

#[test]
fn cancelled_and_stationary_drags_do_not_start_kinetic_scrolling() {
    for cancelled in [true, false] {
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut paint = |events, elapsed| {
            time += elapsed;
            paint_at(&ctx, events, None, Some(time))
        };
        for _ in 0..3 {
            paint(vec![], 0.016);
        }
        let mut pos = Pos2::new(120.0, 190.0);
        paint(vec![Event::PointerMoved(pos), pointer(pos, true)], 0.016);
        for _ in 0..5 {
            pos.y -= 20.0;
            paint(vec![Event::PointerMoved(pos)], 0.016);
        }
        let (offset, _) = if cancelled {
            paint(vec![Event::PointerCancelled], 0.016)
        } else {
            // No redraws during the stationary hold: release must expire the
            // last drag sample rather than fling using its stale velocity.
            paint(vec![pointer(pos, false)], 0.5)
        };
        assert!(offset > 50.0);
        for _ in 0..20 {
            let (settled, clicked) = paint(vec![], 0.016);
            assert!((settled - offset).abs() < 0.01);
            assert!(!clicked);
        }
    }
}

#[test]
fn successive_swipes_continue_from_the_current_position() {
    for coalesced in [false, true] {
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut paint = |events| {
            time += 1.0 / 60.0;
            paint_at(&ctx, events, None, Some(time))
        };
        for _ in 0..3 {
            paint(vec![]);
        }
        for _ in 0..2 {
            let before = paint(vec![]).0;
            let origin = Pos2::new(120.0, 190.0);
            let mut pos = origin;
            let mut events = vec![Event::PointerMoved(pos), pointer(pos, true)];
            if coalesced {
                // Touch-down and the first movement may arrive in one paint.
                pos.y -= 20.0;
                events.push(Event::PointerMoved(pos));
            }
            let (offset, clicked) = paint(events);
            assert!(!clicked);
            let expected = before + if coalesced { 20.0 } else { 0.0 };
            assert!(
                (offset - expected).abs() < 0.01,
                "new touch moved from {before} to {offset}, expected {expected}"
            );
            for _ in 0..5 {
                pos.y -= 20.0;
                let (offset, clicked) = paint(vec![Event::PointerMoved(pos)]);
                assert!(!clicked);
                assert!(offset >= before);
            }
            assert!(!paint(vec![pointer(pos, false)]).1);
            for _ in 0..90 {
                paint(vec![]);
            }
        }
    }
}

fn paint_scrollbar(ctx: &egui::Context, events: Vec<Event>) -> (f32, Rect) {
    let (offset, handle, _) = paint_scrollbar_visual(ctx, events);
    (offset, handle)
}

fn paint_scrollbar_visual(ctx: &egui::Context, events: Vec<Event>) -> (f32, Rect, Color32) {
    let mut offset = 0.0;
    let mut viewport = Rect::NOTHING;
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 250.0))),
            events,
            ..Default::default()
        },
        |ui| {
            let scroll = crate::scrolling::vertical()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.set_min_size(Vec2::new(380.0, 2000.0));
                });
            viewport = scroll.inner_rect;
            offset = scroll.state.offset.y;
        },
    );
    let handle = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.rect.right() >= 370.0
                    && rect.rect.width() <= 10.1
                    && rect.rect.height() < viewport.height() =>
            {
                Some((rect.rect, rect.fill))
            }
            _ => None,
        })
        .next_back();
    output.drop_without_applying_deltas();
    let (handle, color) = handle.expect("a scrollbar handle");
    (offset, handle, color)
}

#[test]
fn released_scrollbar_shrinks_even_when_the_emulated_mouse_stays_over_it() {
    let ctx = egui::Context::default();
    for _ in 0..30 {
        paint_scrollbar(&ctx, vec![Event::PointerMoved(Pos2::new(100.0, 100.0))]);
    }
    let (_, initial) = paint_scrollbar(&ctx, vec![]);
    let start = initial.center();
    paint_scrollbar(&ctx, vec![Event::PointerMoved(start), pointer(start, true)]);
    let end = start + Vec2::new(0.0, 40.0);
    paint_scrollbar(&ctx, vec![Event::PointerMoved(end)]);
    assert!(ctx.dragged_id().is_some());
    for _ in 0..30 {
        paint_scrollbar(&ctx, vec![]);
    }
    assert!(paint_scrollbar(&ctx, vec![]).1.width() > initial.width() + 1.0);
    let offset = paint_scrollbar(&ctx, vec![pointer(end, false)]).0;
    assert!(ctx.dragged_id().is_none());
    for _ in 0..40 {
        let (settled, handle) = paint_scrollbar(&ctx, vec![]);
        assert!((settled - offset).abs() < 0.01);
        assert!((handle.height() - initial.height()).abs() < 0.01);
    }
    let (_, released) = paint_scrollbar(&ctx, vec![]);
    assert!(
        (released.width() - initial.width()).abs() < 0.1,
        "released scrollbar remains expanded: {released:?}, initial: {initial:?}"
    );
    // A real mouse can hover the bar again after moving, without clicking.
    paint_scrollbar(&ctx, vec![Event::PointerMoved(end - Vec2::new(1.0, 0.0))]);
    for _ in 0..30 {
        paint_scrollbar(&ctx, vec![]);
    }
    assert!(paint_scrollbar(&ctx, vec![]).1.width() > released.width() + 1.0);
}

#[test]
fn edge_back_gesture_does_not_jump_the_scrollbar_on_press_drag_or_cancel() {
    for on_handle in [true, false] {
        let ctx = egui::Context::default();
        for _ in 0..30 {
            paint_scrollbar(&ctx, vec![]);
        }
        let (baseline, handle) = paint_scrollbar(&ctx, vec![]);
        let start = Pos2::new(
            handle.center().x,
            if on_handle { handle.center().y } else { 180.0 },
        );
        let (offset, _) =
            paint_scrollbar(&ctx, vec![Event::PointerMoved(start), pointer(start, true)]);
        assert!(
            (offset - baseline).abs() < 0.01,
            "a press is not yet a scrollbar gesture"
        );
        for step in 1_u16..=5 {
            let pos = start + Vec2::new(-20.0 * f32::from(step), f32::from(step));
            let (offset, _) = paint_scrollbar(&ctx, vec![Event::PointerMoved(pos)]);
            assert!(
                (offset - baseline).abs() < 0.01,
                "a horizontal edge gesture must not drag the bar"
            );
        }
        let (offset, _) = paint_scrollbar(&ctx, vec![Event::PointerCancelled]);
        assert!((offset - baseline).abs() < 0.01);
        assert!(ctx.dragged_id().is_none());
        for _ in 0..10 {
            assert!((paint_scrollbar(&ctx, vec![]).0 - baseline).abs() < 0.01);
        }
        // A later deliberate click on the track still works.
        let track = Pos2::new(handle.center().x, 180.0);
        assert!(
            (paint_scrollbar(&ctx, vec![Event::PointerMoved(track), pointer(track, true)]).0
                - baseline)
                .abs()
                < 0.01
        );
        assert!(paint_scrollbar(&ctx, vec![pointer(track, false)]).0 > baseline + 100.0);
    }
}

#[test]
fn edge_back_does_not_flash_or_expand_the_scrollbar() {
    for native_touch in [false, true] {
        let ctx = egui::Context::default();
        for _ in 0..30 {
            paint_scrollbar_visual(&ctx, vec![]);
        }
        let (offset, handle, color) = paint_scrollbar_visual(&ctx, vec![]);
        let start = Pos2::new(handle.center().x, 180.0);
        let touch = |phase, pos| Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(1),
            phase,
            pos,
            force: None,
        };
        let mut frames = vec![vec![Event::PointerMoved(start), pointer(start, true)]];
        if native_touch {
            frames[0].push(touch(egui::TouchPhase::Start, start));
        }
        // Include a stationary press: Back has not been recognized yet.
        frames.extend((0..4).map(|_| vec![]));
        for step in 1_u16..=5 {
            let pos = start + Vec2::new(-20.0 * f32::from(step), f32::from(step));
            let mut events = vec![Event::PointerMoved(pos)];
            if native_touch {
                events.push(touch(egui::TouchPhase::Move, pos));
            }
            frames.push(events);
        }
        let mut cancelled = vec![Event::PointerCancelled];
        if native_touch {
            cancelled.push(touch(egui::TouchPhase::Cancel, start));
        }
        frames.push(cancelled);
        frames.extend((0..20).map(|_| vec![]));
        for events in frames {
            let (actual_offset, actual_handle, actual_color) = paint_scrollbar_visual(&ctx, events);
            assert!((actual_offset - offset).abs() < 0.01);
            assert!((actual_handle.width() - handle.width()).abs() < 0.1);
            assert_eq!(actual_color, color, "Back must not activate the scrollbar");
        }
    }
}
