use super::*;
use crate::{FrontendApp, Screen};

fn settings_app() -> (crate::tests::test_storage::Scratch, FrontendApp) {
    let scratch = crate::tests::test_storage::Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.startup_splash = None;
    (scratch, app)
}

#[test]
fn translated_library_group_actions_fit_narrow_screens() {
    let (_scratch, mut app) = settings_app();
    app.library_selection.toggle("fixture");
    let ctx = egui::Context::default();
    crate::apply_material_theme(&ctx, &app.material_theme);
    for language in Language::ALL {
        install(&ctx, language);
        for width in [240.0, 320.0, 453.0] {
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 800.0));
            for _ in 0..2 {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(viewport),
                        ..Default::default()
                    },
                    |ui| app.draw_library_selection_bar(ui),
                )
                .drop_without_applying_deltas();
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(viewport),
                    ..Default::default()
                },
                |ui| app.draw_library_selection_bar(ui),
            );
            output.textures_delta.clear();
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    let bounds = text.galley.rect.translate(text.pos.to_vec2());
                    assert!(
                        viewport.expand(0.5).contains_rect(bounds),
                        "{language:?}, width={width}: {:?} at {bounds:?}",
                        text.galley.job.text
                    );
                    assert!(!text.galley.elided);
                }
            }
            let actions = ctx
                .data(|data| {
                    data.get_temp::<Vec<egui::Id>>(egui::Id::new(
                        crate::library_folders::FOLDER_FOCUS,
                    ))
                })
                .unwrap();
            assert_eq!(actions.len(), 3);
            for id in actions {
                let rect = ctx.read_response(id).unwrap().interact_rect;
                assert!(
                    viewport.contains_rect(rect),
                    "{language:?}, width={width}: button at {rect:?}"
                );
                assert!(rect.height() >= 48.0);
            }
            output.drop_without_applying_deltas();
        }
    }
}

#[test]
fn translated_settings_and_editor_pages_stay_within_phone_widths() {
    let (_scratch, mut app) = settings_app();
    let ctx = egui::Context::default();
    crate::apply_material_theme(&ctx, &app.material_theme);
    for language in Language::ALL {
        install(&ctx, language);
        for width in [240.0, 320.0, 453.0] {
            for page in 0..3 {
                app.physical_editor = None;
                app.screen = if page == 1 {
                    Screen::Settings(Box::new(crate::SettingsScreen {
                        target: crate::SettingsTarget::Existing {
                            entry_id: "fixture".into(),
                        },
                        draft: crate::GameSettings::default(),
                        focus_profile: false,
                        show_all_profiles: false,
                        editor_request: None,
                        control_editor: None,
                    }))
                } else {
                    Screen::AppSettings(app.app_settings.clone().into())
                };
                if page >= 2 {
                    app.open_physical_editor();
                }
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 10_000.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default().show(ui, |ui| match page {
                            0 => app.draw_app_settings(ui),
                            1 => app.draw_settings(ui),
                            _ => app.draw_physical_editor(ui),
                        });
                    },
                );
                output.textures_delta.clear();
                let mut text_count = 0;
                for shape in &output.shapes {
                    if let egui::Shape::Text(text) = &shape.shape {
                        text_count += 1;
                        let bounds = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(
                            bounds.left() >= -0.5 && bounds.right() <= width + 0.5,
                            "{language:?}, width={width}, page={page}: {:?} at {bounds:?}",
                            text.galley.job.text
                        );
                        assert!(
                            !text.galley.elided,
                            "{language:?}, width={width}, page={page}: {:?}",
                            text.galley.job.text
                        );
                    }
                }
                assert!(text_count > 0, "{language:?}, width={width}, page={page}");
                output.drop_without_applying_deltas();
            }
        }
    }
}

