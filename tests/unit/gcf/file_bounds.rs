use super::*;

#[cfg(unix)]
#[path = "../../support/isolation.rs"]
mod isolation;

#[test]
fn directory_measurement_and_quota_share_bounds_without_following_links() {
    let directory = Scratch::new();
    fs::create_dir_all(directory.0.join("nested/deep")).unwrap();
    fs::write(directory.0.join("top"), b"12").unwrap();
    fs::write(directory.0.join("nested/middle"), b"345").unwrap();
    fs::write(directory.0.join("nested/deep/bottom"), b"67890").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&directory.0, directory.0.join("loop")).unwrap();
    let limits = Limits {
        max_suite_bytes: 100,
        max_directory_entries: 6,
        ..Limits::default()
    };
    let files = FileSandbox::new(&directory.0, limits.clone()).unwrap();
    assert_eq!(files.directory_size("file:///", false).unwrap(), 2);
    assert_eq!(files.directory_size("file:///", true).unwrap(), 10);
    assert_eq!(files.directory_size("file:///nested/", false).unwrap(), 3);
    assert_eq!(files.directory_size("file:///nested/", true).unwrap(), 8);
    assert_eq!(files.space(0).unwrap(), 100);
    assert_eq!(files.space(1).unwrap(), 90);
    assert_eq!(files.space(2).unwrap(), 10);

    let files = FileSandbox::new(
        &directory.0,
        Limits {
            max_directory_entries: 3,
            ..limits
        },
    )
    .unwrap();
    assert_eq!(files.directory_size("file:///", false).unwrap(), 2);
    assert_eq!(
        files.directory_size("file:///", true).unwrap_err().code(),
        "file-limit"
    );
    assert_eq!(files.space(1).unwrap_err().code(), "file-limit");
    assert_eq!(files.space(2).unwrap_err().code(), "file-limit");
    assert_eq!(files.space(0).unwrap(), 100);
    assert_eq!(files.space(3).unwrap_err().code(), "illegal-argument");
}

#[test]
fn file_url_limits_apply_before_resolution_and_rename_is_atomic_on_overflow() {
    let directory = Scratch::new();
    let files = FileSandbox::new(
        &directory.0,
        Limits {
            max_url_bytes: 32,
            ..Limits::default()
        },
    )
    .unwrap();
    let oversized = format!("file:///{}", "a/".repeat(32));
    assert_eq!(files.resolve(&oversized).unwrap_err().code(), "file-url");
    files.create("file:///old").unwrap();
    files.write("file:///old", b"saved bytes", false).unwrap();
    // The encoded name can exceed the URL limit while remaining a valid host
    // filename. Failure must be decided before the old file is renamed.
    assert_eq!(
        files
            .rename("file:///old", &"Я".repeat(5))
            .unwrap_err()
            .code(),
        "file-url"
    );
    assert_eq!(files.read("file:///old").unwrap(), b"saved bytes");
    assert_eq!(files.list("file:///").unwrap(), ["old"]);
    let renamed = files.rename("file:///old", "Я").unwrap();
    assert_eq!(renamed, "file:///%D0%AF");
    assert_eq!(files.read(&renamed).unwrap(), b"saved bytes");
}

#[test]
fn rejected_replacement_and_append_preserve_the_existing_file() {
    let directory = Scratch::new();
    let files = FileSandbox::new(
        &directory.0,
        Limits {
            max_file_bytes: 8,
            max_suite_bytes: 10,
            ..Limits::default()
        },
    )
    .unwrap();
    files.create("file:///first").unwrap();
    files.create("file:///second").unwrap();
    files.write("file:///first", b"12345", false).unwrap();
    files.write("file:///second", b"67890", false).unwrap();
    for (data, append) in [(&b"123456"[..], false), (&b"x"[..], true)] {
        assert_eq!(
            files
                .write("file:///first", data, append)
                .unwrap_err()
                .code(),
            "file-limit"
        );
        assert_eq!(files.read("file:///first").unwrap(), b"12345");
    }
    assert_eq!(
        files.write_at("file:///first", b"x", 5).unwrap_err().code(),
        "file-limit"
    );
    files.write_at("file:///first", b"AB", 1).unwrap();
    assert_eq!(files.read("file:///first").unwrap(), b"1AB45");
    files.write("file:///first", b"short", false).unwrap();
    assert_eq!(files.read("file:///first").unwrap(), b"short");
}

#[cfg(unix)]
#[test]
fn fifo_storage_is_rejected_without_blocking_the_worker() {
    let directory = Scratch::new();
    if isolation::isolate(&directory.0, Duration::from_secs(3)) {
        return;
    }
    let fifo = directory.0.join("pipe");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let files = FileSandbox::new(&directory.0, Limits::default()).unwrap();
    for result in [
        files.read("file:///pipe").map(|_| ()),
        files.write("file:///pipe", b"x", false),
        files.write_at("file:///pipe", b"x", 0),
        files.output_offset("file:///pipe", 0).map(|_| ()),
    ] {
        assert_eq!(result.unwrap_err().code(), "file-io");
    }
}

#[cfg(unix)]
#[test]
fn file_listing_counts_skipped_symlinks_toward_the_work_limit() {
    use std::os::unix::fs::symlink;

    let directory = Scratch::new();
    let outside = Scratch::new();
    symlink(&outside.0, directory.0.join("first")).unwrap();
    symlink(&outside.0, directory.0.join("second")).unwrap();
    let sandbox = FileSandbox::new(
        &directory.0,
        Limits {
            max_directory_entries: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(sandbox.list("file:///").unwrap_err().code(), "file-limit");
}
