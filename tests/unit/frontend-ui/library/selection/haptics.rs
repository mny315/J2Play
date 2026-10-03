use super::*;
use std::sync::{Arc, Mutex};

struct HapticPlatform(Arc<Mutex<Vec<u8>>>);

impl PlatformBridge for HapticPlatform {
    fn request_document(&mut self, _: DocumentKind) -> Result<(), EmuError> {
        unreachable!("selection feedback fixture does not import")
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }

    fn request_game_haptic(&mut self, strength: u8) -> Result<bool, EmuError> {
        self.0.lock().unwrap().push(strength);
        Ok(true)
    }
}

#[test]
fn selection_ticks_once_on_hold_and_toggle_and_respects_off() {
    for view in [LibraryView::List, LibraryView::Tiles] {
        let scratch = test_storage::Scratch::new();
        let mut app = library_fixture(&scratch.0, 2);
        let ticks = Arc::new(Mutex::new(Vec::new()));
        app.platform = Box::new(HapticPlatform(ticks.clone()));
        app.app_settings.library_view = view;
        app.app_settings.vibration.strength_percent = 37;
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
        paint(&mut app, &ctx, 1.5, vec![]);
        assert!(
            ticks.lock().unwrap().is_empty(),
            "press alone must not vibrate"
        );
        paint(&mut app, &ctx, 1.61, vec![]);
        assert_eq!(*ticks.lock().unwrap(), [37]);
        app.slider_haptic.last_tick = None;
        paint(&mut app, &ctx, 2.0, vec![]);
        paint(&mut app, &ctx, 2.1, vec![pointer(pos, false)]);
        paint(&mut app, &ctx, 2.2, vec![]);
        assert_eq!(
            *ticks.lock().unwrap(),
            [37],
            "hold and release must not repeat feedback"
        );
        // A tap on the game's body deselects it and gives the same tick.
        let pos = game_position(&app, &ctx, 0);
        paint(
            &mut app,
            &ctx,
            3.0,
            vec![Event::PointerMoved(pos), pointer(pos, true)],
        );
        paint(&mut app, &ctx, 3.1, vec![pointer(pos, false)]);
        assert!(app.library_selection.ids.is_empty());
        assert_eq!(*ticks.lock().unwrap(), [37, 37]);
        // The explicit checkbox follows the same feedback path as the body.
        ctx.memory_mut(|memory| {
            memory.request_focus(crate::library_view::launch_id(app.entries[0].id()));
        });
        app.move_library_focus(
            &ctx,
            if view == LibraryView::Tiles {
                egui::FocusDirection::Down
            } else {
                egui::FocusDirection::Right
            },
        );
        let checkbox = ctx.memory(egui::Memory::focused).unwrap();
        let pos = ctx.read_response(checkbox).unwrap().interact_rect.center();
        app.slider_haptic.last_tick = None;
        app.app_settings.vibration.strength_percent = 62;
        paint(
            &mut app,
            &ctx,
            4.0,
            vec![Event::PointerMoved(pos), pointer(pos, true)],
        );
        paint(&mut app, &ctx, 4.1, vec![pointer(pos, false)]);
        assert_eq!(*ticks.lock().unwrap(), [37, 37, 62]);
        app.slider_haptic.last_tick = None;
        app.app_settings.vibration.enabled = false;
        paint(&mut app, &ctx, 5.0, vec![pointer(pos, true)]);
        paint(&mut app, &ctx, 5.1, vec![pointer(pos, false)]);
        assert!(app.library_selection.ids.is_empty());
        assert_eq!(*ticks.lock().unwrap(), [37, 37, 62]);
        app.app_settings.vibration.enabled = true;
        paint(&mut app, &ctx, 5.2, vec![]);
        assert_eq!(
            *ticks.lock().unwrap(),
            [37, 37, 62],
            "Off must discard, not postpone, a tick"
        );
        app.library_selection.clear();
        let pos = game_position(&app, &ctx, 0);
        paint(
            &mut app,
            &ctx,
            6.0,
            vec![Event::PointerMoved(pos), pointer(pos, true)],
        );
        paint(&mut app, &ctx, 6.1, vec![Event::PointerCancelled]);
        paint(&mut app, &ctx, 7.0, vec![]);
        assert_eq!(*ticks.lock().unwrap(), [37, 37, 62]);
        assert_eq!(app.session.state(), SessionState::Idle);
    }
}
