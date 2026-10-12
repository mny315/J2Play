use super::*;

fn allocate_preview(
    host_size: Vec2,
    orientation_control: crate::OrientationControl,
    editor: &ControlEditorState,
) -> (ControlPreview, f32) {
    let ctx = egui::Context::default();
    let mut result = None;
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, host_size)),
            ..Default::default()
        },
        |ui| {
            let width = ui.available_width();
            result = Some((
                ControlPreview::allocate(ui, host_size, orientation_control, editor),
                width,
            ));
        },
    )
    .drop_without_applying_deltas();
    result.unwrap()
}

#[test]
fn desktop_landscape_preview_fills_width_and_keeps_default_side_keypads() {
    let mut editor = ControlEditorState::new(&crate::GameSettings::default(), (240, 320));
    editor.select_orientation(true);
    for host in [
        Vec2::new(1280.0, 1400.0),
        Vec2::new(900.0, 900.0),
        Vec2::new(1280.0, 800.0),
        Vec2::new(360.0, 720.0),
    ] {
        let (preview, width) = allocate_preview(host, crate::OrientationControl::Layout, &editor);
        assert!((preview.rect.width() - width).abs() < 0.01);
        assert!(preview.rect.width() > preview.rect.height() * 2.0);
        let canvas = preview.canvas.unwrap();
        assert!((canvas.height() - preview.rect.height()).abs() < 0.01);
        let geometry = preview.geometry.unwrap().0;
        assert!(geometry.direction_pad_center.x < canvas.left());
        assert!(geometry.fire_center.x > canvas.right());
        for (index, (center, size)) in geometry.number_keys.into_iter().enumerate() {
            let key = Rect::from_center_size(center, Vec2::splat(size));
            assert!(preview.rect.contains_rect(key));
            assert!(!key.intersects(canvas));
            if index < 6 {
                assert!(key.right() < canvas.left());
            } else {
                assert!(key.left() > canvas.right());
            }
            if width >= 900.0 {
                assert!(size >= 24.0, "desktop number keys must remain editable");
            }
        }
        assert!(geometry.number_keys[0].0.y < geometry.number_keys[2].0.y);
        assert!(geometry.number_keys[2].0.y < geometry.number_keys[4].0.y);
    }
}

#[test]
fn phone_landscape_preview_preserves_device_proportions_and_height_limit() {
    let mut editor = ControlEditorState::new(&crate::GameSettings::default(), (240, 320));
    editor.select_orientation(true);
    for host in [
        Vec2::new(960.0, 432.0),
        Vec2::new(800.0, 360.0),
        Vec2::new(1280.0, 800.0),
    ] {
        let (preview, width) =
            allocate_preview(host, crate::OrientationControl::HostWindow, &editor);
        assert_eq!(preview.host_size, Some(host));
        let scale = (width / host.x).min(360.0 / host.y);
        assert!((preview.rect.size() - host * scale).length() < 0.01);
        assert!(preview.rect.height() <= 360.01);
        let layout = gameplay_layout(host, editor.canvas_dimensions, false, true);
        let controls = Rect::from_min_size(
            Pos2::new(0.0, host.y - layout.controls_height),
            Vec2::new(host.x, layout.controls_height),
        );
        let canvas = game_canvas_rect(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(host.x, layout.game_height)),
            editor.canvas_dimensions,
            GameScale::AutomaticFit,
        );
        let gameplay = gameplay_control_geometry(
            controls,
            &editor.layout,
            layout.controls_overlay.then_some(canvas),
        )
        .unwrap();
        let actual = transform_geometry(
            preview.geometry.unwrap().0,
            egui::emath::RectTransform::from_to(
                preview.rect,
                Rect::from_min_size(Pos2::ZERO, host),
            ),
        );
        assert!((actual.direction_pad_center - gameplay.direction_pad_center).length() < 0.01);
        assert!((actual.fire_center - gameplay.fire_center).length() < 0.01);
        for ((center, size), (expected_center, expected_size)) in
            actual.number_keys.into_iter().zip(gameplay.number_keys)
        {
            assert!((center - expected_center).length() < 0.01);
            assert!((size - expected_size).abs() < 0.01);
        }
    }
}

#[test]
fn desktop_landscape_drag_keeps_normalized_offsets_across_window_changes() {
    let mut editor = ControlEditorState::new(&crate::GameSettings::default(), (240, 320));
    editor.select_orientation(true);
    let host = Vec2::new(1280.0, 1400.0);
    let (preview, _) = allocate_preview(host, crate::OrientationControl::Layout, &editor);
    let (geometry, defaults) = preview.geometry.unwrap();
    let before = editor.layout.clone();
    let delta = Vec2::new(0.0, -20.0);
    editor.selected = crate::ControlSelection::NumberKey(0);
    editor.layout.number_keys[0] = super::super::selection::transform_after_drag(
        before.number_keys[0],
        geometry.number_keys[0].0,
        defaults.number_keys[0].0,
        delta,
        preview.controls,
        false,
    );
    assert!(!preview.reject_screen_overlap(&mut editor, &before));
    let (moved, _) = allocate_preview(host, crate::OrientationControl::Layout, &editor);
    assert!(
        (moved.geometry.unwrap().0.number_keys[0].0 - geometry.number_keys[0].0 - delta).length()
            < 0.1
    );
    let normalize = |preview: &ControlPreview| {
        let center = preview.geometry.unwrap().0.number_keys[0].0;
        (center - preview.controls.min) / preview.controls.size()
    };
    let (resized, _) = allocate_preview(
        Vec2::new(800.0, 600.0),
        crate::OrientationControl::Layout,
        &editor,
    );
    assert!((normalize(&resized) - normalize(&moved)).length() < 0.001);
    assert_eq!(editor.other_layout, before);
}

