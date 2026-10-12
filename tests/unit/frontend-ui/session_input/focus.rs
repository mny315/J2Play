use super::physical::gameplay_fixture;
use super::*;
use eframe::App;

fn key(pressed: bool) -> Event {
    Event::Key {
        key: Key::ArrowUp,
        physical_key: Some(Key::ArrowUp),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn render(app: &mut FrontendApp, ctx: &egui::Context, focused: bool, events: Vec<Event>) {
    let mut frame = eframe::Frame::_new_kittest();
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(453.0, 960.0),
            )),
            focused,
            events,
            ..Default::default()
        },
        |ui| {
            app.logic(ui.ctx(), &mut frame);
            app.ui(ui, &mut frame);
        },
    )
    .drop_without_applying_deltas();
    assert!(app.display_error.is_none(), "{:?}", app.display_error);
}

#[test]
fn focus_loss_keeps_key_releases_from_the_same_batch() {
    for text_input in [false, true] {
        for release_first in [false, true] {
            let (_scratch, mut app, ctx) = gameplay_fixture();
            app.startup_splash = None;
            app.fullscreen_help = crate::FullscreenHelpState::Acknowledged;
            app.set_platform_text_input(text_input);
            render(&mut app, &ctx, true, vec![]);
            render(&mut app, &ctx, true, vec![key(true)]);
            app.observed_input = Some(Vec::new());
            let mut events = vec![Event::WindowFocused(false), key(false)];
            if release_first {
                events.reverse();
            }
            render(&mut app, &ctx, false, events);
            assert!(app.observed_input.as_ref().unwrap().is_empty());
            render(
                &mut app,
                &ctx,
                true,
                vec![Event::WindowFocused(true), key(true), key(false)],
            );
            assert_eq!(
                app.observed_input.take().unwrap(),
                [KeyState::Pressed, KeyState::Released].map(|state| InputEvent::Key {
                    action: HostAction::Up,
                    state,
                }),
                "text_input={text_input}, release_first={release_first}"
            );
        }
    }
}

#[test]
fn focus_loss_blocks_new_presses_until_the_keys_are_released() {
    for text_input in [false, true] {
        let (_scratch, mut app, ctx) = gameplay_fixture();
        app.startup_splash = None;
        app.fullscreen_help = crate::FullscreenHelpState::Acknowledged;
        app.set_platform_text_input(text_input);
        render(&mut app, &ctx, true, vec![]);
        app.observed_input = Some(Vec::new());
        render(
            &mut app,
            &ctx,
            false,
            vec![Event::WindowFocused(false), key(true)],
        );
        render(
            &mut app,
            &ctx,
            true,
            vec![Event::WindowFocused(true), key(true)],
        );
        assert!(app.observed_input.as_ref().unwrap().is_empty());
        render(
            &mut app,
            &ctx,
            true,
            vec![key(false), key(true), key(false)],
        );
        assert_eq!(
            app.observed_input.take().unwrap(),
            [KeyState::Pressed, KeyState::Released].map(|state| InputEvent::Key {
                action: HostAction::Up,
                state,
            })
        );
    }
}

#[test]
fn focus_loss_blocks_host_shortcuts() {
    for paused in [false, true] {
        for key in [Key::F11, Key::BrowserBack] {
            let (_scratch, mut app, ctx) = gameplay_fixture();
            app.startup_splash = None;
            app.fullscreen_help = crate::FullscreenHelpState::Acknowledged;
            if paused {
                app.session
                    .set_paused(frontend_core::PauseReason::User, true)
                    .unwrap();
            }
            render(&mut app, &ctx, true, vec![]);
            // Losing and regaining focus can arrive together, with the final
            // InputState already focused. The entire batch remains a barrier.
            render(
                &mut app,
                &ctx,
                true,
                vec![
                    Event::WindowFocused(false),
                    Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                    Event::WindowFocused(true),
                ],
            );
            assert!(matches!(&app.screen, Screen::Gameplay(game) if !game.fullscreen));
            assert!(!app.exit_confirmation, "paused={paused}, key={key:?}");
        }
    }
}
