use super::*;

fn text_shape(output: &egui::FullOutput, label: &str) -> (Pos2, u8) {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => Some((
                text.pos + text.galley.rect.center().to_vec2(),
                text.fallback_color.a(),
            )),
            _ => None,
        })
        .expect(label)
}

#[test]
fn saving_and_cancelling_editors_paint_the_clicked_page_before_fading_the_parent() {
    for physical in [false, true] {
        for action in ["Save", "Cancel"] {
            let scratch = crate::tests::test_storage::Scratch::new();
            let mut app =
                FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
            app.startup_splash = None;
            let ctx = egui::Context::default();
            render(&mut app, &ctx);
            app.screen = Screen::AppSettings(app.app_settings.clone().into());
            render(&mut app, &ctx);
            if physical {
                app.open_physical_editor();
            } else {
                app.open_control_editor();
            }
            render(&mut app, &ctx);
            app.ui_motion.settle();
            let editor_title = if physical {
                "Physical controls"
            } else {
                "Control layout"
            };
            let output = render_output(&mut app, &ctx);
            let (pos, _) = text_shape(&output, action);
            output.drop_without_applying_deltas();
            for pressed in [true, false] {
                let output = render_events(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                assert_eq!(text_shape(&output, editor_title).1, 255);
                output.drop_without_applying_deltas();
            }
            assert_eq!(app.motion_page(), Page::AppSettings);
            let output = render_output(&mut app, &ctx);
            let alpha = text_shape(&output, "App settings").1;
            assert!(alpha > 0 && alpha < 128, "parent entry alpha={alpha}");
            output.drop_without_applying_deltas();
        }
    }
}
