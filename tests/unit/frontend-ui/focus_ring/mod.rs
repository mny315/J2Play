use super::*;
use std::time::Duration;

fn target(id: u64, x: f32, y: f32) -> FocusTarget {
    FocusTarget {
        id: egui::Id::new(id),
        rect: egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(180.0, 48.0)),
        clip: egui::Rect::EVERYTHING,
        layer: egui::LayerId::background(),
        radius: 24.0,
        pass: 0,
    }
}

#[test]
fn programmatic_focus_waits_for_navigation_and_touch_or_capture_does_not_reveal_it() {
    let ctx = egui::Context::default();
    let theme = crate::MaterialTheme::fallback(crate::PlatformThemeMode::Dark);
    crate::apply_material_theme(&ctx, &theme);
    let mut ring = FocusRing::default();
    let mut paint = |events, capturing| {
        let output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                ring.observe_input(&ctx, capturing);
                let response = ui.add(crate::material_primary_button(&theme, "Initial selection"));
                response.request_focus();
                track(ui, &response, 24.0);
                ring.paint(&ctx, &theme, true, ui.clip_rect());
            },
        );
        let painted = output.shapes.iter().any(|shape| {
            matches!(&shape.shape,
            egui::Shape::Rect(rect) if (rect.stroke.width - 2.5).abs() < f32::EPSILON)
        });
        output.drop_without_applying_deltas();
        painted
    };
    assert!(!paint(vec![], false));
    assert!(!paint(vec![], false));
    let down = egui::Event::Key {
        key: egui::Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    assert!(
        !paint(vec![down.clone()], true),
        "capturing a physical key must not enter navigation"
    );
    assert!(paint(vec![down.clone()], false));
    assert!(paint(vec![], false));
    assert!(!paint(
        vec![egui::Event::PointerButton {
            pos: egui::pos2(12.0, 12.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE
        }],
        false
    ));
    assert!(!paint(vec![], false));
    assert!(paint(vec![down], false));
}

#[test]
fn keyboard_navigation_reveals_the_ring_before_consuming_the_arrow() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app =
        crate::FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.screen = crate::Screen::AppSettings(app.app_settings.clone().into());
    let ctx = egui::Context::default();
    let mut ids = Vec::new();
    for frame in 0..3 {
        let events = if frame == 1 {
            vec![egui::Event::Key {
                key: egui::Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]
        } else {
            vec![]
        };
        let output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                app.process_physical_inputs(&ctx, true, false);
                ids.clear();
                for index in 0..2 {
                    let response = ui.add_sized(
                        [180.0, 48.0],
                        crate::material_outlined_button(&app.material_theme, index.to_string()),
                    );
                    if frame == 0 && index == 0 {
                        response.request_focus();
                    }
                    ids.push(response.id);
                }
                app.focus_ring
                    .paint(&ctx, &app.material_theme, true, ui.clip_rect());
            },
        );
        let visible = output.shapes.iter().any(|shape| {
            matches!(&shape.shape,
            egui::Shape::Rect(rect) if (rect.stroke.width - 2.5).abs() < f32::EPSILON)
        });
        assert_eq!(visible, frame > 0);
        output.drop_without_applying_deltas();
    }
    assert_eq!(ctx.memory(egui::Memory::focused), Some(ids[1]));
}

