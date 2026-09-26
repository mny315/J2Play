use std::path::PathBuf;

#[test]
fn java_system_out_crosses_the_host_diagnostic_bridge() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "java_system_out_crosses_the_host_diagnostic_bridge"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
            .run_static("fixtures/CldcConsole", "consoleProbe", "()I");
    assert!(output.success(), "{}", output.diagnostics);
    assert_eq!(output.diagnostics, "console:12trueZ\n");
    assert_eq!(output.int_value(), Some(43));
}

#[test]
fn configured_wall_clock_reaches_guest_system_current_time_millis() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "configured_wall_clock_reaches_guest_system_current_time_millis"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .wall_clock(-1_000_000_000_000)
            .run_static("fixtures/MethodFixtures", "method0419", "()I");
    assert!(output.success(), "{}", output.diagnostics);
    let diagnostics = &output.diagnostics;
    assert_eq!(output.int_value(), Some(0), "{diagnostics}");
}

#[test]
fn rust_owned_system_properties_distinguish_null_and_empty_values() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "rust_owned_system_properties_distinguish_null_and_empty_values"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (method, expected) in [("propertyNullProbe", 20), ("propertyEmptyProbe", 23)] {
        let output =
            crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
                .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
                .run_static("fixtures/CldcConsole", method, "()I");
        assert!(output.success(), "{method}: {}", output.diagnostics);
        let diagnostics = &output.diagnostics;
        assert_eq!(
            output.int_value(),
            Some(expected),
            "{method}: {diagnostics}"
        );
    }
}
