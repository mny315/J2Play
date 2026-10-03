use super::*;

fn field_class(names: &[(&str, &str)], is_static: bool) -> ClassFile {
    let mut pool = Vec::new();
    let utf8 = |pool: &mut Vec<u8>, text: &str| {
        pool.push(1);
        pool.extend_from_slice(&u16::try_from(text.len()).unwrap().to_be_bytes());
        pool.extend_from_slice(text.as_bytes());
    };
    utf8(&mut pool, "Fields");
    pool.extend_from_slice(&[7, 0, 1]);
    utf8(&mut pool, "java/lang/Object");
    pool.extend_from_slice(&[7, 0, 3]);
    for (index, &(name, descriptor)) in names.iter().enumerate() {
        let name_index = u16::try_from(5 + index * 4).unwrap();
        utf8(&mut pool, name);
        utf8(&mut pool, descriptor);
        pool.push(12);
        pool.extend_from_slice(&name_index.to_be_bytes());
        pool.extend_from_slice(&(name_index + 1).to_be_bytes());
        pool.extend_from_slice(&[9, 0, 2]);
        pool.extend_from_slice(&(name_index + 2).to_be_bytes());
    }
    let mut bytes = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 50];
    bytes.extend_from_slice(&u16::try_from(5 + names.len() * 4).unwrap().to_be_bytes());
    bytes.extend(pool);
    for value in [0x21, 2, 4, 0, u16::try_from(names.len()).unwrap()] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    for index in 0..names.len() {
        let name_index = u16::try_from(5 + index * 4).unwrap();
        for value in [if is_static { 9 } else { 1 }, name_index, name_index + 1, 0] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    classfile::parse(&bytes).unwrap()
}

fn field_program(names: &[(&str, &str)], is_static: bool) -> Program {
    let class = field_class(names, is_static);
    let mut program = Program::new();
    for name in ["java/lang/Object", "Bar", "Foo:LBar"] {
        program
            .classes
            .insert(name.into(), test_class_definition(None));
    }
    program.add_class(&class, &Limits::default()).unwrap();
    let (write, read) = if is_static {
        (
            vec![0x2b, 0xb3, 0, 8, 0x01, 0xb3, 0, 12, 0xb1],
            vec![0xb2, 0, 8, 0xb0],
        )
    } else {
        (
            vec![0x2a, 0x2b, 0xb5, 0, 8, 0x2a, 0x01, 0xb5, 0, 12, 0xb1],
            vec![0x2a, 0xb4, 0, 8, 0xb0],
        )
    };
    for (name, descriptor, code) in [
        ("write", "(LFields;LBar;)V", write),
        ("read", "(LFields;)LBar;", read),
    ] {
        let method = runtime_method(
            "Fields",
            name,
            descriptor,
            &code,
            2,
            2,
            class.constant_pool.clone(),
            true,
        );
        program.methods.insert(method.key.clone(), method);
    }
    program
}

#[test]
fn field_names_and_descriptors_cannot_alias_storage_or_resolution() {
    for names in [
        [("value:LFoo", "LBar;"), ("value", "LFoo:LBar;")],
        [("value:LFoo", "LBar;"), ("value\\", "LFoo:LBar;")],
        [("value\\:LFoo", "LBar;"), ("value\\", "LFoo:LBar;")],
        [("é:LFoo", "LBar;"), ("é", "LFoo:LBar;")],
    ] {
        for is_static in [false, true] {
            let program = field_program(&names, is_static);
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let fields = machine.initial_instance_fields("Fields").unwrap();
            let instance = machine
                .allocate_linked_object("Fields", fields, &[], &[])
                .unwrap();
            let value = machine
                .heap
                .managed
                .allocate_object("Bar", HashMap::new())
                .unwrap();
            machine
                .call(
                    &program.methods[&MethodKey::new("Fields", "write", "(LFields;LBar;)V")],
                    vec![
                        Value::Reference(Some(instance)),
                        Value::Reference(Some(value)),
                    ],
                    1,
                )
                .unwrap();
            let read = MethodKey::new("Fields", "read", "(LFields;)LBar;");
            assert!(
                matches!(machine.call(&program.methods[&read], vec![Value::Reference(Some(instance))], 1).unwrap(),
                CallOutcome::Return(Some(Value::Reference(Some(actual)))) if actual == value),
                "{names:?}, static={is_static}"
            );

            let bytes = machine.encode_checkpoint(instance).unwrap();
            let relinked = field_program(&names, is_static);
            let mut context = DefaultNativeContext;
            let mut restored = relinked.machine(Limits::default(), false, &mut context);
            restored.restore_checkpoint("Fields", &bytes).unwrap();
            assert!(
                matches!(restored.call(&relinked.methods[&read], vec![Value::Reference(Some(instance))], 1).unwrap(),
                CallOutcome::Return(Some(Value::Reference(Some(actual)))) if actual == value)
            );
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn field_resolution_throughput() {
    use std::{hint::black_box, time::Instant};
    for name in ["value", "value:LFoo", "value\\:LFoo"] {
        let program = field_program(&[(name, "LBar;"), ("other", "LFoo:LBar;")], false);
        let mut context = DefaultNativeContext;
        let machine = program.machine(Limits::default(), false, &mut context);
        let reference = FieldRef {
            class: "Fields".into(),
            name: name.into(),
            descriptor: "LBar;".into(),
        };
        let started = Instant::now();
        let mut checksum = 0;
        for _ in 0..100_000 {
            let field = black_box(machine.find_field(black_box(&reference)).unwrap());
            checksum += usize::from(field.kind == ValueKind::Reference);
        }
        eprintln!(
            "name={name:?}: {:?}; checksum={checksum}",
            started.elapsed()
        );
    }
}