#[test]
fn focus_motion_retargets_without_jumping_or_delaying_the_selected_id() {
    let now = Instant::now();
    let mut ring = FocusRing::default();
    let first = target(1, 10.0, 10.0);
    let second = target(2, 10.0, 210.0);
    ring.sync(Some(first), now);
    let settled = now + Duration::from_millis(240);
    ring.sync(Some(first), settled);
    assert!(ring.started.is_none());
    ring.sync(Some(second), settled);
    assert_eq!(ring.target.unwrap().id, second.id);
    assert_eq!(ring.sample(settled).unwrap().0, first.rect);
    let mid = settled + Duration::from_millis(80);
    let moving = ring.sample(mid).unwrap().0;
    assert!(moving.top() > first.rect.top() && moving.top() < second.rect.top());
    let mut third = target(3, 260.0, 210.0);
    third.rect.set_width(96.0);
    ring.sync(Some(third), mid);
    assert_eq!(ring.sample(mid).unwrap().0, moving);
    let mut scrolled = third;
    scrolled.rect = third.rect.translate(egui::vec2(0.0, -30.0));
    ring.sync(Some(scrolled), mid + Duration::from_millis(40));
    assert_eq!(ring.started, Some(mid));
    let width = ring
        .sample(mid + Duration::from_millis(40))
        .unwrap()
        .0
        .width();
    assert!(width > third.rect.width() && width < moving.width());
    ring.sync(Some(scrolled), mid + Duration::from_millis(240));
    assert_eq!(
        ring.sample(mid + Duration::from_millis(240)).unwrap().0,
        scrolled.rect
    );
    assert!(ring.started.is_none());
    ring.sync(None, mid + Duration::from_millis(250));
    assert!(ring.sample(mid + Duration::from_millis(250)).is_none());
}

#[test]
fn every_material_button_has_a_visible_focus_ring_and_disabled_buttons_do_not() {
    use crate::theme::*;
    for mode in [PlatformThemeMode::Light, PlatformThemeMode::Dark] {
        let ctx = egui::Context::default();
        let theme = MaterialTheme::fallback(mode);
        apply_material_theme(&ctx, &theme);
        let mut ring = FocusRing::default();
        ring.navigate();
        let buttons = || {
            [
                material_primary_button(&theme, "Primary"),
                material_tonal_button(&theme, "Settings"),
                material_choice_button("Selected", true),
                material_outlined_button(&theme, "Outlined"),
                material_text_button(&theme, "Cancel"),
                material_danger_button(&theme, "Danger"),
                material_error_outlined_button(&theme, "Error"),
            ]
        };
        let mut ids = Vec::new();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            for button in buttons() {
                ids.push(ui.add(button).id);
            }
        })
        .drop_without_applying_deltas();
        for id in ids {
            ctx.memory_mut(|memory| memory.request_focus(id));
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                for button in buttons() {
                    ui.add(button);
                }
                let current = ctx
                    .data(|data| data.get_temp::<FocusTarget>(egui::Id::new(TARGET_ID)))
                    .unwrap();
                ring.sync(
                    Some(current),
                    Instant::now()
                        .checked_sub(Duration::from_millis(300))
                        .unwrap(),
                );
                ring.paint(&ctx, &theme, true, ui.clip_rect());
            });
            assert_eq!(ring.target.unwrap().id, id);
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(rect) if rect.stroke.color == theme.primary && (rect.stroke.width - 2.5).abs() < f32::EPSILON)));
            output.drop_without_applying_deltas();
        }
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.add_enabled(false, material_primary_button(&theme, "Disabled"));
            ring.paint(&ctx, &theme, true, ui.clip_rect());
        })
        .drop_without_applying_deltas();
        assert!(ring.target.is_none());
    }
}

