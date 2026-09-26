use super::*;
use std::sync::{Arc, Mutex};

struct HapticPlatform(Arc<Mutex<Vec<u8>>>);

impl PlatformBridge for HapticPlatform {
    fn orientation_control(&self) -> crate::OrientationControl {
        crate::OrientationControl::Layout
    }

    fn request_document(&mut self, _: crate::DocumentKind) -> Result<(), crate::EmuError> {
        unreachable!("editor feedback fixture does not import")
    }

    fn poll_document(&mut self) -> Option<Result<crate::DocumentOutcome, crate::EmuError>> {
        None
    }

    fn request_game_haptic(&mut self, strength: u8) -> Result<bool, crate::EmuError> {
        self.0.lock().unwrap().push(strength);
        Ok(true)
    }
}

fn start_drag(editor: &mut Editor) -> egui::Pos2 {
    let position = editor.focused().rect.center();
    editor
        .paint(
            &[],
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .drop_without_applying_deltas();
    position
}

fn drag_to(editor: &mut Editor, position: egui::Pos2) {
    // Advance the shared haptic throttle without sleeping in the test.
    editor.app.slider_haptic.last_tick = None;
    editor
        .paint(&[], vec![egui::Event::PointerMoved(position)])
        .drop_without_applying_deltas();
}

#[test]
fn grid_drag_ticks_only_on_movement_and_respects_disabled_feedback_and_cancel() {
    for landscape in [false, true] {
        let mut editor = Editor::new(landscape);
        editor.focus_number_one();
        let ticks = Arc::new(Mutex::new(Vec::new()));
        editor.app.platform = Box::new(HapticPlatform(ticks.clone()));
        editor.state().vibration.strength_percent = 37;
        let origin = start_drag(&mut editor);
        assert!(ticks.lock().unwrap().is_empty());
        let position = origin - egui::vec2(0.0, 24.0);
        drag_to(&mut editor, position);
        assert_eq!(*ticks.lock().unwrap(), [37]);
        drag_to(&mut editor, position);
        editor.settle();
        assert_eq!(
            *ticks.lock().unwrap(),
            [37],
            "stationary contact must not tick"
        );
        drag_to(&mut editor, origin - egui::vec2(0.0, 48.0));
        assert_eq!(*ticks.lock().unwrap(), [37, 37]);
        editor.state().vibration.enabled = false;
        drag_to(&mut editor, origin - egui::vec2(0.0, 72.0));
        assert_eq!(*ticks.lock().unwrap(), [37, 37]);
        editor.state().vibration.enabled = true;
        editor.state().grid_enabled = false;
        drag_to(&mut editor, origin - egui::vec2(0.0, 96.0));
        assert_eq!(*ticks.lock().unwrap(), [37, 37]);
        editor.state().grid_enabled = true;
        editor
            .paint(&[], vec![egui::Event::PointerCancelled])
            .drop_without_applying_deltas();
        drag_to(&mut editor, origin - egui::vec2(0.0, 120.0));
        assert_eq!(*ticks.lock().unwrap(), [37, 37]);
    }
}

#[test]
fn dragging_against_a_clamped_boundary_does_not_keep_ticking() {
    let mut editor = Editor::new(false);
    editor.focus_number_one();
    let ticks = Arc::new(Mutex::new(Vec::new()));
    editor.app.platform = Box::new(HapticPlatform(ticks.clone()));
    let origin = start_drag(&mut editor);
    drag_to(&mut editor, origin + egui::vec2(0.0, 120.0));
    assert_eq!(ticks.lock().unwrap().len(), 1);
    drag_to(&mut editor, origin + egui::vec2(0.0, 144.0));
    assert_eq!(ticks.lock().unwrap().len(), 1);
}

#[test]
fn stick_switch_is_below_the_preview_and_keyboard_changes_only_the_editor_draft() {
    let mut editor = Editor::new(false);
    let id = editor
        .inventory()
        .into_iter()
        .find(|(_, _, labels)| labels.iter().any(|label| label == "Use virtual stick"))
        .unwrap()
        .0;
    editor.focus(id);
    let position = editor.focused().rect;
    editor.key(egui::Key::Space);
    assert!(editor.state().layout.stick_enabled);
    let Screen::AppSettings(settings) = &editor.app.screen else {
        unreachable!()
    };
    assert!(!settings.draft.control_layout.stick_enabled);
    assert!(position.height() >= crate::SETTINGS_TOUCH_TARGET_HEIGHT);
    assert!(position.top() > editor.app.safe_content_rect.top() + 200.0);
    editor.key(egui::Key::Space);
    assert!(!editor.state().layout.stick_enabled);
}

#[test]
fn diagonal_switch_changes_only_selected_orientation_until_the_editor_is_saved() {
    for landscape in [false, true] {
        let mut editor = Editor::new(landscape);
        let diagonal_label =
            crate::i18n::Translator(frontend_core::Language::English).text("Two-key diagonals");
        let id = editor
            .inventory()
            .into_iter()
            .find(|(_, _, labels)| labels.iter().any(|label| label == &diagonal_label))
            .unwrap()
            .0;
        editor.focus(id);
        editor.key(egui::Key::Space);
        assert!(editor.state().layout.two_key_diagonals);
        assert!(!editor.state().other_layout.two_key_diagonals);
        let Screen::AppSettings(settings) = &editor.app.screen else {
            unreachable!()
        };
        assert!(!settings.draft.control_layout.two_key_diagonals);
        assert!(!settings.draft.landscape_control_layout.two_key_diagonals);
        let mut saved = crate::GameSettings::default();
        editor.state().clone().apply_to(&mut saved);
        assert_eq!(saved.control_layout.two_key_diagonals, !landscape);
        assert_eq!(saved.landscape_control_layout.two_key_diagonals, landscape);
        assert!(saved.controls_override);
        editor.state().reset_layout();
        assert!(!editor.state().layout.two_key_diagonals);
    }
}
