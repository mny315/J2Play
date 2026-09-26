use super::*;
use natives::RmsMetadataField;

#[test]
fn bulk_store_encoding_preserves_the_existing_checkpoint_wire_format() {
    let saved = Checkpoint {
        next_handle: 42,
        stores: vec![Vec::new(), (0..=255).collect(), vec![127; 65_536]],
        open: vec![("progress".to_owned(), 41, 2)],
    };
    let previous = save_state::encode(&(saved.next_handle, &saved.stores, &saved.open)).unwrap();
    assert_eq!(save_state::encode(&saved).unwrap(), previous);
    let decoded: Checkpoint = save_state::decode(&previous).unwrap();
    assert_eq!(decoded.next_handle, saved.next_handle);
    assert_eq!(decoded.stores, saved.stores);
    assert_eq!(decoded.open, saved.open);
}

#[test]
fn restores_record_versions_and_balanced_handles_without_touching_the_active_root() {
    let (root, _, suite) = crate::tests::repository();
    let mut original = Runtime::new(root.0.join("active"), suite.clone(), Limits::default());
    let handle = original.open("progress", true, 10).unwrap();
    original.open("progress", false, 10).unwrap();
    let record = original.add(handle, b"level 4", 20).unwrap();
    let bytes = original.encode_checkpoint().unwrap();
    original.set(handle, record, b"level 5", 30).unwrap();
    let mut restored =
        Runtime::restore_checkpoint(root.0.join("resumed"), suite, Limits::default(), &bytes)
            .unwrap();
    assert_eq!(restored.get(handle, record).unwrap(), b"level 4");
    assert_eq!(original.get(handle, record).unwrap(), b"level 5");
    assert_eq!(
        restored
            .metadata(handle, RmsMetadataField::Version)
            .unwrap(),
        1
    );
    assert_eq!(
        restored
            .metadata(handle, RmsMetadataField::LastModified)
            .unwrap(),
        20
    );
    assert_eq!(
        original
            .metadata(handle, RmsMetadataField::Version)
            .unwrap(),
        2
    );
    restored.close(handle).unwrap();
    assert_eq!(restored.get(handle, record).unwrap(), b"level 4");
    restored.close(handle).unwrap();
    assert!(restored.get(handle, record).is_err());
    assert_ne!(restored.open("progress", false, 40).unwrap(), handle);
}

#[test]
fn checkpoint_retains_committed_bytes_for_open_and_closed_stores_within_suite_quota() {
    let (root, repository, suite) = crate::tests::repository();
    let mut runtime = Runtime::new(&root.0, suite.clone(), Limits::default());
    let mut expected = Vec::new();
    for (name, close) in [("open", false), ("closed", true)] {
        let handle = runtime.open(name, true, 10).unwrap();
        runtime.add(handle, name.as_bytes(), 20).unwrap();
        if close {
            runtime.close(handle).unwrap();
        }
        expected.push(std::fs::read(repository.store_path(suite.digest(), name)).unwrap());
    }
    let total = expected.iter().map(Vec::len).sum::<usize>();
    runtime.repository.limits.max_suite_bytes = total;
    let mut saved: Checkpoint = save_state::decode(&runtime.encode_checkpoint().unwrap()).unwrap();
    expected.sort();
    saved.stores.sort();
    assert_eq!(saved.stores, expected);
    assert_eq!(saved.open.len(), 1);
    assert_eq!(saved.open[0].0, "open");

    runtime.repository.limits.max_suite_bytes = total - 1;
    assert_eq!(
        runtime.encode_checkpoint().unwrap_err().code(),
        "corrupt-store"
    );
    for name in ["open", "closed"] {
        let bytes = std::fs::read(repository.store_path(suite.digest(), name)).unwrap();
        assert!(expected.contains(&bytes));
    }
}

#[test]
fn cancellation_precedes_directory_access_and_interrupts_between_stores() {
    let (root, repository, suite) = crate::tests::repository();
    let mut runtime = Runtime::new(&root.0, suite.clone(), Limits::default());
    for name in ["one", "two"] {
        runtime.open(name, true, 10).unwrap();
    }
    let calls = std::cell::Cell::new(0);
    assert_eq!(
        runtime
            .encode_checkpoint_cancellable(&|| {
                calls.set(calls.get() + 1);
                calls.get() == 3
            })
            .unwrap_err()
            .code(),
        "checkpoint-cancelled"
    );
    assert_eq!(calls.get(), 3);

    std::fs::write(repository.store_path(suite.digest(), "one"), b"corrupt").unwrap();
    assert_eq!(
        runtime
            .encode_checkpoint_cancellable(&|| true)
            .unwrap_err()
            .code(),
        "checkpoint-cancelled"
    );
    assert_eq!(
        runtime.encode_checkpoint().unwrap_err().code(),
        "corrupt-store"
    );
}
