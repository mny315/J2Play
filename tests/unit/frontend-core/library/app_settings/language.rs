use super::*;
use crate::Language;

#[test]
fn version_eight_with_null_language_upgrades_and_preserves_custom_settings_and_file() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let mut settings = AppSettings {
        ui_scale_percent: 78,
        fullscreen: FullscreenMode::On,
        ..AppSettings::default()
    };
    settings.control_layout.fire.offset_x = 1350;
    settings.landscape_control_layout.fire.size_percent = 125;
    settings
        .physical_bindings
        .assign(
            crate::physical_input::PhysicalControl::AndroidKey { code: 110 },
            crate::physical_input::PhysicalAction::StopGame,
        )
        .unwrap();
    repository.save_app_settings(&settings).unwrap();
    let path = scratch.0.join("frontend-state/app-settings.json");
    let mut value = serde_json::to_value(&settings).unwrap();
    assert_eq!(value["schema_version"], 12);
    value["schema_version"] = 8.into();
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(value["language"].is_null());
    assert_eq!(repository.load_app_settings().unwrap(), settings);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn version_seven_without_language_upgrades_and_explicit_languages_round_trip() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let settings = AppSettings::default();
    repository.save_app_settings(&settings).unwrap();
    let path = scratch.0.join("frontend-state/app-settings.json");
    let mut value = serde_json::to_value(&settings).unwrap();
    value["schema_version"] = 7.into();
    value.as_object_mut().unwrap().remove("language");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(repository.load_app_settings().unwrap(), settings);
    for language in Language::ALL {
        let settings = AppSettings {
            language: Some(language),
            ..settings.clone()
        };
        repository.save_app_settings(&settings).unwrap();
        assert_eq!(repository.load_app_settings().unwrap(), settings);
    }
}

#[test]
fn locales_accept_bcp47_and_posix_names_without_affecting_guest_profiles() {
    for (locale, expected) in [
        ("ru-RU", Language::Russian),
        ("uk_UA.UTF-8", Language::Ukrainian),
        ("pt_PT", Language::Portuguese),
        ("zh_CN.UTF-8", Language::Chinese),
        ("in-ID", Language::Indonesian),
        ("AR-eg", Language::Arabic),
        ("C.UTF-8", Language::English),
        ("ja-JP", Language::English),
    ] {
        assert_eq!(Language::from_locale(locale), expected, "{locale}");
    }
}
