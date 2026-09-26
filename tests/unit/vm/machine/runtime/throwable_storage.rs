use super::*;
use crate::machine::tests::interpreter::NativeContextTestState;

#[derive(Default)]
struct ThrowableConsoleContext {
    lines: Vec<String>,
    cancel_after: usize,
}

impl HostServices for ThrowableConsoleContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
    fn write_console_error(&mut self, line: &str) -> Result<(), EmuError> {
        self.lines.push(line.to_owned());
        Ok(())
    }
    fn execution_cancelled(&self) -> bool {
        self.lines.len() >= self.cancel_after
    }
}

#[test]
fn printing_throwable_trace_observes_cancellation_between_frames() {
    let mut registry = NativeRegistry::default();
    cldc::register_core_natives(&mut registry).unwrap();
    let signature = natives::NativeSignature::new("java/lang/Throwable", "printStackTrace", "()V");
    for cancel_after in [0, 2, usize::MAX] {
        let mut state = NativeContextTestState::new(Heap::new(4096));
        let throwable = state
            .heap
            .managed
            .allocate_object(
                "java/lang/Throwable",
                HashMap::from([(
                    "java/lang/Throwable.detailMessage:Ljava/lang/String;".to_owned(),
                    HeapValue::Reference(None),
                )]),
            )
            .unwrap();
        let key = Arc::new(MethodKey {
            class: "fixture/Throwing".to_owned(),
            name: "fail".to_owned(),
            descriptor: "()V".to_owned(),
        });
        state.heap.throwable_traces.insert(
            throwable,
            (0..4)
                .map(|pc| JavaStackFrame {
                    key: Arc::clone(&key),
                    bytecode_pc: pc,
                })
                .collect(),
        );
        let mut host = ThrowableConsoleContext {
            cancel_after,
            ..ThrowableConsoleContext::default()
        };
        let result = registry.invoke(
            &signature,
            &mut state.context(&mut host, vec![throwable]),
            &[NativeValue::Reference(Some(throwable.to_raw()))],
        );
        if cancel_after == usize::MAX {
            assert_eq!(result.unwrap(), None);
            assert_eq!(
                host.lines,
                [
                    "java.lang.Throwable",
                    "at fixture.Throwing.fail(()V:pc=0)",
                    "at fixture.Throwing.fail(()V:pc=1)",
                    "at fixture.Throwing.fail(()V:pc=2)",
                    "at fixture.Throwing.fail(()V:pc=3)"
                ]
            );
        } else {
            assert_eq!(result.unwrap_err().code(), "execution-cancelled");
            assert_eq!(host.lines.len(), cancel_after.max(1));
        }
        assert_eq!(state.heap.throwable_traces[&throwable].len(), 4);
    }
}

#[test]
fn diagnostic_stack_preserves_frame_boundaries_with_one_shared_byte_budget() {
    use crate::machine::diagnostic_helpers::{MAX_DIAGNOSTIC_BYTES, diagnostic_stack};
    for unit in ["x", "é", "界", "🙂"] {
        for length in [1, 16, 4000, 8170, 8174, 8176, 8192] {
            let key = MethodKey {
                class: unit.repeat(length),
                name: "method <- name".to_owned(),
                descriptor: "()V".to_owned(),
            };
            for count in [0, 1, 2, 257] {
                let lines = diagnostic_stack((0..count).map(|pc| (&key, pc)));
                assert!(lines.iter().map(String::len).sum::<usize>() <= MAX_DIAGNOSTIC_BYTES);
                let expected = (0..count)
                    .map(|pc| format!("{}::{}{} pc={pc}", key.class, key.name, key.descriptor));
                if expected.clone().map(|line| line.len()).sum::<usize>()
                    <= MAX_DIAGNOSTIC_BYTES - 3
                {
                    assert_eq!(lines, expected.collect::<Vec<_>>());
                } else {
                    assert!(lines.last().unwrap().ends_with("..."));
                    for (actual, expected) in lines[..lines.len() - 1].iter().zip(expected) {
                        assert_eq!(actual, &expected);
                    }
                }
            }
        }
    }
}

