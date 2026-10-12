use super::*;
use crate::tests::{refresh_checksum, repository};

#[test]
fn all_repository_scans_bound_ignored_directory_entries() {
    let (_root, mut repository, suite) = repository();
    repository.limits.max_stores_per_suite = 2;
    let mut store = repository.open(&suite, "save", true, 0).unwrap();
    store.add(b"retained", 1).unwrap();
    let directory = repository.suite_path(suite.digest());
    for index in 0..5 {
        fs::write(directory.join(format!("leftover-{index}.tmp")), b"").unwrap();
    }
    assert_eq!(repository.list(&suite).unwrap_err().code(), "store-limit");
    assert_eq!(
        repository
            .checkpoint_stores(&suite, &|| false)
            .unwrap_err()
            .code(),
        "store-limit"
    );
    assert_eq!(
        repository.latest_modified(&suite).unwrap_err().code(),
        "store-limit"
    );
    assert_eq!(store.size_available().unwrap_err().code(), "store-limit");
    assert_eq!(store.add(b"new", 2).unwrap_err().code(), "store-limit");
    assert_eq!(store.get(1), Some(b"retained".as_slice()));
}

#[cfg(unix)]
#[test]
fn repository_rejects_store_symlinks_before_reading() {
    let (_root, repository, suite) = repository();
    let store = repository.open(&suite, "save", true, 0).unwrap();
    let original = repository.store_path(suite.digest(), "save");
    let moved = original.with_extension("backup");
    fs::rename(&original, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &original).unwrap();
    assert_eq!(
        repository
            .open(&suite, "save", false, 0)
            .unwrap_err()
            .code(),
        "corrupt-store"
    );
    assert_eq!(repository.list(&suite).unwrap_err().code(), "corrupt-store");
    assert_eq!(
        repository
            .checkpoint_stores(&suite, &|| false)
            .unwrap_err()
            .code(),
        "corrupt-store"
    );
    assert_eq!(store.size_available().unwrap_err().code(), "corrupt-store");
}

#[cfg(unix)]
#[test]
fn suite_symlinks_cannot_redirect_reads_metadata_or_deletion() {
    let (root, repository, suite) = repository();
    let mut store = repository.open(&suite, "save", true, 0).unwrap();
    store.add(b"retained", 1).unwrap();
    let directory = repository.suite_path(suite.digest());
    let relocated = root.0.join("relocated-suite");
    fs::rename(&directory, &relocated).unwrap();
    std::os::unix::fs::symlink(&relocated, &directory).unwrap();

    assert_eq!(
        repository
            .open(&suite, "save", false, 2)
            .unwrap_err()
            .code(),
        "corrupt-store"
    );
    assert_eq!(repository.list(&suite).unwrap_err().code(), "corrupt-store");
    assert_eq!(
        repository.latest_modified(&suite).unwrap_err().code(),
        "corrupt-store"
    );
    assert_eq!(store.size_available().unwrap_err().code(), "corrupt-store");
    assert_eq!(
        repository.delete(&suite, "save").unwrap_err().code(),
        "corrupt-store"
    );
    let snapshot = relocated.join(format!("{}.rms", hex(&store_digest("save"))));
    assert_eq!(
        decode_file(&snapshot, repository.limits, suite.digest(), "save")
            .unwrap()
            .records[&1]
            .as_ref(),
        b"retained"
    );
}

#[test]
fn metadata_scans_reject_snapshots_stored_under_another_name() {
    let (_root, repository, suite) = repository();
    repository.open(&suite, "save", true, 0).unwrap();
    let path = repository.store_path(suite.digest(), "save");
    let replacement = StoreState::empty(suite.digest(), "different", 1)
        .encode(repository.limits)
        .unwrap();
    fs::write(&path, &replacement).unwrap();

    assert_eq!(repository.list(&suite).unwrap_err().code(), "corrupt-store");
    assert_eq!(
        repository
            .checkpoint_stores(&suite, &|| false)
            .unwrap_err()
            .code(),
        "corrupt-store"
    );
    assert_eq!(
        repository.latest_modified(&suite).unwrap_err().code(),
        "corrupt-store"
    );
    assert_eq!(
        repository.delete(&suite, "save").unwrap_err().code(),
        "corrupt-store"
    );
    assert_eq!(fs::read(&path).unwrap(), replacement);
}

