use super::*;
use crate::{GameSettings, ImportSource, PreparedImport, inspect_import};

fn recover(repository: &LibraryRepository, entry: &mut LibraryEntry) -> Result<(), EmuError> {
    if let Some(info) = repository.read_game_info(entry, || false)? {
        assert!(repository.cache_game_info(entry, info)?);
    }
    Ok(())
}

fn prepared(properties: &str) -> PreparedImport {
    let jad =
        format!("MIDlet-1: Metadata Fixture, , fixtures.Stage7LifecycleMidlet\n{properties}\n");
    inspect_import(ImportSource::new(
        Some("fixture-2004.jar".to_owned()),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/java-me/conformance.jar"
        ))
        .to_vec(),
        Some(jad.into_bytes()),
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap()
}

#[test]
fn explicit_jad_information_overrides_manifest_and_survives_restart() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = prepared("midlet-vendor: Fixture Studio\nMIDlet-Release-Year: 2008");
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    assert_eq!(entry.vendor(), Some("Fixture Studio"));
    assert_eq!(entry.release_year(), Some(2008));
    assert_eq!(repository.load().unwrap().entries, [entry]);
}

#[test]
fn legacy_information_is_recovered_once_without_changing_settings_or_archives() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = prepared("MIDlet-Vendor: Fixture Studio\nMIDlet-Year: 2007");
    let expected = repository
        .commit_import(
            &prepared,
            GameSettings {
                fps_limit: crate::FpsLimit::Manual {
                    frames_per_second: 30,
                },
                ..GameSettings::default()
            },
        )
        .unwrap();
    let metadata = repository
        .entries_dir()
        .join(format!("{}.json", expected.id()));
    let mut old: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
    old.as_object_mut().unwrap().remove("game_info");
    std::fs::write(&metadata, serde_json::to_vec(&old).unwrap()).unwrap();
    let mut entry = repository.load().unwrap().entries.remove(0);
    assert_eq!(entry.vendor(), None);
    recover(&repository, &mut entry).unwrap();
    assert_eq!(entry, expected);
    assert_eq!(
        std::fs::read(repository.private_jar_path(&entry)).unwrap(),
        prepared.jar_bytes()
    );
    let saved = std::fs::read(&metadata).unwrap();
    // Once saved, displaying the information needs no archive access or writes.
    std::fs::remove_file(repository.private_jar_path(&entry)).unwrap();
    let mut reloaded = repository.load().unwrap().entries.remove(0);
    recover(&repository, &mut reloaded).unwrap();
    assert_eq!(reloaded, expected);
    assert_eq!(std::fs::read(&metadata).unwrap(), saved);
}

#[test]
fn missing_release_year_is_not_inferred_from_copyright_build_or_archive_dates() {
    let source =
        prepared("MIDlet-Copyright: Copyright 2009 Fixture Studio\nBuild-Date: 2012-01-01");
    let info = GameInfo::from_properties(&source.suite().properties);
    assert_eq!(info.release_year, None);
    for value in [
        "",
        "2007.0",
        "+2007",
        "2007-2009",
        "0000",
        "9999",
        "2007-06-01",
    ] {
        let prepared = prepared(&format!("MIDlet-Year: {value}"));
        assert_eq!(
            GameInfo::from_properties(&prepared.suite().properties).release_year,
            None,
            "{value}"
        );
    }
}

#[test]
fn vendor_text_is_bounded_unicode_on_one_line() {
    let vendor = format!("  Fixture\t\u{0000} Studio\n{}", "界".repeat(300));
    let info = GameInfo::from_properties(&BTreeMap::from([("midlet-vendor".to_owned(), vendor)]));
    let text = info.vendor.as_ref().unwrap();
    assert!(text.starts_with("Fixture Studio 界"));
    assert!(text.len() <= MAX_VENDOR_BYTES);
    assert!(!text.chars().any(char::is_control));
    info.validate().unwrap();
    let invalid = GameInfo {
        vendor: Some("x".repeat(MAX_VENDOR_BYTES + 1)),
        ..GameInfo::default()
    };
    assert_eq!(invalid.validate().unwrap_err().code(), "library-game-info");
}

