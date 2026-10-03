use super::*;
use flate2::read::ZlibDecoder;
use std::io::Read;

#[test]
fn checkpoint_rejects_incomplete_zlib_streams_even_with_a_valid_outer_checksum() {
    let (_scratch, repository, entry, checkpoint) = fixture();
    let epoch = repository.begin_checkpoint_session(&entry).unwrap();
    repository
        .write_checkpoint(&entry, epoch, &checkpoint, &|| false)
        .unwrap();
    let path = repository.checkpoint_root(&entry).join("automatic.save");
    let original = fs::read(&path).unwrap();
    for missing in 1..=4 {
        let mut truncated = original[..original.len() - missing].to_vec();
        let mut digest = Sha256::new();
        digest.update(&truncated[..28]);
        digest.update(&truncated[HEADER_BYTES..]);
        truncated[28..HEADER_BYTES].copy_from_slice(&digest.finalize());
        fs::write(&path, &truncated).unwrap();
        assert!(
            repository
                .load_checkpoint(&entry, &checkpoint.identity)
                .is_err(),
            "accepted zlib stream missing {missing} trailer bytes"
        );
        assert_eq!(fs::read(&path).unwrap(), truncated);
    }
}

#[test]
fn checkpoint_compression_preserves_zlib_payload_and_cancels_before_replacing_the_save() {
    use std::io::Write;
    let (_scratch, repository, entry, mut checkpoint) = fixture();
    let epoch = repository.begin_checkpoint_session(&entry).unwrap();
    checkpoint.vm = (0..=255).cycle().take(1024 * 1024).collect();
    checkpoint.mmapi = checkpoint.vm.clone();
    let expected = save_state::encode(&checkpoint).unwrap();
    let (length, compressed) = compression::encode(&checkpoint, &|| false).unwrap();
    assert_eq!(length, expected.len());
    let mut decoded = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut decoded)
        .unwrap();
    assert_eq!(decoded, expected);
    repository
        .write_checkpoint(&entry, epoch, &checkpoint, &|| false)
        .unwrap();
    let path = repository.checkpoint_root(&entry).join("automatic.save");
    let mut previous = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    previous.write_all(&expected).unwrap();
    let previous = previous.finish().unwrap();
    let mut original = fs::read(&path).unwrap()[..28].to_vec();
    let mut digest = Sha256::new();
    digest.update(&original);
    digest.update(&previous);
    original.extend_from_slice(&digest.finalize());
    original.extend_from_slice(&previous);
    fs::write(&path, &original).unwrap();
    let polls = std::cell::Cell::new(0);
    assert!(
        repository
            .write_checkpoint(&entry, epoch, &checkpoint, &|| {
                polls.set(polls.get() + 1);
                polls.get() > 8
            })
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    let restored = repository
        .load_checkpoint(&entry, &checkpoint.identity)
        .unwrap()
        .unwrap();
    assert_eq!(restored.vm, checkpoint.vm);
    assert_eq!(restored.mmapi, checkpoint.mmapi);
}

fn fixture() -> (
    crate::test_storage::Scratch,
    LibraryRepository,
    LibraryEntry,
    Checkpoint,
) {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepared = crate::inspect_import(crate::ImportSource::new(
        None,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/java-me/conformance.jar"
        ))
        .to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(2)
    .unwrap();
    let entry = repository
        .commit_import(&prepared, crate::GameSettings::default())
        .unwrap();
    let checkpoint = Checkpoint {
        identity: CheckpointIdentity {
            jar_sha256: "fixture".into(),
            midlet_class: "Input".into(),
            profile_catalog: "catalog".into(),
            persona: "phone".into(),
            runtime_host: "host".into(),
            canvas: (240, 320),
            properties: [0; 32],
        },
        saved_at_millis: 1000,
        monotonic_millis: 400,
        wall_clock_millis: 1000,
        active_canvas: (240, 320),
        frame: None,
        vm: vec![5; 65536],
        driver: vec![3],
        ams: vec![],
        rms: vec![],
        mmapi: vec![],
        files: vec![],
    };
    (scratch, repository, entry, checkpoint)
}

