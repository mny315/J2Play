use super::entry::MAX_TITLE_BYTES;
use super::*;
use std::fs::File;

#[cfg(target_os = "linux")]
#[path = "../../../support/isolation.rs"]
mod isolation;

const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/java-me/conformance.jar"
));

#[test]
fn launch_preparation_can_cancel_at_each_read_or_inspection_boundary() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let checks = std::cell::Cell::new(0);
    let actual = repository
        .prepare_launch_cancellable(&entry, || {
            checks.set(checks.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(actual.jar_sha256(), prepared.jar_sha256());
    assert!(checks.get() > 4);
    for cancel_at in 1..=checks.get() {
        let calls = std::cell::Cell::new(0);
        assert!(
            repository
                .prepare_launch_cancellable(&entry, || {
                    calls.set(calls.get() + 1);
                    calls.get() >= cancel_at
                })
                .is_err(),
            "cancel at {cancel_at}"
        );
        assert_eq!(calls.get(), cancel_at);
    }
    assert_eq!(
        fs::read(repository.private_jar_path(&entry)).unwrap(),
        FIXTURE
    );
    assert_eq!(repository.load().unwrap().entries, vec![entry]);
}

#[test]
fn icon_reads_preserve_resource_bytes_and_can_cancel_before_or_after_file_read() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let mut entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    entry.icon_resource = Some("META-INF/MANIFEST.MF".into());
    let expected = jar::read_resource_bytes(FIXTURE, "META-INF/MANIFEST.MF").unwrap();
    let mut checks = 0;
    assert_eq!(
        repository
            .read_icon_bytes(&entry, || {
                checks += 1;
                false
            })
            .unwrap(),
        expected
    );
    assert!(checks > 1);
    for cancel_at in [1, checks] {
        let mut calls = 0;
        assert_eq!(
            repository
                .read_icon_bytes(&entry, || {
                    calls += 1;
                    calls >= cancel_at
                })
                .unwrap_err()
                .code(),
            "library-icon-read"
        );
        assert_eq!(calls, cancel_at);
    }
    assert_eq!(
        fs::read(repository.private_jar_path(&entry)).unwrap(),
        FIXTURE
    );
}

#[test]
fn shell_cache_root_is_disposable_and_independent_of_persistent_library_data() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let cache = root.join("xdg-cache");
    let data = root.join("xdg-data");
    let repository = LibraryRepository::open_with_cache(&data, &cache).unwrap();
    assert_eq!(repository.native_code_cache_root(), cache);
    assert!(!cache.exists());
    let legacy = LibraryRepository::open(root.join("android")).unwrap();
    assert_eq!(
        legacy.native_code_cache_root(),
        root.join("android/native-code-cache")
    );
}

#[test]
fn import_round_trips_without_source_path() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let inspected = inspect_import(ImportSource::new(
        Some("/secret/provider/fixture.jar".to_owned()),
        FIXTURE.to_vec(),
        None,
    ))
    .unwrap();
    let prepared = inspected.select_midlet(1).unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let loaded = repository.load().unwrap();
    assert_eq!(loaded.entries, vec![entry.clone()]);
    assert!(loaded.warnings.is_empty());
    assert_eq!(
        repository
            .prepare_launch(&entry)
            .unwrap()
            .midlet()
            .class_name,
        entry.midlet_class()
    );
    let metadata = fs::read_to_string(
        repository
            .entries_dir()
            .join(format!("{}.json", entry.id())),
    )
    .unwrap();
    assert!(!metadata.contains("/secret/provider"));
}

