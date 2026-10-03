use super::*;
use crate::test_storage::Scratch;
use std::cell::Cell;

#[test]
fn bytes_are_bounded_even_when_a_stream_lies_about_its_length() {
    assert_eq!(read_bounded(&b"1234"[..], 4, || Ok(())).unwrap(), b"1234");
    assert_eq!(
        read_bounded(std::io::repeat(0), 4, || Ok(()))
            .unwrap_err()
            .code(),
        "linux-document-size"
    );
}

#[test]
fn reading_is_cancellable_between_chunks() {
    let polls = Cell::new(0);
    let result = read_bounded(std::io::repeat(0), JAR_LIMIT, || {
        polls.set(polls.get() + 1);
        if polls.get() == 3 {
            Err(error("cancelled", "cancelled"))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.unwrap_err().code(), "cancelled");
    assert_eq!(polls.get(), 3);
}

#[test]
fn portal_uri_reads_only_local_regular_files_and_returns_a_leaf() {
    let scratch = Scratch::new();
    let path = scratch.0.join("Unicode файл.jar");
    std::fs::write(&path, b"opened bytes").unwrap();
    let uri = url::Url::from_file_path(&path).unwrap();
    let selected = read_uri(uri.as_str(), DocumentKind::Jar, || Ok(())).unwrap();
    assert_eq!(selected.display_name.as_deref(), Some("Unicode файл.jar"));
    assert_eq!(selected.bytes, b"opened bytes");
    for uri in [
        "https://example.com/game.jar",
        "file://remote/tmp/game.jar",
        "file:///tmp/a?secret=1",
        "file:///dev/zero",
    ] {
        let diagnostic = read_uri(uri, DocumentKind::Jar, || Ok(())).unwrap_err();
        assert!(!diagnostic.message().contains(uri));
    }
    assert!(
        read_uri(
            url::Url::from_directory_path(&scratch.0).unwrap().as_str(),
            DocumentKind::Jar,
            || Ok(())
        )
        .is_err()
    );
}