#[test]
fn portrait_preview_preserves_every_control_at_gameplay_scale() {
    let mut settings = crate::GameSettings::default();
    settings.control_layout.left_soft_key.size_percent = 150;
    settings.control_layout.left_soft_key.offset_y = 900;
    settings.control_layout.right_soft_key.size_percent = 70;
    settings.control_layout.right_soft_key.offset_x = -800;
    settings.control_layout.direction_pad.size_percent = 120;
    settings.control_layout.fire.offset_y = 700;
    for (index, key) in settings.control_layout.number_keys.iter_mut().enumerate() {
        key.size_percent = 60 + u16::try_from(index).unwrap() * 10;
        key.offset_y = -500;
    }
    let ctx = egui::Context::default();
    for host_size in [
        Vec2::new(320.0, 640.0),
        Vec2::new(453.0, 960.0),
        Vec2::new(700.0, 1200.0),
    ] {
        for percent in [None, Some(50), Some(115)] {
            settings.portrait_frame_percent = percent;
            let editor = ControlEditorState::new(&settings, (240, 320));
            let controls = portrait_controls_rect(host_size, &editor);
            let expected = gameplay_control_geometry(controls, &editor.layout, None).unwrap();
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 800.0))),
                    ..Default::default()
                },
                |ui| {
                    let preview = ControlPreview::allocate(
                        ui,
                        host_size,
                        crate::OrientationControl::HostWindow,
                        &editor,
                    );
                    let actual = transform_geometry(
                        preview.geometry.unwrap().0,
                        egui::emath::RectTransform::from_to(preview.controls, controls),
                    );
                    let elements = |geometry: VirtualControlGeometry| {
                        [
                            (
                                geometry.direction_pad_center,
                                geometry.direction_button_size,
                            ),
                            (geometry.fire_center, geometry.fire_size),
                            (geometry.left_soft_key_center, geometry.left_soft_key_size),
                            (geometry.right_soft_key_center, geometry.right_soft_key_size),
                        ]
                        .into_iter()
                        .chain(geometry.number_keys)
                    };
                    for ((actual_center, actual_size), (expected_center, expected_size)) in
                        elements(actual).zip(elements(expected))
                    {
                        assert!((actual_center - expected_center).length() < 0.01);
                        assert!((actual_size - expected_size).abs() < 0.01);
                    }
                },
            )
            .drop_without_applying_deltas();
        }
    }
}

#[test]
fn landscape_button_requests_rotation_then_opens_the_saved_layout() {
    let mut editor = ControlEditorState::new(&crate::GameSettings::default(), (240, 320));
    editor.other_layout.fire.offset_x = 750;
    let ctx = egui::Context::default();
    let theme = crate::MaterialTheme::fallback(crate::PlatformThemeMode::Dark);
    let portrait = Vec2::new(453.0, 960.0);
    let mut button = Pos2::ZERO;
    let mut prompted = false;
    let mut content_bottom: Option<f32> = None;
    for frame in 0..4 {
        let events = if frame == 2 {
            vec![
                egui::Event::PointerMoved(button),
                egui::Event::PointerButton {
                    pos: button,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: button,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        } else {
            Vec::new()
        };
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, portrait)),
                time: Some(f64::from(frame)),
                events,
                ..Default::default()
            },
            |ui| {
                orientation_selector(
                    ui,
                    &theme,
                    portrait,
                    crate::OrientationControl::HostWindow,
                    &mut editor,
                );
                let bottom = ui.cursor().top();
                if let Some(before) = content_bottom {
                    assert!(
                        (bottom - before).abs() < 0.01,
                        "the prompt must not shift editor content"
                    );
                }
                content_bottom = Some(bottom);
            },
        );
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                if text.galley.job.text == "Landscape" {
                    button = text.pos + text.galley.size() * 0.5;
                }
                prompted |= text.galley.job.text == "Rotate your phone";
                if text.galley.job.text == "Rotate your phone" {
                    assert!(text.pos.y > portrait.y * 0.3, "prompt must be centered");
                }
            }
        }
        output.drop_without_applying_deltas();
    }
    assert!(prompted);
    assert!(editor.landscape_requested);
    assert!(!editor.landscape);
    ctx.run_ui(egui::RawInput::default(), |ui| {
        orientation_selector(
            ui,
            &theme,
            Vec2::new(960.0, 453.0),
            crate::OrientationControl::HostWindow,
            &mut editor,
        );
    })
    .drop_without_applying_deltas();
    assert!(editor.landscape);
    assert!(!editor.landscape_requested);
    assert_eq!(editor.layout.fire.offset_x, 750);
}