#[test]
fn rejected_import_does_not_publish_private_archives() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let jad = format!(
        "MIDlet-1: {}, , fixtures.Stage7LifecycleMidlet\n",
        "x".repeat(MAX_TITLE_BYTES + 1)
    );
    let prepared = inspect_import(ImportSource::new(
        None,
        FIXTURE.to_vec(),
        Some(jad.into_bytes()),
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    assert_eq!(
        repository
            .commit_import(&prepared, GameSettings::default())
            .unwrap_err()
            .code(),
        "library-title"
    );
    assert_eq!(fs::read_dir(repository.archives_dir()).unwrap().count(), 0);
    assert!(repository.load().unwrap().entries.is_empty());
}

#[test]
fn corrupt_entry_does_not_hide_valid_entries() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    fs::write(repository.entries_dir().join("broken.json"), b"{").unwrap();
    let loaded = repository.load().unwrap();
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.warnings.len(), 1);
    assert_eq!(
        repository.load_entry(loaded.entries[0].id()).unwrap(),
        loaded.entries[0]
    );
}

#[test]
#[cfg(unix)]
fn non_unicode_metadata_filename_cannot_bypass_entry_identity() {
    use std::os::unix::ffi::OsStringExt;

    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let invalid_name = std::ffi::OsString::from_vec(b"\xff.json".to_vec());
    fs::write(
        repository.entries_dir().join(invalid_name),
        serde_json::to_vec(&entry).unwrap(),
    )
    .unwrap();
    let loaded = repository.load().unwrap();
    assert_eq!(loaded.entries, [entry]);
    assert_eq!(loaded.warnings.len(), 1);
    assert_eq!(loaded.warnings[0].code, "library-entry-name");
}

#[test]
fn selected_entry_reports_corrupt_metadata_and_rejects_path_components() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    for invalid_id in ["../outside", "/outside", "entry/child", "entry\\child", ""] {
        assert_eq!(
            repository.load_entry(invalid_id).unwrap_err().code(),
            "library-entry-id"
        );
    }
    let metadata = repository
        .entries_dir()
        .join(format!("{}.json", entry.id()));
    let renamed = repository.entries_dir().join("renamed.json");
    fs::copy(&metadata, &renamed).unwrap();
    assert_eq!(
        repository.load_entry("renamed").unwrap_err().code(),
        "library-entry-name"
    );
    fs::write(&metadata, b"{").unwrap();
    assert_eq!(
        repository.load_entry(entry.id()).unwrap_err().code(),
        "library-entry-json"
    );
    fs::remove_file(metadata).unwrap();
    assert_eq!(
        repository.load_entry(entry.id()).unwrap_err().code(),
        "library-entry-read"
    );
}

#[test]
fn corrupt_prefix_does_not_consume_the_recoverable_entry_limit() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    for index in 0..MAX_LIBRARY_ENTRIES {
        fs::write(
            repository
                .entries_dir()
                .join(format!("-broken-{index:04}.json")),
            b"{",
        )
        .unwrap();
    }
    let loaded = repository.load().unwrap();
    assert_eq!(loaded.entries, [entry]);
    assert_eq!(loaded.warnings.len(), MAX_LIBRARY_ENTRIES);
}

#[cfg(target_os = "linux")]
#[test]
fn a_fifo_metadata_record_does_not_block_library_recovery() {
    let scratch = crate::test_storage::Scratch::new();
    if isolation::isolate(&scratch.0, std::time::Duration::from_secs(3)) {
        return;
    }
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let path = repository.entries_dir().join("broken.json");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let loaded = repository.load().unwrap();
    assert!(loaded.entries.is_empty());
    assert_eq!(loaded.warnings.len(), 1);
    assert_eq!(loaded.warnings[0].code, "library-entry-read");
}

#[test]
fn unchanged_launch_cache_does_not_rewrite_metadata() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let mut entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let plan = prepared.launch_plan(entry.settings()).unwrap();
    let path = repository
        .entries_dir()
        .join(format!("{}.json", entry.id()));
    let old_time = std::time::UNIX_EPOCH + std::time::Duration::from_hours(1);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(old_time))
        .unwrap();
    let original_time = fs::metadata(&path).unwrap().modified().unwrap();
    repository
        .cache_launch_resolution(&mut entry, &plan)
        .unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().modified().unwrap(),
        original_time
    );
    entry.cached_automatic_profile.catalog_fingerprint = "0".repeat(64);
    repository.write_entry(&entry).unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(old_time))
        .unwrap();
    repository
        .cache_launch_resolution(&mut entry, &plan)
        .unwrap();
    assert_ne!(
        fs::metadata(&path).unwrap().modified().unwrap(),
        original_time
    );
    assert_eq!(repository.load().unwrap().entries, [entry]);
}

