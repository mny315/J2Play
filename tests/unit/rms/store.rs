use super::*;
use crate::{Limits, repository::CommitPhase, tests::repository};
use std::fs;

#[test]
fn records_persist_and_deleted_ids_are_not_reused() {
    let (_root, repository, suite) = repository();
    let mut store = repository.open(&suite, "save", true, 10).unwrap();
    assert_eq!(store.add(b"one", 11).unwrap(), 1);
    assert_eq!(store.add(b"two", 12).unwrap(), 2);
    store.delete(1, 13).unwrap();
    assert_eq!(store.add(b"three", 14).unwrap(), 3);
    assert_eq!(store.version(), 4);
    assert_eq!(store.last_modified(), 14);
    drop(store);

    let reopened = repository.open(&suite, "save", false, 99).unwrap();
    assert_eq!(reopened.get(2), Some(&b"two"[..]));
    assert_eq!(reopened.get(3), Some(&b"three"[..]));
    assert_eq!(reopened.get(1), None);
    assert_eq!(reopened.next_record_id(), 4);
}

#[test]
fn interrupted_temporary_snapshot_does_not_replace_committed_state() {
    let (_root, repository, suite) = repository();
    let mut store = repository.open(&suite, "save", true, 10).unwrap();
    store.add(b"committed", 11).unwrap();
    let final_path = repository.store_path(suite.digest(), "save");
    let temporary = final_path.with_extension("tmp");
    let mut candidate = store.state.clone();
    candidate
        .records
        .insert(1, Arc::from(b"interrupted".as_slice()));
    fs::write(temporary, candidate.encode(repository.limits).unwrap()).unwrap();
    drop(store);

    let reopened = repository.open(&suite, "save", false, 99).unwrap();
    assert_eq!(reopened.get(1), Some(&b"committed"[..]));
}

#[test]
fn failed_mutation_does_not_change_visible_state() {
    let (root, _repository, suite) = repository();
    let tiny = Repository::new(
        &root.0,
        Limits {
            max_record_bytes: 2,
            ..Limits::default()
        },
    );
    let mut store = tiny.open(&suite, "save", true, 1).unwrap();
    assert_eq!(store.add(b"long", 2).unwrap_err().code(), "record-limit");
    assert_eq!(store.version(), 0);
    assert_eq!(store.record_count(), 0);
    assert_eq!(store.next_record_id(), 1);
}

#[test]
fn every_injected_commit_phase_reopens_as_a_complete_old_or_new_snapshot() {
    for phase in [
        CommitPhase::BeforeTemporaryOpen,
        CommitPhase::AfterTemporaryWrite,
        CommitPhase::AfterTemporarySync,
        CommitPhase::AfterRename,
        CommitPhase::AfterDirectorySync,
    ] {
        let (_root, repository, suite) = repository();
        let mut store = repository.open(&suite, "save", true, 1).unwrap();
        store.add(b"old", 2).unwrap();
        repository.inject_failure(phase);
        assert_eq!(
            store.set(1, b"new", 3).unwrap_err().code(),
            "injected-commit-failure"
        );
        let visible_in_handle: &[u8] = if matches!(
            phase,
            CommitPhase::AfterRename | CommitPhase::AfterDirectorySync
        ) {
            b"new"
        } else {
            b"old"
        };
        assert_eq!(store.get(1), Some(visible_in_handle), "{phase:?}");
        drop(store);
        let reopened = repository.open(&suite, "save", false, 4).unwrap();
        assert_eq!(reopened.get(1), Some(visible_in_handle), "{phase:?}");
    }
}

#[test]
fn rejected_mutations_preserve_state_bytes_and_validation_order() {
    let (_root, mut repository, suite) = repository();
    repository.limits.max_record_bytes = 4;
    let mut store = repository.open(&suite, "save", true, 10).unwrap();
    store.add(b"one", 11).unwrap();
    let original = store.state.clone();
    let path = repository.store_path(suite.digest(), "save");
    let bytes = fs::read(&path).unwrap();
    assert_eq!(
        store.set(99, b"x", 12).unwrap_err().code(),
        "invalid-record-id"
    );
    assert_eq!(
        store.delete(99, 12).unwrap_err().code(),
        "invalid-record-id"
    );
    assert_eq!(store.add(b"large", 12).unwrap_err().code(), "record-limit");
    assert_eq!(store.state, original);

    store.state.version = u32::MAX;
    assert_eq!(
        store.set(99, b"large", 12).unwrap_err().code(),
        "record-limit"
    );
    assert_eq!(
        store.set(99, b"x", 12).unwrap_err().code(),
        "invalid-record-id"
    );
    assert_eq!(
        store.set(1, b"x", 12).unwrap_err().code(),
        "version-exhausted"
    );
    store.state.next_record_id = MAX_RECORD_ID + 1;
    assert_eq!(
        store.add(b"x", 12).unwrap_err().code(),
        "record-id-exhausted"
    );
    assert_eq!(store.get(1), Some(b"one".as_slice()));
    assert_eq!(store.last_modified(), 11);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