#[test]
fn landscape_requires_rotation_and_preserves_edits_when_returning_to_portrait() {
    let mut editor = ControlEditorState::new(&crate::GameSettings::default(), (240, 320));
    editor.select_orientation(true);
    editor.layout.fire.offset_y = 750;
    let ctx = egui::Context::default();
    let theme = crate::MaterialTheme::fallback(crate::PlatformThemeMode::Dark);
    ctx.run_ui(egui::RawInput::default(), |ui| {
        orientation_selector(
            ui,
            &theme,
            Vec2::new(453.0, 960.0),
            crate::OrientationControl::HostWindow,
            &mut editor,
        );
    })
    .drop_without_applying_deltas();
    assert!(!editor.landscape);
    assert_eq!(editor.other_layout.fire.offset_y, 750);
    assert!(!landscape_available(Vec2::new(453.0, 960.0)));
    assert!(landscape_available(Vec2::new(960.0, 453.0)));
    editor.select_orientation(true);
    ctx.run_ui(egui::RawInput::default(), |ui| {
        orientation_selector(
            ui,
            &theme,
            Vec2::new(960.0, 453.0),
            crate::OrientationControl::HostWindow,
            &mut editor,
        );
    })
    .drop_without_applying_deltas();
    assert!(editor.landscape);
    assert_eq!(editor.layout.fire.offset_y, 750);
}

#[test]
fn landscape_rejects_dragging_and_resizing_keys_over_the_game_screen() {
    let mut editor = ControlEditorState::new(&crate::GameSettings::default(), (240, 320));
    editor.select_orientation(true);
    editor.selected = crate::ControlSelection::NumberKey(0);
    let host = Vec2::new(960.0, 432.0);
    let rect = Rect::from_min_size(Pos2::ZERO, host);
    let preview = ControlPreview::landscape(rect, host, &editor);
    let (geometry, defaults) = preview.geometry.unwrap();
    let canvas = preview.canvas.unwrap();
    let before = editor.layout.clone();
    editor.layout.number_keys[0] = super::super::selection::transform_after_drag(
        before.number_keys[0],
        geometry.number_keys[0].0,
        defaults.number_keys[0].0,
        canvas.center() - geometry.number_keys[0].0,
        preview.controls,
        false,
    );
    assert!(preview.reject_screen_overlap(&mut editor, &before));
    assert_eq!(editor.layout, before);
    let beside = Pos2::new(
        canvas.left() - geometry.number_keys[0].1 * 0.5 - 4.0,
        geometry.number_keys[0].0.y,
    );
    editor.layout.number_keys[0] = super::super::selection::transform_after_drag(
        before.number_keys[0],
        geometry.number_keys[0].0,
        defaults.number_keys[0].0,
        beside - geometry.number_keys[0].0,
        preview.controls,
        false,
    );
    assert!(!preview.reject_screen_overlap(&mut editor, &before));
    let beside_layout = editor.layout.clone();
    editor.layout.number_keys[0].size_percent = 200;
    assert!(preview.reject_screen_overlap(&mut editor, &beside_layout));
    assert_eq!(editor.layout, beside_layout);
}

#[test]
fn scaled_landscape_preview_drag_matches_gameplay_coordinates() {
    let mut editor = ControlEditorState::new(&crate::GameSettings::default(), (240, 320));
    editor.select_orientation(true);
    let host = Vec2::new(960.0, 432.0);
    let preview_rect = Rect::from_min_size(Pos2::new(15.0, 20.0), host * 0.5);
    let preview = ControlPreview::landscape(preview_rect, host, &editor);
    let (geometry, defaults) = preview.geometry.unwrap();
    let canvas = preview.canvas.unwrap();
    for (index, (center, size)) in geometry.number_keys.iter().enumerate() {
        let rect = Rect::from_center_size(*center, Vec2::splat(*size));
        assert!(!rect.intersects(canvas));
        assert_eq!(center.x < canvas.left(), index < 6);
    }
    let delta = Vec2::new(5.0, -10.0);
    editor.layout.number_keys[0] = super::super::selection::transform_after_drag(
        editor.layout.number_keys[0],
        geometry.number_keys[0].0,
        defaults.number_keys[0].0,
        delta,
        preview.controls,
        false,
    );
    let moved = ControlPreview::landscape(preview_rect, host, &editor)
        .geometry
        .unwrap()
        .0;
    assert!((moved.number_keys[0].0 - geometry.number_keys[0].0 - delta).length() < 0.1);
    editor.layout.direction_pad.offset_x = -500;
    let shifted = ControlPreview::landscape(preview_rect, host, &editor)
        .geometry
        .unwrap()
        .0;
    assert_eq!(shifted.number_keys, moved.number_keys);
    assert_ne!(shifted.direction_pad_center, moved.direction_pad_center);
}