#[test]
fn delayed_launch_cache_preserves_newer_settings_and_folder_membership() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let mut stale = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let plan = prepared.launch_plan(stale.settings()).unwrap();
    stale.cached_automatic_profile.catalog_fingerprint = "0".repeat(64);
    let mut current = stale.clone();
    let mut settings = current.settings().clone();
    settings.vibration.enabled = false;
    repository.save_settings(&mut current, settings).unwrap();
    let (_, folder) = repository.create_folder("Fixtures").unwrap();
    repository
        .move_to_folder(&mut current, Some(folder))
        .unwrap();
    repository
        .cache_launch_resolution(&mut stale, &plan)
        .unwrap();
    assert_eq!(stale, current);
    assert_eq!(repository.load().unwrap().entries, [current]);
}

#[test]
fn delayed_launch_cache_cannot_undo_reimport_or_a_manual_profile() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    for reimport in [false, true] {
        let mut stale = repository
            .commit_import(&prepared, GameSettings::default())
            .unwrap();
        let plan = prepared.launch_plan(stale.settings()).unwrap();
        stale.cached_automatic_profile.catalog_fingerprint = "0".repeat(64);
        let current = if reimport {
            let changed = inspect_import(ImportSource::new(
                None,
                FIXTURE.to_vec(),
                Some(b"MIDlet-1: Updated title, , fixtures.Stage7LifecycleMidlet\nMIDlet-Platform: Nokia\nNokia-MIDlet-Target-Display-Size: 128,160\n".to_vec()),
            ))
            .unwrap()
            .select_midlet(1)
            .unwrap();
            let current = repository
                .commit_import(&changed, GameSettings::default())
                .unwrap();
            assert_ne!(
                current.cached_automatic_profile,
                CachedAutomaticProfile::from_summary(
                    plan.profile_catalog.clone(),
                    plan.profile_summary()
                ),
            );
            current
        } else {
            let mut current = stale.clone();
            let settings = GameSettings {
                device_profile: ProfileChoice::Manual {
                    profile_id: launch::builtin_device_profiles().unwrap()[0]
                        .profile_id()
                        .to_owned(),
                },
                ..GameSettings::default()
            };
            repository.save_settings(&mut current, settings).unwrap();
            current
        };
        repository
            .cache_launch_resolution(&mut stale, &plan)
            .unwrap();
        assert_eq!(stale, current);
        assert_eq!(repository.load().unwrap().entries, [current]);
    }
}

#[test]
fn delayed_launch_cache_cannot_replace_resolution_from_a_new_archive_name() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepare = |name: &str| {
        inspect_import(ImportSource::new(
            Some(name.to_owned()),
            FIXTURE.to_vec(),
            None,
        ))
        .unwrap()
        .select_midlet(1)
        .unwrap()
    };
    let prepared = prepare("fixture_NOKIA_5800_build.jar");
    let mut stale = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let plan = prepared.launch_plan(stale.settings()).unwrap();
    stale.cached_automatic_profile.catalog_fingerprint = "0".repeat(64);
    let current = repository
        .commit_import(
            &prepare("fixture_nokia_e71_build.jar"),
            GameSettings::default(),
        )
        .unwrap();
    assert_eq!(stale.id(), current.id());
    assert_eq!(stale.jad_sha256, current.jad_sha256);
    assert_ne!(
        current.cached_automatic_profile,
        CachedAutomaticProfile::from_summary(plan.profile_catalog.clone(), plan.profile_summary()),
    );
    repository
        .cache_launch_resolution(&mut stale, &plan)
        .unwrap();
    assert_eq!(stale, current);
    assert_eq!(repository.load().unwrap().entries, [current]);
}

