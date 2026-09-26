use super::*;
use crate::library_folders::{FolderDialog, FolderFilter};

#[test]
fn folder_actions_filter_both_views_persist_and_back_keeps_games() {
    for view in [LibraryView::List, LibraryView::Tiles] {
        let scratch = test_storage::Scratch::new();
        let mut app = library_fixture(&scratch.0, 8);
        app.app_settings.library_view = view;
        app.library_folders.edit(None, "Аркады".into());
        app.confirm_folder_dialog();
        let FolderFilter::Folder(id) = app.library_search.folder else {
            panic!("new folder must be selected")
        };
        let ctx = egui::Context::default();
        apply_material_theme(&ctx, &app.material_theme);
        let size = Vec2::new(453.0, 600.0);
        navigation::paint_library_size(&mut app, &ctx, size, None);
        assert!(app.library_search.indices.is_empty());
        app.library_folders.dialog = Some(FolderDialog::Move {
            entry_ids: vec![app.entries[3].id().to_owned()],
            folder: Some(id),
        });
        app.confirm_folder_dialog();
        navigation::paint_library_size(&mut app, &ctx, size, None);
        assert_eq!(app.library_search.indices, [3]);
        app.library_search.open = true;
        app.library_search.query = "no match".into();
        app.library_search.invalidate();
        navigation::paint_library_size(&mut app, &ctx, size, None);
        assert!(app.library_search.indices.is_empty());
        app.navigate_back();
        navigation::paint_library_size(&mut app, &ctx, size, None);
        assert_eq!(app.library_search.indices, [3]);
        assert_eq!(app.library_search.folder, FolderFilter::Folder(id));
        app.library_folders.edit(Some(id), "Новое имя".into());
        app.navigate_back();
        assert_eq!(
            app.repository
                .load_folders()
                .unwrap()
                .get(id)
                .unwrap()
                .name(),
            "Аркады"
        );
        app.library_folders.dialog = Some(FolderDialog::Delete {
            id,
            name: "Аркады".into(),
        });
        app.confirm_folder_dialog();
        navigation::paint_library_size(&mut app, &ctx, size, None);
        assert_eq!(app.library_search.indices.len(), 8);
        assert_eq!(app.repository.load().unwrap().entries.len(), 8);
        assert_eq!(app.library_search.folder, FolderFilter::All);
        assert!(!app.exit_confirmation);
    }
}

#[test]
fn common_selection_action_is_available_with_keyboard_and_controller_in_both_views() {
    for view in [LibraryView::List, LibraryView::Tiles] {
        for controller in [false, true] {
            let scratch = test_storage::Scratch::new();
            let mut app = library_fixture(&scratch.0, 2);
            app.app_settings.library_view = view;
            let ctx = egui::Context::default();
            apply_material_theme(&ctx, &app.material_theme);
            let paint = |app: &mut FrontendApp, key| {
                navigation::paint_library_navigation(
                    app,
                    &ctx,
                    Vec2::new(640.0, 600.0),
                    key,
                    controller,
                );
            };
            paint(&mut app, None);
            for key in [Key::ArrowDown, Key::ArrowUp, Key::ArrowRight, Key::Enter] {
                paint(&mut app, Some(key));
            }
            assert!(
                app.library_selection.active,
                "{view:?}, controller {controller}"
            );
            let first = crate::library_view::launch_id(app.entries[0].id());
            ctx.memory_mut(|memory| memory.request_focus(first));
            paint(&mut app, Some(Key::Enter));
            assert!(app.library_selection.ids.contains(app.entries[0].id()));
            paint(&mut app, Some(Key::ArrowUp));
            paint(&mut app, Some(Key::Enter));
            assert!(
                matches!(&app.library_folders.dialog, Some(FolderDialog::Move { entry_ids, .. }) if entry_ids == &[app.entries[0].id()])
            );
            assert_eq!(app.session.state(), SessionState::Idle);
        }
    }
}