#[test]
fn checkpoint_validates_checksum_identity_and_launch_epoch_and_keeps_the_old_file_on_failure() {
    let (_scratch, repository, entry, checkpoint) = fixture();
    let epoch = repository.begin_checkpoint_session(&entry).unwrap();
    repository
        .write_checkpoint(&entry, epoch, &checkpoint, &|| false)
        .unwrap();
    let saved = repository
        .load_checkpoint(&entry, &checkpoint.identity)
        .unwrap()
        .unwrap();
    assert_eq!(saved.vm, checkpoint.vm);
    assert!(
        repository
            .write_checkpoint(&entry, epoch, &checkpoint, &|| true)
            .is_err()
    );
    assert!(
        repository
            .load_checkpoint(&entry, &checkpoint.identity)
            .unwrap()
            .is_some()
    );
    let mut changed = checkpoint.identity.clone();
    changed.canvas = (320, 240);
    assert!(repository.load_checkpoint(&entry, &changed).is_err());
    let path = repository.checkpoint_root(&entry).join("automatic.save");
    let bytes = fs::read(&path).unwrap();
    // A valid checksum cannot make continuations from an old bootstrap safe.
    let mut previous_format = bytes.clone();
    previous_format[8..12].copy_from_slice(&(FORMAT - 1).to_be_bytes());
    let mut digest = Sha256::new();
    digest.update(&previous_format[..28]);
    digest.update(&previous_format[HEADER_BYTES..]);
    previous_format[28..HEADER_BYTES].copy_from_slice(&digest.finalize());
    fs::write(&path, &previous_format).unwrap();
    let error = repository
        .load_checkpoint(&entry, &checkpoint.identity)
        .err()
        .unwrap();
    assert_eq!(
        error.message(),
        "This automatic save uses an unsupported format."
    );
    assert_eq!(fs::read(&path).unwrap(), previous_format);
    let mut damaged = bytes.clone();
    *damaged.last_mut().unwrap() ^= 1;
    fs::write(&path, damaged).unwrap();
    assert!(
        repository
            .load_checkpoint(&entry, &checkpoint.identity)
            .is_err()
    );
    fs::write(&path, bytes).unwrap();
    repository.invalidate_checkpoint(&entry, epoch).unwrap();
    assert!(
        repository
            .load_checkpoint(&entry, &checkpoint.identity)
            .unwrap()
            .is_none()
    );
    assert!(path.exists());
}

#[test]
fn checkpoint_restoration_publishes_storage_only_after_validation_and_reuses_bounded_slots() {
    let (_scratch, repository, entry, _) = fixture();
    let original = repository.runtime_storage(&entry).unwrap();
    fs::create_dir_all(&original.files).unwrap();
    fs::write(original.files.join("score"), b"old").unwrap();
    let entries = capture_storage(&original.files, || false).unwrap();
    fs::write(original.files.join("score"), b"current").unwrap();
    let candidate = repository
        .prepare_restored_storage(&entry, &entries)
        .unwrap();
    assert_eq!(
        repository.runtime_storage(&entry).unwrap().files,
        original.files
    );
    assert_eq!(fs::read(candidate.files.join("score")).unwrap(), b"old");
    repository
        .publish_restored_storage(&entry, &candidate)
        .unwrap();
    assert_eq!(
        repository.runtime_storage(&entry).unwrap().files,
        candidate.files
    );
    for _ in 0..3 {
        let next = repository
            .prepare_restored_storage(&entry, &entries)
            .unwrap();
        assert!(
            fs::read(
                repository
                    .runtime_storage(&entry)
                    .unwrap()
                    .files
                    .join("score")
            )
            .is_ok()
        );
        repository.publish_restored_storage(&entry, &next).unwrap();
    }
    let directories = fs::read_dir(repository.checkpoint_root(&entry))
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_type().unwrap().is_dir())
        .map(|entry| entry.file_name())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        directories,
        ["storage-0", "storage-1"]
            .map(std::ffi::OsString::from)
            .into()
    );
    assert_eq!(fs::read(original.files.join("score")).unwrap(), b"current");
    repository.delete_runtime_data(&entry).unwrap();
    assert!(!repository.checkpoint_root(&entry).exists());
}

