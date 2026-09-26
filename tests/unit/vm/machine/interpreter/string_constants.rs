use super::*;

mod cached;

fn parsed_literal(bytes: &[u8]) -> Vec<Option<Constant>> {
    let mut class = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 48, 0, 3, 1];
    class.extend_from_slice(&u16::try_from(bytes.len()).unwrap().to_be_bytes());
    class.extend_from_slice(bytes);
    class.extend_from_slice(&[8, 0, 1]);
    classfile::parse_prefix(&class).unwrap().constant_pool
}

#[test]
fn loaded_string_constants_keep_exact_utf16_and_interning_identity() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let cases: &[(&[u8], &[u16])] = &[
        (&[0xed, 0xa0, 0x80], &[0xd800]),
        (&[0xed, 0xb0, 0x80], &[0xdc00]),
        (&[0xef, 0xbf, 0xbd], &[0xfffd]),
        (
            &[0xc0, 0x80, 0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80],
            &[0, 0xd83d, 0xde00],
        ),
        (
            &[0xed, 0xb0, 0x80, 65, 0xed, 0xa0, 0x80],
            &[0xdc00, 65, 0xd800],
        ),
    ];
    let mut handles = Vec::new();
    for (encoded, expected) in cases {
        let method = runtime_method(
            "Strings",
            "literal",
            "()Ljava/lang/String;",
            &[0x12, 2, 0xb0],
            1,
            0,
            parsed_literal(encoded),
            true,
        );
        let CallOutcome::Return(Some(Value::Reference(Some(handle)))) =
            machine.call(&method, [], 0).unwrap()
        else {
            panic!("ldc must return a String");
        };
        assert_eq!(machine.heap.string_values[&handle], *expected);
        assert!(
            matches!(machine.call(&method, [], 0).unwrap(), CallOutcome::Return(Some(Value::Reference(Some(repeated)))) if repeated == handle)
        );
        assert!(!handles.contains(&handle));
        handles.push(handle);
        machine.collect_heap(machine.roots(&[], &[]));
        assert!(machine.heap.managed.get(handle).is_ok());
    }
}

fn constant_value_class() -> ClassFile {
    let encoded = [0xed, 0xb0, 0x80, 0xc0, 0x80, 0xed, 0xa0, 0x80];
    let mut bytes = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 48, 0, 13, 1, 0, 8];
    bytes.extend_from_slice(&encoded);
    bytes.extend_from_slice(&[8, 0, 1]);
    for (text, class_name) in [
        ("Strings", Some(3_u16)),
        ("value", None),
        ("Ljava/lang/String;", None),
        ("ConstantValue", None),
        ("java/lang/Object", Some(8)),
        ("literal", None),
        ("()Ljava/lang/String;", None),
        ("Code", None),
    ] {
        bytes.push(1);
        bytes.extend_from_slice(&u16::try_from(text.len()).unwrap().to_be_bytes());
        bytes.extend_from_slice(text.as_bytes());
        if let Some(index) = class_name {
            bytes.push(7);
            bytes.extend_from_slice(&index.to_be_bytes());
        }
    }
    // One static final String with ConstantValue #2, and a method returning ldc #2.
    for word in [0x21_u16, 4, 9, 0, 1, 0x19, 5, 6, 1, 7] {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    bytes.extend_from_slice(&2_u32.to_be_bytes());
    for word in [2_u16, 1, 9, 10, 11, 1, 12] {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    bytes.extend_from_slice(&15_u32.to_be_bytes());
    bytes.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 3, 0x12, 2, 0xb0, 0, 0, 0, 0, 0, 0]);
    classfile::parse(&bytes).unwrap()
}

#[test]
fn parsed_constant_value_and_ldc_share_the_exact_string() {
    let class = constant_value_class();
    let mut program = Program::new();
    program.add_class(&class, &Limits::default()).unwrap();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    machine
        .prepare_class_static_fields(&program.classes["Strings"])
        .unwrap();
    let value = *machine
        .classes
        .static_fields
        .get("Strings.value:Ljava/lang/String;")
        .unwrap();
    let Value::Reference(Some(handle)) = value else {
        panic!("static String must be prepared")
    };
    assert_eq!(machine.heap.string_values[&handle], [0xdc00, 0, 0xd800]);
    assert!(
        matches!(machine.call(program.methods.values().next().unwrap(), [], 0).unwrap(), CallOutcome::Return(Some(loaded)) if loaded == value)
    );
    machine.collect_heap(machine.roots(&[], &[]));
    assert_eq!(machine.heap.string_values[&handle], [0xdc00, 0, 0xd800]);
}

