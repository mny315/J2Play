use super::selection::{BindingTarget, Picker};
use super::*;
mod navigation;
mod stop_game;
mod touch_cancel;

#[test]
fn trigger_edits_confirm_conflicts_and_update_both_reports_atomically() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    let editor = app.physical_editor.as_mut().unwrap();
    let target = BindingTarget::Trigger { right: true };
    let controls: Vec<_> = target.controls().collect();
    let fire = PhysicalAction::Phone(HostAction::Fire);
    editor
        .draft
        .assign(controls[0], PhysicalAction::Menu)
        .unwrap();
    editor.draft.assign(controls[1], fire).unwrap();
    let original = editor.draft.clone();
    editor.select(Picker::Binding(target));
    editor.choose_action(target, fire);
    assert_eq!(editor.pending, Some((target, fire)));
    assert_eq!(editor.draft, original);
    editor.cancel_capture();
    assert_eq!(editor.draft, original);
    editor.assign(target, fire);
    for control in &controls {
        assert_eq!(editor.draft.resolve(*control), Some(fire));
    }
    editor.remove(target);
    for control in &controls {
        assert_eq!(editor.draft.resolve(*control), None);
    }
    editor.draft.bindings.clear();
    for control in (1..=512)
        .map(|code| PhysicalControl::AndroidKey { code })
        .filter(|control| control.valid())
        .take(frontend_core::physical_input::MAX_PHYSICAL_BINDINGS - 1)
    {
        editor.draft.assign(control, fire).unwrap();
    }
    let original = editor.draft.clone();
    editor.select(Picker::Binding(target));
    editor.choose_action(target, fire);
    assert_eq!(
        editor.draft, original,
        "a capacity error must not bind just one report"
    );
    assert!(!editor.status.is_empty());
    assert!(editor.picker.is_some(), "keep the failed edit open");
}

#[test]
fn navigation_barrier_consumes_queued_presses_before_capture_starts() {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    struct InputPlatform(Arc<Mutex<VecDeque<PhysicalInputEvent>>>);
    impl crate::PlatformBridge for InputPlatform {
        fn request_document(
            &mut self,
            _: crate::DocumentKind,
        ) -> Result<(), diagnostics::EmuError> {
            unreachable!("input fixture never imports documents")
        }
        fn poll_document(
            &mut self,
        ) -> Option<Result<crate::DocumentOutcome, diagnostics::EmuError>> {
            None
        }
        fn poll_physical_input(&mut self) -> Option<PhysicalInputEvent> {
            self.0.lock().unwrap().pop_front()
        }
    }
    let control = PhysicalControl::Gamepad {
        button: GamepadButton::Extra(1),
    };
    let press = PhysicalInputEvent::Button {
        device: 1,
        control,
        pressed: true,
    };
    let release = PhysicalInputEvent::Button {
        device: 1,
        control,
        pressed: false,
    };
    let input = Arc::new(Mutex::new(VecDeque::from([press])));
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(InputPlatform(Arc::clone(&input)))).unwrap();
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    assert!(input.lock().unwrap().is_empty());
    let action = PhysicalAction::Phone(HostAction::Num5);
    app.physical_editor.as_mut().unwrap().listening = Some(Instant::now());
    let ctx = egui::Context::default();
    ctx.input_mut(|input| input.focused = true);
    app.process_physical_event(&ctx, press, false);
    assert!(app.physical_editor.as_ref().unwrap().listening.is_some());
    app.process_physical_event(&ctx, release, false);
    app.process_physical_event(&ctx, press, false);
    let editor = app.physical_editor.as_mut().unwrap();
    assert_eq!(
        editor.picker,
        Some(Picker::Binding(BindingTarget::Control(control)))
    );
    assert_eq!(editor.draft.resolve(control), None);
    editor.choose_action(BindingTarget::Control(control), action);
    assert_eq!(
        app.physical_editor.as_ref().unwrap().draft.resolve(control),
        Some(action)
    );
}

