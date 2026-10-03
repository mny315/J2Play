use super::*;
fn object_string_program(hash_code: Option<&[u8]>, hash_value: i32) -> Program {
    let mut program = Program::new();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(1, 1) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    program.classes.insert(
        "test/HashObject".into(),
        Class {
            super_name: Some("java/lang/Object".into()),
            ..program.classes["java/lang/Object"].clone()
        },
    );
    if let Some(code) = hash_code {
        let hash = runtime_method(
            "test/HashObject",
            "hashCode",
            "()I",
            code,
            1,
            1,
            vec![None, Some(Constant::Integer(hash_value))],
            false,
        );
        program.methods.insert(hash.key.clone(), hash);
    }
    program
}

fn object_to_string_method(program: &Program) -> &Method {
    &program.methods[&MethodKey {
        class: "java/lang/Object".into(),
        name: "toString".into(),
        descriptor: "()Ljava/lang/String;".into(),
    }]
}

#[test]
fn object_to_string_uses_the_virtual_hash_code_as_unsigned_hexadecimal() {
    for hash in [None, Some(0), Some(42), Some(-1), Some(i32::MIN)] {
        let program =
            object_string_program(hash.map(|_| [0x12, 1, 0xac].as_slice()), hash.unwrap_or(0));
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let object = machine
            .heap
            .managed
            .allocate_object("test/HashObject", HashMap::new())
            .unwrap();
        let CallOutcome::Return(Some(Value::Reference(Some(string)))) = machine
            .call(
                object_to_string_method(&program),
                [Value::Reference(Some(object))],
                1,
            )
            .unwrap()
        else {
            panic!("Object.toString did not return a string");
        };
        let hash = hash.unwrap_or_else(|| (object.to_raw() ^ (object.to_raw() >> 32)) as i32);
        let expected = format!("test.HashObject@{:x}", hash as u32);
        assert_eq!(
            machine.heap.string_values.get(&string),
            Some(&expected.encode_utf16().collect::<Vec<_>>())
        );
    }
}

#[test]
fn object_to_string_preserves_hash_code_exceptions_and_instruction_limits() {
    for (code, expected) in [
        ([0x01, 0xbf].as_slice(), "java/lang/NullPointerException"),
        ([0xa7, 0, 0].as_slice(), "instruction-limit"),
    ] {
        let program = object_string_program(Some(code), 0);
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                max_instructions: 64,
                ..Limits::default()
            },
            false,
            &mut host,
        );
        let object = machine
            .heap
            .managed
            .allocate_object("test/HashObject", HashMap::new())
            .unwrap();
        let actual = match machine.call(
            object_to_string_method(&program),
            [Value::Reference(Some(object))],
            1,
        ) {
            Ok(CallOutcome::Throw(exception)) => {
                machine.object_class(exception).unwrap().into_owned()
            }
            Err(error) => error.code().to_owned(),
            Ok(_) => panic!("Object.toString bypassed hashCode"),
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn object_to_string_resumes_after_the_hash_code_yields() {
    let program = object_string_program(Some(&[0x12, 1, 0xac]), 42);
    for quantum in 1..=4 {
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let object = machine
            .heap
            .managed
            .allocate_object("test/HashObject", HashMap::new())
            .unwrap();
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
        let CallOutcome::Suspend(continuation) = machine
            .call(
                object_to_string_method(&program),
                [Value::Reference(Some(object))],
                1,
            )
            .unwrap()
        else {
            panic!("Object.toString did not preserve the worker quantum");
        };
        machine.scheduler.quantum_remaining = WORKER_QUANTUM;
        let CallOutcome::Return(Some(Value::Reference(Some(string)))) =
            machine.resume_suspended_call(continuation, 1).unwrap()
        else {
            panic!("Object.toString did not finish after resume");
        };
        assert_eq!(
            machine.heap.string_values.get(&string),
            Some(&"test.HashObject@2a".encode_utf16().collect::<Vec<_>>())
        );
    }
}
