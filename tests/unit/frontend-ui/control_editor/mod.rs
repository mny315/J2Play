use super::*;
use crate::{ControlEditorState, GameSettings};

mod navigation;

#[test]
fn editor_changes_are_transactional_until_applied() {
    let original = GameSettings::default();
    let mut settings = original.clone();
    let mut editor = ControlEditorState::new(&settings, (240, 320));
    editor.layout.fire.offset_x = 1_250;
    editor.layout.stick_enabled = true;
    editor.select_orientation(true);
    editor.layout.fire.offset_x = -500;
    editor.layout.number_keys[0].visible = false;
    editor.drag_origin = Some((ControlTransform::default(), egui::Pos2::ZERO));
    editor.navigation.moving = Some(egui::Id::new("moving-control"));
    editor.select_orientation(false);
    assert!(editor.drag_origin.is_none());
    assert!(editor.navigation.moving.is_none());
    assert_eq!(editor.layout.fire.offset_x, 1_250);
    editor.select_orientation(true);
    editor.vibration = VibrationSettings {
        enabled: true,
        strength_percent: 37,
    };

    assert_eq!(settings, original);
    editor.apply_to(&mut settings);
    assert_eq!(settings.control_layout.fire.offset_x, 1_250);
    assert!(settings.control_layout.stick_enabled);
    assert!(!settings.landscape_control_layout.stick_enabled);
    assert_eq!(settings.landscape_control_layout.fire.offset_x, -500);
    assert!(!settings.landscape_control_layout.number_keys[0].visible);
    assert_eq!(settings.vibration.strength_percent, 37);
}

#[test]
fn reset_and_cancel_preserve_the_other_orientation_and_shared_feedback() {
    let mut settings = GameSettings::default();
    settings.control_layout.fire.offset_x = 800;
    settings.control_layout.stick_enabled = true;
    settings.landscape_control_layout.fire.offset_y = -900;
    settings.landscape_control_layout.stick_enabled = true;
    settings.vibration.strength_percent = 37;
    let original = settings.clone();
    let mut screen = crate::SettingsScreen {
        target: crate::SettingsTarget::Existing {
            entry_id: "fixture".into(),
        },
        draft: settings,
        focus_profile: false,
        show_all_profiles: false,
        editor_request: None,
        control_editor: Some(ControlEditorState::new(&original, (240, 320))),
    };
    let editor = screen.control_editor.as_mut().unwrap();
    editor.select_orientation(true);
    editor.navigation.moving = Some(egui::Id::new("moving-control"));
    editor.reset_layout();
    assert!(editor.navigation.moving.is_none());
    assert!(editor.layout.is_default());
    assert_eq!(editor.other_layout.fire.offset_x, 800);
    assert!(editor.other_layout.stick_enabled);
    assert_eq!(editor.vibration.strength_percent, 37);
    screen.discard_control_editor();
    assert_eq!(screen.draft, original);
    screen.control_editor = Some(ControlEditorState::new(&original, (240, 320)));
    screen.control_editor.as_mut().unwrap().reset_layout();
    screen.commit_control_editor();
    assert!(screen.draft.control_layout.is_default());
    assert_eq!(
        screen.draft.landscape_control_layout,
        original.landscape_control_layout
    );
}
