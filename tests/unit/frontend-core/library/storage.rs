use super::*;

#[test]
fn bounded_reads_accept_empty_exact_and_chunked_files_and_reject_oversized_data() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let path = root.join("data");
    for size in [0, 1, 65_536, 65_537, 1024 * 1024] {
        let expected = vec![0x97; size];
        fs::write(&path, &expected).unwrap();
        let bytes = read_bounded_file(&path, size as u64, "test-read").unwrap();
        assert_eq!(bytes, expected);
        if size > 0 {
            assert!(read_bounded_file(&path, (size - 1) as u64, "test-read").is_err());
        }
    }
    assert!(read_bounded_file(root, 1024, "test-read").is_err());
}

#[test]
fn a_large_file_can_be_cancelled_during_reading_without_changing_the_source() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let path = root.join("data");
    let expected = vec![0x5a; 1024 * 1024];
    fs::write(&path, &expected).unwrap();
    let mut polls = 0;
    let error = read_bounded_file_cancellable(&path, expected.len() as u64, "test-read", || {
        polls += 1;
        polls == 5
    })
    .unwrap_err();
    assert!(error.message().contains("interrupted"));
    assert_eq!(polls, 5);
    assert_eq!(fs::read(&path).unwrap(), expected);
    let error =
        read_bounded_file_cancellable(&root.join("missing"), 1, "test-read", || true).unwrap_err();
    assert!(error.message().contains("interrupted"));
}

#[test]
fn reads_reject_files_that_grow_or_shrink_after_the_opened_length_was_checked() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let path = root.join("data");
    for (before, after) in [(0, 1), (65_536, 65_537), (65_536, 17)] {
        fs::write(&path, vec![0; before]).unwrap();
        let mut polls = 0;
        let error = read_bounded_file_cancellable(&path, 128 * 1024, "test-read", || {
            polls += 1;
            if polls == 2 {
                fs::write(&path, vec![0; after]).unwrap();
            }
            false
        })
        .unwrap_err();
        assert!(
            error.message().contains("changed size"),
            "{before} -> {after}"
        );
        assert_eq!(fs::metadata(&path).unwrap().len(), after as u64);
    }
}
