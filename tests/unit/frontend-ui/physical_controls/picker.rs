use super::*;

#[test]
fn compact_picker_retains_all_phone_and_app_actions_exactly_once() {
    let actions: Vec<_> = [ActionPage::Phone, ActionPage::Keypad, ActionPage::App]
        .into_iter()
        .flat_map(action_cells)
        .flatten()
        .map(|(_, action)| *action)
        .collect();
    assert_eq!(actions.len(), 28);
    for action in super::super::labels::PHONE_ACTIONS
        .into_iter()
        .map(PhysicalAction::Phone)
        .chain([
            PhysicalAction::Menu,
            PhysicalAction::Fullscreen,
            PhysicalAction::FastForward,
            PhysicalAction::DebugOverlay,
            PhysicalAction::StopGame,
        ])
    {
        assert_eq!(
            actions
                .iter()
                .filter(|candidate| **candidate == action)
                .count(),
            1
        );
        assert!(
            action_cells(ActionPage::for_action(action))
                .iter()
                .flatten()
                .any(|(_, candidate)| *candidate == action)
        );
    }
}

#[test]
fn translated_action_picker_fits_narrow_phones_and_keeps_all_labels() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.screen = crate::Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    let editor = app.physical_editor.as_mut().unwrap();
    let target = BindingTarget::Trigger { right: false };
    let ctx = egui::Context::default();
    crate::apply_material_theme(&ctx, &app.material_theme);
    for language in frontend_core::Language::ALL {
        crate::i18n::install(&ctx, language);
        for width in [176.0, 256.0, 389.0] {
            for page in [ActionPage::Phone, ActionPage::Keypad, ActionPage::App] {
                editor.page = page;
                for tick in 0..3 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 2000.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            apply_settings_style(ui);
                            ui.push_id((language, width.to_bits(), page), |ui| {
                                editor.draw_actions(ui, target, false, 1600.0);
                            });
                        },
                    );
                    output.textures_delta.clear();
                    if tick == 2 {
                        let mut labels = std::collections::BTreeSet::new();
                        for shape in &output.shapes {
                            if let egui::Shape::Text(text) = &shape.shape {
                                labels.insert(text.galley.text());
                                let bounds = text.galley.rect.translate(text.pos.to_vec2());
                                assert!(
                                    bounds.left() >= -0.5 && bounds.right() <= width + 0.5,
                                    "{language:?}, {width}, {page:?}: {:?} at {bounds:?}",
                                    text.galley.job.text
                                );
                                assert!(!text.galley.elided);
                            }
                        }
                        let tr = crate::i18n::Translator(language);
                        for (label, _) in action_cells(page).iter().flatten() {
                            assert!(
                                labels.contains(tr.text(label).as_str()),
                                "{language:?}, {width}, {page:?}: missing {label}"
                            );
                        }
                    }
                    output.drop_without_applying_deltas();
                }
            }
        }
    }
}