#[test]
fn isolated_surrogate_storage_is_included_in_the_runtime_budget() {
    let exact = constant_value_class();
    let mut replaced = exact.clone();
    replaced.constant_pool[1] = Some(Constant::Utf8("�\0�".into()));
    let mut reference = Program::new();
    reference.add_class(&replaced, &Limits::default()).unwrap();
    let limits = Limits {
        max_runtime_bytes: reference.runtime_bytes,
        ..Limits::default()
    };
    let mut program = Program::new();
    assert_eq!(
        program.add_class(&exact, &limits).unwrap_err().code(),
        "memory-limit"
    );
    assert!(program.classes.is_empty());
    assert!(program.methods.is_empty());
    program.add_class(&exact, &Limits::default()).unwrap();
    assert!(program.runtime_bytes > reference.runtime_bytes);
}

#[test]
fn string_constants_are_interned_by_identity() {
    let constants = vec![
        None,
        Some(Constant::String { string_index: 2 }),
        Some(Constant::Utf8("same".into())),
    ];
    let main = runtime_method(
        "T",
        "main",
        "()I",
        &[0x12, 1, 0x12, 1, 0xa5, 0, 5, 0x03, 0xac, 0x04, 0xac],
        2,
        0,
        constants,
        true,
    );
    let mut program = Program::new();
    program.methods.insert(main.key.clone(), main);
    assert_eq!(
        program
            .execute("T", "main", "()I", Limits::default(), false)
            .unwrap()
            .value,
        Some(Value::Int(1))
    );
}

#[test]
fn first_string_intern_gc_preserves_current_frame_references() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        max_heap_bytes: 40,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    let live = machine
        .heap
        .managed
        .allocate_object("Live", HashMap::new())
        .unwrap();
    let garbage = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 8)
        .unwrap();
    let locals = [Some(Value::Reference(Some(live)))];

    let string = machine.intern_string("x", &locals, &[]).unwrap();

    assert!(machine.heap.managed.get(live).is_ok());
    assert!(machine.heap.managed.get(string).is_ok());
    assert_eq!(
        machine.heap.managed.get(garbage).unwrap_err(),
        HeapError::InvalidHandle
    );
}

#[test]
fn string_intern_returns_one_canonical_utf16_instance() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let literal = machine.intern_string("x", &[], &[]).unwrap();
    let equal = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    machine
        .heap
        .string_values
        .insert(equal, vec![u16::from(b'x')]);
    assert_eq!(machine.intern_existing_string(equal, &[]).unwrap(), literal);

    let first_surrogate = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    let second_surrogate = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    machine
        .heap
        .string_values
        .insert(first_surrogate, vec![0xd800]);
    machine
        .heap
        .string_values
        .insert(second_surrogate, vec![0xd800]);
    assert_eq!(
        machine
            .intern_existing_string(first_surrogate, &[])
            .unwrap(),
        first_surrogate
    );
    assert_eq!(
        machine
            .intern_existing_string(second_surrogate, &[])
            .unwrap(),
        first_surrogate
    );
}

#[test]
fn canonical_strings_reject_payload_mutation_without_leaking_intern_keys() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let canonical = machine.intern_string("a", &[], &[]).unwrap();

    let error = machine
        .store_string_units(canonical, vec![u16::from(b'b')], 1, &[], &[])
        .unwrap_err();

    assert_eq!(error.code(), "illegal-state");
    assert_eq!(machine.heap.string_values[&canonical], [u16::from(b'a')]);
    assert_eq!(machine.heap.interned_strings.len(), 1);
    assert_eq!(
        machine.heap.interned_strings[&vec![u16::from(b'a')]],
        canonical
    );
}
