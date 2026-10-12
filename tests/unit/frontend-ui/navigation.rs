use super::*;
use crate::gameplay_toolbar::{
    GAMEPLAY_TOOLBAR_HEIGHT, GameplayToolbarRequest, gameplay_toolbar_geometry,
    gameplay_toolbar_spacing,
};
use std::sync::atomic::{AtomicBool, Ordering};

#[path = "navigation/desktop_close.rs"]
mod desktop_close;

struct BackPlatform(Arc<AtomicBool>);

#[test]
fn back_keeps_host_request_when_the_worker_cannot_receive_the_denial() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
    app.runtime.shutdown().unwrap();
    app.session.start("fixture").unwrap();
    app.host_request = Some(
        HostRequest::new(
            7,
            HostRequestKind::PlatformRequest {
                url: "https://example.invalid/fixture".into(),
            },
        )
        .unwrap(),
    );

    app.navigate_back();
    assert!(app.display_error.is_some());
    assert_eq!(
        app.host_request.as_ref().map(|request| request.request_id),
        Some(7)
    );
    app.navigate_back();
    assert!(app.display_error.is_none());
    assert_eq!(
        app.host_request.as_ref().map(|request| request.request_id),
        Some(7)
    );
    assert!(!app.exit_confirmation);
}

struct DesktopClosePlatform(Arc<AtomicBool>);

impl PlatformBridge for DesktopClosePlatform {
    fn close_exits_app(&self) -> bool {
        true
    }

    fn cancel_pending_operations(&mut self) {
        self.0.store(true, Ordering::Release);
    }

    fn request_document(&mut self, _kind: DocumentKind) -> Result<(), EmuError> {
        Ok(())
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }
}

#[test]
fn desktop_close_cancels_a_pending_picker_and_joins_the_worker_even_during_splash() {
    use eframe::App;
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut app =
        FrontendApp::new(root, Box::new(DesktopClosePlatform(cancelled.clone()))).unwrap();
    app.request_picker(DocumentKind::Jar);
    assert!(app.picker_pending);
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let mut frame = eframe::Frame::_new_kittest();
    ctx.run_ui(input, |ui| app.logic(ui.ctx(), &mut frame))
        .drop_without_applying_deltas();
    assert!(cancelled.load(Ordering::Acquire));
    assert!(app.exit_requested);
    assert!(!app.exit_confirmation);
    app.runtime.shutdown().unwrap();
}

struct OrientationPlatform(Arc<std::sync::Mutex<Vec<PlatformOrientation>>>);

impl PlatformBridge for OrientationPlatform {
    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        UnavailablePlatformBridge.request_document(kind)
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }

    fn set_orientation(&mut self, orientation: PlatformOrientation) -> Result<(), EmuError> {
        self.0.lock().unwrap().push(orientation);
        Ok(())
    }
}

#[test]
fn rotation_replaces_pause_preserves_session_and_restores_system_orientation() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut app = FrontendApp::new(root, Box::new(OrientationPlatform(requests.clone()))).unwrap();
    let ctx = egui::Context::default();
    for landscape in [false, true] {
        let gameplay = GameplayScreen {
            entry_id: "fixture".into(),
            title: "Rotation fixture".into(),
            canvas_dimensions: (240, 320),
            pointer_events: false,
            game_scale: GameScale::AutomaticFit,
            portrait_frame_percent: None,
            control_layout: VirtualControlLayout::default(),
            landscape_control_layout: VirtualControlLayout::default(),
            vibration: VibrationSettings::default(),
            orientation: None,
            fast_forward: false,
            fullscreen: false,
        };
        app.screen = Screen::Gameplay(Box::new(gameplay.clone()));
        let command = app.session.start("fixture").unwrap();
        let row = Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, GAMEPLAY_TOOLBAR_HEIGHT));
        let button =
            gameplay_toolbar_geometry(row, gameplay_toolbar_spacing(row.width(), true), true)
                .rotate
                .unwrap();
        let mut clicked = None;
        for pressed in [None, Some(true), Some(false)] {
            let mut events = vec![Event::PointerMoved(button.center())];
            if let Some(pressed) = pressed {
                events.push(Event::PointerButton {
                    pos: button.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                });
            }
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 960.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.spacing_mut().item_spacing.x = gameplay_toolbar_spacing(row.width(), true);
                    clicked =
                        app.draw_gameplay_toolbar_row(ui, row, &gameplay, SessionState::Running);
                },
            )
            .drop_without_applying_deltas();
        }
        assert_eq!(clicked, Some(GameplayToolbarRequest::RotateScreen));
        app.canvas_rect = Some(row);
        app.control_regions
            .push((row, crate::TouchOwner::VirtualKey(Some(HostAction::Fire))));
        app.touch_owners
            .insert(1, TouchOwner::VirtualKey(Some(HostAction::Fire)));
        app.rotate_gameplay_screen(landscape);
        let first = if landscape {
            PlatformOrientation::Portrait
        } else {
            PlatformOrientation::Landscape
        };
        assert_eq!(app.host_orientation, first);
        assert!(app.touch_owners.is_empty());
        assert!(app.control_regions.is_empty());
        assert!(app.canvas_rect.is_none());
        // A second tap can arrive before the viewport has changed.
        app.rotate_gameplay_screen(landscape);
        assert_ne!(app.host_orientation, first);
        assert_eq!(
            app.session.active_ids(),
            Some((command.session_id, command.attempt_id))
        );
        assert_eq!(app.session.state(), SessionState::Starting);
        let Screen::Gameplay(current) = &app.screen else {
            panic!()
        };
        assert_eq!(current.canvas_dimensions, (240, 320));
        assert_eq!(current.orientation, None);
        app.session.apply_event(&SessionEvent::terminal_error(
            command.session_id,
            command.attempt_id,
            "Fixture",
            "Finished",
        ));
        app.stop_active_session();
        assert!(matches!(app.screen, Screen::Library));
        assert_eq!(app.host_orientation, PlatformOrientation::Automatic);
        assert_eq!(
            requests.lock().unwrap().last(),
            Some(&PlatformOrientation::Automatic)
        );
    }
}