#[test]
fn physical_editor_save_only_updates_parent_draft_and_cancel_preserves_saved_settings() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    let control = PhysicalControl::Gamepad {
        button: GamepadButton::Extra(1),
    };
    let editor = app.physical_editor.as_mut().unwrap();
    editor
        .draft
        .assign(control, PhysicalAction::StopGame)
        .unwrap();
    app.close_physical_editor(true);
    assert_eq!(
        app.repository
            .load_app_settings()
            .unwrap()
            .physical_bindings
            .resolve(control),
        None
    );
    app.save_settings_screen();
    assert_eq!(
        app.repository
            .load_app_settings()
            .unwrap()
            .physical_bindings
            .resolve(control),
        Some(PhysicalAction::StopGame)
    );
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    app.physical_editor.as_mut().unwrap().draft.bindings.clear();
    app.navigate_back();
    assert!(app.physical_editor.is_none());
    app.save_settings_screen();
    assert_eq!(
        app.app_settings.physical_bindings.resolve(control),
        Some(PhysicalAction::StopGame)
    );
}

#[test]
fn capture_conflict_requires_confirmation_and_cannot_navigate() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    app.physical_editor.as_mut().unwrap().listening = Some(Instant::now());
    let ctx = egui::Context::default();
    ctx.input_mut(|input| input.focused = true);
    let control = PhysicalControl::Gamepad {
        button: GamepadButton::Start,
    };
    app.process_physical_event(
        &ctx,
        PhysicalInputEvent::Button {
            device: 1,
            control,
            pressed: true,
        },
        false,
    );
    let editor = app.physical_editor.as_mut().unwrap();
    assert_eq!(editor.draft.resolve(control), Some(PhysicalAction::Menu));
    assert_eq!(
        editor.picker,
        Some(Picker::Binding(BindingTarget::Control(control)))
    );
    let target = BindingTarget::Control(control);
    editor.choose_action(target, PhysicalAction::Phone(HostAction::Num9));
    assert_eq!(editor.draft.resolve(control), Some(PhysicalAction::Menu));
    assert_eq!(
        editor.pending,
        Some((target, PhysicalAction::Phone(HostAction::Num9)))
    );
    assert!(!app.exit_confirmation);
    app.release_all_input();
    assert!(app.physical_editor.as_ref().unwrap().pending.is_none());
}

#[test]
fn unfocused_input_cannot_capture_or_navigate_and_requires_a_fresh_press() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(crate::UnavailablePlatformBridge)).unwrap();
    let ctx = egui::Context::default();
    ctx.input_mut(|input| input.focused = false);
    let event = |button, pressed| PhysicalInputEvent::Button {
        device: 1,
        control: PhysicalControl::Gamepad { button },
        pressed,
    };
    app.process_physical_event(&ctx, event(GamepadButton::East, true), false);
    assert!(
        !app.exit_requested,
        "background input must not exit the library"
    );
    app.screen = Screen::AppSettings(app.app_settings.clone().into());
    app.open_physical_editor();
    app.physical_editor.as_mut().unwrap().listening = Some(Instant::now());
    app.process_physical_event(&ctx, event(GamepadButton::Start, true), false);
    assert!(app.physical_editor.as_ref().unwrap().picker.is_none());
    ctx.input_mut(|input| input.focused = true);
    app.process_physical_event(&ctx, event(GamepadButton::Start, true), false);
    assert!(app.physical_editor.as_ref().unwrap().listening.is_some());
    app.process_physical_event(&ctx, event(GamepadButton::Start, false), false);
    app.process_physical_event(&ctx, event(GamepadButton::Start, true), false);
    assert_eq!(
        app.physical_editor.as_ref().unwrap().picker,
        Some(Picker::Binding(BindingTarget::Control(
            PhysicalControl::Gamepad {
                button: GamepadButton::Start,
            }
        )))
    );
}
