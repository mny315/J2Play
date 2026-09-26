use super::*;

use super::test_storage::Scratch;

#[test]
fn platform_cancellation_discards_only_the_current_attempts_deferred_link() {
    #[derive(Default)]
    struct Calls {
        cancellations: usize,
        opened: Vec<String>,
    }
    struct Bridge(std::sync::Arc<std::sync::Mutex<Calls>>);
    impl PlatformBridge for Bridge {
        fn request_document(&mut self, _: DocumentKind) -> Result<(), EmuError> {
            unreachable!()
        }
        fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
            None
        }
        fn cancel_guest_operations(&mut self) {
            self.0.lock().unwrap().cancellations += 1;
        }
        fn open_external_url(&mut self, url: &str) -> Result<(), EmuError> {
            self.0.lock().unwrap().opened.push(url.to_owned());
            Ok(())
        }
    }
    let scratch = Scratch::new();
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Calls::default()));
    let mut app = FrontendApp::new(&scratch.0, Box::new(Bridge(calls.clone()))).unwrap();
    let first = app.session.start("fixture").unwrap();
    app.session.apply_event(&SessionEvent::terminal_error(
        first.session_id,
        first.attempt_id,
        "Fixture",
        "Retry",
    ));
    let current = app.session.retry().unwrap();
    app.external_after_stop = Some("https://example.invalid/".into());
    let ctx = egui::Context::default();
    app.process_runtime_event(
        &ctx,
        SessionEvent {
            session_id: first.session_id,
            attempt_id: first.attempt_id,
            kind: SessionEventKind::PlatformRequestCancelled,
        },
    );
    assert!(app.external_after_stop.is_some());
    assert_eq!(calls.lock().unwrap().cancellations, 0);
    app.session.stop().unwrap();
    for kind in [
        SessionEventKind::PlatformRequestCancelled,
        SessionEventKind::Stopped,
    ] {
        app.process_runtime_event(
            &ctx,
            SessionEvent {
                session_id: current.session_id,
                attempt_id: current.attempt_id,
                kind,
            },
        );
    }
    assert!(app.external_after_stop.is_none());
    let calls = calls.lock().unwrap();
    assert_eq!(calls.cancellations, 1);
    assert!(calls.opened.is_empty());
    assert_eq!(app.session.state(), SessionState::Idle);
}

#[test]
fn presentation_rejects_a_frame_from_a_previous_attempt() {
    let scratch = Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
    let first = app.session.start("fixture").unwrap();
    app.session.apply_event(&SessionEvent::terminal_error(
        first.session_id,
        first.attempt_id,
        "Fixture failure",
        "Retry fixture",
    ));
    let current = app.session.retry().unwrap();
    let ctx = egui::Context::default();
    let frame = |command: &frontend_core::SessionCommand, pixel| {
        Arc::new(
            Frame::new(
                command.session_id,
                command.attempt_id,
                1,
                1,
                Arc::from([pixel]),
            )
            .unwrap(),
        )
    };
    app.accept_frame(&ctx, frame(&current, 0xff00_ff00));
    app.canvas_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0)));
    app.accept_frame(&ctx, frame(&first, 0xffff_0000));
    assert_eq!(
        app.latest_frame.as_ref().unwrap().attempt_id,
        current.attempt_id
    );
    assert_eq!(app.latest_frame.as_ref().unwrap().pixels[0], 0xff00_ff00);
    assert!(app.canvas_rect.is_some());
    // Keep the last presented image while the worker tears down the same
    // attempt; late cleanup paints must not flash through before navigation.
    let _ = app.session.stop().unwrap();
    app.accept_frame(&ctx, frame(&current, 0xff00_0000));
    assert_eq!(app.latest_frame.as_ref().unwrap().pixels[0], 0xff00_ff00);
    app.session.apply_event(&SessionEvent::terminal_error(
        current.session_id,
        current.attempt_id,
        "Fixture failure",
        "Stop fixture",
    ));
    app.accept_frame(&ctx, frame(&current, 0xff00_0000));
    assert_eq!(app.latest_frame.as_ref().unwrap().pixels[0], 0xff00_ff00);
}

#[test]
fn runtime_wake_repaints_without_an_extra_idle_frame() {
    let scratch = Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
    let command = app.session.start("fixture").unwrap();
    let frame = Arc::new(
        Frame::new(
            command.session_id,
            command.attempt_id,
            1,
            1,
            Arc::from([0xff00_ff00]),
        )
        .unwrap(),
    );
    let ctx = egui::Context::default();
    for _ in 0..3 {
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
    }
    assert!(!ctx.has_requested_repaint());
    let delay = std::sync::Arc::new(std::sync::Mutex::new(None));
    let observed = delay.clone();
    ctx.set_request_repaint_callback(move |request| {
        *observed.lock().unwrap() = Some(request.delay);
    });
    crate::eframe_app::repaint_for_runtime_event(&ctx);
    assert!(
        delay
            .lock()
            .unwrap()
            .is_some_and(|delay| delay <= Duration::from_millis(1))
    );
    ctx.run_ui(egui::RawInput::default(), |ui| {
        app.accept_frame(ui.ctx(), frame.clone());
    })
    .drop_without_applying_deltas();
    assert!(app.game_texture.is_some());
    assert_eq!(app.latest_frame.as_ref().unwrap().pixels[0], 0xff00_ff00);
    assert!(!ctx.has_requested_repaint());
}

#[test]
fn unchanged_frames_reuse_the_texture_but_restore_it_after_release() {
    let scratch = Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
    let command = app.session.start("fixture").unwrap();
    let ctx = egui::Context::default();
    let frame = Arc::new(
        Frame::new(
            command.session_id,
            command.attempt_id,
            2,
            1,
            Arc::from([0xff00_ff00; 2]),
        )
        .unwrap(),
    );
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.accept_frame(ui.ctx(), frame.clone());
    });
    assert!(!output.textures_delta.is_empty());
    output.drop_without_applying_deltas();

    // Equal content can arrive in a different allocation from the worker.
    let duplicate = Arc::new(
        Frame::new(
            command.session_id,
            command.attempt_id,
            2,
            1,
            Arc::from([0xff00_ff00; 2]),
        )
        .unwrap(),
    );
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.accept_frame(ui.ctx(), duplicate.clone());
    });
    assert!(output.textures_delta.is_empty());
    output.drop_without_applying_deltas();

    let changed = Arc::new(
        Frame::new(
            command.session_id,
            command.attempt_id,
            2,
            1,
            Arc::from([0xff00_ff00, 0xff00_ff01]),
        )
        .unwrap(),
    );
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.accept_frame(ui.ctx(), changed.clone());
    });
    assert!(!output.textures_delta.is_empty());
    output.drop_without_applying_deltas();

    app.game_texture = None;
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.accept_frame(ui.ctx(), changed.clone());
    });
    assert!(!output.textures_delta.is_empty());
    assert!(app.game_texture.is_some());
    output.drop_without_applying_deltas();

    let mut geometry = (*changed).clone();
    geometry.canvas_region.width = 1;
    let geometry = Arc::new(geometry);
    app.canvas_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0)));
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.accept_frame(ui.ctx(), geometry.clone());
    });
    assert!(app.canvas_rect.is_none());
    assert_eq!(app.latest_frame.as_ref().unwrap().canvas_region.width, 1);
    output.drop_without_applying_deltas();
}
