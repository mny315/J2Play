use super::*;
use eframe::App;

fn paint(app: &mut FrontendApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    let mut frame = eframe::Frame::_new_kittest();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(453.0, 600.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.logic(ui.ctx(), &mut frame);
            app.ui(ui, &mut frame);
        },
    );
    output.textures_delta.clear();
    output
}

fn touch(pos: egui::Pos2, phase: egui::TouchPhase) -> Vec<egui::Event> {
    let mut events = vec![egui::Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(1),
        phase,
        pos,
        force: None,
    }];
    if phase == egui::TouchPhase::Cancel {
        events.push(egui::Event::PointerCancelled);
    } else {
        events.push(egui::Event::PointerMoved(pos));
        if phase == egui::TouchPhase::Start {
            events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
    events
}

fn scrollbar(output: &egui::FullOutput) -> egui::Rect {
    output
        .shapes
        .iter()
        .rev()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.rect.right() > 430.0
                    && rect.rect.width() <= 10.0
                    && rect.rect.height() > 30.0 =>
            {
                Some(rect.rect)
            }
            _ => None,
        })
        .expect("settings have a vertical scrollbar")
}

#[test]
fn cancelled_touch_releases_the_scrollbar_in_the_editor_and_after_back() {
    for global in [false, true] {
        for go_back in [false, true] {
            let scratch = crate::tests::test_storage::Scratch::new();
            let root = &scratch.0;
            let mut app =
                FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
            app.startup_splash = None;
            super::navigation::open_settings_page(&mut app, global);
            let ctx = egui::Context::default();
            paint(&mut app, &ctx, vec![]);
            app.open_physical_editor();
            for _ in 0..20 {
                paint(&mut app, &ctx, vec![]);
            }
            let start = scrollbar(&paint(&mut app, &ctx, vec![])).center();
            let end = start + egui::vec2(0.0, 40.0);
            paint(&mut app, &ctx, touch(start, egui::TouchPhase::Start));
            paint(&mut app, &ctx, touch(end, egui::TouchPhase::Move));
            assert!(ctx.dragged_id().is_some(), "the touch must drag the bar");
            if go_back {
                app.navigate_back();
            }
            paint(&mut app, &ctx, touch(end, egui::TouchPhase::Cancel));
            assert!(!ctx.input(|input| input.pointer.any_down()));
            assert!(ctx.dragged_id().is_none());
            for _ in 0..40 {
                paint(&mut app, &ctx, vec![]);
            }
            let bar = scrollbar(&paint(&mut app, &ctx, vec![]));
            assert!(bar.width() <= 2.1, "cancelled bar stays expanded: {bar:?}");
            assert_eq!(app.physical_editor.is_none(), go_back);
        }
    }
}
