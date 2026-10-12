use std::path::PathBuf;

#[test]
fn java_mmapi_surface_runs_with_the_headless_audio_backend() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "java_mmapi_surface_runs_with_the_headless_audio_backend"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (method, expected) in [
        ("playerLifecycle", 1_300),
        ("toneAndVolume", 1_310),
        ("midiAndQueries", 1_320),
    ] {
        let output =
            crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
                .instruction_limit(50_000_000)
                .run_static("fixtures/Stage13Fixtures", method, "()I");
        assert!(output.success(), "{method}: {}", output.diagnostics);
        let diagnostics = &output.diagnostics;
        assert_eq!(
            output.int_value(),
            Some(expected),
            "{method}: {diagnostics}"
        );
        assert!(output.audio_frames > 0, "{method}: {diagnostics}");
    }
}

#[test]
fn midlet_delivers_pcm_to_a_bounded_audio_sink() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "midlet_delivers_pcm_to_a_bounded_audio_sink"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .profile("se-featurephone")
            .instruction_limit(50_000_000)
            .host_events(&[midp::HostEvent::Launch])
            .midlet(6)
            .run_midlet();
    assert!(output.success(), "{}", output.diagnostics);
    let diagnostics = &output.diagnostics;
    assert_eq!(
        output.ams_state,
        Some(midp::LifecycleState::Destroyed),
        "{diagnostics}"
    );
    assert!(output.audio_frames > 0, "{diagnostics}");
}
