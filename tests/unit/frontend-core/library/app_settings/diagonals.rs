use super::*;

#[test]
fn diagonal_setting_migrates_defaults_and_persists_global_and_game_choices() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let mut settings = AppSettings::default();
    settings.control_layout.direction_pad.offset_x = 500;
    settings.landscape_control_layout.stick_enabled = true;
    repository.save_app_settings(&settings).unwrap();
    let mut old = serde_json::to_value(&settings).unwrap();
    old["schema_version"] = 10.into();
    for name in ["control_layout", "landscape_control_layout"] {
        old[name]
            .as_object_mut()
            .unwrap()
            .remove("two_key_diagonals");
    }
    fs::write(
        scratch.0.join("frontend-state/app-settings.json"),
        serde_json::to_vec(&old).unwrap(),
    )
    .unwrap();
    assert_eq!(repository.load_app_settings().unwrap(), settings);
    settings.landscape_control_layout.two_key_diagonals = true;
    repository.save_app_settings(&settings).unwrap();
    let loaded = repository.load_app_settings().unwrap();
    assert_eq!(loaded, settings);
    let mut game = loaded.control_defaults();
    assert!(!game.control_layout.two_key_diagonals);
    assert!(game.landscape_control_layout.two_key_diagonals);
    game.controls_override = true;
    game.landscape_control_layout.two_key_diagonals = false;
    loaded.apply_control_defaults(&mut game);
    assert!(!game.landscape_control_layout.two_key_diagonals);
    game.controls_override = false;
    loaded.apply_control_defaults(&mut game);
    assert!(game.landscape_control_layout.two_key_diagonals);
}
