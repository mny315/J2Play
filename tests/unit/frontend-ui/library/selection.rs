use super::*;
use crate::library_folders::{FolderDialog, FolderFilter};

mod haptics;
mod touch;

fn paint(app: &mut FrontendApp, ctx: &egui::Context, time: f64, events: Vec<Event>) {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 800.0))),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ui| app.draw_library(ui),
    )
    .drop_without_applying_deltas();
}

fn pointer(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn game_position(app: &FrontendApp, ctx: &egui::Context, index: usize) -> Pos2 {
    ctx.read_response(crate::library_view::launch_id(app.entries[index].id()))
        .unwrap()
        .interact_rect
        .center()
}

#[test]
fn hold_selects_multiple_games_without_launch_and_bulk_move_survives_restart() {
    for view in [LibraryView::List, LibraryView::Tiles] {
        for touch in [false, true] {
            let scratch = test_storage::Scratch::new();
            let mut app = library_fixture(&scratch.0, 3);
            app.app_settings.library_view = view;
            let (catalog, folder) = app.repository.create_folder("Picked").unwrap();
            app.library_folders.catalog = catalog;
            let ctx = egui::Context::default();
            apply_material_theme(&ctx, &app.material_theme);
            for time in [0.0, 0.5, 1.0] {
                paint(&mut app, &ctx, time, vec![]);
            }
            let pos = game_position(&app, &ctx, 0);
            let mut events = vec![Event::PointerMoved(pos), pointer(pos, true)];
            if touch {
                events.push(Event::Touch {
                    device_id: egui::TouchDeviceId(1),
                    id: egui::TouchId(1),
                    phase: egui::TouchPhase::Start,
                    pos,
                    force: None,
                });
            }
            paint(&mut app, &ctx, 1.1, events);
            paint(&mut app, &ctx, 1.61, vec![]);
            assert!(app.library_selection.active, "{view:?}, touch {touch}");
            assert_eq!(app.library_selection.ids.len(), 1);
            let mut events = vec![pointer(pos, false)];
            if touch {
                events.push(Event::Touch {
                    device_id: egui::TouchDeviceId(1),
                    id: egui::TouchId(1),
                    phase: egui::TouchPhase::End,
                    pos,
                    force: None,
                });
            }
            paint(&mut app, &ctx, 1.7, events);
            paint(&mut app, &ctx, 2.0, vec![]);
            assert_eq!(
                app.library_selection.ids.len(),
                1,
                "release must not toggle twice"
            );
            let second = game_position(&app, &ctx, 1);
            paint(
                &mut app,
                &ctx,
                2.1,
                vec![Event::PointerMoved(second), pointer(second, true)],
            );
            paint(&mut app, &ctx, 2.2, vec![pointer(second, false)]);
            assert_eq!(app.library_selection.ids.len(), 2);
            assert_eq!(app.session.state(), SessionState::Idle);
            app.open_move_to_folder();
            app.navigate_back();
            assert_eq!(
                app.library_selection.ids.len(),
                2,
                "dialog cancel preserves selection"
            );
            app.open_move_to_folder();
            let Some(FolderDialog::Move { folder: target, .. }) = &mut app.library_folders.dialog
            else {
                panic!("move dialog expected")
            };
            *target = Some(folder);
            app.confirm_folder_dialog();
            assert!(!app.library_selection.active);
            assert_eq!(app.library_search.folder, FolderFilter::Folder(folder));
            let stored = app.repository.load().unwrap();
            let catalog = app.repository.load_folders().unwrap();
            assert_eq!(
                stored
                    .entries
                    .iter()
                    .filter(|entry| catalog.folder_for(entry) == Some(folder))
                    .count(),
                2
            );
            assert_eq!(stored.entries.len(), 3);
        }
    }
}

#[test]
fn swipe_cancel_and_focus_loss_cancel_pending_selection() {
    for view in [LibraryView::List, LibraryView::Tiles] {
        for reason in 0..3 {
            let scratch = test_storage::Scratch::new();
            let mut app = library_fixture(&scratch.0, 8);
            app.app_settings.library_view = view;
            let ctx = egui::Context::default();
            for time in [0.0, 0.5, 1.0] {
                paint(&mut app, &ctx, time, vec![]);
            }
            let pos = game_position(&app, &ctx, 0);
            paint(
                &mut app,
                &ctx,
                1.1,
                vec![Event::PointerMoved(pos), pointer(pos, true)],
            );
            let cancel = match reason {
                0 => Event::PointerMoved(pos - Vec2::new(0.0, 40.0)),
                1 => Event::PointerCancelled,
                _ => Event::WindowFocused(false),
            };
            paint(&mut app, &ctx, 1.2, vec![cancel]);
            paint(&mut app, &ctx, 1.7, vec![]);
            assert!(!app.library_selection.active, "{view:?}, reason {reason}");
            assert_eq!(app.session.state(), SessionState::Idle);
        }
    }
}

#[test]
fn failed_bulk_move_keeps_only_unmoved_games_selected_for_retry() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 3);
    let (catalog, folder) = app.repository.create_folder("Picked").unwrap();
    app.library_folders.catalog = catalog;
    for entry in &app.entries {
        app.library_selection.toggle(entry.id());
    }
    app.open_move_to_folder();
    let Some(FolderDialog::Move { folder: target, .. }) = &mut app.library_folders.dialog else {
        panic!("move dialog expected")
    };
    *target = Some(folder);
    let record = scratch
        .0
        .join("library/entries")
        .join(format!("{}.json", app.entries[1].id()));
    let saved = std::fs::read(&record).unwrap();
    std::fs::write(&record, b"broken").unwrap();
    app.confirm_folder_dialog();
    assert!(app.display_error.is_some());
    assert_eq!(app.library_selection.ids.len(), 2);
    assert_eq!(
        app.library_folders.catalog.folder_for(&app.entries[0]),
        Some(folder)
    );
    assert!(
        matches!(&app.library_folders.dialog, Some(FolderDialog::Move { entry_ids, .. }) if entry_ids.len() == 2)
    );
    std::fs::write(&record, saved).unwrap();
    app.navigate_back();
    app.confirm_folder_dialog();
    assert!(app.library_folders.dialog.is_none());
    assert!(!app.library_selection.active);
    assert!(
        app.repository
            .load()
            .unwrap()
            .entries
            .iter()
            .all(|entry| app.library_folders.catalog.folder_for(entry) == Some(folder))
    );
}

