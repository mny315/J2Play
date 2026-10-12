use super::*;
use crate::tests::{Scratch, fixture};
use std::io::Write;
use zip::write::SimpleFileOptions;

#[test]
fn opened_archive_keeps_classes_and_resources_from_the_same_bytes() {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("First.class", b"\xca\xfe\xba\xbe\x01".as_slice()),
        ("Second.CLASS", b"\xca\xfe\xba\xbe\x02".as_slice()),
        ("notes.class", b"not a class".as_slice()),
        ("data.bin", b"original".as_slice()),
    ] {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    let bytes = writer.finish().unwrap().into_inner();
    let scratch = Scratch::new();
    let path = scratch.0.join("classes.jar");
    std::fs::write(&path, &bytes).unwrap();
    let archive = ResourceArchive::open(&path).unwrap();
    std::fs::write(&path, fixture(b"Manifest-Version: 1.0\n").into_inner()).unwrap();
    let expected = [
        ClassResource {
            name: "First.class".into(),
            bytes: b"\xca\xfe\xba\xbe\x01".to_vec(),
        },
        ClassResource {
            name: "Second.CLASS".into(),
            bytes: b"\xca\xfe\xba\xbe\x02".to_vec(),
        },
    ];
    for _ in 0..2 {
        assert_eq!(archive.class_entries().unwrap(), expected);
        assert_eq!(
            archive.read("DATA.BIN").unwrap().as_deref(),
            Some(b"original".as_slice())
        );
    }
    assert_eq!(
        ResourceArchive::from_bytes(&bytes)
            .unwrap()
            .class_entries()
            .unwrap(),
        expected
    );
    assert!(read_class_entries_path(path).unwrap().is_empty());
}

#[test]
fn reads_one_unambiguous_resource_with_legacy_case_folding() {
    let jar = fixture(b"Manifest-Version: 1.0\r\nMIDlet-1: Demo, , demo.Main\r\n\r\n");
    let scratch = Scratch::new();
    let path = scratch.0.join("resources.jar");
    std::fs::write(&path, jar.into_inner()).unwrap();
    let result = read_resource_path(&path, "/GAME/DATA.BIN").unwrap();
    assert_eq!(result.as_deref(), Some([1, 2, 3].as_slice()));
}

#[test]
fn exact_resource_names_win_and_ambiguous_case_folding_is_rejected() {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        for (name, bytes) in [
            ("Data.bin", b"upper".as_slice()),
            ("data.bin", b"lower".as_slice()),
            ("image.bin", b"only".as_slice()),
        ] {
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
    let archive = ResourceArchive::from_owned_bytes(output.into_inner()).unwrap();
    for (name, expected) in [
        ("Data.bin", Some(b"upper".as_slice())),
        ("/data.bin", Some(b"lower".as_slice())),
        ("IMAGE.BIN", Some(b"only".as_slice())),
        ("DATA.BIN", None),
        ("absent.bin", None),
    ] {
        assert_eq!(archive.read(name).unwrap().as_deref(), expected, "{name}");
    }
    for name in ["image.bin", "IMAGE.BIN"] {
        assert_eq!(
            archive.read_with_limit(name, 3).unwrap_err().code(),
            "entry-too-large"
        );
    }
}

#[test]
fn ignores_non_class_payload_with_class_suffix() {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        writer
            .start_file("README.class", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"installer note").unwrap();
        writer.finish().unwrap();
    }
    assert!(
        read_class_entries_bytes(output.get_ref())
            .unwrap()
            .is_empty()
    );
}
