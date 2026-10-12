use super::*;
use std::error::Error as _;

fn catching_method(owner: &str) -> Method {
    let mut method = runtime_method(
        owner,
        "catch",
        "()V",
        &[0x01, 0xbf, 0x57, 0xb1],
        1,
        0,
        vec![],
        true,
    );
    Arc::make_mut(&mut method.exception_table).push(ExceptionHandler {
        start_pc: 0,
        end_pc: 2,
        handler_pc: 2,
        catch_type: 0,
    });
    method
}

#[test]
fn caught_exception_sites_do_not_merge_ambiguous_class_names() {
    let mut program = program_with_exception("fixture/A");
    program.classes.insert(
        "fixture/A at fixture/B".into(),
        test_class_definition(Some("java/lang/Throwable")),
    );
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    for (exception, owner, count) in [
        ("fixture/A at fixture/B", "fixture/C", 2),
        ("fixture/A", "fixture/B at fixture/C", 3),
    ] {
        let method = catching_method(owner);
        let throwable = machine
            .allocate_exception(exception, None, &[], &[])
            .unwrap();
        for _ in 0..count {
            assert_eq!(
                machine.find_handler(&method, 1, throwable).unwrap(),
                Some(2)
            );
        }
    }
    let mut counts: Vec<_> = machine
        .execution
        .caught_exceptions
        .values()
        .map(|record| record.count)
        .collect();
    counts.sort_unstable();
    assert_eq!(counts, [2, 3]);
}

#[test]
#[ignore = "release caught exception site throughput measurement"]
fn caught_exception_site_throughput() {
    for name_bytes in [16, 4096] {
        let exception = format!("Error{}", "X".repeat(name_bytes));
        let method = catching_method(&format!("Owner{}", "X".repeat(name_bytes)));
        let program = program_with_exception(&exception);
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let throwable = machine
            .allocate_exception(&exception, None, &[], &[])
            .unwrap();
        let started = std::time::Instant::now();
        for _ in 0..16_384 {
            assert_eq!(
                machine.find_handler(&method, 1, throwable).unwrap(),
                Some(2)
            );
        }
        let record = machine.execution.caught_exceptions.values().next().unwrap();
        assert_eq!(record.count, 16_384);
        eprintln!(
            "caught-site bytes={name_bytes} ns={} checksum={}:{}",
            started.elapsed().as_nanos(),
            record.count,
            record.last_description.len()
        );
    }
}

#[test]
fn instruction_trace_and_method_profile_bound_long_unicode_names() {
    use crate::machine::diagnostic_helpers::MAX_DIAGNOSTIC_BYTES;
    let class = format!("test/{}", "界".repeat(MAX_DIAGNOSTIC_BYTES));
    let method = runtime_method(&class, "run", "()I", &[0x05, 0xac], 1, 0, vec![], true);
    let mut program = Program::new();
    program.methods.insert(method.key.clone(), method);
    let result = program
        .execute(&class, "run", "()I", Limits::default(), true)
        .unwrap();
    assert_eq!(result.value, Some(Value::Int(2)));
    assert_eq!(result.instructions, 2);
    assert_eq!(result.trace.len(), 4);
    for line in &result.trace {
        assert!(
            line.len() <= MAX_DIAGNOSTIC_BYTES,
            "trace retained {} bytes in one line",
            line.len()
        );
    }
    assert!(result.trace[0].ends_with("..."));
    assert_eq!(result.trace[2], "method-profile:");
    assert!(result.trace[3].ends_with("..."));
}