#[test]
fn unfiled_filter_is_hidden_when_it_duplicates_all_games_and_fades_finish() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 2);
    let (catalog, id) = app.repository.create_folder("Folder").unwrap();
    app.library_folders.catalog = catalog;
    app.library_search
        .refresh(&app.entries, &app.library_folders.catalog);
    assert!(!app.library_search.has_filed_games);
    app.repository
        .move_to_folder(&mut app.entries[0], Some(id))
        .unwrap();
    app.library_search.invalidate();
    app.library_search
        .refresh(&app.entries, &app.library_folders.catalog);
    assert!(app.library_search.has_filed_games);
    app.select_library_folder(FolderFilter::Unfiled);
    app.library_search
        .refresh(&app.entries, &app.library_folders.catalog);
    assert_eq!(app.library_search.indices, [1]);
    let ctx = egui::Context::default();
    let mut fade = crate::library_folders::FolderFade::default();
    fade.request();
    let opacity = |fade: &mut crate::library_folders::FolderFade, time| {
        let mut result = 0.0;
        ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                ..Default::default()
            },
            |_ui| result = fade.opacity(&ctx),
        )
        .drop_without_applying_deltas();
        result
    };
    assert!(opacity(&mut fade, 10.0) < 0.1);
    assert!((0.4..0.7).contains(&opacity(&mut fade, 10.12)));
    assert!((opacity(&mut fade, 10.25) - 1.0).abs() < f32::EPSILON);
    assert!((opacity(&mut fade, 11.0) - 1.0).abs() < f32::EPSILON);
    fade.request();
    fade.settle();
    assert!((opacity(&mut fade, 12.0) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn folder_name_uses_unicode_native_editor_and_cancel_does_not_create_metadata() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 1);
    let ctx = egui::Context::default();
    app.safe_content_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 500.0));
    app.library_folders.edit(None, String::new());
    for mut event in [
        None,
        None,
        Some(PlatformTextInputEvent::Commit("Папка😀".into())),
        Some(PlatformTextInputEvent::DeleteUtf16 {
            before: 2,
            after: 0,
        }),
    ] {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(app.safe_content_rect),
                ..Default::default()
            },
            |_ui| {
                if let Some(event) = event.take() {
                    app.library_folders.native_input(&ctx, event);
                }
                app.draw_folder_dialog(&ctx);
            },
        )
        .drop_without_applying_deltas();
    }
    let Some(FolderDialog::Edit { name, .. }) = &app.library_folders.dialog else {
        panic!("name editor expected")
    };
    assert_eq!(name, "Папка");
    app.navigate_back();
    assert!(!app.overlay_active());
    assert!(app.repository.load_folders().unwrap().folders().is_empty());
    assert_eq!(app.session.state(), SessionState::Idle);
}

#[test]
fn corrupt_folders_keep_the_library_available_and_disable_folder_edits() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 1);
    app.runtime.shutdown().unwrap();
    drop(app);
    std::fs::write(scratch.0.join("library/folders.json"), b"broken").unwrap();
    let app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
    assert_eq!(app.entries.len(), 1);
    assert!(!app.library_folders.available);
    assert!(!app.library_warnings.is_empty());
}

#[test]
fn translated_folder_controls_fit_narrow_screens_and_long_names() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 1);
    let (catalog, id) = app
        .repository
        .create_folder(&"Long name ".repeat(6))
        .unwrap();
    app.library_folders.catalog = catalog;
    app.library_search.folder = FolderFilter::Folder(id);
    let ctx = egui::Context::default();
    for language in frontend_core::Language::ALL {
        crate::i18n::install(&ctx, language);
        for width in [240.0, 320.0, 453.0] {
            let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 600.0));
            app.safe_content_rect = bounds;
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(bounds),
                    ..Default::default()
                },
                |ui| {
                    app.draw_library_folder_bar(ui);
                    assert!(
                        ui.min_rect().right() <= width + 0.5,
                        "{language:?}, {width}: {:?}",
                        ui.min_rect()
                    );
                },
            )
            .drop_without_applying_deltas();
            app.library_folders.dialog = Some(FolderDialog::Delete {
                id,
                name: "Long name ".repeat(6),
            });
            for _ in 0..2 {
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(bounds),
                        ..Default::default()
                    },
                    |_ui| app.draw_folder_dialog(&ctx),
                );
                for shape in &output.shapes {
                    if let egui::Shape::Text(text) = &shape.shape {
                        let rect = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(
                            rect.left() >= -0.5 && rect.right() <= width + 0.5,
                            "{language:?}, {width}: {:?} at {rect:?}",
                            text.galley.job.text
                        );
                    }
                }
                output.drop_without_applying_deltas();
            }
            app.library_folders.dialog = None;
        }
    }
}