#[test]
fn corruption_is_bounded_to_one_store() {
    let (_root, repository, suite) = repository();
    repository.open(&suite, "good", true, 1).unwrap();
    repository.open(&suite, "bad", true, 1).unwrap();
    let bad = repository.store_path(suite.digest(), "bad");
    fs::write(&bad, b"corrupt").unwrap();
    assert_eq!(
        repository.open(&suite, "bad", false, 2).unwrap_err().code(),
        "corrupt-store"
    );
    assert_eq!(
        repository.open(&suite, "good", false, 2).unwrap().name(),
        "good"
    );
}

#[test]
fn missing_stores_are_created_only_when_requested() {
    let (_root, repository, suite) = repository();
    assert_eq!(
        repository
            .open(&suite, "save", false, 0)
            .unwrap_err()
            .code(),
        "store-not-found"
    );
    assert_eq!(
        repository.delete(&suite, "save").unwrap_err().code(),
        "store-not-found"
    );
    repository.open(&suite, "save", true, 0).unwrap();
    repository.delete(&suite, "save").unwrap();
    assert_eq!(
        repository.delete(&suite, "save").unwrap_err().code(),
        "store-not-found"
    );
}

#[test]
fn non_file_store_paths_are_reported_as_corruption() {
    let (_root, repository, suite) = repository();
    let path = repository.store_path(suite.digest(), "save");
    fs::create_dir_all(&path).unwrap();
    let retained = path.join("retained");
    fs::write(&retained, b"data").unwrap();
    for create in [false, true] {
        assert_eq!(
            repository
                .open(&suite, "save", create, 0)
                .unwrap_err()
                .code(),
            "corrupt-store"
        );
    }
    assert_eq!(
        repository.delete(&suite, "save").unwrap_err().code(),
        "corrupt-store"
    );
    assert_eq!(fs::read(retained).unwrap(), b"data");
}

#[cfg(unix)]
#[test]
fn dangling_store_symlinks_are_reported_as_corruption() {
    let (root, repository, suite) = repository();
    let path = repository.store_path(suite.digest(), "save");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let missing = root.0.join("missing");
    std::os::unix::fs::symlink(&missing, &path).unwrap();
    for create in [false, true] {
        assert_eq!(
            repository
                .open(&suite, "save", create, 0)
                .unwrap_err()
                .code(),
            "corrupt-store"
        );
    }
    assert_eq!(
        repository.delete(&suite, "save").unwrap_err().code(),
        "corrupt-store"
    );
    assert_eq!(fs::read_link(path).unwrap(), missing);
    assert!(!missing.exists());
}

#[test]
fn actual_store_read_is_bounded_independently_of_metadata() {
    assert_eq!(
        read_store_bytes(&b"oversized"[..], 4).unwrap_err().code(),
        "corrupt-store"
    );
    assert_eq!(read_store_bytes(&b"four"[..], 4).unwrap(), b"four");
}

#[test]
fn suite_and_store_names_never_become_path_components() {
    let (root, repository, _) = repository();
    let first = SuiteId::new("Vendor/../A", "Same").unwrap();
    let second = SuiteId::new("Vendor", "../A/Same").unwrap();
    repository.open(&first, "../save", true, 1).unwrap();
    repository.open(&second, "../save", true, 1).unwrap();
    assert_ne!(first.digest(), second.digest());
    assert_eq!(repository.list(&first).unwrap(), vec!["../save"]);
    assert_eq!(repository.list(&second).unwrap(), vec!["../save"]);
    assert!(!root.0.join("save").exists());
}

