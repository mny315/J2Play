use super::*;
use std::hint::black_box;
use std::time::Instant;

fn skip_method(program: &Program) -> &Method {
    &program.methods[&MethodKey {
        class: "java/io/Reader".into(),
        name: "skip".into(),
        descriptor: "(J)J".into(),
    }]
}

fn skip_characters(machine: &mut Machine<'_, '_>, reader: Handle, count: i64) -> i64 {
    let method = skip_method(machine.program).clone();
    let outcome = machine
        .call(
            &method,
            [Value::Reference(Some(reader)), Value::Long(count)],
            1,
        )
        .unwrap();
    let CallOutcome::Return(Some(Value::Long(skipped))) = finish_reader_call(machine, outcome)
    else {
        panic!("Reader.skip must return a character count");
    };
    skipped
}

#[test]
fn reader_skip_preserves_characters_across_buffer_boundaries_and_eof() {
    let program = program(false);
    let text = "Aé🙂中".repeat(80);
    let expected: Vec<_> = text.encode_utf16().collect();
    for count in [
        0,
        1,
        2,
        3,
        127,
        128,
        129,
        255,
        256,
        257,
        399,
        400,
        401,
        i64::MAX,
    ] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let input = byte_stream_bytes(&mut machine, BYTE_STREAM, text.as_bytes());
        let reader = character_reader(&mut machine, input);
        let skipped = count.min(i64::try_from(expected.len()).unwrap());
        assert_eq!(skip_characters(&mut machine, reader, count), skipped);
        let following = read_character(&mut machine, reader).unwrap();
        let next = expected
            .get(usize::try_from(skipped).unwrap())
            .map_or(-1, |unit| i32::from(*unit));
        assert!(matches!(following, CallOutcome::Return(Some(Value::Int(value))) if value == next));
    }
}

fn override_reader(read_code: &[u8]) -> Program {
    let mut program = program(false);
    let class = "test/SkipReader";
    program.classes.insert(
        class.into(),
        Class {
            super_name: Some("java/io/Reader".into()),
            fields: Vec::new(),
            is_abstract: false,
            ..program.classes["java/io/Reader"].clone()
        },
    );
    let read = runtime_method(class, "read", "([CII)I", read_code, 3, 4, vec![None], false);
    program.methods.insert(read.key.clone(), read);
    program
}

#[test]
fn reader_skip_honors_short_reads_zero_progress_and_errors() {
    for (read_code, count, expected) in [
        // Access the last requested array slot to check the buffer bounds,
        // then report at most three (zero-initialized) characters per read.
        (
            &[
                0x2b, 0x1d, 0x04, 0x64, 0x34, 0x57, 0x1d, 0x06, 0xa4, 0, 5, 0x06, 0xac, 0x1d, 0xac,
            ][..],
            517,
            Some(517),
        ),
        (&[0x03, 0xac][..], 517, Some(0)),
        (&[0x02, 0xac][..], 517, Some(0)),
        (&[0x01, 0xbf][..], 517, None),
        (&[0x01, 0xbf][..], 0, Some(0)),
    ] {
        let program = override_reader(read_code);
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let reader = machine
            .heap
            .managed
            .allocate_object("test/SkipReader", HashMap::new())
            .unwrap();
        let outcome = machine
            .call(
                skip_method(&program),
                [Value::Reference(Some(reader)), Value::Long(count)],
                1,
            )
            .unwrap();
        if let Some(expected) = expected {
            assert!(
                matches!(outcome, CallOutcome::Return(Some(Value::Long(actual))) if actual == expected)
            );
        } else {
            let CallOutcome::Throw(exception) = outcome else {
                panic!("read exception must propagate")
            };
            assert_eq!(
                machine.object_class(exception).unwrap(),
                "java/lang/NullPointerException"
            );
        }
    }
}

#[test]
fn reader_skip_rejects_negative_counts_before_reading() {
    let program = override_reader(&[0x01, 0xbf]);
    for count in [-1, i64::MIN] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let reader = machine
            .heap
            .managed
            .allocate_object("test/SkipReader", HashMap::new())
            .unwrap();
        let outcome = machine
            .call(
                skip_method(&program),
                [Value::Reference(Some(reader)), Value::Long(count)],
                1,
            )
            .unwrap();
        let CallOutcome::Throw(exception) = outcome else {
            panic!("negative count must throw")
        };
        assert_eq!(
            machine.object_class(exception).unwrap(),
            "java/lang/IllegalArgumentException"
        );
    }
}