#[test]
fn instruction_trace_has_a_total_budget_and_still_reports_method_profile() {
    use crate::machine::diagnostic_helpers::MAX_DIAGNOSTIC_BYTES;
    let class = format!("test/{}", "T".repeat(1_024));
    let mut code = vec![0x00; 5_000];
    code.extend_from_slice(&[0x05, 0xac]);
    let method = runtime_method(&class, "run", "()I", &code, 1, 0, vec![], true);
    let mut program = Program::new();
    program.methods.insert(method.key.clone(), method);
    let result = program
        .execute(&class, "run", "()I", Limits::default(), true)
        .unwrap();
    assert_eq!(result.value, Some(Value::Int(2)));
    assert_eq!(result.instructions, 5_002);
    let bytes = result.trace.iter().map(String::len).sum::<usize>();
    assert!(
        bytes <= 4 * 1_024 * 1_024 + 42 * MAX_DIAGNOSTIC_BYTES,
        "trace retained {bytes} bytes"
    );
    assert_eq!(
        result
            .trace
            .iter()
            .filter(|line| line.starts_with("trace truncated"))
            .count(),
        1
    );
    assert_eq!(result.trace[result.trace.len() - 2], "method-profile:");
    assert!(
        result
            .trace
            .last()
            .unwrap()
            .contains("method-profile       5002 ")
    );
}

#[test]
fn diagnostic_messages_bound_utf8_without_changing_short_messages() {
    use crate::machine::diagnostic_helpers::{MAX_DIAGNOSTIC_BYTES, diagnostic_message};
    for unit in ["x", "é", "界", "🙂"] {
        for extra in [0, 1, 2, 3, 4] {
            let message = format!(
                "{}{}",
                "x".repeat(extra),
                unit.repeat(MAX_DIAGNOSTIC_BYTES / unit.len())
            );
            let actual = diagnostic_message(|output| write!(output, "{message}"));
            if message.len() <= MAX_DIAGNOSTIC_BYTES {
                assert_eq!(actual, message);
            } else {
                assert!(actual.len() <= MAX_DIAGNOSTIC_BYTES);
                assert!(actual.ends_with("..."));
                assert!(message.starts_with(&actual[..actual.len() - 3]));
            }
        }
    }
}

#[test]
fn long_java_stacks_are_bounded_before_caught_diagnostics_are_retained() {
    use crate::machine::diagnostic_helpers::MAX_DIAGNOSTIC_BYTES;
    let program = program_with_exception("java/lang/RuntimeException");
    let class = format!("test/{}", "界".repeat(2000));
    let mut method = runtime_method(
        &class,
        "catch",
        "()V",
        &[0x01, 0xbf, 0x57, 0xb1],
        1,
        0,
        vec![],
        true,
    );
    Arc::make_mut(&mut method.exception_table).push(ExceptionHandler {
        start_pc: 0,
        end_pc: 2,
        handler_pc: 2,
        catch_type: 0,
    });
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    for pc in 0..32 {
        machine
            .execution
            .call_stack
            .push(method.active_stack_frame(pc));
    }
    let error = machine.contextual_error(vm_error("fixture", "failed"));
    assert!(error.message().len() <= MAX_DIAGNOSTIC_BYTES);
    assert!(error.message().ends_with("..."));
    let throwable = machine
        .allocate_exception("java/lang/RuntimeException", None, &[], &[])
        .unwrap();
    machine
        .record_exception_frame(throwable, &method, 1, &[], &[])
        .unwrap();
    for _ in 0..2 {
        assert_eq!(
            machine.find_handler(&method, 1, throwable).unwrap(),
            Some(2)
        );
    }
    let record = machine.execution.caught_exceptions.values().next().unwrap();
    assert_eq!(record.count, 2);
    assert!(record.first_description.len() <= MAX_DIAGNOSTIC_BYTES);
    assert!(record.last_description.len() <= MAX_DIAGNOSTIC_BYTES);
    assert!(record.first_description.ends_with("..."));
    assert_eq!(record.first_description, record.last_description);
}