#[test]
fn failed_metadata_recovery_keeps_the_entry_and_can_be_retried() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = prepared("MIDlet-Vendor: Fixture Studio");
    let mut entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    entry.game_info = None;
    repository.write_entry(&entry).unwrap();
    let descriptor = repository
        .archives_dir()
        .join(entry.private_jad.as_ref().unwrap());
    std::fs::write(&descriptor, b"MIDlet-Vendor: Changed\n").unwrap();
    assert_eq!(
        recover(&repository, &mut entry).unwrap_err().code(),
        "library-jad-integrity"
    );
    assert_eq!(entry.game_info, None);
    assert_eq!(repository.load().unwrap().entries, [entry.clone()]);
    std::fs::write(descriptor, prepared.jad_bytes().unwrap()).unwrap();
    recover(&repository, &mut entry).unwrap();
    assert_eq!(entry.vendor(), Some("Fixture Studio"));
    assert_eq!(entry.release_year(), None);
}

#[test]
fn delayed_metadata_preserves_settings_changed_after_the_read() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let mut entry = repository
        .commit_import(
            &prepared("MIDlet-Vendor: Fixture Studio"),
            GameSettings::default(),
        )
        .unwrap();
    entry.game_info = None;
    repository.write_entry(&entry).unwrap();
    let recovered = repository
        .read_game_info(&entry, || false)
        .unwrap()
        .unwrap();
    let mut current = entry.clone();
    let settings = GameSettings {
        fps_limit: crate::FpsLimit::Manual {
            frames_per_second: 30,
        },
        ..GameSettings::default()
    };
    repository
        .save_settings(&mut current, settings.clone())
        .unwrap();
    let (_, folder) = repository.create_folder("Fixtures").unwrap();
    repository
        .move_to_folder(&mut current, Some(folder))
        .unwrap();
    assert!(repository.cache_game_info(&mut entry, recovered).unwrap());
    current.game_info = entry.game_info.clone();
    assert_eq!(entry, current);
    assert_eq!(entry.settings(), &settings);
    assert_eq!(entry.vendor(), Some("Fixture Studio"));
    assert_eq!(repository.load().unwrap().entries, [entry]);
}

#[test]
fn delayed_metadata_cannot_undo_a_reimport_or_recreate_a_deleted_entry() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    for delete in [false, true] {
        let mut stale = repository
            .commit_import(
                &prepared("MIDlet-Vendor: Old Studio"),
                GameSettings::default(),
            )
            .unwrap();
        stale.game_info = None;
        repository.write_entry(&stale).unwrap();
        let recovered = repository
            .read_game_info(&stale, || false)
            .unwrap()
            .unwrap();
        if delete {
            repository.delete_entry_record(&stale).unwrap();
            let original = stale.clone();
            assert!(repository.cache_game_info(&mut stale, recovered).is_err());
            assert_eq!(stale, original);
            assert!(repository.load().unwrap().entries.is_empty());
        } else {
            let current = repository
                .commit_import(
                    &prepared("MIDlet-Vendor: New Studio"),
                    GameSettings::default(),
                )
                .unwrap();
            assert!(!repository.cache_game_info(&mut stale, recovered).unwrap());
            assert_eq!(stale, current);
            assert_eq!(repository.load().unwrap().entries, [current]);
        }
    }
}

#[test]
fn recovered_metadata_rejects_a_different_descriptor_or_entry() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let mut original = repository
        .commit_import(
            &prepared("MIDlet-Vendor: Old Studio"),
            GameSettings::default(),
        )
        .unwrap();
    original.game_info = None;
    repository.write_entry(&original).unwrap();
    for changed_entry in [false, true] {
        let recovered = repository
            .read_game_info(&original, || false)
            .unwrap()
            .unwrap();
        let mut current = original.clone();
        if changed_entry {
            current.id.push('1');
        } else {
            current.jad_sha256 = Some("0".repeat(64));
        }
        let expected = current.clone();
        assert!(!repository.cache_game_info(&mut current, recovered).unwrap());
        assert_eq!(current, expected);
        assert_eq!(repository.load().unwrap().entries, [original.clone()]);
    }
}

#[test]
fn metadata_read_cancellation_keeps_the_record_unchanged() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let mut entry = repository
        .commit_import(
            &prepared("MIDlet-Vendor: Fixture Studio"),
            GameSettings::default(),
        )
        .unwrap();
    entry.game_info = None;
    repository.write_entry(&entry).unwrap();
    assert_eq!(
        repository
            .read_game_info(&entry, || true)
            .unwrap_err()
            .code(),
        "library-game-info-read"
    );
    assert_eq!(repository.load().unwrap().entries, [entry]);
}