#[test]
fn folder_dialog_preserves_background_opacity_and_focus_through_initial_measurement() {
    use eframe::App;
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 1);
    app.startup_splash = None;
    let ctx = egui::Context::default();
    apply_material_theme(&ctx, &app.material_theme);
    let mut frame = eframe::Frame::_new_kittest();
    for (time, open) in [
        (0.0, false),
        (0.5, false),
        (1.0, true),
        (1.02, false),
        (1.3, false),
    ] {
        if open {
            app.library_folders.edit(None, String::new());
        }
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 800.0))),
                time: Some(time),
                ..Default::default()
            },
            |ui| app.ui(ui, &mut frame),
        );
        let title = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "J2Play" => Some(text),
                _ => None,
            })
            .unwrap();
        assert!(
            (title.opacity_factor - 1.0).abs() < f32::EPSILON,
            "background must be dimmed only by scrim: {}",
            title.opacity_factor
        );
        output.drop_without_applying_deltas();
    }
    assert_eq!(
        ctx.memory(egui::Memory::focused),
        Some(crate::library_folders::name_field_id())
    );
    app.navigate_back();
    app.library_selection.toggle(app.entries[0].id());
    app.navigate_back();
    assert!(!app.library_selection.active);
    assert!(!app.exit_confirmation);
}

#[test]
fn moving_a_group_highlights_every_moved_game_with_one_shared_expiry() {
    for view in [LibraryView::List, LibraryView::Tiles] {
        let scratch = test_storage::Scratch::new();
        let mut app = library_fixture(&scratch.0, 4);
        app.app_settings.library_view = view;
        let (catalog, folder) = app.repository.create_folder("Destination").unwrap();
        app.library_folders.catalog = catalog;
        app.repository
            .move_to_folder(&mut app.entries[0], Some(folder))
            .unwrap();
        for index in [1, 3] {
            app.library_selection.toggle(app.entries[index].id());
        }
        app.open_move_to_folder();
        let Some(FolderDialog::Move { folder: target, .. }) = &mut app.library_folders.dialog
        else {
            panic!("move dialog expected")
        };
        *target = Some(folder);
        app.confirm_folder_dialog();
        let ctx = egui::Context::default();
        apply_material_theme(&ctx, &app.material_theme);
        paint(&mut app, &ctx, 100.0, vec![]);
        assert_eq!(
            ctx.memory(egui::Memory::focused),
            Some(crate::library_view::launch_id(app.entries[1].id()))
        );
        for (time, expected) in [(100.0, 1.0), (105.0, 0.5), (110.1, 0.0)] {
            ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    for index in 0..4 {
                        let strength = app.library_added_strength(ui, index);
                        let target = if [1, 3].contains(&index) {
                            expected
                        } else {
                            0.0
                        };
                        assert!(
                            (strength - target).abs() < 0.001,
                            "{view:?}, index {index}, time {time}: {strength}"
                        );
                    }
                },
            )
            .drop_without_applying_deltas();
        }
        assert!(app.library_added.is_none());
        assert!(!app.library_selection.active);
    }
}
