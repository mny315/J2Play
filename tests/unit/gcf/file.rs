use super::*;

#[test]
fn file_sandbox_rejects_traversal_and_round_trips() {
    let calls = Rc::new(RefCell::new(0));
    let (runtime, _directory) = runtime(
        &[FILE_READ_PERMISSION, FILE_WRITE_PERMISSION],
        calls,
        vec![],
        vec![],
        false,
    );
    assert_eq!(
        runtime.file_read("file:///../secret").unwrap_err().code(),
        "file-traversal"
    );
    runtime.file_create("file:///save.dat").unwrap();
    runtime
        .file_write("file:///save.dat", b"abc", false)
        .unwrap();
    let revision = runtime.file_revision();
    assert_eq!(runtime.file_read("file:///save.dat").unwrap(), b"abc");
    assert_eq!(
        runtime.file_revision(),
        revision,
        "reads preserve the cache token"
    );
    runtime.file_write("file:///save.dat", b"d", true).unwrap();
    assert_ne!(
        runtime.file_revision(),
        revision,
        "writes invalidate existing readers"
    );
    assert_eq!(runtime.file_read("file:///save.dat").unwrap(), b"abcd");
    let revision = runtime.file_revision();
    let renamed = runtime
        .file_rename("file:///save.dat", "100% done.dat")
        .unwrap();
    assert_eq!(renamed, "file:///100%25%20done.dat");
    assert_ne!(
        runtime.file_revision(),
        revision,
        "renames invalidate old paths"
    );
    assert_eq!(runtime.file_read(&renamed).unwrap(), b"abcd");
}

#[cfg(unix)]
#[test]
fn file_sandbox_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;
    let directory = Scratch::new();
    let outside = Scratch::new();
    symlink(&outside.0, directory.0.join("escape")).unwrap();
    let sandbox = FileSandbox::new(&directory.0, Limits::default()).unwrap();
    assert_eq!(
        sandbox.read("file:///escape/secret").unwrap_err().code(),
        "file-symlink"
    );
}

#[test]
fn renamed_directory_url_points_to_the_new_sibling() {
    let directory = Scratch::new();
    let sandbox = FileSandbox::new(&directory.0, Limits::default()).unwrap();
    sandbox.mkdir("file:///parent").unwrap();
    sandbox.mkdir("file:///parent/old").unwrap();
    sandbox.create("file:///parent/old/save").unwrap();
    sandbox
        .write("file:///parent/old/save", b"saved", false)
        .unwrap();
    let renamed = sandbox.rename("file:///parent/old/", "new name").unwrap();
    assert_eq!(renamed, "file:///parent/new%20name/");
    assert_eq!(sandbox.read(&format!("{renamed}save")).unwrap(), b"saved");
    assert!(!sandbox.metadata("file:///parent/old/").unwrap().exists);
}

#[test]
fn file_truncate_never_extends_or_charges_quota_for_unchanged_bytes() {
    let directory = Scratch::new();
    let sandbox = FileSandbox::new(
        &directory.0,
        Limits {
            max_file_bytes: 8,
            max_suite_bytes: 8,
            ..Limits::default()
        },
    )
    .unwrap();
    sandbox.create("file:///data").unwrap();
    sandbox.write("file:///data", b"save", false).unwrap();
    for offset in [4, 5, 8, 9, u64::MAX] {
        sandbox.truncate("file:///data", offset).unwrap();
        assert_eq!(sandbox.read("file:///data").unwrap(), b"save");
    }
    sandbox.mkdir("file:///folder").unwrap();
    for offset in [0, u64::MAX] {
        for url in ["file:///folder", "file:///missing"] {
            assert_eq!(sandbox.truncate(url, offset).unwrap_err().code(), "file-io");
        }
    }
    // A smaller current profile quota must still allow releasing old storage.
    let restricted = FileSandbox::new(
        &directory.0,
        Limits {
            max_file_bytes: 2,
            max_suite_bytes: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    restricted.truncate("file:///data", 3).unwrap();
    assert_eq!(sandbox.read("file:///data").unwrap(), b"sav");
    restricted.truncate("file:///data", 0).unwrap();
    assert!(sandbox.read("file:///data").unwrap().is_empty());
}

#[test]
fn output_open_uses_write_permission_and_clamps_without_changing_the_file() {
    let calls = Rc::new(RefCell::new(0));
    let (runtime, directory) = runtime(&[FILE_WRITE_PERMISSION], calls, vec![], vec![], false);
    assert!(runtime.permission_allowed(FILE_WRITE_PERMISSION));
    assert!(!runtime.permission_allowed(FILE_READ_PERMISSION));
    runtime.file_create("file:///data").unwrap();
    runtime.file_write("file:///data", b"owned", false).unwrap();
    for (requested, expected) in [(0, 0), (2, 2), (5, 5), (100, 5), (u64::MAX, 5)] {
        assert_eq!(
            runtime
                .file_output_offset("file:///data", requested)
                .unwrap(),
            expected
        );
        assert_eq!(std::fs::read(directory.0.join("data")).unwrap(), b"owned");
    }
    assert!(runtime.file_metadata("file:///data").is_err());
    runtime.file_mkdir("file:///folder").unwrap();
    for url in ["file:///missing", "file:///folder"] {
        assert_eq!(
            runtime.file_output_offset(url, 0).unwrap_err().code(),
            "file-io"
        );
    }
}

#[test]
fn file_size_and_entry_limits_fail_closed() {
    let directory = Scratch::new();
    let limits = Limits {
        max_file_bytes: 3,
        max_suite_bytes: 4,
        ..Limits::default()
    };
    let sandbox = FileSandbox::new(&directory.0, limits).unwrap();
    sandbox.create("file:///a").unwrap();
    assert_eq!(
        sandbox
            .write("file:///a", b"four", false)
            .unwrap_err()
            .code(),
        "file-limit"
    );
    sandbox.write("file:///a", b"abc", false).unwrap();
    sandbox.create("file:///b").unwrap();
    assert_eq!(
        sandbox.write("file:///b", b"xy", false).unwrap_err().code(),
        "file-limit"
    );

    let directory = Scratch::new();
    let limits = Limits {
        max_directory_entries: 1,
        ..Limits::default()
    };
    let sandbox = FileSandbox::new(&directory.0, limits).unwrap();
    sandbox.create("file:///first").unwrap();
    assert_eq!(
        sandbox.create("file:///second").unwrap_err().code(),
        "file-limit"
    );
}

#[test]
fn malformed_file_urls_are_rejected() {
    let directory = Scratch::new();
    let sandbox = FileSandbox::new(&directory.0, Limits::default()).unwrap();
    for url in [
        "/host/path",
        "file://host/path",
        "file:///../x",
        "file:///%2e%2e/x",
        "file:///a%2fb",
        "file:///a\\b",
        "file:///a%00b",
    ] {
        assert!(sandbox.resolve(url).is_err(), "{url}");
    }
}
