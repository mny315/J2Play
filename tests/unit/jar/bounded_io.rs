use super::*;
use crate::MAX_ENTRY_BYTES;
use crate::tests::Scratch;
use std::io::{Cursor, Seek, Write};

#[test]
fn entry_reads_enforce_actual_size_and_declared_size() {
    assert_eq!(
        read_bounded_entry(&mut Cursor::new(vec![1, 2, 3]), 1, 8, "test")
            .unwrap_err()
            .code(),
        "entry-size-mismatch"
    );
    assert_eq!(
        read_bounded_entry(&mut Cursor::new(vec![1, 2, 3]), 1, 2, "test")
            .unwrap_err()
            .code(),
        "entry-size-mismatch"
    );
    assert_eq!(
        read_bounded_entry(&mut Cursor::new(vec![1, 2, 3]), 2, 2, "test")
            .unwrap_err()
            .code(),
        "entry-too-large"
    );
}

#[test]
fn lying_entry_stops_after_one_byte_past_its_declaration() {
    let mut input = Cursor::new(vec![0_u8; 1024 * 1024]);
    assert_eq!(
        read_bounded_entry(&mut input, 7, MAX_ENTRY_BYTES, "test")
            .unwrap_err()
            .code(),
        "entry-size-mismatch"
    );
    assert_eq!(input.position(), 8);
    let mut empty = Cursor::new([1, 2]);
    assert_eq!(
        read_bounded_entry(&mut empty, 0, MAX_ENTRY_BYTES, "test")
            .unwrap_err()
            .code(),
        "entry-size-mismatch"
    );
    assert_eq!(empty.position(), 1);
}

#[test]
fn entry_probe_retries_interruptions_and_propagates_trailer_errors() {
    struct BrokenTrailer(bool);

    impl Read for BrokenTrailer {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(if std::mem::replace(&mut self.0, false) {
                std::io::ErrorKind::Interrupted
            } else {
                std::io::ErrorKind::UnexpectedEof
            }
            .into())
        }
    }

    let mut entry = Cursor::new([1, 2, 3]).chain(BrokenTrailer(true));
    assert_eq!(
        read_bounded_entry(&mut entry, 3, 8, "test")
            .unwrap_err()
            .code(),
        "io"
    );
}

#[test]
fn bounded_file_read_detects_growth_after_metadata() {
    let scratch = Scratch::new();
    let directory = &scratch.0;
    let path = directory.join("growing.jad");
    std::fs::write(&path, [1]).unwrap();
    let mut file = File::open(&path).unwrap();
    let declared = file.metadata().unwrap().len();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&[2, 3])
        .unwrap();

    let error = read_open_file_bounded(
        &mut file,
        declared,
        declared,
        "test-read",
        "test-too-large",
        "test file",
    )
    .unwrap_err();
    assert_eq!(error.code(), "test-too-large");

    let changed_path = directory.join("changed.jad");
    std::fs::write(&changed_path, [1]).unwrap();
    let mut changed_file = File::open(&changed_path).unwrap();
    let changed_declared = changed_file.metadata().unwrap().len();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&changed_path)
        .unwrap()
        .write_all(&[2])
        .unwrap();
    let error = read_open_file_bounded(
        &mut changed_file,
        changed_declared,
        4,
        "test-read",
        "test-too-large",
        "test file",
    )
    .unwrap_err();
    assert_eq!(error.code(), "file-size-changed");
}

#[test]
fn grown_file_stops_after_one_byte_past_its_original_size() {
    let scratch = Scratch::new();
    let directory = &scratch.0;
    let path = directory.join("growing.jar");
    std::fs::write(&path, vec![0_u8; 65_536]).unwrap();
    let observations: Vec<_> = [0, 7]
        .into_iter()
        .map(|declared| {
            let mut file = File::open(&path).unwrap();
            let error = read_open_file_bounded(
                &mut file,
                declared,
                MAX_ENTRY_BYTES,
                "test-read",
                "test-too-large",
                "test file",
            )
            .unwrap_err();
            (declared, error, file.stream_position().unwrap())
        })
        .collect();

    for (declared, error, consumed) in observations {
        assert_eq!(error.code(), "file-size-changed");
        assert_eq!(consumed, declared + 1);
    }
}
