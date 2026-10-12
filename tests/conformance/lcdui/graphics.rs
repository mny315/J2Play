use std::path::PathBuf;

#[test]
fn hidden_canvas_does_not_paint_before_display() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "hidden_canvas_does_not_paint_before_display"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
            .run_static("fixtures/Stage8Fixtures", "run", "()I");
    assert!(output.success(), "{}", output.diagnostics);
    assert_eq!(
        output.int_value(),
        Some(-1_465_968_514),
        "{}",
        output.diagnostics
    );
    assert_eq!(output.frames, 0);
}

#[test]
fn java_graphics_operations_have_independent_golden_checkpoints() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "java_graphics_operations_have_independent_golden_checkpoints"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let checkpoints = [
        ("rectangles", 841_563_233),
        ("arcs", -473_741_375),
        ("triangles", 1_905_117_056),
        ("regions", 367_749_057),
        ("zeroSizedRegionNoOp", 8_164),
        ("rgbAndText", -806_950_479),
        ("clipTranslate", -1_213_752_511),
        ("png", -65_536),
        ("runtime", 8_080),
        ("compatibilityApis", 8_140),
        ("fontMetricsApis", 8_151),
        ("canvasKeyAndDrawCharsApis", 8_162),
        ("canvasNonGameActions", 8_163),
        // Canvas.serviceRepaints must not synchronously drain callSerially;
        // serial callbacks and the deferred initial show belong to the display
        // event turn (__hostIdle). Before that turn, the explicit repaint is
        // the only request serviced by this synchronous fixture.
        ("events", 1),
        ("serialRescheduleYields", 102),
    ];
    for (method, expected) in checkpoints {
        let output =
            crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
                .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
                .run_static("fixtures/Stage8Fixtures", method, "()I");
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
fn hidden_canvas_defers_service_repaints_until_it_is_current() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "hidden_canvas_defers_service_repaints_until_it_is_current"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
            .run_static("fixtures/HiddenCanvasFixture", "run", "()I");
    assert!(output.success(), "{}", output.diagnostics);
    let diagnostics = &output.diagnostics;
    assert_eq!(output.int_value(), Some(8170), "{diagnostics}");
}