#[test]
fn delayed_launch_cache_cannot_restore_missing_or_corrupt_metadata() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let mut entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let plan = prepared.launch_plan(entry.settings()).unwrap();
    entry.cached_automatic_profile.catalog_fingerprint = "0".repeat(64);
    let original = entry.clone();
    repository.delete_entry_record(&entry).unwrap();
    let path = repository
        .entries_dir()
        .join(format!("{}.json", entry.id()));
    assert!(
        repository
            .cache_launch_resolution(&mut entry, &plan)
            .is_err()
    );
    assert_eq!(entry, original);
    assert!(!path.exists());
    fs::write(&path, b"{broken").unwrap();
    assert!(
        repository
            .cache_launch_resolution(&mut entry, &plan)
            .is_err()
    );
    assert_eq!(entry, original);
    assert_eq!(fs::read(path).unwrap(), b"{broken");
}

#[test]
fn independently_deletes_runtime_archives_and_metadata() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    fs::create_dir_all(repository.rms_root(&entry)).unwrap();
    fs::create_dir_all(repository.file_root(&entry)).unwrap();
    fs::write(repository.rms_root(&entry).join("store"), b"rms").unwrap();
    fs::write(repository.file_root(&entry).join("save"), b"save").unwrap();
    let metadata = repository
        .entries_dir()
        .join(format!("{}.json", entry.id()));
    let archive = repository.private_jar_path(&entry);

    repository.delete_runtime_data(&entry).unwrap();
    assert!(!repository.rms_root(&entry).exists());
    assert!(!repository.file_root(&entry).exists());
    assert!(metadata.is_file());
    assert!(archive.is_file());

    repository.delete_private_archives(&entry).unwrap();
    assert!(!archive.exists());
    assert!(metadata.is_file());

    repository.delete_entry_record(&entry).unwrap();
    assert!(!metadata.exists());
    repository.delete_runtime_data(&entry).unwrap();
    repository.delete_private_archives(&entry).unwrap();
    repository.delete_entry_record(&entry).unwrap();
}

#[test]
fn manual_profile_id_does_not_depend_on_automatic_cache_version() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let profiles = launch::builtin_device_profiles().unwrap();
    let profile_id = profiles[0].profile_id().to_owned();
    let entry = repository
        .commit_import(
            &prepared,
            GameSettings {
                device_profile: ProfileChoice::Manual {
                    profile_id: profile_id.clone(),
                },
                ..GameSettings::default()
            },
        )
        .unwrap();
    assert_eq!(
        entry.effective_profile_id("different-catalog"),
        Some(profile_id.as_str())
    );
}

#[test]
fn repeated_import_uses_the_explicit_settings() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let first = repository
        .commit_import(
            &prepared,
            GameSettings {
                game_scale: crate::GameScale::Manual { percent: 175 },
                ..GameSettings::default()
            },
        )
        .unwrap();
    fs::remove_file(repository.private_jar_path(&first)).unwrap();

    let replacement = GameSettings {
        fps_limit: crate::FpsLimit::Manual {
            frames_per_second: 30,
        },
        ..GameSettings::default()
    };
    let repeated = repository
        .commit_import(&prepared, replacement.clone())
        .unwrap();
    assert_eq!(repeated.settings(), &replacement);
    assert!(repository.private_jar_path(&repeated).is_file());
}

