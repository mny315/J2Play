use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{Attribute, ClassFile, ExceptionHandler, Member};

fn reflection_class(
    name: &str,
    super_name: &str,
    flags: u16,
    constructor: Option<u16>,
    target: Option<&str>,
) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(name);
    let super_class = pool.class(super_name);
    let code_name = pool.utf8("Code");
    let marker = Member {
        access_flags: 0x0009,
        name_index: pool.utf8("marker"),
        descriptor_index: pool.utf8("I"),
        attributes: Vec::new(),
    };
    let mut methods = Vec::new();
    if let Some(flags) = constructor {
        let mut code = vec![0x2a]; // aload_0
        emit_reference(&mut code, 0xb7, pool.method(super_name, "<init>", "()V"));
        code.extend_from_slice(&[0x10, 7]); // bipush 7
        emit_reference(&mut code, 0xb3, pool.field(name, "marker", "I"));
        code.push(0xb1);
        methods.push(code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags,
                name: "<init>",
                descriptor: "()V",
                max_stack: 1,
                max_locals: 1,
                code,
            },
        ));
    }
    if let Some(target) = target {
        let mut code = Vec::new();
        emit_reference(&mut code, 0x13, pool.string(&target.replace('/', ".")));
        emit_reference(
            &mut code,
            0xb8,
            pool.method(
                "java/lang/Class",
                "forName",
                "(Ljava/lang/String;)Ljava/lang/Class;",
            ),
        );
        emit_reference(
            &mut code,
            0xb6,
            pool.method("java/lang/Class", "newInstance", "()Ljava/lang/Object;"),
        );
        code.push(0x57); // pop the constructed instance
        emit_reference(&mut code, 0xb2, pool.field(target, "marker", "I"));
        code.push(0xac);
        let end_pc = u16::try_from(code.len()).unwrap();
        let mut handlers = Vec::new();
        for (exception, result) in [
            ("java/lang/IllegalAccessException", -1_i8),
            ("java/lang/InstantiationException", -2_i8),
        ] {
            handlers.push(ExceptionHandler {
                start_pc: 0,
                end_pc,
                handler_pc: u16::try_from(code.len()).unwrap(),
                catch_type: pool.class(exception),
            });
            code.extend_from_slice(&[0x57, 0x10, result.to_be_bytes()[0], 0xac]);
        }
        let mut probe = code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0009,
                name: "probe",
                descriptor: "()I",
                max_stack: 1,
                max_locals: 0,
                code,
            },
        );
        let Attribute::Code(body) = &mut probe.attributes[0] else {
            unreachable!();
        };
        body.exception_table = handlers;
        methods.push(probe);
    }
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: flags | 0x0020,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: vec![marker],
        methods,
        attributes: Vec::new(),
    }
}

#[test]
fn class_new_instance_checks_access_from_the_java_caller() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::class_new_instance_checks_access_from_the_java_caller"
    )) {
        return;
    }
    let profiles = launch::builtin_device_profiles().unwrap();
    let profile = profiles
        .iter()
        .find(|p| p.profile_id() == "nokia-featurephone")
        .unwrap();
    let resolved = profile
        .resolve(device_profile::ProfileOverrides::default())
        .unwrap();
    let target_class = "fixtures/Target";
    let same_package = "fixtures/Caller";
    let other_package = "other/Caller";
    // target, caller, public class, constructor access, caller subclasses target, result
    for (target, caller, public, constructor, subclass, expected) in [
        (target_class, other_package, true, Some(1), false, 7),
        (target_class, same_package, false, Some(1), false, 7),
        (target_class, same_package, false, Some(0), false, 7),
        (target_class, same_package, true, Some(0), false, 7),
        (target_class, same_package, true, Some(4), false, 7),
        (target_class, target_class, false, Some(2), false, 7),
        ("Target", "Caller", false, Some(0), false, 7),
        (target_class, other_package, false, Some(1), false, -1),
        (target_class, other_package, true, Some(0), false, -1),
        (target_class, other_package, true, Some(4), false, -1),
        (target_class, other_package, true, Some(4), true, -1),
        (target_class, same_package, true, Some(2), false, -1),
        (target_class, target_class, true, None, false, -2),
        (
            "fixtures/Target",
            "fixtures/sub/Caller",
            false,
            Some(1),
            false,
            -1,
        ),
        (
            "fixtures/Target",
            "fixtures2/Caller",
            false,
            Some(1),
            false,
            -1,
        ),
        ("Target", "fixtures/Caller", false, Some(1), false, -1),
    ] {
        let limits = vm::Limits {
            max_instructions: 10_000,
            ..vm::Limits::default()
        };
        let mut program = vm::Program::new();
        runtime::install_rust_bootstrap(
            &mut program,
            &limits,
            &resolved,
            &std::collections::HashSet::<String>::new(),
            false,
        )
        .unwrap();
        runtime::register_profile_natives(&mut program, profile).unwrap();
        program
            .add_class(
                &reflection_class(
                    target,
                    "java/lang/Object",
                    u16::from(public),
                    constructor,
                    (caller == target).then_some(target),
                ),
                &limits,
            )
            .unwrap();
        if caller != target {
            program
                .add_class(
                    &reflection_class(
                        caller,
                        if subclass { target } else { "java/lang/Object" },
                        1,
                        None,
                        Some(target),
                    ),
                    &limits,
                )
                .unwrap();
        }
        let execution = program
            .execute(caller, "probe", "()I", limits, false)
            .unwrap();
        assert_eq!(
            execution.value,
            Some(vm::Value::Int(expected)),
            "target={target} caller={caller} public={public} constructor={constructor:?} subclass={subclass}"
        );
    }
}