#[test]
fn throwable_diagnostic_messages_preserve_utf16_lossy_decoding_and_short_context() {
    let program = program_with_bootstrap();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    for units in [
        vec![],
        vec![65, 0, 66],
        vec![0xd83d, 0xde42],
        vec![0xd83d, 65, 0xde42],
    ] {
        let text = machine
            .intern_string_units(units.clone(), &[], &[])
            .unwrap();
        let throwable = machine
            .allocate_object(
                "java/lang/RuntimeException",
                HashMap::from([(
                    "java/lang/Throwable.detailMessage:Ljava/lang/String;".to_owned(),
                    HeapValue::Reference(Some(text)),
                )]),
                &[],
                &[],
            )
            .unwrap();
        let decoded = String::from_utf16_lossy(&units);
        assert_eq!(
            machine.throwable_diagnostic_message(throwable),
            (!decoded.is_empty()).then(|| decoded.clone())
        );
        let prefix = if decoded.is_empty() {
            "startApp: java/lang/RuntimeException".to_owned()
        } else {
            format!("startApp: java/lang/RuntimeException: {decoded}")
        };
        assert_eq!(
            machine
                .require_return(&CallOutcome::Throw(throwable), "startApp")
                .unwrap_err()
                .message(),
            prefix
        );
        let method = runtime_method("Fixture", "fail", "()V", &[0xb1], 0, 0, vec![], true);
        machine
            .record_exception_frame(throwable, &method, 7, &[], &[])
            .unwrap();
        let separator = if decoded.is_empty() {
            ": "
        } else {
            "; java-stack: "
        };
        let error = machine
            .require_return(&CallOutcome::Throw(throwable), "startApp")
            .unwrap_err();
        let expected = format!("{prefix}{separator}Fixture::fail()V pc=7");
        assert_eq!(error.message(), expected);
        machine
            .execution
            .call_stack
            .push(method.active_stack_frame(9));
        assert_eq!(machine.contextual_error(error).message(), expected);
        machine.execution.call_stack.clear();
    }
}

#[test]
fn uncaught_throwable_diagnostics_bound_messages_and_stacks() {
    use crate::machine::diagnostic_helpers::MAX_DIAGNOSTIC_BYTES;
    let program = program_with_bootstrap();
    for long_message in [false, true] {
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let message = if long_message {
            "🙂".repeat(8192)
        } else {
            String::new()
        };
        let throwable = machine
            .allocate_exception("java/lang/RuntimeException", Some(&message), &[], &[])
            .unwrap();
        let method = runtime_method(
            &format!("fixture/{}", "界".repeat(2000)),
            "fail",
            "()V",
            &[0xb1],
            0,
            0,
            vec![],
            true,
        );
        machine.execution.call_stack = (0..32).map(|pc| method.active_stack_frame(pc)).collect();
        machine
            .record_exception_frame(throwable, &method, 1, &[], &[])
            .unwrap();
        let error = machine
            .require_return(&CallOutcome::Throw(throwable), "startApp")
            .unwrap_err();
        assert_eq!(error.code(), "uncaught-exception");
        assert!(
            error
                .message()
                .starts_with("startApp: java/lang/RuntimeException:")
        );
        assert!(error.message().len() <= MAX_DIAGNOSTIC_BYTES);
        assert!(error.message().ends_with("..."));
        let initializer = machine
            .exception_in_initializer_error("Fixture", throwable)
            .unwrap();
        assert_eq!(initializer.code(), "exception-in-initializer");
        assert!(initializer.message().len() <= MAX_DIAGNOSTIC_BYTES);
        assert!(initializer.message().ends_with("..."));
        let HeapValue::Reference(Some(text)) = machine
            .heap
            .managed
            .field(
                throwable,
                "java/lang/Throwable.detailMessage:Ljava/lang/String;",
            )
            .unwrap()
        else {
            panic!("expected the original guest message")
        };
        assert_eq!(
            String::from_utf16_lossy(&machine.heap.string_values[&text]),
            message
        );
        assert_eq!(machine.heap.throwable_traces[&throwable].len(), 32);
    }
}

