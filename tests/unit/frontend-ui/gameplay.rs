use super::*;

#[test]
fn gameplay_background_follows_theme_outside_the_guest_canvas() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let ctx = egui::Context::default();
    let command = app.session.start("fixture").unwrap();
    app.accept_frame(
        &ctx,
        Arc::new(
            frontend_core::Frame::new(
                command.session_id,
                command.attempt_id,
                240,
                320,
                vec![0xff00_0000; 240 * 320].into(),
            )
            .unwrap(),
        ),
    );
    for mode in [PlatformThemeMode::Light, PlatformThemeMode::Dark] {
        app.material_theme = MaterialTheme::fallback(mode);
        apply_material_theme(&ctx, &app.material_theme);
        for fullscreen in [false, true] {
            for game_scale in [GameScale::AutomaticFit, GameScale::Manual { percent: 50 }] {
                app.screen = Screen::Gameplay(Box::new(GameplayScreen {
                    entry_id: "fixture".into(),
                    title: "Background fixture".into(),
                    canvas_dimensions: (240, 320),
                    pointer_events: false,
                    game_scale,
                    portrait_frame_percent: None,
                    control_layout: VirtualControlLayout::default(),
                    landscape_control_layout: VirtualControlLayout::default(),
                    vibration: VibrationSettings::default(),
                    orientation: None,
                    fast_forward: false,
                    fullscreen,
                }));
                for size in [Vec2::new(453.0, 960.0), Vec2::new(960.0, 453.0)] {
                    let output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(Pos2::new(30.0, 12.0), size)),
                            ..Default::default()
                        },
                        |ui| app.draw_gameplay(ui),
                    );
                    let canvas = app.canvas_rect.unwrap();
                    let rects: Vec<_> = output
                        .shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect) => Some(rect),
                            _ => None,
                        })
                        .collect();
                    assert!(rects.iter().any(|rect| {
                        rect.fill == app.material_theme.background
                            && rect.rect.contains_rect(canvas)
                    }));
                    let black_rects: Vec<_> = rects
                        .iter()
                        .filter(|rect| rect.fill == Color32::BLACK)
                        .map(|rect| rect.rect)
                        .collect();
                    assert_eq!(black_rects, [canvas], "{mode:?}, {size:?}, {fullscreen}");
                    output.drop_without_applying_deltas();
                }
            }
        }
    }
}

#[test]
fn landscape_without_side_corridors_keeps_controls_below_the_canvas() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let ctx = egui::Context::default();
    for dimensions in [(320, 240), (640, 360), (240, 320)] {
        app.screen = Screen::Gameplay(Box::new(GameplayScreen {
            entry_id: "fixture".into(),
            title: "Wide canvas fixture".into(),
            canvas_dimensions: dimensions,
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
        for size in [Vec2::new(960.0, 420.0), Vec2::new(640.0, 400.0)] {
            let viewport = Rect::from_min_size(Pos2::ZERO, size);
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(viewport),
                    ..Default::default()
                },
                |ui| app.draw_gameplay(ui),
            )
            .drop_without_applying_deltas();
            let canvas = app.canvas_rect.unwrap();
            assert_eq!(app.control_regions.len(), 23);
            for (rect, key) in &app.control_regions {
                assert!(
                    !rect.intersects(canvas),
                    "{dimensions:?} in {size:?}: {key:?} overlaps Canvas"
                );
                assert!(viewport.contains_rect(*rect), "{key:?} is outside viewport");
            }
        }
    }
}

