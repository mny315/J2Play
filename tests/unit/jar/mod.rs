use super::*;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;

#[path = "../../support/storage.rs"]
mod storage;
pub(crate) use storage::Scratch;

pub(crate) fn fixture(manifest: &[u8]) -> Cursor<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        writer
            .start_file("META-INF/MANIFEST.MF", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(manifest).unwrap();
        writer
            .start_file("game/data.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&[1, 2, 3]).unwrap();
        writer.finish().unwrap();
    }
    output.set_position(0);
    output
}

#[test]
fn borrowed_archive_rejects_oversized_input_before_zip_inspection() {
    let bytes = vec![0; usize::try_from(MAX_ARCHIVE_BYTES).unwrap() + 1];
    assert_eq!(
        ResourceArchive::from_bytes(&bytes).err().unwrap().code(),
        "archive-too-large"
    );
    assert_eq!(
        inspect_bytes(&bytes).unwrap_err().code(),
        "archive-too-large"
    );
    assert_eq!(
        read_class_entries_bytes(&bytes).unwrap_err().code(),
        "archive-too-large"
    );
    assert_eq!(
        ResourceArchive::from_owned_bytes(bytes)
            .err()
            .unwrap()
            .code(),
        "archive-too-large"
    );
}

#[test]
fn path_readers_reject_non_regular_files() {
    let scratch = Scratch::new();
    let directory = &scratch.0;

    assert_eq!(
        parse_jad_path(directory).unwrap_err().code(),
        "not-regular-file"
    );
    assert_eq!(
        inspect_path(directory).unwrap_err().code(),
        "not-regular-file"
    );
    assert_eq!(
        read_class_entries_path(directory).unwrap_err().code(),
        "not-regular-file"
    );
    assert_eq!(
        ResourceArchive::open(directory).err().unwrap().code(),
        "not-regular-file"
    );
}

#[cfg(unix)]
#[test]
fn jad_path_rejects_character_device_without_reading_it() {
    assert_eq!(
        parse_jad_path("/dev/zero").unwrap_err().code(),
        "not-regular-file"
    );
}

#[test]
fn arbitrary_short_inputs_are_controlled() {
    let mut state = 0x05A1_7240_u64;
    for length in 0..512 {
        let mut bytes = vec![0_u8; length];
        for byte in &mut bytes {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            *byte = state.to_le_bytes()[4];
        }
        let _ = inspect_bytes(&bytes);
        let _ = parse_jad(&bytes);
    }
}
