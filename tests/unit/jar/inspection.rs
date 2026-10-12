use super::*;
use crate::tests::fixture;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;

#[test]
fn inspects_manifest_and_resources() {
    let jar = fixture(b"Manifest-Version: 1.0\r\nMIDlet-Name: Emu\r\n let\r\nMIDlet-1: Demo, /icon.png, demo.Main\r\n\r\n");
    let info = inspect_bytes(jar.get_ref()).unwrap();
    assert_eq!(info.manifest["MIDlet-Name"], "Emulet");
    assert_eq!(info.midlets[0].class_name, "demo.Main");
    assert_eq!(info.resources.len(), 2);
    assert_eq!(info.archive_bytes, jar.get_ref().len() as u64);
    assert_eq!(info.sha256.len(), 64);
}

#[test]
fn metadata_inspection_keeps_admission_checks_for_unread_resources() {
    let bytes = fixture(b"Manifest-Version: 1.0\nMIDlet-1: Demo, , demo.Main\n").into_inner();
    let (local, central) = {
        let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let resource = archive.by_name("game/data.bin").unwrap();
        (
            usize::try_from(resource.header_start()).unwrap(),
            usize::try_from(resource.central_header_start()).unwrap(),
        )
    };
    // Set encryption flags or an unknown compression method in both headers.
    for (local_field, central_field, value) in [(6, 8, 1_u16), (8, 10, u16::MAX)] {
        let mut rejected = bytes.clone();
        for offset in [local + local_field, central + central_field] {
            rejected[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        assert_eq!(
            inspect_bytes(&rejected).unwrap_err().code(),
            "invalid-entry",
            "local field={local_field}, value={value}",
        );
    }
}

#[test]
fn zip_timestamp_converts_to_a_unix_epoch() {
    let timestamp = zip::DateTime::from_date_and_time(2010, 4, 23, 15, 57, 0).unwrap();
    assert_eq!(zip_date_time_epoch_millis(timestamp), 1_272_038_220_000);
}

#[test]
fn inspection_prefers_the_manifest_archive_timestamp() {
    let manifest_timestamp = zip::DateTime::from_date_and_time(2010, 4, 23, 15, 58, 0).unwrap();
    let resource_timestamp = zip::DateTime::from_date_and_time(2020, 5, 24, 16, 0, 0).unwrap();
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        writer
            .start_file(
                "META-INF/MANIFEST.MF",
                SimpleFileOptions::default().last_modified_time(manifest_timestamp),
            )
            .unwrap();
        writer
            .write_all(b"Manifest-Version: 1.0\r\nMIDlet-1: Demo, , demo.Main\r\n\r\n")
            .unwrap();
        writer
            .start_file(
                "game/data.bin",
                SimpleFileOptions::default().last_modified_time(resource_timestamp),
            )
            .unwrap();
        writer.write_all(&[1, 2, 3]).unwrap();
        writer.finish().unwrap();
    }

    let info = inspect_bytes(output.get_ref()).unwrap();

    assert_eq!(
        info.archive_timestamp_millis,
        Some(zip_date_time_epoch_millis(manifest_timestamp))
    );
}

#[test]
fn accepts_repeated_attribute_names_in_distinct_sections() {
    let jar = fixture(b"Manifest-Version: 1.0\r\n\r\nName: a.class\r\nSHA-256-Digest: aaa\r\n\r\nName: b.class\r\nSHA-256-Digest: bbb\r\n\r\n");
    let info = inspect_bytes(jar.get_ref()).unwrap();
    assert_eq!(info.manifest_sections.len(), 2);
    assert_eq!(info.manifest_sections[1].name, "b.class");
}

#[test]
fn tolerates_deployed_midlet_manifest_variants() {
    let manifest = b"Manifest-Version:1.0\r\r\nMIDlet-Name: Demo\r\r\nMIDlet-1: Demo, , demo.\r\r\nMain\r\r\nMIDlet-Vendor: First\r\r\nmidlet-vendor: Last\r\r\n\r\nAdapted-By: Installer\r\n\r\n";
    let jar = fixture(manifest);

    let info = inspect_bytes(jar.get_ref()).unwrap();

    assert_eq!(info.midlets[0].class_name, "demo.Main");
    assert_eq!(info.manifest["MIDlet-Vendor"], "Last");
    assert!(info.manifest_sections.is_empty());
}

#[test]
fn tolerates_bare_lf_appended_to_crlf_manifest_attributes() {
    let manifest = b"Manifest-Version: 1.0\r\nMIDlet-Name: Demo\r\n\nMIDlet-Touch-Support: true\r\n\nMIDlet-1: Demo, , demo.Main\r\nMIDlet-Vendor: Vendor\r\n\r\n";
    let jar = fixture(manifest);

    let info = inspect_bytes(jar.get_ref()).unwrap();

    assert_eq!(info.midlets[0].class_name, "demo.Main");
    assert_eq!(info.manifest["MIDlet-Touch-Support"], "true");
    assert_eq!(info.manifest["MIDlet-Vendor"], "Vendor");
    assert!(info.manifest_sections.is_empty());
}

#[test]
fn mixed_line_endings_preserve_a_named_manifest_section() {
    let manifest = b"Manifest-Version: 1.0\r\nMIDlet-1: Demo, , demo.Main\r\n\nName: demo/Main.class\r\nSHA-256-Digest: abc\r\n\r\n";
    let jar = fixture(manifest);

    let info = inspect_bytes(jar.get_ref()).unwrap();

    assert_eq!(info.midlets[0].class_name, "demo.Main");
    assert_eq!(info.manifest_sections.len(), 1);
    assert_eq!(info.manifest_sections[0].name, "demo/Main.class");
    assert_eq!(
        info.manifest_sections[0].attributes["SHA-256-Digest"],
        "abc"
    );
    assert!(!info.manifest.contains_key("Name"));
}

#[test]
fn tolerates_non_utf8_manifest_labels() {
    let jar = fixture(b"Manifest-Version: 1.0\nMIDlet-Name: \xff\nMIDlet-1: Demo, , demo.Main\n");

    let info = inspect_bytes(jar.get_ref()).unwrap();

    assert_eq!(info.midlets[0].class_name, "demo.Main");
}

#[test]
fn rejects_missing_manifest() {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        writer
            .start_file("data.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&[1]).unwrap();
        writer.finish().unwrap();
    }
    let error = inspect_bytes(output.get_ref()).unwrap_err();
    assert_eq!(error.code(), "missing-manifest");
}

#[test]
fn rejects_malformed_archive_without_panicking() {
    let input = b"not a zip archive";
    let error = inspect_bytes(input).unwrap_err();
    assert_eq!(error.code(), "invalid-zip");
}

#[test]
fn rejects_traversal_entry() {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        writer
            .start_file("../escape.class", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&[1]).unwrap();
        writer.finish().unwrap();
    }
    assert_eq!(
        inspect_bytes(output.get_ref()).unwrap_err().code(),
        "unsafe-entry-name"
    );
}

#[test]
fn ignores_empty_zip_root_directory_but_not_absolute_files() {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        writer
            .add_directory("/", SimpleFileOptions::default())
            .unwrap();
        writer
            .start_file("META-INF/MANIFEST.MF", SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(b"MIDlet-Name: Demo\nMIDlet-1: Demo, , Demo\n")
            .unwrap();
        writer.finish().unwrap();
    }
    assert_eq!(inspect_bytes(output.get_ref()).unwrap().midlets.len(), 1);

    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        writer
            .start_file("/payload.class", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&[1]).unwrap();
        writer.finish().unwrap();
    }
    assert_eq!(
        inspect_bytes(output.get_ref()).unwrap_err().code(),
        "unsafe-entry-name"
    );
}
