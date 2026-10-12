use crate::support::guest::utf8;
use frontend_core::{ImportSource, inspect_import};
use std::io::{Cursor, Write};

// A project-owned abstract MIDlet with symbolic property-reader references.
// Import inspects this fixture without executing guest code.
fn descriptor_reader_class() -> Vec<u8> {
    let mut bytes = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 48, 0, 19];
    utf8(&mut bytes, "fixtures/DescriptorProbe"); // 1
    bytes.extend_from_slice(&[7, 0, 1]); // 2: this class
    utf8(&mut bytes, "javax/microedition/midlet/MIDlet"); // 3
    bytes.extend_from_slice(&[7, 0, 3]); // 4: superclass
    utf8(&mut bytes, "getAppProperty"); // 5
    utf8(&mut bytes, "(Ljava/lang/String;)Ljava/lang/String;"); // 6
    bytes.extend_from_slice(&[12, 0, 5, 0, 6]); // 7: name and type
    bytes.extend_from_slice(&[10, 0, 2, 0, 7]); // 8: inherited property reader
    utf8(&mut bytes, "java/lang/String"); // 9
    bytes.extend_from_slice(&[7, 0, 9]); // 10
    utf8(&mut bytes, "equals"); // 11
    utf8(&mut bytes, "(Ljava/lang/Object;)Z"); // 12
    bytes.extend_from_slice(&[12, 0, 11, 0, 12]); // 13
    bytes.extend_from_slice(&[10, 0, 10, 0, 13]); // 14
    utf8(&mut bytes, "SNAKE_SPEED"); // 15
    bytes.extend_from_slice(&[8, 0, 15]); // 16
    utf8(&mut bytes, "null"); // 17
    bytes.extend_from_slice(&[8, 0, 17]); // 18
    bytes.extend_from_slice(&[4, 33, 0, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

fn archive(manifest_property: &str) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    archive.start_file("META-INF/MANIFEST.MF", options).unwrap();
    write!(archive, "Manifest-Version: 1.0\nMIDlet-Name: Descriptor probe\nMIDlet-Vendor: Tests\nMIDlet-Version: 1.0\nMIDlet-1: Probe, , fixtures.DescriptorProbe\n{manifest_property}").unwrap();
    archive
        .start_file("fixtures/DescriptorProbe.class", options)
        .unwrap();
    archive.write_all(&descriptor_reader_class()).unwrap();
    archive.finish().unwrap().into_inner()
}

#[test]
fn descriptorless_import_supplies_only_absent_recognized_metadata() {
    for (manifest, jad, inferred) in [
        ("", None, true),
        ("SNAKE_SPEED: 98304\n", None, false),
        ("SNAKE_SPEED: \n", None, false),
        ("", Some("SNAKE_SPEED: 98304\n"), false),
        ("", Some("Unrelated: explicit descriptor\n"), false),
    ] {
        let prepared = inspect_import(ImportSource::new(
            Some("synthetic.jar".to_owned()),
            archive(manifest),
            jad.map(|value| value.as_bytes().to_vec()),
        ))
        .unwrap()
        .select_midlet(1)
        .unwrap();
        let expected = if inferred {
            vec![("SNAKE_SPEED".to_owned(), "null".to_owned())]
        } else {
            Vec::new()
        };
        assert_eq!(
            prepared.descriptor_defaults(),
            expected,
            "manifest={manifest:?}, jad={jad:?}"
        );
    }
}
