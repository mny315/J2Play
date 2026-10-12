use super::*;
use eframe::App;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct ControllerPlatform(Arc<AtomicBool>);

impl crate::PlatformBridge for ControllerPlatform {
    fn request_document(&mut self, _: crate::DocumentKind) -> Result<(), diagnostics::EmuError> {
        unreachable!("controller fixture does not open the document picker")
    }
    fn poll_document(&mut self) -> Option<Result<crate::DocumentOutcome, diagnostics::EmuError>> {
        None
    }
    fn physical_devices(&self) -> Vec<String> {
        if self.0.load(Ordering::Relaxed) {
            vec!["Fixture gamepad".into()]
        } else {
            vec![]
        }
    }
}

fn frame(app: &mut FrontendApp, ctx: &egui::Context, width: f32, events: Vec<egui::Event>) -> bool {
    let mut frame = eframe::Frame::_new_kittest();
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 700.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.logic(ui.ctx(), &mut frame);
            app.ui(ui, &mut frame);
        },
    );
    let visible = output.shapes.iter().any(|shape| {
        matches!(&shape.shape,
        egui::Shape::Rect(rect) if (rect.stroke.width - 2.5).abs() < f32::EPSILON)
    });
    output.drop_without_applying_deltas();
    visible
}

fn tap_background(app: &mut FrontendApp, ctx: &egui::Context, width: f32, y: f32) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            width,
            vec![egui::Event::PointerButton {
                pos: egui::pos2(width - 2.0, y),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    for _ in 0..3 {
        frame(app, ctx, width, vec![]);
    }
    assert!(frame(app, ctx, width, vec![]));
}

#[test]
fn attached_controller_shows_first_game_without_input_and_keeps_the_ring_after_touch() {
    for width in [453.0, 800.0] {
        let scratch = crate::tests::test_storage::Scratch::new();
        let root = &scratch.0;
        let connected = Arc::new(AtomicBool::new(true));
        let mut app =
            FrontendApp::new(root, Box::new(ControllerPlatform(connected.clone()))).unwrap();
        app.startup_splash = None;
        for index in 1..=2 {
            let prepared = frontend_core::inspect_import(frontend_core::ImportSource::new(
                None,
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/java-me/conformance.jar"
                ))
                .to_vec(),
                None,
            ))
            .unwrap()
            .select_midlet(index)
            .unwrap();
            app.entries.push(
                app.repository
                    .commit_import(&prepared, crate::GameSettings::default())
                    .unwrap(),
            );
        }
        let ctx = egui::Context::default();
        for _ in 0..3 {
            frame(&mut app, &ctx, width, vec![]);
        }
        assert!(
            frame(&mut app, &ctx, width, vec![]),
            "connection must reveal the ring without a press"
        );
        let first = crate::library_view::launch_id(app.entries[0].id());
        assert_eq!(ctx.memory(egui::Memory::focused), Some(first));

        // Pointer interaction must not switch off controller focus visibility.
        tap_background(&mut app, &ctx, width, 680.0);
        assert_eq!(ctx.memory(egui::Memory::focused), Some(first));

        // Reconnecting reveals the existing selection without selecting game one again.
        connected.store(false, Ordering::Relaxed);
        app.focus_ring.hide();
        assert!(!frame(&mut app, &ctx, width, vec![]));
        let second = crate::library_view::launch_id(app.entries[1].id());
        ctx.memory_mut(|memory| memory.request_focus(second));
        connected.store(true, Ordering::Relaxed);
        for _ in 0..3 {
            frame(&mut app, &ctx, width, vec![]);
        }
        assert!(frame(&mut app, &ctx, width, vec![]));
        assert_eq!(ctx.memory(egui::Memory::focused), Some(second));

        open_settings_page(&mut app, false);
        for _ in 0..3 {
            frame(&mut app, &ctx, width, vec![]);
        }
        assert!(
            frame(&mut app, &ctx, width, vec![]),
            "connected controller must reveal settings focus too"
        );
        let selected = ctx.memory(egui::Memory::focused);
        tap_background(&mut app, &ctx, width, 20.0);
        assert_eq!(
            ctx.memory(egui::Memory::focused),
            selected,
            "a background tap must restore the current settings selection"
        );
    }
}