#[test]
fn retained_thread_failures_bound_text_before_publishing() {
    use crate::machine::diagnostic_helpers::MAX_DIAGNOSTIC_BYTES;
    let program = program_with_bootstrap();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let throwable = machine
        .allocate_exception(
            "java/lang/RuntimeException",
            Some(&"🙂".repeat(8192)),
            &[],
            &[],
        )
        .unwrap();
    let method = runtime_method(
        &format!("fixture/{}", "界".repeat(2000)),
        "fail",
        "()V",
        &[0xb1],
        0,
        0,
        vec![],
        true,
    );
    machine.execution.call_stack = (0..32).map(|pc| method.active_stack_frame(pc)).collect();
    machine
        .record_exception_frame(throwable, &method, 1, &[], &[])
        .unwrap();
    machine
        .report_uncaught_thread_exception(throwable, throwable)
        .unwrap();
    let failure = &machine.execution.thread_failures[0];
    assert!(failure.exception_message.as_ref().unwrap().len() <= MAX_DIAGNOSTIC_BYTES);
    assert!(failure.exception_message.as_ref().unwrap().ends_with("..."));
    assert!(failure.stack_trace.iter().map(String::len).sum::<usize>() <= MAX_DIAGNOSTIC_BYTES);
    assert!(failure.stack_trace.last().unwrap().ends_with("..."));
    assert_eq!(machine.execution.thread_failure_count, 1);
    assert!(!failure.is_managed_heap_limit());
}

#[test]
fn heap_recovery_requires_a_vm_allocation_failure() {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(1, 1) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    let mut catcher = runtime_method(
        "test/Recovery",
        "handle",
        "()V",
        &[0x01, 0xbf, 0x57, 0xb1],
        1,
        0,
        vec![],
        true,
    );
    Arc::make_mut(&mut catcher.exception_table).push(ExceptionHandler {
        start_pc: 0,
        end_pc: 2,
        handler_pc: 2,
        catch_type: 0,
    });
    for (class, allocated_by_vm) in [
        ("java/lang/RuntimeException", false),
        ("java/lang/OutOfMemoryError", false),
        ("java/lang/OutOfMemoryError", true),
    ] {
        let mut host = ManagedHeapNoticeContext::default();
        let mut machine = program.machine(
            Limits {
                max_heap_bytes: 4_096,
                ..Limits::default()
            },
            false,
            &mut host,
        );
        let throwable = if allocated_by_vm {
            let error = machine
                .allocate_array(ArrayKind::Byte, 16_384, &[], &[])
                .unwrap_err();
            machine
                .allocate_error_exception(class, &error, &[], &[])
                .unwrap()
        } else {
            machine
                .allocate_exception(class, Some(MANAGED_HEAP_LIMIT_MESSAGE), &[], &[])
                .unwrap()
        };
        if allocated_by_vm {
            machine
                .heap
                .managed
                .set_field(
                    throwable,
                    "java/lang/Throwable.detailMessage:Ljava/lang/String;",
                    Value::Reference(None),
                )
                .unwrap();
        }
        let thread = machine
            .allocate_object(
                "java/lang/Thread",
                HashMap::new(),
                &[],
                &[Value::Reference(Some(throwable))],
            )
            .unwrap();
        machine
            .report_uncaught_thread_exception(thread, throwable)
            .unwrap();
        assert_eq!(
            machine.execution.thread_failures[0].is_managed_heap_limit(),
            allocated_by_vm,
            "{class}"
        );
        for _ in 0..MANAGED_HEAP_NOTICE_CATCHES {
            assert_eq!(
                machine.find_handler(&catcher, 1, throwable).unwrap(),
                Some(2)
            );
        }
        machine
            .report_uncaught_thread_exception(thread, throwable)
            .unwrap();
        assert_eq!(
            machine.execution.thread_failures[1].is_managed_heap_limit(),
            allocated_by_vm,
            "rethrow of {class}"
        );
        let error = machine
            .require_return(&CallOutcome::Throw(throwable), "startApp")
            .unwrap_err();
        assert_eq!(
            error.code(),
            if allocated_by_vm {
                "managed-heap-limit"
            } else {
                "uncaught-exception"
            }
        );
        drop(machine);
        assert_eq!(!host.notices.is_empty(), allocated_by_vm, "{class}");
        assert_eq!(host.uncaught_heap_limits, [allocated_by_vm; 2], "{class}");
    }
}

