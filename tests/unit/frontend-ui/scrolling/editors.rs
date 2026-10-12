use super::*;
use eframe::App;

#[test]
fn both_control_editors_pull_and_spring_back_even_when_the_page_fits() {
    for physical in [false, true] {
        let scratch = crate::tests::test_storage::Scratch::new();
        let mut app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
        app.startup_splash = None;
        app.screen = Screen::AppSettings(app.app_settings.clone().into());
        if physical {
            app.open_physical_editor();
        } else {
            app.open_control_editor();
        }
        let title = if physical {
            "Physical controls"
        } else {
            "Control layout"
        };
        let ctx = egui::Context::default();
        let mut frame = eframe::Frame::_new_kittest();
        let mut time = 0.0;
        let mut paint = |events| {
            time += 0.016;
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 1200.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| app.ui(ui, &mut frame),
            );
            let top = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == title => Some(text.pos.y),
                    _ => None,
                })
                .expect("editor title");
            output.drop_without_applying_deltas();
            top
        };
        for _ in 0..8 {
            paint(vec![]);
        }
        let baseline = paint(vec![]);
        // The header is ordinary scrollable content, outside all draggable controls.
        let mut pos = Pos2::new(12.0, baseline + 8.0);
        paint(vec![Event::PointerMoved(pos), pointer(pos, true)]);
        for _ in 0..6 {
            pos.y += 15.0;
            paint(vec![Event::PointerMoved(pos)]);
        }
        let pulled = paint(vec![]);
        assert!(
            pulled > baseline + 5.0 && pulled < baseline + 73.0,
            "physical={physical}, baseline={baseline}, pulled={pulled}"
        );
        paint(vec![pointer(pos, false)]);
        for _ in 0..80 {
            paint(vec![]);
        }
        assert!((paint(vec![]) - baseline).abs() < 1.0);
    }
}
