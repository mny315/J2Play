use super::*;
use classfile::{CodeAttribute, Member};

fn accessor_fixture(
    class_name: &str,
    method_name: &str,
    field_name: &str,
    prefix: usize,
    local: u8,
    short: bool,
) -> (Program, MethodKey, String) {
    let mut constants = vec![None];
    for index in 0..prefix {
        push_constant(&mut constants, Constant::Utf8(format!("unused-{index}")));
    }
    let class_index = push_class(&mut constants, class_name);
    let field_name_index = push_constant(&mut constants, Constant::Utf8(field_name.into()));
    let field_descriptor = if short { "[S" } else { "[B" };
    let field_descriptor_index =
        push_constant(&mut constants, Constant::Utf8(field_descriptor.into()));
    let field_type = push_constant(
        &mut constants,
        Constant::NameAndType {
            name_index: field_name_index,
            descriptor_index: field_descriptor_index,
        },
    );
    let field_index = push_constant(
        &mut constants,
        Constant::Fieldref {
            class_index,
            name_and_type_index: field_type,
        },
    );
    let mask = push_constant(
        &mut constants,
        Constant::Integer(if short { 65535 } else { 255 }),
    );
    let name_index = push_constant(&mut constants, Constant::Utf8(method_name.into()));
    let descriptor_index = push_constant(&mut constants, Constant::Utf8("(I)I".into()));
    let code_name = push_constant(&mut constants, Constant::Utf8("Code".into()));
    let mut code = vec![0x00; prefix];
    code.extend([0x1b, 0x36, local, 0x19, 0]);
    reference(&mut code, 0xb4, field_index);
    code.extend([0x15, local, if short { 0x35 } else { 0x33 }]);
    reference(&mut code, 0x13, mask);
    code.extend([0x7e, 0xac]);
    let class = ClassFile {
        minor_version: 0,
        major_version: 45,
        access_flags: 0x21,
        this_class: class_index,
        super_class: 0,
        constant_pool: constants,
        interfaces: vec![],
        fields: vec![Member {
            access_flags: 1,
            name_index: field_name_index,
            descriptor_index: field_descriptor_index,
            attributes: vec![],
        }],
        methods: vec![Member {
            access_flags: 1,
            name_index,
            descriptor_index,
            attributes: vec![Attribute::Code(CodeAttribute {
                name_index: code_name,
                max_stack: 2,
                max_locals: u16::from(local) + 1,
                code,
                exception_table: vec![],
                attributes: vec![],
            })],
        }],
        attributes: vec![],
    };
    let mut program = program_with_core_natives();
    program.add_class(&class, &Limits::default()).unwrap();
    (
        program,
        MethodKey {
            class: class_name.into(),
            name: method_name.into(),
            descriptor: "(I)I".into(),
        },
        format!("{class_name}.{field_name}:{field_descriptor}"),
    )
}

#[test]
fn general_leaf_execution_accepts_renamed_relocated_and_padded_array_reads() {
    for (class_name, method_name, field_name, prefix, local) in [
        ("ReadValues", "unsigned", "values", 0, 1),
        ("example/Other", "lookup", "entries", 3, 7),
        ("example/Данные", "значение", "массив", 19, 23),
    ] {
        for short in [false, true] {
            let (program, key, field) =
                accessor_fixture(class_name, method_name, field_name, prefix, local, short);
            let method = &program.methods[&key];
            assert!(!compatibility_intrinsic_candidate(method));
            assert!(method.readonly_leaf);
            let mut host = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut host);
            machine.initialize_class(class_name, 1).unwrap();
            let receiver = machine.allocate_native_instance(class_name, &[]).unwrap();
            let data = machine
                .heap
                .managed
                .allocate_array(
                    if short {
                        ArrayKind::Short
                    } else {
                        ArrayKind::Byte
                    },
                    1,
                )
                .unwrap();
            machine
                .heap
                .managed
                .set_field(receiver, &field, HeapValue::Reference(Some(data)))
                .unwrap();
            let args = [Value::Reference(Some(receiver)), Value::Int(0)];
            assert!(
                machine
                    .invoke_compatibility_intrinsic(method, &args, 1)
                    .unwrap()
                    .is_none()
            );
            // The first ordinary call resolves field metadata for the general
            // bytecode engine. Later calls still observe every guest write.
            machine.call(method, args, 1).unwrap();
            for value in [-2, 7, -1] {
                machine
                    .heap
                    .managed
                    .array_set(data, 0, HeapValue::Int(value))
                    .unwrap();
                let before = machine.execution.instructions;
                let outcome = machine.try_readonly_leaf(method, &args, 1).unwrap();
                let expected = value & if short { 65535 } else { 255 };
                assert!(
                    matches!(outcome, CallOutcome::Return(Some(Value::Int(actual))) if actual == expected)
                );
                assert_eq!(
                    machine.execution.instructions - before,
                    method.instructions.len() as u64
                );
            }
            // A failed speculative read leaves exception delivery to the
            // interpreter and does not consume the guest instruction budget.
            let invalid = [Value::Reference(Some(receiver)), Value::Int(1)];
            let before = machine.execution.instructions;
            assert!(machine.try_readonly_leaf(method, &invalid, 1).is_none());
            assert_eq!(machine.execution.instructions, before);
            let CallOutcome::Throw(exception) = machine.call(method, invalid, 1).unwrap() else {
                panic!("out-of-bounds application read must throw");
            };
            assert_eq!(
                machine.object_class(exception).unwrap(),
                "java/lang/ArrayIndexOutOfBoundsException"
            );
        }
    }
}
