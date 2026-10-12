#[test]
fn high_level_lcdui_scripted_navigation_and_contracts() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "high_level_lcdui_scripted_navigation_and_contracts"
    )) {
        return;
    }
    let jar = format!(
        "{}/../../tests/fixtures/java-me/conformance.jar",
        env!("CARGO_MANIFEST_DIR")
    );
    for (method, expected) in [
        ("publicSurface", 21_187),
        ("navigation", 1_101),
        ("listAndTextBox", 1_321),
        ("alertReturn", 111),
        // Compact glyphs sit above the baseline reported by Font.
        ("formSnapshot", 1_458_728_723),
        ("listSnapshot", 1_242_393_470),
        // The focused editor paints a caret and clips text to its field.
        ("textBoxSnapshot", 1_612_891_940),
        ("alertSnapshot", 1_818_731_047),
        ("exceptionContracts", 1_183),
    ] {
        let output = crate::support::Fixture::new(&jar)
            .instruction_limit(50_000_000)
            .run_static("fixtures/Stage11Fixtures", method, "()I");
        assert!(output.success(), "{method}: {}", output.diagnostics);
        let diagnostics = &output.diagnostics;
        assert_eq!(
            output.int_value(),
            Some(expected),
            "{method}: {diagnostics}"
        );
        if method != "publicSurface" && method != "exceptionContracts" {
            assert!(
                output
                    .frame
                    .as_ref()
                    .is_some_and(|(width, height, _)| (*width, *height) == (240, 320)),
                "{method}: {diagnostics}"
            );
        }
    }
}

#[test]
fn exception_recovery_restores_command_dispatch_and_repainting() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::exception_recovery_restores_command_dispatch_and_repainting"
    )) {
        return;
    }
    let limits = vm::Limits::default();
    let mut program = vm::Program::new();
    for class in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    let jar = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/java-me/conformance.jar"
    );
    for resource in jar::read_class_entries_path(jar).unwrap() {
        let mut class = classfile::parse(&resource.bytes).unwrap();
        let name = class.class_name(class.this_class).unwrap().to_owned();
        if name == "fixtures/Stage11Fixtures" {
            let index = class
                .methods
                .iter()
                .position(|method| class.utf8(method.name_index) == Some("exceptionRecovery"))
                .unwrap();
            let classfile::Attribute::Code(code) = &mut class.methods[index].attributes[0] else {
                panic!("owned recovery fixture must have bytecode");
            };
            // Keep the owned fixture's throwing listener, subsequent command,
            // throwing paint and successful repaint. Return the listener count
            // directly: its legacy sentinel also required a white first pixel,
            // so expecting zero hid a broken command dispatcher on the dark UI.
            assert_eq!(code.code.len(), 179);
            assert_eq!(&code.code[155..159], &[0x2d, 0xb4, 0x01, 0x87]);
            code.code.truncate(159);
            code.code.push(0xac); // ireturn CountingListener.count
        }
        if !program.contains_class(&name) {
            program.add_class(&class, &limits).unwrap();
        }
    }
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    midp::register_natives(program.native_registry_mut()).unwrap();
    let mut context = super::canvas_modes::FrameCapture::default();
    let execution = program
        .execute_with_context(
            "fixtures/Stage11Fixtures",
            "exceptionRecovery",
            "()I",
            limits,
            false,
            &mut context,
        )
        .unwrap();
    assert_eq!(execution.value, Some(vm::Value::Int(1)));
    let first = context
        .frames
        .first()
        .expect("initial form must be painted");
    let last = context.frames.last().unwrap();
    assert_eq!((last.0, last.1, last.2.len()), (240, 320, 240 * 320));
    assert_ne!(
        first.2, last.2,
        "the recovered form must replace the initial frame"
    );
}