#[test]
fn back_dismisses_rotation_prompt_without_discarding_control_edits() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let back = Arc::new(AtomicBool::new(true));
    let mut app = FrontendApp::new(root, Box::new(BackPlatform(Arc::clone(&back)))).unwrap();
    let draft = GameSettings::default();
    let mut editor = ControlEditorState::new(&draft, (240, 320));
    editor.layout.fire.offset_x = 750;
    editor.landscape_requested = true;
    app.screen = Screen::Settings(Box::new(SettingsScreen {
        target: SettingsTarget::Existing {
            entry_id: "fixture".into(),
        },
        draft,
        focus_profile: false,
        show_all_profiles: false,
        editor_request: None,
        control_editor: Some(editor),
    }));
    let ctx = egui::Context::default();
    ctx.run_ui(egui::RawInput::default(), |ui| {
        app.handle_back(ui.ctx(), true);
    })
    .drop_without_applying_deltas();
    let Screen::Settings(settings) = &app.screen else {
        panic!("editor must stay open")
    };
    let editor = settings.control_editor.as_ref().unwrap();
    assert!(!editor.landscape_requested);
    assert_eq!(editor.layout.fire.offset_x, 750);
}

impl PlatformBridge for BackPlatform {
    fn request_document(&mut self, _: DocumentKind) -> Result<(), EmuError> {
        unreachable!("navigation tests do not open documents")
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }

    fn uses_platform_navigation_back(&self) -> bool {
        true
    }

    fn poll_navigation_back(&mut self) -> bool {
        self.0.swap(false, Ordering::AcqRel)
    }
}

#[test]
fn back_is_not_duplicated_and_confirmed_exit_is_not_cancelled() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let back = Arc::new(AtomicBool::new(false));
    let mut app = FrontendApp::new(root, Box::new(BackPlatform(Arc::clone(&back)))).unwrap();
    let ctx = egui::Context::default();
    ctx.run_ui(
        egui::RawInput {
            time: Some(1.0),
            events: vec![Event::Key {
                key: Key::BrowserBack,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ui| app.handle_back(ui.ctx(), true),
    )
    .drop_without_applying_deltas();
    assert!(!app.exit_confirmation);
    back.store(true, Ordering::Release);
    ctx.run_ui(egui::RawInput::default(), |ui| {
        app.handle_back(ui.ctx(), true);
    })
    .drop_without_applying_deltas();
    assert!(app.exit_confirmation);

    let output = ctx.run_ui(egui::RawInput::default(), |ui| app.confirm_exit(ui.ctx()));
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Close)
    );
    output.drop_without_applying_deltas();
    assert!(app.exit_requested);
    assert!(!app.exit_confirmation);

    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let output = ctx.run_ui(input, |ui| app.handle_back(ui.ctx(), true));
    assert!(
        !output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    output.drop_without_applying_deltas();
    assert!(app.exit_requested);
}
