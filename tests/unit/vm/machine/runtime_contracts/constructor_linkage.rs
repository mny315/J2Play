use super::*;

fn caller(owner: &str, name: &str) -> Method {
    let mut method = runtime_method(
        "Caller",
        "run",
        "(LChild;)V",
        &[0x2a, 0xb7, 0, 1, 0xb1],
        1,
        1,
        vec![
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
            Some(Constant::Utf8(owner.into())),
            Some(Constant::Utf8(name.into())),
            Some(Constant::Utf8("()V".into())),
        ],
        true,
    );
    method.constant_pool_id = Some(0);
    method
}

fn program(child_constructor: bool) -> Program {
    let mut program = Program::new();
    program.constant_pool_count = 1;
    program
        .classes
        .insert("Parent".into(), test_class_definition(None));
    program
        .classes
        .insert("Child".into(), test_class_definition(Some("Parent")));
    for (owner, name) in [
        ("Parent", "<init>"),
        ("Parent", "member"),
        ("Child", "<init>"),
    ] {
        if owner == "Child" && !child_constructor {
            continue;
        }
        let method = runtime_method(owner, name, "()V", &[0xb1], 0, 1, vec![], false);
        program.methods.insert(method.key.clone(), method);
    }
    program
}

#[test]
fn inherited_constructor_is_rejected_before_null_receiver_and_cache_insertion() {
    let program = program(false);
    let caller = caller("Child", "<init>");
    for trace in [false, true] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), trace, &mut context);
        let receiver = machine
            .heap
            .managed
            .allocate_object("Child", HashMap::new())
            .unwrap();
        for receiver in [Some(receiver), None, Some(receiver), None] {
            assert_eq!(
                machine
                    .call(&caller, [Value::Reference(receiver)], 1)
                    .err()
                    .expect("constructor must not be inherited")
                    .code(),
                "method-not-found",
            );
            assert!(
                machine
                    .fixed_method_inline_cached(&caller, 1, 0xb7)
                    .is_none()
            );
        }
    }
}

#[test]
fn declared_constructors_and_inherited_ordinary_methods_keep_working() {
    for (child_constructor, owner, name) in [
        (false, "Parent", "<init>"),
        (true, "Child", "<init>"),
        (false, "Child", "member"),
    ] {
        let program = program(child_constructor);
        let caller = caller(owner, name);
        for trace in [false, true] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), trace, &mut context);
            let receiver = machine
                .heap
                .managed
                .allocate_object("Child", HashMap::new())
                .unwrap();
            for _ in 0..2 {
                assert!(matches!(
                    machine
                        .call(&caller, [Value::Reference(Some(receiver))], 1)
                        .unwrap(),
                    CallOutcome::Return(None)
                ));
                assert_eq!(
                    machine
                        .call(&caller, [Value::Reference(None)], 1)
                        .err()
                        .unwrap()
                        .code(),
                    "null-pointer-exception"
                );
            }
        }
    }
}
