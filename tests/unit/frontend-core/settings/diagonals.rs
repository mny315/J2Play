use super::*;

#[test]
fn previous_settings_keep_single_key_diagonals_and_new_layouts_round_trip_independently() {
    let mut settings = GameSettings {
        controls_override: true,
        ..GameSettings::default()
    };
    settings.control_layout.direction_pad.offset_x = 900;
    settings.landscape_control_layout.stick_enabled = true;
    let mut old = serde_json::to_value(&settings).unwrap();
    old["schema_version"] = 11.into();
    for name in ["control_layout", "landscape_control_layout"] {
        old[name]
            .as_object_mut()
            .unwrap()
            .remove("two_key_diagonals");
    }
    let mut restored: GameSettings = serde_json::from_value(old).unwrap();
    assert!(restored.migrate_to_current().unwrap());
    restored.validate(&[]).unwrap();
    assert_eq!(restored, settings);
    assert!(!restored.migrate_to_current().unwrap());
    settings.control_layout.two_key_diagonals = true;
    let restored: GameSettings =
        serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
    assert_eq!(restored, settings);
    assert!(!restored.landscape_control_layout.two_key_diagonals);
}