#[test]
fn reimported_jad_keeps_the_previous_published_entry_recoverable() {
    const JAD_ONE: &[u8] = b"MIDlet-Name: First descriptor\n\
MIDlet-Version: 1.0.0\n\
MIDlet-Vendor: J2Play\n\
MIDlet-1: Lifecycle Fixture, , fixtures.Stage7LifecycleMidlet\n";
    const JAD_TWO: &[u8] = b"MIDlet-Name: Second descriptor\n\
MIDlet-Version: 1.0.0\n\
MIDlet-Vendor: J2Play\n\
MIDlet-1: Lifecycle Fixture, , fixtures.Stage7LifecycleMidlet\n";

    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let first_prepared = inspect_import(ImportSource::new(
        None,
        FIXTURE.to_vec(),
        Some(JAD_ONE.to_vec()),
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    let first = repository
        .commit_import(&first_prepared, GameSettings::default())
        .unwrap();
    let mut unsigned_slot = first.clone();
    unsigned_slot.jad_sha256 = None;
    assert_eq!(
        unsigned_slot.validate().unwrap_err().code(),
        "library-jad-digest"
    );
    let metadata_path = repository
        .entries_dir()
        .join(format!("{}.json", first.id()));
    let previous_metadata = fs::read(&metadata_path).unwrap();

    let second_prepared = inspect_import(ImportSource::new(
        None,
        FIXTURE.to_vec(),
        Some(JAD_TWO.to_vec()),
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    let second = repository
        .commit_import(&second_prepared, GameSettings::default())
        .unwrap();
    assert_ne!(first.private_jad, second.private_jad);
    assert_eq!(
        repository.prepare_launch(&second).unwrap().jad_bytes(),
        Some(JAD_TWO)
    );

    // A crash before the second metadata rename would leave the first
    // record published. Its descriptor must still be intact.
    fs::write(&metadata_path, previous_metadata).unwrap();
    let restored = repository.load().unwrap().entries.remove(0);
    assert_eq!(
        repository.prepare_launch(&restored).unwrap().jad_bytes(),
        Some(JAD_ONE)
    );

    repository.delete_private_archives(&second).unwrap();
    for slot in 0..2 {
        assert!(
            !repository
                .archives_dir()
                .join(descriptor_slot_name(second.id(), slot))
                .exists()
        );
    }
}

#[test]
fn control_layout_and_vibration_persist_with_one_library_entry() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let mut entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let mut settings = entry.settings().clone();
    settings.control_layout.fire.offset_x = -1_250;
    settings.control_layout.fire.offset_y = 750;
    settings.control_layout.fire.size_percent = 135;
    settings.control_layout.fire.visible = false;
    settings.control_layout.number_keys[10].size_percent = 80;
    settings.control_layout.visible = false;
    settings.control_layout.corner_radius_percent = 0;
    settings.landscape_control_layout.direction_pad.offset_x = 500;
    settings.landscape_control_layout.number_keys[0].offset_y = -750;
    settings.landscape_control_layout.number_keys[11].visible = false;
    settings.vibration = crate::VibrationSettings {
        enabled: false,
        strength_percent: 42,
    };
    repository
        .save_settings(&mut entry, settings.clone())
        .unwrap();

    let loaded = repository.load().unwrap();
    assert!(loaded.warnings.is_empty());
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.entries[0].settings(), &settings);
}

#[test]
fn saving_settings_preserves_newer_import_metadata_and_folder_membership() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let mut stale = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let jad = b"MIDlet-1: Updated title, , fixtures.Stage7LifecycleMidlet\n";
    let prepared = inspect_import(ImportSource::new(
        None,
        FIXTURE.to_vec(),
        Some(jad.to_vec()),
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    let mut current = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let (_, folder) = repository.create_folder("Fixtures").unwrap();
    repository
        .move_to_folder(&mut current, Some(folder))
        .unwrap();
    let mut settings = stale.settings().clone();
    settings.vibration.enabled = false;
    repository
        .save_settings(&mut stale, settings.clone())
        .unwrap();
    current.settings = settings;
    assert_eq!(stale, current);
    assert_eq!(repository.load_entry(stale.id()).unwrap(), current);
    assert_eq!(
        repository.prepare_launch(&stale).unwrap().jad_bytes(),
        Some(jad.as_slice())
    );
}

#[test]
fn saving_settings_does_not_recreate_deleted_or_replace_corrupt_metadata() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let mut entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let original = entry.clone();
    repository.delete_entry_record(&entry).unwrap();
    let path = repository
        .entries_dir()
        .join(format!("{}.json", entry.id()));
    let mut settings = entry.settings().clone();
    settings.vibration.enabled = false;
    assert!(
        repository
            .save_settings(&mut entry, settings.clone())
            .is_err()
    );
    assert_eq!(entry, original);
    assert!(!path.exists());
    fs::write(&path, b"{broken").unwrap();
    assert!(repository.save_settings(&mut entry, settings).is_err());
    assert_eq!(entry, original);
    assert_eq!(fs::read(path).unwrap(), b"{broken");
}

#[test]
fn version_one_entry_loads_with_previous_control_defaults() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let metadata_path = repository
        .entries_dir()
        .join(format!("{}.json", entry.id()));
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(&metadata_path).unwrap()).unwrap();
    let settings = metadata
        .get_mut("settings")
        .and_then(serde_json::Value::as_object_mut)
        .unwrap();
    settings.insert("schema_version".to_owned(), serde_json::json!(1));
    settings.remove("control_layout");
    settings.remove("vibration");
    fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .unwrap();

    let loaded = repository.load().unwrap();
    assert!(loaded.warnings.is_empty());
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.entries[0].settings(), &GameSettings::default());
    assert_eq!(
        repository.load_entry(entry.id()).unwrap(),
        loaded.entries[0]
    );
}

