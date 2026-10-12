use crate::support::guest::{utf8, write_fixture};

use std::path::PathBuf;

#[test]
fn java_rms_surface_enumeration_listeners_and_exceptions() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "java_rms_surface_enumeration_listeners_and_exceptions"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = crate::support::Scratch::new();
    for (method, expected) in [
        ("core", 1_200),
        ("enumeration", 1_210),
        ("listAndExceptions", 1_220),
        ("contracts", 1_230),
    ] {
        let output =
            crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
                .rms_root(&root.0)
                .run_static("fixtures/Stage12Fixtures", method, "()I");
        assert!(output.success(), "{method}: {}", output.diagnostics);
        let diagnostics = &output.diagnostics;
        assert_eq!(
            output.int_value(),
            Some(expected),
            "{method}: {diagnostics}"
        );
    }
}

#[test]
fn rms_persists_across_independent_vm_processes() {
    let Some((phase, root)) = crate::support::process_phase(
        concat!(
            module_path!(),
            "::rms_persists_across_independent_vm_processes"
        ),
        2,
    ) else {
        return;
    };
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (method, expected) = [("persistenceWrite", 1_221), ("persistenceRead", 1_222)][phase];
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .rms_root(&root)
            .run_static("fixtures/Stage12Fixtures", method, "()I");
    assert!(output.success(), "{method}: {}", output.diagnostics);
    assert_eq!(output.int_value(), Some(expected));
}

#[test]
fn scripted_midlet_resumes_identical_saved_state_after_restart() {
    let Some((phase, root)) = crate::support::process_phase(
        concat!(
            module_path!(),
            "::scripted_midlet_resumes_identical_saved_state_after_restart"
        ),
        2,
    ) else {
        return;
    };
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let expected = ["stage12-save:written", "stage12-save:resumed"][phase];
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .rms_root(&root)
            .midlet(5)
            .run_midlet();
    assert!(output.success(), "{}", output.diagnostics);
    assert!(
        output.diagnostics.contains(expected),
        "{}",
        output.diagnostics
    );
    assert_eq!(output.ams_state, Some(midp::LifecycleState::Destroyed));
}

#[test]
fn owned_rms_open_accepts_authmode_any_and_writable_flag() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "owned_rms_open_accepts_authmode_any_and_writable_flag"
    )) {
        return;
    }
    let scratch = crate::support::Scratch::new();
    let directory = &scratch.0;
    let jar_path = directory.join("fixture.jar");
    write_fixture(&jar_path, b"Manifest-Version: 1.0\r\nMIDlet-Vendor: J2Play\r\nMIDlet-Name: RMS auth mode fixture\r\n\r\n", "AuthModeFixture.class", &rms_authmode_fixture_class());

    let output = crate::support::Fixture::new(&jar_path)
        .rms_root(directory.join("rms"))
        .run_static("AuthModeFixture", "main", "()I");
    assert!(output.success(), "{}", output.diagnostics);
    let diagnostics = &output.diagnostics;
    assert_eq!(output.int_value(), Some(1234), "{diagnostics}");
}

fn rms_authmode_fixture_class() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xcafe_babe_u32.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&50_u16.to_be_bytes());
    bytes.extend_from_slice(&20_u16.to_be_bytes());
    utf8(&mut bytes, "AuthModeFixture");
    bytes.extend_from_slice(&[7, 0, 1]);
    utf8(&mut bytes, "java/lang/Object");
    bytes.extend_from_slice(&[7, 0, 3]);
    utf8(&mut bytes, "main");
    utf8(&mut bytes, "()I");
    utf8(&mut bytes, "Code");
    utf8(&mut bytes, "AuthAny");
    bytes.extend_from_slice(&[8, 0, 8]);
    utf8(&mut bytes, "javax/microedition/rms/RecordStore");
    bytes.extend_from_slice(&[7, 0, 10]);
    utf8(&mut bytes, "openRecordStore");
    utf8(
        &mut bytes,
        "(Ljava/lang/String;ZIZ)Ljavax/microedition/rms/RecordStore;",
    );
    bytes.extend_from_slice(&[12, 0, 12, 0, 13]);
    bytes.extend_from_slice(&[10, 0, 11, 0, 14]);
    utf8(&mut bytes, "closeRecordStore");
    utf8(&mut bytes, "()V");
    bytes.extend_from_slice(&[12, 0, 16, 0, 17]);
    bytes.extend_from_slice(&[10, 0, 11, 0, 18]);
    bytes.extend_from_slice(&0x0021_u16.to_be_bytes());
    bytes.extend_from_slice(&2_u16.to_be_bytes());
    bytes.extend_from_slice(&4_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&0x0009_u16.to_be_bytes());
    bytes.extend_from_slice(&5_u16.to_be_bytes());
    bytes.extend_from_slice(&6_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&7_u16.to_be_bytes());
    bytes.extend_from_slice(&29_u32.to_be_bytes());
    bytes.extend_from_slice(&4_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&17_u32.to_be_bytes());
    bytes.extend_from_slice(&[
        0x12, 0x09, // ldc "AuthAny"
        0x04, // createIfNecessary = true
        0x04, // AUTHMODE_ANY
        0x04, // writable = true
        0xb8, 0x00, 0x0f, // RecordStore.openRecordStore
        0x4b, 0x2a, 0xb6, 0x00, 0x13, // store.closeRecordStore()
        0x11, 0x04, 0xd2, 0xac, // return 1234
    ]);
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes
}