#[test]
fn clearing_resume_save_preserves_current_storage_and_settings_after_resume() {
    let (_scratch, repository, entry, checkpoint) = fixture();
    let original = repository.runtime_storage(&entry).unwrap();
    fs::create_dir_all(&original.rms).unwrap();
    fs::create_dir_all(&original.files).unwrap();
    fs::write(original.rms.join("score"), b"original rms").unwrap();
    fs::write(original.files.join("save"), b"original files").unwrap();
    let restored = repository.prepare_restored_storage(&entry, &[]).unwrap();
    fs::write(restored.rms.join("score"), b"current rms").unwrap();
    fs::write(restored.files.join("save"), b"current files").unwrap();
    repository
        .publish_restored_storage(&entry, &restored)
        .unwrap();
    let epoch = repository.begin_checkpoint_session(&entry).unwrap();
    repository
        .write_checkpoint(&entry, epoch, &checkpoint, &|| false)
        .unwrap();

    for _ in 0..2 {
        repository.delete_resume_save(&entry).unwrap();
        assert!(
            !repository
                .checkpoint_root(&entry)
                .join("automatic.save")
                .exists()
        );
        assert!(
            repository
                .load_checkpoint(&entry, &checkpoint.identity)
                .unwrap()
                .is_none()
        );
        let current = repository.runtime_storage(&entry).unwrap();
        assert_eq!(current.rms, restored.rms);
        assert_eq!(fs::read(current.rms.join("score")).unwrap(), b"current rms");
        assert_eq!(
            fs::read(current.files.join("save")).unwrap(),
            b"current files"
        );
        assert_eq!(
            fs::read(original.rms.join("score")).unwrap(),
            b"original rms"
        );
        assert_eq!(
            fs::read(original.files.join("save")).unwrap(),
            b"original files"
        );
        assert_eq!(
            repository.load().unwrap().entries,
            std::slice::from_ref(&entry)
        );
        assert!(repository.private_jar_path(&entry).is_file());
    }
}

#[test]
fn deleting_a_game_removes_original_and_restored_storage_and_is_retryable() {
    let (_scratch, repository, entry, checkpoint) = fixture();
    let epoch = repository.begin_checkpoint_session(&entry).unwrap();
    repository
        .write_checkpoint(&entry, epoch, &checkpoint, &|| false)
        .unwrap();
    for root in [repository.rms_root(&entry), repository.file_root(&entry)] {
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("save"), b"data").unwrap();
    }
    for _ in 0..2 {
        let restored = repository.prepare_restored_storage(&entry, &[]).unwrap();
        fs::write(restored.rms.join("score"), b"restored rms").unwrap();
        repository
            .publish_restored_storage(&entry, &restored)
            .unwrap();
    }
    for slot in [
        format!("{}.jad", entry.id()),
        crate::library::descriptor_slot_name(entry.id(), 0),
        crate::library::descriptor_slot_name(entry.id(), 1),
    ] {
        fs::write(repository.archives_dir().join(slot), b"descriptor").unwrap();
    }
    for _ in 0..2 {
        repository.delete_game(&entry).unwrap();
        assert!(!repository.rms_root(&entry).exists());
        assert!(!repository.file_root(&entry).exists());
        assert!(!repository.checkpoint_root(&entry).exists());
        assert!(!repository.private_jar_path(&entry).exists());
        assert_eq!(fs::read_dir(repository.archives_dir()).unwrap().count(), 0);
        assert!(repository.load().unwrap().entries.is_empty());
    }
}

#[test]
fn checkpoint_rejects_escaping_paths_and_symlinks() {
    for path in ["../escape", "/absolute", "a/../b", "a//b", "a/./b"] {
        let entries = vec![storage::StorageEntry {
            path: path.into(),
            bytes: Some(vec![1]),
        }];
        assert!(storage::validate_storage(&entries).is_err(), "{path}");
    }
    #[cfg(unix)]
    {
        let (_scratch, repository, entry, _) = fixture();
        let root = repository.file_root(&entry);
        fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink("/etc", root.join("outside")).unwrap();
        assert!(capture_storage(&root, || false).is_err());
    }
}
