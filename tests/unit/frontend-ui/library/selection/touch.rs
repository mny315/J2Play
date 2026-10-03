use super::*;

fn touch(pos: Pos2, phase: egui::TouchPhase) -> Event {
    Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(1),
        phase,
        pos,
        force: None,
    }
}

#[test]
fn native_touch_hold_selects_once_through_motion_release_and_pointer_gone() {
    for view in [LibraryView::List, LibraryView::Tiles] {
        for duration in [0.55, 0.75, 1.5, 3.0] {
            let scratch = test_storage::Scratch::new();
            let mut app = library_fixture(&scratch.0, 3);
            app.app_settings.library_view = view;
            let ctx = egui::Context::default();
            for time in [0.0, 0.5, 1.0] {
                paint(&mut app, &ctx, time, vec![]);
            }
            let id = app.entries[0].id().to_owned();
            let pos = game_position(&app, &ctx, 0);
            paint(
                &mut app,
                &ctx,
                1.1,
                vec![
                    touch(pos, egui::TouchPhase::Start),
                    Event::PointerMoved(pos),
                    pointer(pos, true),
                ],
            );
            paint(&mut app, &ctx, 1.61, vec![]);
            assert!(app.library_selection.ids.contains(&id));
            paint(
                &mut app,
                &ctx,
                1.1 + duration,
                vec![touch(pos, egui::TouchPhase::Move), Event::PointerMoved(pos)],
            );
            assert!(
                app.library_selection.ids.contains(&id),
                "still held: {view:?}, {duration}"
            );
            // egui-winit sends PointerGone after the release on Android.
            paint(
                &mut app,
                &ctx,
                1.11 + duration,
                vec![
                    touch(pos, egui::TouchPhase::End),
                    pointer(pos, false),
                    Event::PointerGone,
                ],
            );
            paint(&mut app, &ctx, 1.5 + duration, vec![]);
            assert_eq!(
                app.library_selection.ids.len(),
                1,
                "released: {view:?}, {duration}"
            );
            assert!(app.library_selection.ids.contains(&id));
            assert_eq!(app.session.state(), SessionState::Idle);

            // The next deliberate tap can deselect the game normally.
            let pos = game_position(&app, &ctx, 0);
            paint(
                &mut app,
                &ctx,
                2.0 + duration,
                vec![
                    touch(pos, egui::TouchPhase::Start),
                    Event::PointerMoved(pos),
                    pointer(pos, true),
                ],
            );
            paint(
                &mut app,
                &ctx,
                2.1 + duration,
                vec![
                    touch(pos, egui::TouchPhase::End),
                    pointer(pos, false),
                    Event::PointerGone,
                ],
            );
            assert!(app.library_selection.ids.is_empty());
        }
    }
}
