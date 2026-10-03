use std::path::PathBuf;

#[test]
fn uncaught_java_exception_reports_cross_frame_trace() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "uncaught_java_exception_reports_cross_frame_trace"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
            .run_static("fixtures/CldcConsole", "uncaughtTrace", "()I");
    assert!(!output.success());
    let stderr = &output.diagnostics;
    assert!(
        stderr.contains("java/lang/IllegalArgumentException"),
        "{stderr}"
    );
    assert!(stderr.contains("traceInner"), "{stderr}");
    assert!(stderr.contains("traceOuter"), "{stderr}");
    assert!(stderr.contains("uncaughtTrace"), "{stderr}");
}

#[test]
fn throwable_print_stack_trace_uses_java_frames() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "throwable_print_stack_trace_uses_java_frames"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
            .run_static("fixtures/CldcConsole", "printTrace", "()I");
    assert!(output.success(), "{}", output.diagnostics);
    assert_eq!(output.int_value(), Some(33));
    let stderr = &output.diagnostics;
    assert!(
        stderr.contains("java.lang.IllegalArgumentException: trace"),
        "{stderr}"
    );
    assert!(stderr.contains("traceInner"), "{stderr}");
    assert!(stderr.contains("traceOuter"), "{stderr}");
    assert!(stderr.contains("printTrace"), "{stderr}");
}

#[test]
fn throwable_trace_is_captured_at_construction() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "throwable_trace_is_captured_at_construction"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
            .run_static("fixtures/CldcConsole", "printStoredTrace", "()I");
    assert!(output.success(), "{}", output.diagnostics);
    assert_eq!(output.int_value(), Some(35));
    let stderr = &output.diagnostics;
    assert!(stderr.contains("makeStoredException"), "{stderr}");
    assert!(stderr.contains("printStoredTrace"), "{stderr}");
}
