use crate::support::{Fixture, Probe, Scratch};
use midp::LifecycleAction::{Destroy, Pause, Resume, Start};
use natives::MidletNotification::{Destroyed, Paused, ResumeRequested};
use std::{fs, io::Write};
use zip::write::SimpleFileOptions;

const DESTROY: midp::LifecycleAction = Destroy {
    unconditional: true,
};

const JAR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/java-me/conformance.jar"
);
const JAD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/java-me/lifecycle.jad"
);
const DESTROYED_JAD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/java-me/destroyed.jad"
);

fn fixture() -> Fixture {
    Fixture::new(JAR).jad(JAD)
}

fn allow_fixture_policy(fixture: Fixture) -> Fixture {
    fixture
        .allow_platform(true)
        .permissions(&["javax.microedition.io.Connector.http"])
        .unknown_permissions(&["javax.microedition.io.Connector.socket"])
}

fn run_mode(mode: &str) -> Probe {
    let scratch = Scratch::new();
    let path = scratch.0.join("lifecycle.jad");
    fs::write(
        &path,
        format!(
            "MIDlet-Name: Lifecycle {mode}\nMIDlet-Version: 1.0.0\nMIDlet-Vendor: J2Play\nMIDlet-1: Lifecycle Fixture, , fixtures.Stage7LifecycleMidlet\nLifecycle-Mode: {mode}\n"
        ),
    )
    .unwrap();
    Fixture::new(JAR).jad(&path).run_midlet()
}

#[test]
fn lifecycle_fixture_runs_on_rust_ams() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "lifecycle_fixture_runs_on_rust_ams"
    )) {
        return;
    }
    let output = allow_fixture_policy(fixture()).run_midlet();
    assert!(output.success(), "{}", output.diagnostics);
    assert_eq!(output.midlet.as_ref().unwrap().name, "Lifecycle Fixture");
    assert_eq!(output.callbacks, [Start, Pause, Resume, DESTROY]);
    assert_eq!(output.lifecycle, [Paused, ResumeRequested, Destroyed]);
    assert_eq!(output.platform_requests, 4);
}

#[test]
fn denied_platform_request_is_catchable_and_stops_the_midlet() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "denied_platform_request_is_catchable_and_stops_the_midlet"
    )) {
        return;
    }
    let output = fixture()
        .permissions(&["javax.microedition.io.Connector.http"])
        .unknown_permissions(&["javax.microedition.io.Connector.socket"])
        .run_midlet();
    assert!(output.success(), "{}", output.diagnostics);
    assert_eq!(output.callbacks, [Start, DESTROY]);
    assert_eq!(output.platform_requests, 0);
    assert_eq!(output.ams_state, Some(midp::LifecycleState::Destroyed));
}

#[test]
fn permission_denial_does_not_grant_the_protected_operation() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "permission_denial_does_not_grant_the_protected_operation"
    )) {
        return;
    }
    let output = fixture()
        .allow_platform(true)
        .unknown_permissions(&["javax.microedition.io.Connector.socket"])
        .run_midlet();
    assert!(output.success(), "{}", output.diagnostics);
    assert_eq!(output.callbacks, [Start, Resume, DESTROY]);
    assert_eq!(output.platform_requests, 1);
    assert_eq!(output.ams_state, Some(midp::LifecycleState::Destroyed));
}

#[test]
fn notify_destroyed_stops_later_callbacks() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "notify_destroyed_stops_later_callbacks"
    )) {
        return;
    }
    for launch_only in [false, true] {
        let fixture = Fixture::new(JAR).jad(DESTROYED_JAD);
        let output = if launch_only {
            fixture.host_events(&[midp::HostEvent::Launch])
        } else {
            fixture
        }
        .run_midlet();
        assert!(output.success(), "{}", output.diagnostics);
        assert_eq!(output.lifecycle, [Destroyed]);
        assert_eq!(output.callbacks, [Start]);
        assert_eq!(output.ams_state, Some(midp::LifecycleState::Destroyed));
    }
}

