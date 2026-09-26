use super::*;
use crate::{
    Event, FrontendApp, GameSettings, GameplayScreen, Pos2, Screen, UnavailablePlatformBridge,
    VibrationSettings, VirtualControlLayout, egui,
};

fn fixture() -> (crate::tests::test_storage::Scratch, FrontendApp) {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
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
    .select_midlet(1)
    .unwrap();
    let entry = app
        .repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    app.screen = Screen::Gameplay(Box::new(GameplayScreen {
        entry_id: entry.id().to_owned(),
        title: "Portrait fixture".into(),
        canvas_dimensions: (240, 320),
        pointer_events: false,
        game_scale: GameScale::AutomaticFit,
        portrait_frame_percent: None,
        control_layout: VirtualControlLayout::default(),
        landscape_control_layout: VirtualControlLayout::default(),
        vibration: VibrationSettings::default(),
        orientation: None,
        fast_forward: false,
        fullscreen: true,
    }));
    app.entries.push(entry);
    (scratch, app)
}

fn render(app: &mut FrontendApp, ctx: &egui::Context, events: Vec<Event>, size: Vec2) {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| app.draw_gameplay(ui),
    )
    .drop_without_applying_deltas();
}

#[test]
fn fullscreen_transition_publishes_painted_targets_and_stop_clears_them() {
    let (_scratch, mut app) = fixture();
    app.acknowledge_fullscreen_help();
    let ctx = egui::Context::default();
    let size = Vec2::new(453.0, 960.0);
    render(&mut app, &ctx, vec![], size);
    assert!(app.canvas_rect.is_some());
    app.set_gameplay_fullscreen(false).unwrap();
    assert!(app.gameplay_transition.busy());
    assert!(app.canvas_rect.is_none());
    assert!(app.control_regions.is_empty());
    render(&mut app, &ctx, vec![], size);
    assert!(app.canvas_rect.is_some());
    assert!(!app.control_regions.is_empty());
    app.stop_active_session();
    assert!(!app.gameplay_transition.busy());
    assert!(app.canvas_rect.is_none());
    assert!(app.control_regions.is_empty());
}

#[test]
fn rotation_reports_an_unavailable_platform_and_clears_input_geometry() {
    let (_scratch, mut app) = fixture();
    let ctx = egui::Context::default();
    render(&mut app, &ctx, vec![], Vec2::new(453.0, 960.0));
    assert!(app.canvas_rect.is_some());
    app.rotate_gameplay_screen(false);
    assert!(app.canvas_rect.is_none());
    assert!(app.control_regions.is_empty());
    assert_eq!(app.host_orientation, crate::PlatformOrientation::Automatic);
    assert!(
        app.display_error
            .as_ref()
            .unwrap()
            .technical_details
            .contains("orientation-unavailable")
    );
}

#[test]
fn same_orientation_window_resize_invalidates_input_geometry() {
    let (_scratch, mut app) = fixture();
    let ctx = egui::Context::default();
    let size = Vec2::new(453.0, 960.0);
    app.safe_content_rect = Rect::from_min_size(Pos2::ZERO, size);
    render(&mut app, &ctx, vec![], size);
    assert!(!app.invalidate_gameplay_geometry(app.safe_content_rect));
    assert!(app.canvas_rect.is_some());
    assert!(
        app.invalidate_gameplay_geometry(Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 800.0)))
    );
    assert!(app.canvas_rect.is_none());
    assert!(app.control_regions.is_empty());
}

#[test]
fn guest_frame_remapping_drops_old_hit_regions() {
    let (_scratch, mut app) = fixture();
    let command = app.session.start("fixture").unwrap();
    let ctx = egui::Context::default();
    let size = Vec2::new(453.0, 960.0);
    let frame = |width, height, canvas_region| {
        std::sync::Arc::new(
            frontend_core::Frame::with_canvas_region(
                command.session_id,
                command.attempt_id,
                width,
                height,
                canvas_region,
                vec![0xff00_0000; usize::try_from(width * height).unwrap()].into(),
            )
            .unwrap(),
        )
    };
    let initial = frame(
        240,
        320,
        platform::LogicalRect {
            x: 0,
            y: 0,
            width: 240,
            height: 320,
        },
    );
    app.accept_frame(&ctx, initial.clone());
    for next in [
        frame(
            240,
            320,
            platform::LogicalRect {
                x: 0,
                y: 20,
                width: 240,
                height: 300,
            },
        ),
        frame(
            320,
            240,
            platform::LogicalRect {
                x: 0,
                y: 0,
                width: 320,
                height: 240,
            },
        ),
    ] {
        app.accept_frame(&ctx, initial.clone());
        render(&mut app, &ctx, vec![], size);
        app.accept_frame(&ctx, initial.clone());
        assert!(
            app.canvas_rect.is_some(),
            "ordinary frames preserve input geometry"
        );
        app.accept_frame(&ctx, next);
        assert!(app.canvas_rect.is_none());
        assert!(app.control_regions.is_empty());
        render(&mut app, &ctx, vec![], size);
        assert_eq!(app.entries[0].settings().portrait_frame_percent, None);
    }
}

