#[test]
fn game_api_scripted_playthrough_and_conformance_cases() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "game_api_scripted_playthrough_and_conformance_cases"
    )) {
        return;
    }
    let jar = format!(
        "{}/../../tests/fixtures/java-me/conformance.jar",
        env!("CARGO_MANIFEST_DIR")
    );
    for (method, expected) in [
        ("transforms", 1008),
        ("collisions", 1010),
        ("animatedTiles", 1020),
        ("layerScene", 1030),
        ("playthrough", 1047),
        ("hostileCoordinates", 1050),
        ("publicSurface", 1060),
        ("collisionTransformTable", 1070),
        ("managerRestoresGraphics", 1080),
        ("exceptionContracts", 1090),
    ] {
        let output = crate::support::Fixture::new(&jar).run_static(
            "fixtures/Stage10Fixtures",
            method,
            "()I",
        );
        assert!(output.success(), "{method}: {}", output.diagnostics);
        assert_eq!(
            output.int_value(),
            Some(expected),
            "{method}: {}",
            output.diagnostics
        );
    }
}