#[test]
fn scroll_view_keeps_the_entire_focus_ring_visible_at_each_content_edge() {
    for viewport in [egui::vec2(360.0, 160.0), egui::vec2(220.0, 100.0)] {
        let ctx = egui::Context::default();
        let theme = MaterialTheme::fallback(crate::PlatformThemeMode::Dark);
        crate::apply_material_theme(&ctx, &theme);
        let mut style = (*ctx.global_style()).clone();
        style.scroll_animation = egui::style::ScrollAnimation::none();
        ctx.set_global_style(style);
        let mut ring = FocusRing::default();
        let paint = |ring: &mut FocusRing| {
            let mut ids = Vec::new();
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(420.0, 280.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    ids.clear();
                    egui::CentralPanel::default().show(ui, |ui| {
                        egui::ScrollArea::both()
                            .max_width(viewport.x)
                            .max_height(viewport.y)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                egui::Grid::new("focus-scroll-buttons").num_columns(3).show(
                                    ui,
                                    |ui| {
                                        for index in 0..12 {
                                            let response = ui.add_sized(
                                                [120.0, 52.0],
                                                crate::material_choice_button(
                                                    index.to_string(),
                                                    false,
                                                ),
                                            );
                                            ids.push(response.id);
                                            if index % 3 == 2 {
                                                ui.end_row();
                                            }
                                        }
                                    },
                                );
                            });
                        ring.paint(&ctx, &theme, true, ui.clip_rect());
                    });
                },
            );
            (output, ids)
        };
        // IDs are stable, including the buttons outside the initial viewport.
        let (output, ids) = paint(&mut ring);
        output.drop_without_applying_deltas();
        for index in [0, 1, 2, 9, 10, 11, 0] {
            ctx.memory_mut(|memory| memory.request_focus(ids[index]));
            ring = FocusRing::default();
            ring.navigate();
            for _ in 0..4 {
                paint(&mut ring).0.drop_without_applying_deltas();
            }
            let (output, _) = paint(&mut ring);
            assert_eq!(ring.target.unwrap().id, ids[index]);
            let outlines: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if (rect.stroke.width - 2.5).abs() < f32::EPSILON => {
                        Some((shape.clip_rect, rect.rect))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(outlines.len(), 1, "the focused button needs one outline");
            let (clip, outline) = outlines[0];
            assert!(
                clip.contains_rect(outline),
                "button {index} at {viewport:?}: outline {outline:?} is clipped by {clip:?}"
            );
            output.drop_without_applying_deltas();
        }
    }
}

#[test]
fn shrinking_the_window_keeps_the_same_focused_setting_visible() {
    let ctx = egui::Context::default();
    let theme = crate::MaterialTheme::fallback(crate::PlatformThemeMode::Dark);
    crate::apply_material_theme(&ctx, &theme);
    let mut style = (*ctx.global_style()).clone();
    style.scroll_animation = egui::style::ScrollAnimation::none();
    ctx.set_global_style(style);
    let mut ring = FocusRing::default();
    let mut paint = |height| {
        let mut focused = None;
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, height),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for index in 0..12 {
                            let response = ui.add_sized(
                                [300.0, 48.0],
                                crate::material_choice_button(format!("Setting {index}"), false),
                            );
                            if index == 8 {
                                focused = Some(response.id);
                            }
                        }
                    });
                });
                ring.paint(&ctx, &theme, true, ui.max_rect());
            },
        )
        .drop_without_applying_deltas();
        focused.unwrap()
    };
    let id = paint(800.0);
    ctx.memory_mut(|memory| memory.request_focus(id));
    for _ in 0..3 {
        paint(800.0);
    }
    for _ in 0..4 {
        paint(220.0);
    }
    assert!(ctx.memory(|memory| memory.has_focus(id)));
    let target = ring.target.unwrap();
    assert!(
        target.clip.contains_rect(target.rect),
        "focused setting is outside the resized scroll viewport"
    );
}

#[test]
fn focus_motion_requests_every_display_frame_at_high_refresh_rates() {
    for refresh_rate in [60.0, 120.0, 240.0] {
        let ctx = egui::Context::default();
        let theme = MaterialTheme::fallback(crate::PlatformThemeMode::Dark);
        let mut ring = FocusRing::default();
        ring.navigate();
        for _ in 0..8 {
            let output = ctx.run_ui(
                egui::RawInput {
                    predicted_dt: 1.0 / refresh_rate,
                    ..Default::default()
                },
                |ui| {
                    let response = ui.button("Focused action");
                    response.request_focus();
                    track(ui, &response, 24.0);
                    ring.paint(&ctx, &theme, true, ui.clip_rect());
                },
            );
            assert!(ring.started.is_some());
            assert_eq!(
                output.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
                Duration::ZERO
            );
            output.drop_without_applying_deltas();
        }
    }
}
