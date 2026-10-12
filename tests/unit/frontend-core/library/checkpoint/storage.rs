use super::*;
use std::io::{self, Write};

#[test]
fn validation_rejects_file_ancestors_in_any_order_before_restoring_storage() {
    for ancestor in ["parent", "parent/child"] {
        let mut entries = vec![
            StorageEntry {
                path: ancestor.into(),
                bytes: Some(vec![]),
            },
            StorageEntry {
                path: "parent/child/data".into(),
                bytes: Some(vec![1]),
            },
        ];
        assert!(validate_storage(&entries).is_err());
        entries.reverse();
        assert!(validate_storage(&entries).is_err());
    }
}

#[test]
fn restoration_allows_implicit_parents_and_distinct_names_with_common_prefixes() {
    let scratch = crate::test_storage::Scratch::new();
    let entries = [
        StorageEntry {
            path: "parent".into(),
            bytes: Some(vec![1]),
        },
        StorageEntry {
            path: "parent-more/child/data".into(),
            bytes: Some(vec![2]),
        },
        StorageEntry {
            path: "parent-more".into(),
            bytes: None,
        },
    ];
    validate_storage(&entries).unwrap();
    restore_storage(&scratch.0, &entries).unwrap();
    assert_eq!(fs::read(scratch.0.join("parent")).unwrap(), [1]);
    assert_eq!(
        fs::read(scratch.0.join("parent-more/child/data")).unwrap(),
        [2]
    );
}

#[cfg(unix)]
#[test]
fn capture_rejects_dangling_root_links_instead_of_saving_empty_storage() {
    let scratch = crate::test_storage::Scratch::new();
    let root = scratch.0.join("files");
    assert!(capture_storage(&root, || false).unwrap().is_empty());
    let missing = scratch.0.join("missing");
    std::os::unix::fs::symlink(&missing, &root).unwrap();
    assert!(capture_storage(&root, || false).is_err());
    assert_eq!(fs::read_link(&root).unwrap(), missing);
}

#[test]
fn capturing_one_large_file_honors_cancellation_and_keeps_source_data() {
    let scratch = crate::test_storage::Scratch::new();
    let root = scratch.0.join("files");
    assert!(capture_storage(&root, || true).is_err());
    fs::create_dir_all(&root).unwrap();
    assert!(capture_storage(&root, || true).is_err());
    let expected = vec![0x4d; 1024 * 1024];
    fs::write(root.join("data"), &expected).unwrap();
    let mut polls = 0;
    let error = capture_storage(&root, || {
        polls += 1;
        polls == 8
    })
    .err()
    .expect("cancellation is observed inside the only file");
    assert!(error.message().contains("interrupted"));
    assert_eq!(polls, 8);
    assert_eq!(fs::read(root.join("data")).unwrap(), expected);
    let saved = capture_storage(&root, || false).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].bytes.as_deref(), Some(expected.as_slice()));
}

#[derive(Default)]
struct RecordingWriter {
    bytes: Vec<u8>,
    calls: usize,
}

impl Write for RecordingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn file_contents_use_block_writes_without_changing_the_checkpoint_format() {
    for bytes in [
        None,
        Some(Vec::new()),
        Some((0..=255).collect()),
        Some(vec![0x8f; 1024 * 1024]),
    ] {
        let entry = StorageEntry {
            path: "data/fixture".into(),
            bytes,
        };
        let legacy = save_state::encode(&(&entry.path, &entry.bytes)).unwrap();
        let mut writer = RecordingWriter::default();
        let length = save_state::write(
            &entry,
            &mut writer,
            save_state::MAX_COMPONENT_BYTES,
            &|| false,
        )
        .unwrap();
        assert_eq!(writer.bytes, legacy);
        assert_eq!(length, legacy.len());
        assert!(
            writer.calls <= 32,
            "{} writes for {length} bytes",
            writer.calls
        );
        let restored: StorageEntry = save_state::decode(&legacy).unwrap();
        assert_eq!(restored.path, entry.path);
        assert_eq!(restored.bytes, entry.bytes);
    }
}

#[test]
fn a_large_file_remains_cancellable_within_its_byte_payload() {
    let entry = StorageEntry {
        path: "fixture".into(),
        bytes: Some(vec![17; 1024 * 1024]),
    };
    let checks = std::cell::Cell::new(0);
    let mut writer = RecordingWriter::default();
    assert!(
        save_state::write(
            &entry,
            &mut writer,
            save_state::MAX_COMPONENT_BYTES,
            &|| {
                checks.set(checks.get() + 1);
                checks.get() == 3
            }
        )
        .is_err()
    );
    assert_eq!(checks.get(), 3);
    assert!(!writer.bytes.is_empty() && writer.bytes.len() < 1024 * 1024);
    assert_eq!(entry.bytes.as_deref().unwrap(), vec![17; 1024 * 1024]);
}
