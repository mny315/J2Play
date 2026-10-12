use super::*;
use crate::physical_input::{
    GamepadButton as B, MAX_PHYSICAL_BINDINGS, PhysicalAction as A, PhysicalBinding,
    PhysicalBindings, PhysicalControl as C,
};
use platform::HostAction as H;

pub(super) fn legacy_bindings() -> PhysicalBindings {
    let mut bindings = PhysicalBindings::default();
    bindings
        .bindings
        .retain(|binding| !matches!(binding.control, C::Keyboard { .. }));
    // Frozen keyboard shipped in app schemas 1 through 11.
    bindings.bindings.splice(
        0..0,
        [
            (82, A::Phone(H::Up)),
            (81, A::Phone(H::Down)),
            (80, A::Phone(H::Left)),
            (79, A::Phone(H::Right)),
            (40, A::Phone(H::Fire)),
            (44, A::Phone(H::Fire)),
            (58, A::Phone(H::SoftLeft)),
            (59, A::Phone(H::SoftRight)),
            (60, A::Phone(H::Back)),
            (42, A::Phone(H::Clear)),
            (39, A::Phone(H::Num0)),
            (30, A::Phone(H::Num1)),
            (31, A::Phone(H::Num2)),
            (32, A::Phone(H::Num3)),
            (33, A::Phone(H::Num4)),
            (34, A::Phone(H::Num5)),
            (35, A::Phone(H::Num6)),
            (36, A::Phone(H::Num7)),
            (37, A::Phone(H::Num8)),
            (38, A::Phone(H::Num9)),
            (41, A::Menu),
            (68, A::Fullscreen),
            (67, A::DebugOverlay),
        ]
        .map(|(usage, action)| PhysicalBinding {
            control: C::Keyboard { usage },
            action,
        }),
    );
    bindings
}

pub(super) fn with_updated_keyboard(mut settings: AppSettings) -> AppSettings {
    let mut updated = PhysicalBindings::default();
    updated
        .bindings
        .retain(|binding| matches!(binding.control, C::Keyboard { .. }));
    updated.bindings.extend(
        settings
            .physical_bindings
            .bindings
            .into_iter()
            .filter(|binding| !matches!(binding.control, C::Keyboard { .. })),
    );
    updated.dead_zone_percent = settings.physical_bindings.dead_zone_percent;
    settings.physical_bindings = updated;
    settings
}

fn write_legacy(repository: &LibraryRepository, settings: &AppSettings, version: u32) {
    repository.save_app_settings(settings).unwrap();
    let mut value = serde_json::to_value(settings).unwrap();
    value["schema_version"] = version.into();
    fs::write(
        repository.root.join("frontend-state/app-settings.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
}

#[test]
fn untouched_keyboard_upgrades_independently_of_controller_settings_and_order() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    for version in 9..=11 {
        let mut settings = AppSettings {
            physical_bindings: legacy_bindings(),
            ui_scale_percent: 125,
            language: Some(crate::Language::Russian),
            ..AppSettings::default()
        };
        settings.physical_bindings.dead_zone_percent = 60;
        settings
            .physical_bindings
            .assign(C::Gamepad { button: B::South }, A::Phone(H::Num7))
            .unwrap();
        settings
            .physical_bindings
            .assign(C::AndroidKey { code: 24 }, A::Phone(H::Num9))
            .unwrap();
        settings.physical_bindings.bindings.reverse();
        write_legacy(&repository, &settings, version);
        let loaded = repository.load_app_settings().unwrap();
        assert_eq!(loaded, with_updated_keyboard(settings));
        repository.save_app_settings(&loaded).unwrap();
        assert_eq!(repository.load_app_settings().unwrap(), loaded);
    }
}

#[test]
fn customized_empty_and_explicitly_saved_old_keyboards_are_preserved() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    for variant in 0..4 {
        let mut settings = AppSettings {
            physical_bindings: legacy_bindings(),
            ..AppSettings::default()
        };
        match variant {
            0 => settings
                .physical_bindings
                .assign(C::Keyboard { usage: 44 }, A::Phone(H::Num0))
                .unwrap(),
            1 => settings
                .physical_bindings
                .assign(C::Keyboard { usage: 26 }, A::Phone(H::Num2))
                .unwrap(),
            2 => settings.physical_bindings.remove(C::Keyboard { usage: 58 }),
            _ => settings
                .physical_bindings
                .bindings
                .retain(|binding| !matches!(binding.control, C::Keyboard { .. })),
        }
        for version in 1..=11 {
            write_legacy(&repository, &settings, version);
            assert_eq!(repository.load_app_settings().unwrap(), settings);
        }
    }
    let settings = AppSettings {
        physical_bindings: legacy_bindings(),
        ..AppSettings::default()
    };
    repository.save_app_settings(&settings).unwrap();
    assert_eq!(repository.load_app_settings().unwrap(), settings);

    let game = GameSettings {
        physical_bindings: Some(legacy_bindings()),
        ..GameSettings::default()
    };
    let mut restored: GameSettings =
        serde_json::from_slice(&serde_json::to_vec(&game).unwrap()).unwrap();
    restored.migrate_to_current().unwrap();
    assert_eq!(restored, game);
}

#[test]
fn keyboard_upgrade_keeps_a_full_custom_hardware_map_within_the_binding_limit() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let mut settings = AppSettings {
        physical_bindings: legacy_bindings(),
        ..AppSettings::default()
    };
    for code in 1..=512 {
        let control = C::AndroidKey { code };
        if control.valid() {
            settings.physical_bindings.assign(control, A::Menu).unwrap();
        }
        if settings.physical_bindings.bindings.len() == MAX_PHYSICAL_BINDINGS {
            break;
        }
    }
    write_legacy(&repository, &settings, 11);
    assert_eq!(repository.load_app_settings().unwrap(), settings);
}