#[test]
fn smaller_guest_frames_and_scales_keep_portrait_controls_at_the_bottom() {
    let (_scratch, mut app) = fixture();
    let ctx = egui::Context::default();
    let command = app.session.start("fixture").unwrap();
    for fullscreen in [false, true] {
        for customized in [false, true] {
            let mut controls = VirtualControlLayout::default();
            if customized {
                controls.direction_pad.offset_y = -400;
                controls.fire.offset_y = 300;
                controls.left_soft_key.offset_y = -500;
                controls.right_soft_key.size_percent = 80;
                controls.number_keys[0].offset_y = -300;
            }
            let mut reference = None;
            for dimensions in [(240, 320), (128, 160), (130, 130), (176, 208), (320, 240)] {
                app.accept_frame(
                    &ctx,
                    std::sync::Arc::new(
                        frontend_core::Frame::new(
                            command.session_id,
                            command.attempt_id,
                            dimensions.0,
                            dimensions.1,
                            vec![0xff00_0000; (dimensions.0 * dimensions.1) as usize].into(),
                        )
                        .unwrap(),
                    ),
                );
                for (scale, percent) in [
                    (GameScale::AutomaticFit, None),
                    (GameScale::Manual { percent: 50 }, None),
                    (GameScale::AutomaticFit, Some(25)),
                    (GameScale::AutomaticFit, Some(100)),
                ] {
                    let Screen::Gameplay(gameplay) = &mut app.screen else {
                        unreachable!();
                    };
                    gameplay.fullscreen = fullscreen;
                    gameplay.game_scale = scale;
                    gameplay.portrait_frame_percent = percent;
                    gameplay.control_layout = controls.clone();
                    // Compare settled layouts, not an orientation/fullscreen animation.
                    app.gameplay_transition =
                        crate::gameplay_transition::GameplayTransition::default();
                    render(&mut app, &ctx, vec![], Vec2::new(453.0, 960.0));
                    let canvas = app.canvas_rect.unwrap();
                    assert_eq!(app.control_regions.len(), 23);
                    for (rect, action) in &app.control_regions {
                        assert!(
                            rect.top() >= canvas.bottom() - 0.1,
                            "{action:?} overlaps {canvas:?}: {rect:?}"
                        );
                        if !customized {
                            assert_eq!(app.action_at(rect.center()), Some(*action));
                        }
                    }
                    let expected = reference.get_or_insert_with(|| app.control_regions.clone());
                    for ((actual, action), (expected, expected_action)) in
                        app.control_regions.iter().zip(expected.iter())
                    {
                        assert_eq!(action, expected_action);
                        assert!(
                            (actual.min - expected.min).length() < 0.1
                                && (actual.max - expected.max).length() < 0.1,
                            "{action:?} moved for {dimensions:?}, {scale:?}, {percent:?}: \
                             {expected:?} -> {actual:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn portrait_bounds_preserve_aspect_and_reserve_usable_controls() {
    let available = Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 960.0));
    let manual = portrait_layout(
        available,
        (240, 320),
        GameScale::Manual { percent: 25 },
        None,
    )
    .unwrap();
    assert!((manual.game_height - 80.0).abs() < 0.01);
    for size in [
        Vec2::new(320.0, 600.0),
        Vec2::new(453.0, 960.0),
        Vec2::new(700.0, 1200.0),
    ] {
        for dimensions in [(128, 160), (240, 320), (320, 240)] {
            let available = Rect::from_min_size(Pos2::new(12.0, 30.0), size);
            let mut previous_height = 0.0;
            for percent in [25, 50, 75, 100, 115] {
                let layout = portrait_layout(
                    available,
                    dimensions,
                    GameScale::AutomaticFit,
                    Some(percent),
                )
                .unwrap();
                assert!(layout.game_height >= previous_height);
                assert!(layout.controls_height >= MIN_RESIZED_CONTROLS_HEIGHT);
                assert!(
                    crate::controls::virtual_control_metrics(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(size.x, layout.controls_height)
                    ))
                    .is_some()
                );
                assert!(layout.gap.abs() < f32::EPSILON);
                assert!(
                    (layout.game_top_padding
                        + layout.game_height
                        + layout.controls_height
                        + layout.gap
                        - size.y)
                        .abs()
                        < 0.01
                );
                let canvas = super::super::layout::game_canvas_rect(
                    Rect::from_min_size(
                        available.min + Vec2::new(0.0, layout.game_top_padding),
                        Vec2::new(size.x, layout.game_height),
                    ),
                    dimensions,
                    GameScale::AutomaticFit,
                );
                assert!(canvas.width() <= available.width());
                assert!(available.contains_rect(canvas));
                let free_height = (size.y - GAMEPLAY_MIN_CONTROLS_HEIGHT).max(layout.game_height);
                assert!((canvas.center().y - available.top() - free_height * 0.5).abs() < 0.1);
                previous_height = layout.game_height;
            }
        }
    }
    assert!(
        portrait_layout(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 320.0)),
            (240, 320),
            GameScale::AutomaticFit,
            None
        )
        .is_none()
    );
}

#[test]
fn portrait_can_grow_fifteen_percent_without_changing_the_saved_reference_scale() {
    let available = Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 600.0));
    let previous =
        portrait_layout(available, (240, 320), GameScale::AutomaticFit, Some(100)).unwrap();
    let expanded =
        portrait_layout(available, (240, 320), GameScale::AutomaticFit, Some(115)).unwrap();
    assert!((expanded.game_height - previous.game_height * 1.15).abs() < 0.01);
    assert!((expanded.controls_height - MIN_RESIZED_CONTROLS_HEIGHT).abs() < 0.01);
    let width_limited =
        portrait_layout(available, (320, 240), GameScale::AutomaticFit, Some(115)).unwrap();
    assert!((width_limited.game_height - available.width() * 0.75).abs() < 0.01);
}