#[test]
fn metadata_id_must_match_archive_and_midlet_identity() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(None, FIXTURE.to_vec(), None))
        .unwrap()
        .select_midlet(1)
        .unwrap();
    let mut entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    entry.id = "unrelated-safe-id".to_owned();
    assert_eq!(entry.validate().unwrap_err().code(), "library-entry-id");
}

#[test]
fn private_jad_is_integrity_checked_before_launch() {
    const JAD: &[u8] = b"MIDlet-Name: J2Play Lifecycle Fixture\n\
MIDlet-Version: 1.0.0\n\
MIDlet-Vendor: J2Play\n\
MIDlet-1: Lifecycle Fixture, , fixtures.Stage7LifecycleMidlet\n";

    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let prepared = inspect_import(ImportSource::new(
        None,
        FIXTURE.to_vec(),
        Some(JAD.to_vec()),
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let descriptor = repository
        .archives_dir()
        .join(entry.private_jad.as_ref().unwrap());
    fs::write(
        descriptor,
        b"MIDlet-1: Changed, , fixtures.Stage7LifecycleMidlet\n",
    )
    .unwrap();

    let error = repository
        .prepare_launch(&entry)
        .err()
        .expect("a changed private descriptor must fail closed");
    assert_eq!(error.code(), "library-jad-integrity");
}

#[test]
fn fullscreen_help_acknowledgement_is_versioned_and_atomic() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    assert!(!repository.fullscreen_help_acknowledged().unwrap());

    repository.acknowledge_fullscreen_help().unwrap();
    assert!(repository.fullscreen_help_acknowledged().unwrap());

    fs::write(repository.fullscreen_help_state_path(), b"unknown").unwrap();
    assert_eq!(
        repository
            .fullscreen_help_acknowledged()
            .unwrap_err()
            .code(),
        "frontend-state-fullscreen-help"
    );
}

#[test]
fn new_import_is_rejected_at_library_capacity() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    for index in 0..MAX_LIBRARY_ENTRIES {
        File::create(
            repository
                .entries_dir()
                .join(format!("occupied-{index}.json")),
        )
        .unwrap();
    }
    assert_eq!(
        repository.ensure_new_entry_capacity().unwrap_err().code(),
        "library-entry-capacity"
    );
}