#[cfg(unix)]
#[test]
fn temporary_and_suite_symlinks_cannot_redirect_commits() {
    use std::os::unix::fs::symlink;

    let (root, repository, suite) = repository();
    let mut store = repository.open(&suite, "save", true, 1).unwrap();
    let victim = root.0.join("victim");
    fs::write(&victim, b"untouched").unwrap();
    let temporary = repository
        .store_path(suite.digest(), "save")
        .with_extension("tmp");
    symlink(&victim, &temporary).unwrap();
    store.add(b"state", 2).unwrap();
    assert_eq!(fs::read(&victim).unwrap(), b"untouched");

    let redirected_suite = SuiteId::new("Redirect", "Suite").unwrap();
    let outside = root.0.join("outside");
    fs::create_dir(&outside).unwrap();
    let suite_path = repository.suite_path(redirected_suite.digest());
    symlink(&outside, suite_path).unwrap();
    assert_eq!(
        repository
            .open(&redirected_suite, "save", true, 1)
            .unwrap_err()
            .code(),
        "create-suite"
    );
    assert!(fs::read_dir(outside).unwrap().next().is_none());
}

#[test]
fn store_suite_and_count_quotas_are_enforced_before_commit() {
    let (root, _repository, suite) = repository();
    let store_limited = Repository::new(
        &root.0,
        Limits {
            max_store_bytes: 115,
            ..Limits::default()
        },
    );
    let mut store = store_limited.open(&suite, "save", true, 1).unwrap();
    assert_eq!(store.add(&[0; 8], 2).unwrap_err().code(), "store-full");
    assert_eq!(store.record_count(), 0);

    let count_suite = SuiteId::new("Quota", "Count").unwrap();
    let count_limited = Repository::new(
        &root.0,
        Limits {
            max_stores_per_suite: 1,
            ..Limits::default()
        },
    );
    count_limited.open(&count_suite, "one", true, 1).unwrap();
    assert_eq!(
        count_limited
            .open(&count_suite, "two", true, 1)
            .unwrap_err()
            .code(),
        "store-limit"
    );

    let bytes_suite = SuiteId::new("Quota", "Bytes").unwrap();
    let suite_limited = Repository::new(
        &root.0,
        Limits {
            max_suite_bytes: 190,
            ..Limits::default()
        },
    );
    suite_limited.open(&bytes_suite, "one", true, 1).unwrap();
    assert_eq!(
        suite_limited
            .open(&bytes_suite, "two", true, 1)
            .unwrap_err()
            .code(),
        "store-full"
    );
}

#[test]
fn metadata_scans_reject_corrupt_record_tables_and_checksums() {
    let (_root, repository, suite) = repository();
    let mut store = repository.open(&suite, "save", true, 10).unwrap();
    store.add(b"one", 11).unwrap();
    store.add(b"", 12).unwrap();
    let path = repository.store_path(suite.digest(), "save");
    let valid = fs::read(&path).unwrap();
    assert_eq!(repository.list(&suite).unwrap(), ["save"]);
    assert_eq!(repository.latest_modified(&suite).unwrap(), Some(12));

    let mut unordered = valid.clone();
    let second_record = 68 + 8 + 3;
    unordered[second_record..second_record + 4].copy_from_slice(&1_u32.to_be_bytes());
    refresh_checksum(&mut unordered);
    let mut oversized = valid.clone();
    oversized[second_record + 4..second_record + 8].copy_from_slice(&u32::MAX.to_be_bytes());
    refresh_checksum(&mut oversized);
    let mut bad_checksum = valid.clone();
    bad_checksum[68 + 8] ^= 1;
    for invalid in [unordered, oversized, bad_checksum] {
        fs::write(&path, invalid).unwrap();
        assert_eq!(repository.list(&suite).unwrap_err().code(), "corrupt-store");
        assert_eq!(
            repository
                .checkpoint_stores(&suite, &|| false)
                .unwrap_err()
                .code(),
            "corrupt-store"
        );
        assert_eq!(
            repository.latest_modified(&suite).unwrap_err().code(),
            "corrupt-store"
        );
        assert_eq!(
            repository
                .open(&suite, "save", false, 99)
                .unwrap_err()
                .code(),
            "corrupt-store"
        );
        assert_eq!(
            repository.delete(&suite, "save").unwrap_err().code(),
            "corrupt-store"
        );
        assert!(path.is_file());
    }
    fs::write(&path, valid).unwrap();
    assert_eq!(repository.list(&suite).unwrap(), ["save"]);
    assert_eq!(repository.latest_modified(&suite).unwrap(), Some(12));
}