#[test]
fn translated_save_cancel_and_reset_are_fully_readable_at_phone_widths() {
    let (_scratch, app) = settings_app();
    let ctx = egui::Context::default();
    crate::apply_material_theme(&ctx, &app.material_theme);
    for language in Language::ALL {
        install(&ctx, language);
        for width in [144.0, 240.0, 320.0, 453.0] {
            for with_reset in [false, true] {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 400.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        let actions = crate::settings_actions::SettingsActions::new(
                            ui,
                            ui.available_width(),
                            with_reset,
                        );
                        let _ = app.draw_settings_actions(ui, &actions);
                    },
                );
                output.textures_delta.clear();
                let mut count = 0;
                for shape in &output.shapes {
                    if let egui::Shape::Text(text) = &shape.shape {
                        count += 1;
                        assert!(
                            !text.galley.elided,
                            "{language:?} at {width}: {:?}",
                            text.galley.job.text
                        );
                        assert!(
                            shape
                                .clip_rect
                                .contains_rect(text.galley.rect.translate(text.pos.to_vec2())),
                            "{language:?} at {width}: {:?}",
                            text.galley.job.text
                        );
                    }
                }
                assert_eq!(count, if with_reset { 3 } else { 2 });
                output.drop_without_applying_deltas();
            }
        }
    }
}

#[test]
fn translated_confirmation_dialogs_keep_text_and_actions_on_screen() {
    let (_scratch, mut app) = settings_app();
    let ctx = egui::Context::default();
    crate::apply_material_theme(&ctx, &app.material_theme);
    for language in Language::ALL {
        install(&ctx, language);
        for width in [320.0, 453.0] {
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 800.0));
            app.safe_content_rect = viewport;
            for dialog in 0..9 {
                app.exit_confirmation = dialog == 5;
                app.data_action = (!(3..7).contains(&dialog)).then(|| crate::DataAction {
                    entry_ids: vec!["fixture".into(), "second".into()],
                    kind: match dialog {
                        0 => crate::DataActionKind::EntryRecord,
                        1 => crate::DataActionKind::PrivateArchives,
                        7 => crate::DataActionKind::ResumeSave,
                        8 => crate::DataActionKind::Game,
                        _ => crate::DataActionKind::RuntimeData,
                    },
                });
                app.display_error = (dialog == 6).then(|| crate::DisplayError {
                    title: "Could not save game".into(),
                    user_message: "Could not save the game state before stopping. Existing save files are kept.".into(),
                    technical_details: "Fixture error".into(),
                });
                for tick in 0..3 {
                    let mut output = ctx.run_ui(egui::RawInput {
                        screen_rect: Some(viewport), ..Default::default()
                    }, |ui| match dialog {
                        0..=2 | 7 | 8 => app.draw_data_action_confirmation(ui.ctx()),
                        3 | 4 => app.draw_resume_request(ui.ctx(), 1, "Fixture 123", dialog == 3,
                            if dialog == 3 { "An automatic save is available. Continue from where you left off?" }
                            else { "The automatic save cannot be used. You can start normally." }),
                        5 => app.draw_exit_confirmation(ui.ctx()),
                        _ => app.draw_error(ui.ctx()),
                    });
                    output.textures_delta.clear();
                    if tick == 2 {
                        let mut count = 0;
                        for shape in &output.shapes {
                            if let egui::Shape::Text(text) = &shape.shape {
                                count += 1;
                                let bounds = text.galley.rect.translate(text.pos.to_vec2());
                                assert!(
                                    viewport.expand(0.5).contains_rect(bounds),
                                    "{language:?}, width={width}, dialog={dialog}: {:?} at {bounds:?}",
                                    text.galley.job.text
                                );
                                assert!(
                                    !text.galley.elided,
                                    "{language:?}, width={width}, dialog={dialog}: {:?}",
                                    text.galley.job.text
                                );
                                assert!(
                                    shape.clip_rect.expand(0.5).contains_rect(bounds),
                                    "{language:?}, width={width}, dialog={dialog}: {:?} at {bounds:?} clipped by {:?}",
                                    text.galley.job.text,
                                    shape.clip_rect
                                );
                            }
                        }
                        assert!(count >= 3);
                    }
                    output.drop_without_applying_deltas();
                }
            }
        }
    }
}