#[test]
fn gameplay_selects_independent_layouts_and_keeps_the_portrait_number_row() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let mut portrait = VirtualControlLayout::default();
    portrait.fire.visible = false;
    let mut landscape = VirtualControlLayout::default();
    landscape.number_keys[0].visible = false;
    app.screen = Screen::Gameplay(Box::new(GameplayScreen {
        entry_id: "fixture".into(),
        title: "Layout fixture".into(),
        canvas_dimensions: (240, 320),
        pointer_events: false,
        game_scale: GameScale::AutomaticFit,
        portrait_frame_percent: None,
        control_layout: portrait,
        landscape_control_layout: landscape,
        vibration: VibrationSettings::default(),
        orientation: None,
        fast_forward: false,
        fullscreen: true,
    }));
    let ctx = egui::Context::default();
    for (size, is_landscape) in [
        (Vec2::new(453.0, 960.0), false),
        (Vec2::new(960.0, 453.0), true),
    ] {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                ..Default::default()
            },
            |ui| app.draw_gameplay(ui),
        )
        .drop_without_applying_deltas();
        assert_eq!(
            app.control_regions
                .iter()
                .any(|(_, key)| *key == crate::TouchOwner::VirtualKey(Some(HostAction::Fire))),
            is_landscape
        );
        let number_ones = app
            .control_regions
            .iter()
            .filter(|(_, key)| *key == crate::TouchOwner::VirtualKey(Some(HostAction::Num1)))
            .count();
        assert_eq!(number_ones, if is_landscape { 1 } else { 2 });
        if !is_landscape {
            let bottom_row = &app.control_regions[8..20];
            let y = bottom_row[0].0.center().y;
            assert!(
                bottom_row
                    .iter()
                    .all(|(rect, _)| (rect.center().y - y).abs() < 0.01)
            );
            assert!(y > app.canvas_rect.unwrap().bottom());
        }
    }
}

#[test]
fn landscape_frame_uses_full_height_with_controls_and_preserves_manual_scale() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let ctx = egui::Context::default();
    let viewport = Rect::from_min_size(Pos2::new(30.0, 12.0), Vec2::new(960.0, 420.0));
    for fullscreen in [false, true] {
        let mut automatic_controls = Vec::new();
        for game_scale in [GameScale::AutomaticFit, GameScale::Manual { percent: 50 }] {
            app.screen = Screen::Gameplay(Box::new(GameplayScreen {
                entry_id: "fixture".into(),
                title: "Landscape fixture".into(),
                canvas_dimensions: (240, 320),
                pointer_events: false,
                game_scale,
                portrait_frame_percent: None,
                control_layout: VirtualControlLayout::default(),
                landscape_control_layout: VirtualControlLayout::default(),
                vibration: VibrationSettings::default(),
                orientation: None,
                fast_forward: false,
                fullscreen,
            }));
            for _ in 0..3 {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(viewport),
                        ..Default::default()
                    },
                    |ui| app.draw_gameplay(ui),
                )
                .drop_without_applying_deltas();
            }
            let canvas = app.canvas_rect.unwrap();
            assert!(viewport.contains_rect(canvas));
            assert!((canvas.width() / canvas.height() - 0.75).abs() < 0.01);
            assert_eq!(app.control_regions.len(), 23);
            if game_scale == GameScale::AutomaticFit {
                assert!((canvas.bottom() - viewport.bottom()).abs() < 1.0);
                if fullscreen {
                    assert!((canvas.height() - viewport.height()).abs() < 1.0);
                } else {
                    assert!(canvas.height() > viewport.height() * 0.75);
                }
                let key = |action| {
                    app.control_regions
                        .iter()
                        .rev()
                        .find(|(_, key)| *key == crate::TouchOwner::VirtualKey(Some(action)))
                        .unwrap()
                        .0
                };
                assert!(key(HostAction::Left).right() < canvas.left());
                assert!(key(HostAction::Fire).left() > canvas.right());
                assert!(key(HostAction::Num5).center().y > canvas.center().y);
                for (_, action, index) in NUMBER_ROW_KEYS {
                    let rect = key(action);
                    assert!(rect.width() >= 24.0);
                    assert!(!rect.intersects(canvas), "{action:?}: {rect:?}");
                    if index < 6 {
                        assert!(rect.right() < canvas.left());
                    } else {
                        assert!(rect.left() > canvas.right());
                    }
                }
                assert!(
                    (key(HostAction::Num1).center().y - key(HostAction::Num7).center().y).abs()
                        < 0.01
                );
                assert!(
                    (key(HostAction::Num1).center().x - key(HostAction::Num5).center().x).abs()
                        < 0.01
                );
                automatic_controls.clone_from(&app.control_regions);
            } else {
                assert!((canvas.height() - 160.0).abs() < 1.0);
                assert_eq!(app.control_regions, automatic_controls);
            }
        }
    }
}
