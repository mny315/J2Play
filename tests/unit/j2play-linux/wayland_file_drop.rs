//! Executed against the prepared winit patch by test_winit_patches.py.

use crate::reader::read_uris;
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;

#[test]
fn local_uri_list_preserves_escaped_and_non_utf8_names() {
    let bytes = b"# comment\r\nfile:///tmp/game%20%23%25.JAR\r\nfile://localhost/tmp/%FF.jar\n";
    let paths = read_uris(&mut &bytes[..], &mut Vec::new())
        .unwrap()
        .unwrap();
    assert_eq!(paths[0].as_os_str().as_bytes(), b"/tmp/game #%.JAR");
    assert_eq!(paths[1].as_os_str().as_bytes(), b"/tmp/\xff.jar");
}

#[test]
fn remote_malformed_empty_and_excessive_lists_are_rejected_atomically() {
    for uri in [
        "https://host/a.jar",
        "file://host/a.jar",
        "file:///tmp/a%00.jar",
        "file:///tmp/a%.jar",
        "file:///tmp/a%GG.jar",
        "file:///tmp/a.jar?q=1",
        "file:///tmp/a.jar#x",
        "file:relative.jar",
        "# no files\n",
    ] {
        assert!(
            read_uris(&mut uri.as_bytes(), &mut Vec::new()).is_err(),
            "{uri}"
        );
    }
    let many = "file:///tmp/a.jar\n".repeat(65);
    assert!(read_uris(&mut many.as_bytes(), &mut Vec::new()).is_err());
    let large = format!("file:///{}", "a".repeat(16 * 1024));
    assert!(read_uris(&mut large.as_bytes(), &mut Vec::new()).is_err());
}

#[test]
fn continuous_source_yields_and_then_reaches_the_payload_limit() {
    let mut bytes = Vec::new();
    let mut source = io::repeat(b'a');
    assert!(read_uris(&mut source, &mut bytes).unwrap().is_none());
    assert!(bytes.len() <= 128 * 1024);
    assert!(read_uris(&mut source, &mut bytes).unwrap().is_none());
    assert!(read_uris(&mut source, &mut bytes).is_err());
    assert!(bytes.len() <= 256 * 1024);
}

#[test]
fn partial_nonblocking_reads_retain_bytes_until_eof() {
    struct Partial(bool);
    impl Read for Partial {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if std::mem::replace(&mut self.0, false) {
                out[..8].copy_from_slice(b"file:///");
                Ok(8)
            } else {
                Err(io::ErrorKind::WouldBlock.into())
            }
        }
    }
    let mut bytes = Vec::new();
    assert!(read_uris(&mut Partial(true), &mut bytes).unwrap().is_none());
    let paths = read_uris(&mut &b"tmp/a.jar\r\n"[..], &mut bytes)
        .unwrap()
        .unwrap();
    assert_eq!(paths, [std::path::PathBuf::from("/tmp/a.jar")]);
}
