use super::*;

fn wait_for_state(app: &mut FrontendApp, ctx: &egui::Context, expected: SessionState) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.session.state() != expected && Instant::now() < deadline {
        app.process_runtime_events(ctx);
        if expected == SessionState::Running
            && let Some(request) = app.host_request.clone()
            && matches!(
                request.kind,
                frontend_core::HostRequestKind::ResumeGame {
                    can_resume: true,
                    ..
                }
            )
        {
            app.resolve_resume_request(request.request_id, true, false);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(app.session.state(), expected, "{:?}", app.display_error);
}

fn press(app: &mut FrontendApp, ctx: &egui::Context, control: PhysicalControl, pressed: bool) {
    // Layout changes may block guest input, but must not delay Stop.
    app.process_physical_event(
        ctx,
        PhysicalInputEvent::Button {
            device: 1,
            control,
            pressed,
        },
        false,
    );
}

fn stop_current_game(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    control: PhysicalControl,
    state: SessionState,
) {
    ctx.run_ui(egui::RawInput::default(), |ui| app.draw_gameplay(ui))
        .drop_without_applying_deltas();
    app.set_gameplay_fullscreen(true).unwrap();
    assert!(app.gameplay_transition.busy());
    ctx.input_mut(|input| input.focused = false);
    press(app, ctx, control, true);
    ctx.input_mut(|input| input.focused = true);
    press(app, ctx, control, true);
    assert_eq!(
        app.session.state(),
        state,
        "focus loss requires a fresh press"
    );
    press(app, ctx, control, false);
    app.platform_suspended = true;
    press(app, ctx, control, true);
    assert_eq!(
        app.session.state(),
        state,
        "suspended input cannot stop a game"
    );
    app.platform_suspended = false;
    press(app, ctx, control, false);
    app.touch_owners
        .insert(1, crate::TouchOwner::VirtualKey(Some(HostAction::Fire)));
    press(app, ctx, control, true);
    assert_eq!(app.session.state(), SessionState::Stopping);
    assert!(app.touch_owners.is_empty());
    assert!(!app.gameplay_transition.busy());
    wait_for_state(app, ctx, SessionState::Idle);
    assert!(matches!(app.screen, Screen::Library));
    assert!(!app.exit_requested && !app.exit_confirmation);
    assert!(app.display_error.is_none(), "{:?}", app.display_error);
    press(app, ctx, control, true);
    assert!(
        matches!(app.screen, Screen::Library),
        "a held Stop cannot activate the library"
    );
    press(app, ctx, control, false);
}

#[test]
fn stop_binding_returns_loading_running_and_paused_games_to_the_library() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    let prepared = frontend_core::inspect_import(frontend_core::ImportSource::new(
        None,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/java-me/conformance.jar"
        ))
        .to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(2)
    .unwrap();
    let entry = app
        .repository
        .commit_import(&prepared, crate::GameSettings::default())
        .unwrap();
    let id = entry.id().to_owned();
    app.entries.push(entry);
    app.fullscreen_help = crate::FullscreenHelpState::Acknowledged;
    let ctx = egui::Context::default();
    for control in [
        PhysicalControl::Keyboard { usage: 68 },
        PhysicalControl::Gamepad {
            button: GamepadButton::Start,
        },
    ] {
        app.app_settings
            .physical_bindings
            .assign(control, PhysicalAction::StopGame)
            .unwrap();
        for state in [
            SessionState::Starting,
            SessionState::Running,
            SessionState::Paused {
                user: true,
                lifecycle: false,
            },
        ] {
            app.open_game(&id);
            if state != SessionState::Starting {
                wait_for_state(&mut app, &ctx, SessionState::Running);
            }
            if matches!(state, SessionState::Paused { .. }) {
                let pause = app
                    .session
                    .set_paused(frontend_core::PauseReason::User, true)
                    .unwrap();
                app.runtime.submit(pause).unwrap();
            }
            stop_current_game(&mut app, &ctx, control, state);
        }
    }
    app.runtime.shutdown().unwrap();
}