#[test]
fn invalid_midlet_entry_has_a_controlled_diagnostic() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "invalid_midlet_entry_has_a_controlled_diagnostic"
    )) {
        return;
    }
    let output = fixture().midlet(99).run_midlet();
    assert!(!output.success());
    let stderr = &output.diagnostics;
    assert!(stderr.contains("midlet-selection"), "{stderr}");
    assert!(
        stderr.contains("requested MIDlet entry does not exist"),
        "{stderr}"
    );
}

#[test]
fn suite_descriptor_selects_the_first_entry_when_unspecified() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "suite_descriptor_selects_the_first_entry_when_unspecified"
    )) {
        return;
    }
    let output = Fixture::new(JAR).run_midlet();
    assert!(output.success(), "{}", output.diagnostics);
    let midlet = output.midlet.unwrap();
    assert_eq!(midlet.name, "Lifecycle Fixture");
    assert_eq!(midlet.class_name, "fixtures.Stage7LifecycleMidlet");
}

#[test]
fn jad_without_midlet_entries_launches_the_manifest_entry() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "jad_without_midlet_entries_launches_the_manifest_entry"
    )) {
        return;
    }
    let scratch = Scratch::new();
    let path = scratch.0.join("properties.jad");
    fs::write(&path, format!(
        "MIDlet-Name: J2Play Stage 7 Fixture\nMIDlet-Version: 1.0.0\nMIDlet-Vendor: J2Play\nMIDlet-Jar-URL: conformance.jar\nMIDlet-Jar-Size: {}\nLifecycle-Mode: start-rejected\n",
        fs::metadata(JAR).unwrap().len(),
    )).unwrap();
    let output = Fixture::new(JAR).jad(&path).run_midlet();
    assert!(output.success(), "{}", output.diagnostics);
    assert_eq!(
        output.midlet.unwrap().class_name,
        "fixtures.Stage7LifecycleMidlet"
    );
    assert_eq!(output.callbacks, [Start, Resume]);
    assert_eq!(output.ams_state, Some(midp::LifecycleState::Destroyed));
}

#[test]
fn distinct_suites_do_not_share_properties_or_runtime_state() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "distinct_suites_do_not_share_properties_or_runtime_state"
    )) {
        return;
    }
    for value in ["first-suite", "second-suite"] {
        let scratch = Scratch::new();
        let path = scratch.0.join("properties.jad");
        fs::write(
            &path,
            format!(
                "MIDlet-Name: {value}\nMIDlet-Version: 1.0.0\nMIDlet-Vendor: J2Play\nMIDlet-1: Lifecycle Fixture, , fixtures.Stage7LifecycleMidlet\nCustom-Property: {value}\nExpected-Property: {value}\n"
            ),
        )
        .unwrap();
        let output = allow_fixture_policy(Fixture::new(JAR).jad(&path)).run_midlet();
        assert!(output.success(), "{}", output.diagnostics);
        assert_eq!(output.callbacks, [Start, Pause, Resume, DESTROY]);
        assert_eq!(output.platform_requests, 4);
        assert_eq!(output.ams_state, Some(midp::LifecycleState::Destroyed));
    }
}

#[test]
fn checked_start_refusal_is_retried_without_vm_failure() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "checked_start_refusal_is_retried_without_vm_failure"
    )) {
        return;
    }
    let output = run_mode("start-rejected");
    assert!(output.success(), "{}", output.diagnostics);
    assert_eq!(output.callbacks, [Start, Resume]);
    assert_eq!(output.ams_state, Some(midp::LifecycleState::Destroyed));
}

