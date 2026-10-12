use super::*;

fn timer() -> StartupSplash {
    StartupSplash {
        pending_image: None,
        texture: None,
        elapsed: Duration::ZERO,
        last_paint: None,
    }
}

#[test]
fn logic_ticks_and_unfocused_paints_do_not_consume_the_splash() {
    let ctx = egui::Context::default();
    let mut splash = timer();
    let now = Instant::now();
    for _ in 0..10 {
        assert!(splash.update(&ctx));
    }
    assert_eq!(splash.elapsed, Duration::ZERO);
    assert_eq!(splash.last_paint, None);
    splash.advance(now);
    splash.advance(now + Duration::from_millis(16));
    let elapsed = splash.elapsed;
    let mut output = ctx.run_ui(
        egui::RawInput {
            focused: false,
            ..Default::default()
        },
        |ui| {
            assert!(splash.update(ui.ctx()));
            splash.paint(ui, Color32::BLACK);
        },
    );
    output.textures_delta.clear();
    assert_eq!(splash.elapsed, elapsed);
    assert_eq!(splash.last_paint, None);
    splash.advance(now + Duration::from_secs(15));
    assert_eq!(splash.elapsed, elapsed);
}

#[test]
fn a_delayed_first_present_preserves_the_animation_then_regular_paints_finish_it() {
    let mut splash = timer();
    let now = Instant::now();
    splash.advance(now);
    let resumed = now + Duration::from_secs(15);
    splash.advance(resumed);
    assert!(splash.elapsed < Duration::from_secs_f32(SPLASH_FADE_IN_SECONDS));
    let ctx = egui::Context::default();
    assert!(splash.update(&ctx));
    for frame in 1..=100 {
        splash.advance(resumed + Duration::from_millis(16) * frame);
    }
    assert!(!splash.update(&ctx));
}

#[test]
fn embedded_splash_is_bounded_high_resolution_and_transparent() {
    let splash = StartupSplash::load().expect("embedded splash must decode");
    let image = splash.pending_image.as_ref().expect("decoded splash image");
    assert_eq!(image.size, [1_254, 1_254]);
    assert!(image.pixels.iter().any(|pixel| pixel.a() == 0));
    assert!(image.pixels.iter().any(|pixel| pixel.a() == 255));
}

#[test]
fn splash_animation_is_bounded_and_finishes_transparently() {
    for elapsed in [
        Duration::ZERO,
        Duration::from_millis(190),
        Duration::from_millis(700),
        SPLASH_DURATION,
    ] {
        let (scale, opacity) = splash_animation(elapsed);
        assert!((0.88..=1.0).contains(&scale));
        assert!((0.0..=1.0).contains(&opacity));
    }
    assert!(splash_animation(Duration::ZERO).1.abs() < f32::EPSILON);
    assert!(splash_animation(SPLASH_DURATION).1.abs() < f32::EPSILON);
}

#[test]
fn splash_requests_every_display_frame_at_high_refresh_rates() {
    for refresh_rate in [60.0, 120.0, 240.0] {
        let ctx = egui::Context::default();
        let mut splash = timer();
        for _ in 0..8 {
            let output = ctx.run_ui(
                egui::RawInput {
                    predicted_dt: 1.0 / refresh_rate,
                    ..Default::default()
                },
                |ui| {
                    assert!(splash.update(ui.ctx()));
                },
            );
            assert_eq!(
                output.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
                Duration::ZERO
            );
            output.drop_without_applying_deltas();
        }
    }
}

#[test]
fn paused_splash_stops_requesting_frames_and_resumes_with_window_focus() {
    for (focused, minimized, occluded) in [
        (false, false, false),
        (true, true, false),
        (true, false, true),
    ] {
        let ctx = egui::Context::default();
        let mut splash = timer();
        let elapsed = Duration::from_millis(400);
        splash.elapsed = elapsed;
        splash.last_paint = Some(Instant::now());
        for frame in 0..8 {
            let mut input = egui::RawInput {
                focused,
                ..Default::default()
            };
            let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
            viewport.minimized = Some(minimized);
            viewport.occluded = Some(occluded);
            let output = ctx.run_ui(input, |ui| {
                assert!(splash.update(ui.ctx()));
                splash.paint(ui, Color32::BLACK);
            });
            // Allow egui's initial layout and outstanding repaint to settle.
            if frame >= 4 {
                assert_eq!(
                    output.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
                    Duration::MAX,
                    "paused splash kept scheduling frames: {focused}/{minimized}/{occluded}"
                );
            }
            output.drop_without_applying_deltas();
        }
        assert_eq!(splash.elapsed, elapsed);
        assert_eq!(splash.last_paint, None);

        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(splash.update(ui.ctx()));
            splash.paint(ui, Color32::BLACK);
        });
        assert_eq!(
            output.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
            Duration::ZERO
        );
        assert_eq!(splash.elapsed, elapsed);
        assert!(splash.last_paint.is_some());
        output.drop_without_applying_deltas();
    }
}