#[test]
fn collection_releases_dead_throwable_metadata_and_preserves_live_traces() {
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let method = runtime_method(
        "fixture/Throwing",
        "work",
        "()V",
        &[0xb1],
        0,
        0,
        vec![],
        true,
    );
    let live = machine
        .allocate_object("java/lang/Throwable", HashMap::new(), &[], &[])
        .unwrap();
    let dead = machine
        .allocate_object("java/lang/Throwable", HashMap::new(), &[], &[])
        .unwrap();
    for (handle, pc) in [(live, 1), (dead, 2)] {
        machine
            .record_exception_frame(handle, &method, pc, &[], &[])
            .unwrap();
        machine.heap.managed_heap_limit_throwables.insert(handle);
    }
    let trace = machine.heap.throwable_traces[&live].clone();
    machine.collect_heap(vec![live]);
    assert!(machine.heap.managed.get(dead).is_err());
    assert_eq!(machine.heap.throwable_traces.len(), 1);
    assert_eq!(machine.heap.throwable_traces[&live], trace);
    assert_eq!(
        machine.heap.managed_heap_limit_throwables,
        HashSet::from([live])
    );
    machine.collect_heap(vec![]);
    assert!(machine.heap.throwable_traces.is_empty());
    assert!(machine.heap.managed_heap_limit_throwables.is_empty());
}

#[test]
fn native_collection_does_not_accumulate_captured_throwables() {
    let mut state = NativeContextTestState::new(Heap::new(128));
    let mut host = DefaultNativeContext;
    for _ in 0..1024 {
        // Reclaim the previous native array, then model a freshly constructed Throwable.
        state.heap.managed.collect([]);
        let handle = state
            .heap
            .managed
            .allocate_object("java/lang/Throwable", HashMap::new())
            .unwrap();
        state
            .context(&mut host, vec![handle])
            .capture_throwable_trace(handle.to_raw())
            .unwrap();
        assert!(state.heap.throwable_traces.contains_key(&handle));
        // This allocation requires the real native allocation retry / GC path.
        state
            .context(&mut host, vec![])
            .allocate_java_byte_array(&[0; 104])
            .unwrap();
        assert!(state.heap.managed.get(handle).is_err());
        assert!(state.heap.throwable_traces.is_empty());
    }
}

#[test]
fn vm_exception_trace_is_a_complete_snapshot_and_rethrow_cannot_grow_it() {
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let caller = runtime_method("fixture/Outer", "run", "()V", &[0xb1], 0, 0, vec![], true);
    let failing = runtime_method("fixture/Inner", "work", "()V", &[0xb1], 0, 0, vec![], true);
    let handle = machine
        .allocate_object("java/lang/Throwable", HashMap::new(), &[], &[])
        .unwrap();
    machine.execution.call_stack =
        vec![caller.active_stack_frame(12), failing.active_stack_frame(3)];
    machine
        .record_exception_frame(handle, &failing, 3, &[], &[])
        .unwrap();
    let expected = vec![
        JavaStackFrame {
            key: Arc::clone(&failing.stack_key),
            bytecode_pc: 3,
        },
        JavaStackFrame {
            key: Arc::clone(&caller.stack_key),
            bytecode_pc: 12,
        },
    ];
    assert_eq!(machine.heap.throwable_traces[&handle], expected);
    machine.execution.call_stack.pop();
    for pc in 0..4096 {
        machine.execution.call_stack[0].bytecode_pc = pc;
        machine
            .record_exception_frame(handle, &caller, pc, &[], &[])
            .unwrap();
    }
    assert_eq!(machine.heap.throwable_traces[&handle], expected);
}

