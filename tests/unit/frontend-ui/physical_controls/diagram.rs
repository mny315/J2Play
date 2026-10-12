use super::*;

#[test]
fn artwork_keeps_a_transparent_background_and_opaque_body() {
    let image = image::load_from_memory_with_format(BODY_BYTES, image::ImageFormat::Png)
        .unwrap()
        .into_rgba8();
    assert_eq!(image.dimensions(), (1774, 887));
    for (x, y) in [(0, 0), (887, 0), (1773, 886), (887, 800)] {
        assert_eq!(image.get_pixel(x, y)[3], 0, "background at {x}, {y}");
    }
    assert!(image.get_pixel(887, 400)[3] >= 250);
}

#[test]
fn every_control_and_its_focus_margin_stays_inside_the_artwork_at_all_scales() {
    let image = image::load_from_memory_with_format(BODY_BYTES, image::ImageFormat::Png)
        .unwrap()
        .into_rgba8();
    for width in [160.0, 213.0, 304.0, 360.0, 480.0, 640.0, 800.0] {
        let controls = parts(width);
        for (index, part) in controls.iter().enumerate() {
            let rect = egui::Rect::from_center_size(part.center, part.size);
            for other in &controls[index + 1..] {
                assert!(
                    !rect
                        .intersect(egui::Rect::from_center_size(other.center, other.size))
                        .is_positive(),
                    "{} overlaps {} at {width}",
                    part.label,
                    other.label
                );
            }
            // Check the complete target, including the focus outline, against
            // the actual PNG alpha rather than an approximation of its contour.
            let footprint = rect.expand(f32::from(crate::focus_ring::OUTSET));
            for y in 0..=20_u8 {
                for x in 0..=20_u8 {
                    let point = footprint.min
                        + footprint.size() * egui::vec2(f32::from(x) / 20.0, f32::from(y) / 20.0);
                    if part.round
                        && !rect.contains(point)
                        && point.distance(rect.center()) > footprint.width() * 0.5
                    {
                        continue;
                    }
                    let pixel = BODY_CROP.min + point.to_vec2() / width * BODY_CROP.width();
                    assert!(
                        pixel.x >= 0.0 && pixel.x < 1774.0 && pixel.y >= 0.0 && pixel.y < 887.0
                    );
                    // The bounds above make flooring a sample to its PNG pixel safe.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let alpha = image.get_pixel(pixel.x as u32, pixel.y as u32)[3];
                    assert!(
                        alpha >= 250,
                        "{} leaves the body at width {width}: {pixel:?}",
                        part.label
                    );
                }
            }
        }
    }
}

#[test]
fn diagram_and_accessible_controls_fit_phone_tablet_and_desktop_views() {
    for width in [160.0, 213.0, 304.0, 360.0, 480.0, 640.0, 800.0, 1280.0] {
        let ctx = egui::Context::default();
        let theme = crate::MaterialTheme::fallback(crate::PlatformThemeMode::Dark);
        let mut diagram = Diagram::default();
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 1200.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let available = ui.available_rect_before_wrap();
                    diagram.draw(ui, &theme, &PhysicalBindings::default(), false);
                    assert!(
                        ui.min_rect().right() <= available.right() + 0.1,
                        "overflow at {width}"
                    );
                },
            );
            output.textures_delta.clear();
            if width < DESIGN_WIDTH {
                let buttons: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect)
                            if rect.fill == theme.surface_container
                                && rect.rect.top() > body_size(width).y =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(buttons.len(), parts(width).len());
                for rect in buttons {
                    assert!(rect.width() >= 48.0 && rect.height() >= 48.0);
                    assert!(rect.right() <= width);
                }
            }
        }
        assert!(diagram.texture.as_ref().unwrap().is_ok());
    }
}

#[test]
fn scaled_diagram_controls_open_their_picker_by_pointer_and_keyboard() {
    for width in [213.0, 360.0, 800.0] {
        let ctx = egui::Context::default();
        let theme = crate::MaterialTheme::fallback(crate::PlatformThemeMode::Dark);
        let mut diagram = Diagram::default();
        let mut frame = |events, focus| {
            let mut selected = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 1200.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    selected = diagram.draw(ui, &theme, &PhysicalBindings::default(), focus);
                },
            );
            output.textures_delta.clear();
            selected
        };
        frame(Vec::new(), false);
        for part in parts(width) {
            for pressed in [true, false] {
                let selected = frame(
                    vec![
                        egui::Event::PointerMoved(part.center),
                        egui::Event::PointerButton {
                            pos: part.center,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    false,
                );
                if !pressed {
                    assert_eq!(selected, Some(part.picker), "{} at {width}", part.label);
                }
            }
        }
        frame(Vec::new(), true);
        let selected = frame(
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            false,
        );
        assert_eq!(
            selected,
            Some(Picker::Binding(BindingTarget::Trigger { right: false }))
        );
    }
}

#[test]
fn translated_control_labels_fit_their_faces_at_phone_and_desktop_scales() {
    let ctx = egui::Context::default();
    let theme = crate::MaterialTheme::fallback(crate::PlatformThemeMode::Dark);
    for language in frontend_core::Language::ALL {
        crate::i18n::install(&ctx, language);
        for width in [213.0, 320.0, 640.0] {
            for part in parts(width) {
                let rect = egui::Rect::from_center_size(part.center, part.size);
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    paint_part(
                        ui.painter(),
                        &theme,
                        &part,
                        rect,
                        false,
                        width / DESIGN_WIDTH,
                    );
                });
                output.textures_delta.clear();
                for shape in &output.shapes {
                    if let egui::Shape::Text(text) = &shape.shape {
                        let bounds = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(
                            rect.contains_rect(bounds),
                            "{language:?}, {width}: {} at {bounds:?}",
                            part.label
                        );
                        assert!(!text.galley.elided);
                    }
                }
                output.drop_without_applying_deltas();
            }
        }
    }
}