#[test]
fn guest_app_properties_strip_padding_from_manifest_defaults_and_jad_overrides() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "guest_app_properties_strip_padding_from_manifest_defaults_and_jad_overrides"
    )) {
        return;
    }
    let scratch = Scratch::new();
    let path = scratch.0.join("owned.jar");
    let jad_path = scratch.0.join("owned.jad");
    let mut source = zip::ZipArchive::new(fs::File::open(JAR).unwrap()).unwrap();
    let class_name = "fixtures/Stage7LifecycleMidlet.class";
    for use_jad in [false, true] {
        let mut archive = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        archive
            .start_file("META-INF/MANIFEST.MF", SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(b"Manifest-Version: 1.0\r\nMIDlet-Name: Property Fixture\r\nMIDlet-1: Property Fixture, , fixtures.Stage7LifecycleMidlet\r\n")
            .unwrap();
        archive
            .write_all(if use_jad {
                b"Lifecycle-Mode: start-runtime\r\n"
            } else {
                b"Lifecycle-Mode: \t start-\r\n rejected \t\r\n"
            })
            .unwrap();
        archive
            .start_file(class_name, SimpleFileOptions::default())
            .unwrap();
        std::io::copy(&mut source.by_name(class_name).unwrap(), &mut archive).unwrap();
        archive.finish().unwrap();

        let mut fixture = Fixture::new(&path);
        if use_jad {
            fs::write(
                &jad_path,
                b"MIDlet-1: Property Fixture, , fixtures.Stage7LifecycleMidlet\r\nLifecycle-Mode:\t start-rejected \t\r\n",
            )
            .unwrap();
            fixture = fixture.jad(&jad_path);
        }
        let output = fixture.run_midlet();
        assert!(output.success(), "{}", output.diagnostics);
        assert_eq!(output.callbacks, [Start, Resume], "jad={use_jad}");
        assert_eq!(output.ams_state, Some(midp::LifecycleState::Destroyed));
    }
}

#[test]
fn runtime_failures_trigger_or_complete_unconditional_destroy() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "runtime_failures_trigger_or_complete_unconditional_destroy"
    )) {
        return;
    }
    for (mode, callbacks) in [
        ("start-runtime", vec![Start, DESTROY]),
        ("pause-runtime", vec![Start, Pause, DESTROY]),
        ("destroy-runtime", vec![Start, Pause, Resume, DESTROY]),
    ] {
        let output = run_mode(mode);
        assert!(output.success(), "{mode}: {}", output.diagnostics);
        assert_eq!(output.callbacks, callbacks, "{mode}");
        assert_eq!(
            output.ams_state,
            Some(midp::LifecycleState::Destroyed),
            "{mode}"
        );
    }
}

fn run_with_entry(class_name: &str, title: &str) -> Probe {
    let scratch = Scratch::new();
    let path = scratch.0.join("suite.jar");
    let mut source = zip::ZipArchive::new(fs::File::open(JAR).unwrap()).unwrap();
    let mut archive = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    archive
        .start_file("META-INF/MANIFEST.MF", SimpleFileOptions::default())
        .unwrap();
    write!(
        archive,
        "Manifest-Version: 1.0\r\nMIDlet-Name: {title}\r\nMIDlet-1: {title}, , {class_name}\r\n\r\n"
    )
    .unwrap();
    let resource = format!("{}.class", class_name.replace('.', "/"));
    archive
        .start_file(&resource, SimpleFileOptions::default())
        .unwrap();
    std::io::copy(&mut source.by_name(&resource).unwrap(), &mut archive).unwrap();
    archive.finish().unwrap();
    Fixture::new(&path).run_midlet()
}

#[test]
fn external_midlet_uses_rust_owned_bootstrap_classes() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "external_midlet_uses_rust_owned_bootstrap_classes"
    )) {
        return;
    }
    let result = run_with_entry("fixtures.Stage7DestroyedMidlet", "External Fixture");
    assert!(result.success(), "{}", result.diagnostics);
    assert_eq!(result.midlet.as_ref().unwrap().name, "External Fixture");
}

#[test]
fn manifest_entry_must_name_a_real_midlet_subclass() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "manifest_entry_must_name_a_real_midlet_subclass"
    )) {
        return;
    }
    let result = run_with_entry("fixtures.MethodFixtures", "Invalid Fixture");
    assert!(!result.success());
    assert!(result.diagnostics.contains("midlet-class"));
}

#[test]
fn midlet_must_expose_a_public_no_argument_constructor() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "midlet_must_expose_a_public_no_argument_constructor"
    )) {
        return;
    }
    let result = run_with_entry("fixtures.Stage7PrivateConstructorMidlet", "Invalid Fixture");
    assert!(!result.success());
    assert!(result.diagnostics.contains("midlet-constructor"));
}