#[test]
fn caught_arithmetic_exception_keeps_its_original_trace_across_guest_rethrows() {
    let mut program = program_with_exception("java/lang/ArithmeticException");
    let failing = runtime_method(
        "T",
        "divide",
        "()I",
        &[0x04, 0x03, 0x6c, 0xac],
        2,
        0,
        vec![],
        true,
    );
    program.methods.insert(failing.key.clone(), failing);
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("T".into())),
        Some(Constant::Utf8("divide".into())),
        Some(Constant::Utf8("()I".into())),
        Some(Constant::NameAndType {
            name_index: 3,
            descriptor_index: 4,
        }),
        Some(Constant::Methodref {
            class_index: 1,
            name_and_type_index: 5,
        }),
    ];
    let mut caller = runtime_method(
        "T",
        "catch",
        "()Ljava/lang/Throwable;",
        &[0xb8, 0, 6, 0x57, 0x01, 0xb0, 0xb0],
        1,
        0,
        constants,
        true,
    );
    Arc::make_mut(&mut caller.exception_table).push(ExceptionHandler {
        start_pc: 0,
        end_pc: 3,
        handler_pc: 6,
        catch_type: 0,
    });
    let mut rethrow = runtime_method(
        "T",
        "rethrow",
        "(Ljava/lang/Throwable;)V",
        &[0x2a, 0xbf, 0x57, 0xb1],
        1,
        1,
        vec![],
        true,
    );
    Arc::make_mut(&mut rethrow.exception_table).push(ExceptionHandler {
        start_pc: 0,
        end_pc: 2,
        handler_pc: 2,
        catch_type: 0,
    });
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let CallOutcome::Return(Some(Value::Reference(Some(handle)))) =
        machine.call(&caller, [], 1).unwrap()
    else {
        panic!("owned divide-by-zero fixture must return the caught exception");
    };
    let trace = machine.heap.throwable_traces[&handle].clone();
    assert_eq!(
        trace
            .iter()
            .map(|frame| (frame.key.name.as_str(), frame.bytecode_pc))
            .collect::<Vec<_>>(),
        [("divide", 2), ("catch", 0)]
    );
    for _ in 0..64 {
        assert!(matches!(
            machine
                .call(&rethrow, [Value::Reference(Some(handle))], 1)
                .unwrap(),
            CallOutcome::Return(None)
        ));
        assert_eq!(machine.heap.throwable_traces[&handle], trace);
    }
    machine.collect_heap(vec![handle]);
    assert_eq!(machine.heap.throwable_traces[&handle], trace);
}

#[test]
fn managed_heap_recovery_recognizes_only_the_managed_heap_limit_diagnostic() {
    let heap = EmuError::new(
        Category::Vm,
        crate::MANAGED_HEAP_LIMIT_CODE,
        "java/lang/OutOfMemoryError: managed heap limit exceeded",
    );
    let other_oom = EmuError::new(
        Category::Vm,
        "uncaught-exception",
        "java/lang/OutOfMemoryError: M3G render budget exceeded",
    );
    let wrong_category = EmuError::new(
        Category::Api,
        crate::MANAGED_HEAP_LIMIT_CODE,
        "managed heap limit exceeded",
    );
    let wrong_code = EmuError::new(
        Category::Vm,
        "profile-invalid",
        "managed heap limit exceeded",
    );
    let caught_retry = EmuError::new(
        Category::Vm,
        "managed-heap-profile-change-requested",
        "managed heap limit exceeded repeatedly",
    );
    assert!(is_managed_heap_limit_error(&heap));
    assert!(is_managed_heap_limit_error(&caught_retry));
    assert!(!is_managed_heap_limit_error(&other_oom));
    assert!(!is_managed_heap_limit_error(&wrong_category));
    assert!(!is_managed_heap_limit_error(&wrong_code));
    let metadata = heap_error(HeapError::MetadataLimitExceeded);
    assert!(!is_managed_heap_limit_error(&metadata));
    assert_eq!(
        java_error_class(&metadata),
        Some("java/lang/OutOfMemoryError")
    );
    for code in ["uncaught-exception", "uncaught-thread-exception"] {
        for class in ["java/lang/RuntimeException", "java/lang/OutOfMemoryError"] {
            let error = EmuError::new(
                Category::Vm,
                code,
                format!("{class}: {}", crate::MANAGED_HEAP_LIMIT_MESSAGE),
            );
            assert!(!is_managed_heap_limit_error(&error), "{code}: {class}");
        }
    }
}
