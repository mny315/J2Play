use super::*;
use std::io::Cursor;

struct InterruptedReader<'a> {
    input: Cursor<&'a [u8]>,
    interrupt_next: bool,
}

impl Read for InterruptedReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        self.interrupt_next = !self.interrupt_next;
        if self.interrupt_next {
            Err(std::io::ErrorKind::Interrupted.into())
        } else {
            self.input.read(output)
        }
    }
}

#[test]
fn draft_header_failures_do_not_read_the_archive_payload() {
    let mut oversized = Cursor::new(u32::MAX.to_le_bytes());
    assert!(read_draft(&mut oversized, MAX_PRIVATE_JAR_BYTES).is_err());
    assert_eq!(oversized.position(), 4);

    for (schema_version, jar_length, payload_length) in [
        (2, 3, 3),
        (1, usize::try_from(MAX_PRIVATE_JAR_BYTES).unwrap() + 1, 3),
        (1, 3, 4),
    ] {
        let header = serde_json::to_vec(&DraftHeader {
            schema_version,
            archive_leaf_name: None,
            jar_length,
            jad_length: None,
        })
        .unwrap();
        let mut bytes = u32::try_from(header.len()).unwrap().to_le_bytes().to_vec();
        bytes.extend_from_slice(&header);
        bytes.extend(vec![7; payload_length]);
        let length = bytes.len() as u64;
        let mut reader = Cursor::new(bytes);
        assert!(read_draft(&mut reader, length).is_err());
        assert_eq!(reader.position(), 4 + header.len() as u64);
    }
}

#[test]
fn draft_streaming_retries_interruptions_and_rejects_changed_lengths() {
    for jad_length in [None, Some(0), Some(2)] {
        let header = serde_json::to_vec(&DraftHeader {
            schema_version: 1,
            archive_leaf_name: None,
            jar_length: 3,
            jad_length,
        })
        .unwrap();
        let mut bytes = u32::try_from(header.len()).unwrap().to_le_bytes().to_vec();
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&[1, 2, 3]);
        bytes.extend(vec![4; jad_length.unwrap_or_default()]);
        let length = bytes.len() as u64;
        let (_, jar, jad) = read_draft(&mut Cursor::new(&bytes), length)
            .unwrap()
            .into_parts();
        assert_eq!(jar, [1, 2, 3]);
        assert_eq!(jad, jad_length.map(|length| vec![4; length]));
        let mut interrupted = InterruptedReader {
            input: Cursor::new(bytes.as_slice()),
            interrupt_next: false,
        };
        assert_eq!(
            read_draft(&mut interrupted, length).unwrap().into_parts(),
            (None, jar, jad)
        );
        assert!(read_draft(&mut Cursor::new(&bytes[..bytes.len() - 1]), length).is_err());
        bytes.push(5);
        assert!(read_draft(&mut Cursor::new(&bytes), length).is_err());
    }
}

#[test]
fn unfinished_import_preserves_opened_bytes_across_recreation() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let jar = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/java-me/conformance.jar"
    ));
    repository
        .stage_import(&ImportSource::new(
            Some("/provider/тест.jar".into()),
            jar.to_vec(),
            None,
        ))
        .unwrap();
    drop(repository);
    let repository = LibraryRepository::open(root).unwrap();
    let (name, restored, jad) = repository.restore_import().unwrap().unwrap().into_parts();
    assert_eq!(name.as_deref(), Some("тест.jar"));
    assert_eq!(restored, jar);
    assert!(jad.is_none());
    repository
        .stage_import(&ImportSource::new(
            name,
            restored,
            Some(b"descriptor bytes".to_vec()),
        ))
        .unwrap();
    fs::write(
        root.join(".pending-import.bin.interrupted.tmp"),
        b"partial write",
    )
    .unwrap();
    let (_, restored, jad) = repository.restore_import().unwrap().unwrap().into_parts();
    assert_eq!(restored, jar);
    assert_eq!(jad.as_deref(), Some(b"descriptor bytes".as_slice()));
    repository.discard_import_draft().unwrap();
    assert!(repository.restore_import().unwrap().is_none());
}

#[test]
fn draft_rejects_truncation_and_oversized_headers_without_hiding_the_library() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    repository
        .stage_import(&ImportSource::new(None, vec![1, 2, 3], Some(Vec::new())))
        .unwrap();
    let path = root.join(DRAFT_NAME);
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(repository.restore_import().is_err());
    fs::write(&path, u32::MAX.to_le_bytes()).unwrap();
    assert!(repository.restore_import().is_err());
    assert!(repository.load().unwrap().entries.is_empty());
    repository.discard_import_draft().unwrap();
}
