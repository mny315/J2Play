use super::*;

fn literal_method(
    class: &str,
    index: u16,
    constants: Vec<Option<Constant>>,
    pool: Option<usize>,
) -> Method {
    let mut method = runtime_method(
        class,
        "literal",
        "()Ljava/lang/String;",
        &[0x13, (index >> 8) as u8, index as u8, 0xb0],
        1,
        0,
        constants,
        true,
    );
    method.constant_pool_id = pool;
    method
}

fn load_literal(machine: &mut Machine<'_, '_>, method: &Method) -> Handle {
    let CallOutcome::Return(Some(Value::Reference(Some(handle)))) =
        machine.call(method, [], 0).unwrap()
    else {
        panic!("literal must return a String")
    };
    handle
}

#[test]
fn cached_literals_are_scoped_to_pool_and_machine_and_survive_gc() {
    let mut program = Program::new();
    program.constant_pool_count = 2;
    let first = literal_method(
        "First",
        1,
        vec![
            None,
            Some(Constant::String { string_index: 2 }),
            Some(Constant::Utf8("first".into())),
        ],
        Some(0),
    );
    let second = literal_method(
        "Second",
        1,
        vec![
            None,
            Some(Constant::String { string_index: 2 }),
            Some(Constant::Utf8("second".into())),
        ],
        Some(1),
    );
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    // Different handle numbering makes accidental reuse across machines visible.
    machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 1)
        .unwrap();
    let first_handle = load_literal(&mut machine, &first);
    let second_handle = load_literal(&mut machine, &second);
    assert_ne!(first_handle, second_handle);
    for _ in 0..2 {
        machine.collect_heap(machine.roots(&[], &[]));
        assert_eq!(load_literal(&mut machine, &first), first_handle);
        assert_eq!(load_literal(&mut machine, &second), second_handle);
        assert_eq!(
            machine.heap.string_values[&first_handle],
            "first".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            machine.heap.string_values[&second_handle],
            "second".encode_utf16().collect::<Vec<_>>()
        );
    }
    let mut other_context = DefaultNativeContext;
    let mut other = program.machine(Limits::default(), false, &mut other_context);
    let other_handle = load_literal(&mut other, &first);
    assert_ne!(other_handle, first_handle);
    assert_eq!(
        other.heap.string_values[&other_handle],
        machine.heap.string_values[&first_handle]
    );
}

#[test]
fn cached_literals_keep_exact_surrogates_sparse_indices_and_checkpoint_identity() {
    let mut program = Program::new();
    program.constant_pool_count = 1;
    let mut constants = parsed_literal(&[0xed, 0xa0, 0x80, 0xc0, 0x80, 0xed, 0xb0, 0x80]);
    constants.resize(65_535, None);
    constants[65_534] = constants[2].clone();
    let method = literal_method("Surrogates", 65_534, constants, Some(0));
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let handle = load_literal(&mut machine, &method);
    assert_eq!(machine.heap.string_values[&handle], [0xd800, 0, 0xdc00]);
    assert_eq!(load_literal(&mut machine, &method), handle);
    let saved = machine.classes.encode_checkpoint(&|| false).unwrap();
    machine.classes = ClassState::restore_checkpoint(&program, &saved).unwrap();
    assert_eq!(load_literal(&mut machine, &method), handle);
    assert_eq!(machine.heap.interned_strings.len(), 1);
}

#[test]
fn cached_literals_preserve_instruction_and_stack_limits_and_invalid_ldc2() {
    for suffix in [&[0x12, 1, 0xb0][..], &[0x14, 0, 1, 0xb0][..]] {
        for max_stack in [0, 1] {
            for padding in [0, 1021, 1022, 1023] {
                let mut program = Program::new();
                program.constant_pool_count = 1;
                let constants = vec![
                    None,
                    Some(Constant::String { string_index: 2 }),
                    Some(Constant::Utf8("cached".into())),
                ];
                let priming = literal_method("Limits", 1, constants.clone(), Some(0));
                let mut code = vec![0x00; padding];
                code.extend(suffix);
                let mut method = runtime_method(
                    "Limits",
                    "main",
                    "()Ljava/lang/String;",
                    &code,
                    max_stack,
                    0,
                    constants,
                    true,
                );
                method.constant_pool_id = Some(0);
                for limit in [3, 1025, 1030] {
                    let mut outcomes = Vec::new();
                    for tracing in [true, false] {
                        let mut context = DefaultNativeContext;
                        let mut machine = program.machine(
                            Limits {
                                max_instructions: limit,
                                ..Limits::default()
                            },
                            tracing,
                            &mut context,
                        );
                        load_literal(&mut machine, &priming);
                        let outcome = machine
                            .call(&method, [], 0)
                            .map(|value| {
                                let CallOutcome::Return(value) = value else {
                                    panic!("unexpected suspension or Java exception")
                                };
                                value
                            })
                            .map_err(|error| error.to_string());
                        outcomes.push((outcome, machine.execution.instructions));
                    }
                    assert_eq!(outcomes[0], outcomes[1]);
                }
            }
        }
    }
}

#[test]
fn failed_literal_allocation_does_not_publish_a_cached_handle() {
    let mut program = Program::new();
    program.constant_pool_count = 1;
    let method = literal_method(
        "HeapLimit",
        1,
        vec![
            None,
            Some(Constant::String { string_index: 2 }),
            Some(Constant::Utf8("x".repeat(32))),
        ],
        Some(0),
    );
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 16,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    for _ in 0..2 {
        let Err(error) = machine.call(&method, [], 0) else {
            panic!("literal must exceed the heap limit")
        };
        assert_eq!(error.code(), MANAGED_HEAP_LIMIT_CODE);
        assert!(
            machine
                .classes
                .constant_pool_cache(&method)
                .unwrap()
                .string_literals
                .get(2)
                .is_none()
        );
        assert!(machine.heap.interned_strings.is_empty());
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn string_literal_throughput() {
    for (kind, text) in [
        ("empty", String::new()),
        ("ascii", "synthetic/literal".into()),
        ("unicode", "Аあ😀".repeat(256)),
    ] {
        for linked in [false, true] {
            let mut program = Program::new();
            program.constant_pool_count = usize::from(linked);
            let mut method = runtime_method(
                "LiteralLoop",
                "main",
                "()Ljava/lang/String;",
                &[
                    0x03, 0x3b, 0x12, 2, 0x57, 0x84, 0, 1, 0x1a, 0x12, 1, 0xa1, 0xff, 0xf7, 0x12,
                    2, 0xb0,
                ],
                2,
                1,
                vec![
                    None,
                    Some(Constant::Integer(65_536)),
                    Some(Constant::String { string_index: 3 }),
                    Some(Constant::Utf8(text.clone())),
                ],
                true,
            );
            method.constant_pool_id = linked.then_some(0);
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let started = std::time::Instant::now();
            let handle = load_literal(&mut machine, &method);
            let elapsed = started.elapsed();
            assert_eq!(
                machine.heap.string_values[&handle],
                text.encode_utf16().collect::<Vec<_>>()
            );
            assert_eq!(machine.heap.interned_strings.len(), 1);
            eprintln!(
                "string-literal kind={kind} linked={linked} elapsed={elapsed:?} instructions={} handle={:?} bytes={}",
                machine.execution.instructions,
                handle.to_raw(),
                machine.heap.managed.bytes()
            );
        }
    }
}