#[test]
fn reader_skip_keeps_instruction_limits_in_guest_overrides() {
    let program = override_reader(&[0xa7, 0, 0]);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_instructions: 128,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let reader = machine
        .heap
        .managed
        .allocate_object("test/SkipReader", HashMap::new())
        .unwrap();
    let error = machine
        .call(
            skip_method(&program),
            [Value::Reference(Some(reader)), Value::Long(1)],
            1,
        )
        .err()
        .unwrap();
    assert_eq!(error.code(), "instruction-limit");
}

#[test]
fn reader_skip_resumes_with_its_buffer_and_reader_rooted() {
    let program = program(false);
    let text = "Aé🙂中".repeat(70);
    let expected: Vec<_> = text.encode_utf16().collect();
    for count in [1, 257] {
        for quantum in [0, 1, 4, 64] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let input = byte_stream_bytes(&mut machine, BYTE_STREAM, text.as_bytes());
            let reader = character_reader(&mut machine, input);
            let worker = machine
                .heap
                .managed
                .allocate_object("java/lang/Thread", HashMap::new())
                .unwrap();
            machine.scheduler.current_thread = worker.to_raw();
            machine
                .scheduler
                .thread_states
                .insert(worker, ThreadState::Running);
            machine.scheduler.quantum_remaining = quantum;
            let mut outcome = machine
                .call(
                    skip_method(&program),
                    [Value::Reference(Some(reader)), Value::Long(count)],
                    1,
                )
                .unwrap();
            let mut yields = 0;
            while let CallOutcome::Suspend(continuation) = outcome {
                assert!(yields < 16_384, "skip failed to make progress");
                let mut roots = machine.roots(&[], &[]);
                continuation.roots(&mut roots);
                machine.collect_heap(roots);
                machine.scheduler.quantum_remaining = quantum.max(1);
                outcome = machine.resume_suspended_call(continuation, 1).unwrap();
                yields += 1;
            }
            if quantum <= 1 {
                assert!(yields > 0);
            }
            assert!(matches!(
                outcome,
                CallOutcome::Return(Some(Value::Long(actual))) if actual == count
            ));
            let next = i32::from(expected[usize::try_from(count).unwrap()]);
            let following = read_character(&mut machine, reader).unwrap();
            assert!(matches!(
                finish_reader_call(&mut machine, following),
                CallOutcome::Return(Some(Value::Int(value))) if value == next
            ));
        }
    }
}

#[test]
#[ignore = "manual release benchmark for Reader character skipping"]
fn reader_skip_throughput() {
    let program = program(false);
    for (count, iterations) in [(1, 2048), (16, 512), (1024, 64), (32_768, 4)] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                max_instructions: 20_000_000,
                ..Limits::default()
            },
            false,
            &mut context,
        );
        let input = byte_stream_bytes(
            &mut machine,
            BYTE_STREAM,
            &vec![b'A'; usize::try_from(count).unwrap() + 1],
        );
        let reader = character_reader(&mut machine, input);
        let method = skip_method(&program);
        let started = Instant::now();
        for _ in 0..iterations {
            machine
                .heap
                .managed
                .set_field(
                    input,
                    "java/io/ByteArrayInputStream.pos:I",
                    HeapValue::Int(0),
                )
                .unwrap();
            let outcome = machine
                .call(
                    method,
                    [
                        Value::Reference(Some(reader)),
                        Value::Long(black_box(count)),
                    ],
                    1,
                )
                .unwrap();
            assert!(
                matches!(outcome, CallOutcome::Return(Some(Value::Long(actual))) if actual == count)
            );
        }
        let elapsed = started.elapsed();
        assert!(matches!(
            read_character(&mut machine, reader).unwrap(),
            CallOutcome::Return(Some(Value::Int(65)))
        ));
        eprintln!("reader-skip-{count} elapsed={elapsed:?} skipped={count}");
    }
}
