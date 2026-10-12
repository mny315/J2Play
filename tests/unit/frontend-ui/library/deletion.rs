use super::*;
use std::fs;

fn seed_saves(root: &Path, id: &str) -> Vec<std::path::PathBuf> {
    let paths = [
        format!("runtime/rms/{id}/score"),
        format!("runtime/files/{id}/save"),
        format!("runtime/resume/{id}/automatic.save"),
        format!("runtime/resume/{id}/active-storage"),
        format!("runtime/resume/{id}/storage-0/rms/score"),
        format!("runtime/resume/{id}/storage-1/files/save"),
    ];
    paths
        .into_iter()
        .map(|path| {
            let path = root.join("library").join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"saved data").unwrap();
            path
        })
        .collect()
}

#[test]
fn bulk_deletion_requires_confirmation_and_includes_selection_hidden_by_search() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 3);
    let entries = app.entries.clone();
    let saves: Vec<_> = entries
        .iter()
        .map(|entry| seed_saves(&scratch.0, entry.id()))
        .collect();
    let original = scratch.0.join("original.jar");
    fs::write(&original, b"user archive").unwrap();
    for index in [0, 2] {
        app.library_selection.toggle(entries[index].id());
    }
    app.library_search.open = true;
    app.library_search.query = "no matching games".into();
    app.open_delete_selected_games();
    assert_eq!(app.data_action.as_ref().unwrap().entry_ids.len(), 2);
    app.navigate_back();
    assert!(app.data_action.is_none());
    assert_eq!(app.library_selection.ids.len(), 2);
    assert!(saves.iter().flatten().all(|path| path.is_file()));
    assert_eq!(app.repository.load().unwrap().entries.len(), 3);

    app.open_delete_selected_games();
    let action = app.data_action.clone().unwrap();
    app.execute_data_action(&action);
    assert!(app.display_error.is_none());
    assert!(app.data_action.is_none());
    assert!(!app.library_selection.active);
    assert_eq!(app.entries, std::slice::from_ref(&entries[1]));
    assert_eq!(app.repository.load().unwrap().entries, app.entries);
    for (index, entry) in entries.iter().enumerate() {
        assert_eq!(app.repository.private_jar_path(entry).exists(), index == 1);
        for path in &saves[index] {
            assert_eq!(path.exists(), index == 1, "{}", path.display());
        }
    }
    assert_eq!(fs::read(original).unwrap(), b"user archive");
}

#[test]
fn failed_bulk_deletion_keeps_remaining_entries_selected_for_retry() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 3);
    let entries = app.entries.clone();
    for entry in &entries {
        app.library_selection.toggle(entry.id());
    }
    // A directory in place of an archive forces a deterministic I/O failure.
    let archive = app.repository.private_jar_path(&entries[1]);
    fs::remove_file(&archive).unwrap();
    fs::create_dir(&archive).unwrap();
    app.open_delete_selected_games();
    app.execute_data_action(&app.data_action.clone().unwrap());
    assert!(app.display_error.is_some());
    assert_eq!(app.entries, entries[1..]);
    assert_eq!(app.library_selection.ids.len(), 2);
    assert!(!app.library_selection.ids.contains(entries[0].id()));
    assert_eq!(app.repository.load().unwrap().entries.len(), 2);
    fs::remove_dir(archive).unwrap();
    app.navigate_back();
    app.open_delete_selected_games();
    app.execute_data_action(&app.data_action.clone().unwrap());
    assert!(app.display_error.is_none());
    assert!(app.entries.is_empty());
    assert!(app.repository.load().unwrap().entries.is_empty());
    assert!(!app.library_selection.active);
}

#[test]
fn clearing_save_state_keeps_settings_draft_and_in_game_saves() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 1);
    let entry = app.entries[0].clone();
    let saves = seed_saves(&scratch.0, entry.id());
    app.open_settings(entry.id(), false);
    let Screen::Settings(settings) = &mut app.screen else {
        panic!("settings expected")
    };
    settings.draft.fps_limit = FpsLimit::Manual {
        frames_per_second: 25,
    };
    let action = DataAction {
        entry_ids: vec![entry.id().to_owned()],
        kind: DataActionKind::ResumeSave,
    };
    app.execute_data_action(&action);
    assert!(app.display_error.is_none());
    let Screen::Settings(settings) = &app.screen else {
        panic!("settings expected")
    };
    assert_eq!(
        settings.draft.fps_limit,
        FpsLimit::Manual {
            frames_per_second: 25
        }
    );
    for (index, path) in saves.iter().enumerate() {
        assert_eq!(path.exists(), index != 2);
    }
    assert_eq!(app.repository.load().unwrap().entries, [entry]);
}

#[test]
fn selected_games_can_be_deleted_in_both_views_even_without_the_folder_catalog() {
    for view in [LibraryView::List, LibraryView::Tiles] {
        let scratch = test_storage::Scratch::new();
        let mut app = library_fixture(&scratch.0, 1);
        app.app_settings.library_view = view;
        app.library_folders.available = false;
        app.library_selection.toggle(app.entries[0].id());
        let ctx = egui::Context::default();
        apply_material_theme(&ctx, &app.material_theme);
        let paint = |app: &mut FrontendApp, events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 800.0))),
                    events,
                    ..Default::default()
                },
                |ui| app.draw_library(ui),
            )
            .drop_without_applying_deltas();
        };
        paint(&mut app, vec![]);
        let actions = ctx
            .data(|data| {
                data.get_temp::<Vec<egui::Id>>(egui::Id::new(crate::library_folders::FOLDER_FOCUS))
            })
            .unwrap();
        assert_eq!(actions.len(), 2, "only delete and cancel are enabled");
        let pos = ctx
            .read_response(actions[0])
            .unwrap()
            .interact_rect
            .center();
        for pressed in [true, false] {
            paint(
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        pressed,
                        button: egui::PointerButton::Primary,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(app.data_action.as_ref().unwrap().kind, DataActionKind::Game);
        assert_eq!(app.session.state(), SessionState::Idle);
        assert!(app.repository.private_jar_path(&app.entries[0]).is_file());
    }
}

#[test]
fn destructive_actions_do_not_run_during_an_active_session() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 1);
    let id = app.entries[0].id().to_owned();
    let saves = seed_saves(&scratch.0, &id);
    app.session.start(&id).unwrap();
    for kind in [DataActionKind::Game, DataActionKind::ResumeSave] {
        app.execute_data_action(&DataAction {
            entry_ids: vec![id.clone()],
            kind,
        });
        assert!(saves.iter().all(|path| path.is_file()));
        assert!(app.repository.private_jar_path(&app.entries[0]).is_file());
    }
}
