use super::*;

#[test]
fn linkage_error_new_reports_missing_class_with_call_site() {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("Missing".into())),
    ];
    let main = runtime_method(
        "T",
        "main",
        "()I",
        &[0xbb, 0, 1, 0x57, 0x03, 0xac],
        1,
        0,
        constants,
        true,
    );
    let mut program = Program::new();
    program.methods.insert(main.key.clone(), main);

    let error = program
        .execute("T", "main", "()I", Limits::default(), false)
        .unwrap_err();

    assert_eq!(error.code(), "class-not-found");
    let message = error.message();
    assert!(message.contains("unresolved=Missing"));
    assert!(message.contains("call-site=T::main()I pc=0 opcode=new"));
    assert!(!message.contains('/'));
}

#[test]
fn linkage_error_invokestatic_reports_missing_method_with_call_site() {
    let constants = vec![
        None,
        Some(Constant::Methodref {
            class_index: 2,
            name_and_type_index: 3,
        }),
        Some(Constant::Class { name_index: 4 }),
        Some(Constant::NameAndType {
            name_index: 5,
            descriptor_index: 6,
        }),
        Some(Constant::Utf8("Missing".into())),
        Some(Constant::Utf8("missingMethod".into())),
        Some(Constant::Utf8("()I".into())),
    ];
    let main = runtime_method(
        "T",
        "main",
        "()I",
        &[0xb8, 0, 1, 0x03, 0xac],
        1,
        0,
        constants,
        true,
    );
    let mut program = Program::new();
    program.methods.insert(main.key.clone(), main);

    let error = program
        .execute("T", "main", "()I", Limits::default(), false)
        .unwrap_err();

    assert_eq!(error.code(), "method-not-found");
    let message = error.message();
    assert!(message.contains("unresolved=Missing::missingMethod()I"));
    assert!(message.contains("call-site=T::main()I pc=0 opcode=invokestatic"));
    assert!(!message.contains('/'));
}

#[test]
fn failed_class_initialization_preserves_the_original_cause() {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("MissingDependency".into())),
    ];
    let initializer = runtime_method(
        "Broken",
        "<clinit>",
        "()V",
        &[0xbb, 0, 1, 0x57, 0xb1],
        1,
        0,
        constants,
        true,
    );
    let mut program = Program::new();
    program.classes.insert(
        "Broken".into(),
        Class {
            super_name: Some("java/lang/Object".into()),
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    program.methods.insert(initializer.key.clone(), initializer);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);

    let first = machine.initialize_class("Broken", 1).unwrap_err();
    assert_eq!(first.code(), "class-not-found");
    assert!(first.message().contains("MissingDependency"));

    let second = machine.initialize_class("Broken", 1).unwrap_err();
    assert_eq!(second.code(), "class-not-found");
    assert!(second.message().contains("MissingDependency"));
    assert!(second.message().contains("call-site=Broken::<clinit>()V"));
}
