use super::*;

fn button(pos: egui::Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn touch(id: u64, phase: egui::TouchPhase, pos: egui::Pos2) -> Event {
    Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos,
        force: None,
    }
}

fn paint(ctx: &egui::Context, events: Vec<Event>) -> [egui::Response; 2] {
    let mut responses = None;
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 200.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            responses = Some([20.0, 220.0].map(|x| {
                ui.interact(
                    egui::Rect::from_min_size(egui::pos2(x, 20.0), egui::vec2(160.0, 140.0)),
                    egui::Id::new(x.to_bits()),
                    egui::Sense::click_and_drag(),
                )
            }));
        },
    )
    .drop_without_applying_deltas();
    responses.unwrap()
}

fn begin_drag(ctx: &egui::Context) -> egui::Pos2 {
    paint(ctx, vec![]);
    let start = egui::pos2(40.0, 50.0);
    paint(ctx, vec![Event::PointerMoved(start), button(start, true)]);
    let mut pos = start;
    for _ in 0..4 {
        pos.x += 20.0;
        paint(ctx, vec![Event::PointerMoved(pos)]);
    }
    assert!(ctx.dragged_id().is_some());
    assert!(ctx.input(|input| input.pointer.velocity().length() > 0.0));
    pos
}

#[test]
fn cancellation_ends_drag_without_click_or_inertia() {
    let ctx = egui::Context::default();
    begin_drag(&ctx);
    let responses = paint(&ctx, vec![Event::PointerCancelled]);
    assert!(responses[0].drag_stopped());
    assert!(!responses[0].clicked());
    assert!(ctx.dragged_id().is_none());
    ctx.input(|input| {
        assert!(!input.pointer.any_down());
        assert!(input.pointer.primary_released());
        assert!(input.pointer.press_origin().is_none());
        assert!(input.pointer.latest_pos().is_none());
        assert_eq!(input.pointer.velocity(), egui::Vec2::ZERO);
    });
    paint(&ctx, vec![]);
    assert!(ctx.dragged_id().is_none());
}

#[test]
fn cancellation_does_not_click_and_a_later_tap_works() {
    let ctx = egui::Context::default();
    paint(&ctx, vec![]);
    let pos = egui::pos2(40.0, 50.0);
    paint(&ctx, vec![Event::PointerMoved(pos), button(pos, true)]);
    assert!(!paint(&ctx, vec![Event::PointerCancelled, button(pos, false)])[0].clicked());
    assert!(!paint(&ctx, vec![Event::PointerCancelled, button(pos, false)])[0].clicked());
    paint(&ctx, vec![Event::PointerMoved(pos), button(pos, true)]);
    assert!(paint(&ctx, vec![button(pos, false), Event::PointerGone])[0].clicked());
    assert!(!ctx.input(|input| input.pointer.any_down()));
}

#[test]
fn cancellation_preserves_a_new_press_in_the_same_batch() {
    let ctx = egui::Context::default();
    let old = begin_drag(&ctx);
    let new = egui::pos2(260.0, 50.0);
    paint(
        &ctx,
        vec![
            touch(1, egui::TouchPhase::Cancel, old),
            Event::PointerCancelled,
            touch(2, egui::TouchPhase::Start, new),
            Event::PointerMoved(new),
            button(new, true),
        ],
    );
    assert!(ctx.input(|input| input.pointer.primary_down()));
    let responses = paint(&ctx, vec![button(new, false), Event::PointerGone]);
    assert!(!responses[0].clicked());
    assert!(responses[1].clicked());
}

#[test]
fn another_fingers_cancel_does_not_cancel_the_pointer_owner() {
    let ctx = egui::Context::default();
    let pos = begin_drag(&ctx);
    paint(
        &ctx,
        vec![touch(2, egui::TouchPhase::Start, egui::pos2(260.0, 50.0))],
    );
    paint(
        &ctx,
        vec![touch(2, egui::TouchPhase::Cancel, egui::pos2(260.0, 50.0))],
    );
    assert!(ctx.input(|input| input.pointer.primary_down()));
    assert!(ctx.dragged_id().is_some());
    paint(
        &ctx,
        vec![
            touch(1, egui::TouchPhase::Cancel, pos),
            Event::PointerCancelled,
        ],
    );
    assert!(!ctx.input(|input| input.pointer.any_down()));
    assert!(ctx.dragged_id().is_none());
}

#[test]
fn leaving_the_window_preserves_a_mouse_drag_until_release() {
    let ctx = egui::Context::default();
    let pos = begin_drag(&ctx);
    paint(&ctx, vec![Event::PointerGone]);
    assert!(ctx.input(|input| input.pointer.primary_down()));
    assert!(ctx.dragged_id().is_some());
    let responses = paint(&ctx, vec![Event::PointerMoved(pos), button(pos, false)]);
    assert!(responses[0].drag_stopped());
    assert!(!responses[0].clicked());
    assert!(ctx.dragged_id().is_none());
}

#[test]
fn cancellation_breaks_the_fullscreen_double_tap_sequence() {
    let region = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 120.0));
    let pos = egui::pos2(200.0, 60.0);
    let tap = [button(pos, true), button(pos, false)];
    let now = Instant::now();
    let mut gesture = FullscreenExitGesture::default();
    assert!(!gesture.process(&tap, region, now));
    assert!(!gesture.process(&[button(pos, true)], region, now));
    assert!(!gesture.process(&[Event::PointerCancelled], region, now));
    assert!(!gesture.process(&[button(pos, false)], region, now));
    assert!(!gesture.process(&tap, region, now + Duration::from_millis(100)));
    assert!(gesture.process(&tap, region, now + Duration::from_millis(200)));
}