#[test]
fn diagnostic_annotations_preserve_sources_and_stack_order() {
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let method = runtime_method("test/Call", "run", "()V", &[0xb1], 0, 0, vec![None], true);
    let error = EmuError::with_source(
        Category::Vm,
        "field-not-found",
        "missing field",
        std::io::Error::other("fixture source"),
    );
    let error = annotate_resolution("test/Call.value", &method, 3, 0xb4, error);
    assert_eq!(error.source().unwrap().to_string(), "fixture source");
    let message = error.message().to_owned();
    let error = machine.contextual_error(error);
    assert_eq!(error.message(), message);
    assert_eq!(error.source().unwrap().to_string(), "fixture source");
    machine
        .execution
        .call_stack
        .push(method.active_stack_frame(3));
    machine
        .execution
        .call_stack
        .push(method.active_stack_frame(7));
    let error = machine.contextual_error(error);
    assert_eq!(
        error.message(),
        format!("{message}; java-stack: test/Call::run()V pc=7 <- test/Call::run()V pc=3")
    );
    assert_eq!(error.source().unwrap().to_string(), "fixture source");
    let error = annotate_resolution("test/Call.other", &method, 1, 0xb4, error);
    let message = error.message().to_owned();
    assert_eq!(machine.contextual_error(error).message(), message);
}

#[test]
fn native_error_text_cannot_hide_the_call_stack() {
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
        Some(Constant::Utf8("Fixture".into())),
        Some(Constant::Utf8("fail".into())),
        Some(Constant::Utf8("()V".into())),
    ];
    let caller = runtime_method(
        "Fixture",
        "run",
        "()V",
        &[0xb8, 0, 1, 0xb1],
        0,
        0,
        constants,
        true,
    );
    let mut callee = runtime_method("Fixture", "fail", "()V", &[], 0, 0, vec![], true);
    callee.is_native = true;
    let mut program = Program::new();
    program.methods.insert(caller.key.clone(), caller);
    program.methods.insert(callee.key.clone(), callee);
    program
        .native_registry_mut()
        .register(
            natives::NativeSignature::new("Fixture", "fail", "()V"),
            |_, _| {
                Err(EmuError::with_source(
                    Category::Platform,
                    "fixture",
                    "text containing java-stack: is not a stack",
                    std::io::Error::other("fixture source"),
                ))
            },
        )
        .unwrap();
    for tracing in [false, true] {
        let error = program
            .execute("Fixture", "run", "()V", Limits::default(), tracing)
            .unwrap_err();
        assert_eq!(error.category(), Category::Platform);
        assert_eq!(error.code(), "fixture");
        assert_eq!(error.source().unwrap().to_string(), "fixture source");
        assert_eq!(
            error.message(),
            "text containing java-stack: is not a stack; java-stack: Fixture::fail()V pc=0 <- Fixture::run()V pc=0"
        );
    }
}

#[test]
fn static_entry_exceptions_preserve_messages_stacks_and_managed_heap_identity() {
    use crate::machine::diagnostic_helpers::MAX_DIAGNOSTIC_BYTES;
    for (code, message, expected_code) in [
        (
            "illegal-argument-exception",
            "invalid fixture parameter",
            "uncaught-exception",
        ),
        (
            "out-of-memory-error",
            "fixture resource limit",
            "uncaught-exception",
        ),
        (
            MANAGED_HEAP_LIMIT_CODE,
            MANAGED_HEAP_LIMIT_MESSAGE,
            MANAGED_HEAP_LIMIT_CODE,
        ),
    ] {
        for long_message in [false, true] {
            let mut program = program_with_bootstrap();
            let mut entry = runtime_method("Fixture", "entry", "()V", &[], 0, 0, vec![], true);
            entry.is_native = true;
            program.methods.insert(entry.key.clone(), entry);
            let text = if long_message {
                format!("{message}: {}", "🙂".repeat(MAX_DIAGNOSTIC_BYTES))
            } else {
                message.to_owned()
            };
            program
                .native_registry_mut()
                .register(
                    natives::NativeSignature::new("Fixture", "entry", "()V"),
                    move |_, _| Err(vm_error(code, &text)),
                )
                .unwrap();
            let error = program
                .execute("Fixture", "entry", "()V", Limits::default(), false)
                .unwrap_err();
            assert_eq!(error.code(), expected_code);
            assert!(error.message().contains(message), "{}", error.message());
            assert!(error.has_java_stack());
            assert!(error.message().len() <= MAX_DIAGNOSTIC_BYTES);
            if long_message {
                assert!(error.message().ends_with("..."));
            } else {
                assert!(error.message().contains("Fixture::entry()V pc=0"));
            }
        }
    }
}
