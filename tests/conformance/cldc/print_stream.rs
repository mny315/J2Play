use classfile::{Attribute, ClassFile, CodeAttribute, Constant, Member};

fn flush_fixture() -> ClassFile {
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: vec![
            None,
            Some(Constant::Utf8("fixtures/PrintStreamFlush".to_owned())),
            Some(Constant::Class { name_index: 1 }),
            Some(Constant::Utf8("java/lang/Object".to_owned())),
            Some(Constant::Class { name_index: 3 }),
            Some(Constant::Utf8("run".to_owned())),
            Some(Constant::Utf8("()I".to_owned())),
            Some(Constant::Utf8("Code".to_owned())),
            Some(Constant::Utf8("java/io/PrintStream".to_owned())),
            Some(Constant::Class { name_index: 8 }),
            Some(Constant::Utf8("<init>".to_owned())),
            Some(Constant::Utf8("()V".to_owned())),
            Some(Constant::NameAndType {
                name_index: 10,
                descriptor_index: 11,
            }),
            Some(Constant::Methodref {
                class_index: 9,
                name_and_type_index: 12,
            }),
            Some(Constant::Utf8("flush".to_owned())),
            Some(Constant::NameAndType {
                name_index: 14,
                descriptor_index: 11,
            }),
            Some(Constant::Methodref {
                class_index: 9,
                name_and_type_index: 15,
            }),
        ],
        access_flags: 0x0021,
        this_class: 2,
        super_class: 4,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![Member {
            access_flags: 0x0009,
            name_index: 5,
            descriptor_index: 6,
            attributes: vec![Attribute::Code(CodeAttribute {
                name_index: 7,
                max_stack: 2,
                max_locals: 0,
                code: vec![
                    0xbb, 0x00, 0x09, // new PrintStream
                    0x59, // dup
                    0xb7, 0x00, 0x0d, // invokespecial PrintStream.<init>()V
                    0xb6, 0x00, 0x10, // invokevirtual PrintStream.flush()V
                    0x10, 42,   // bipush 42
                    0xac, // ireturn
                ],
                exception_table: Vec::new(),
                attributes: Vec::new(),
            })],
        }],
        attributes: Vec::new(),
    }
}

#[test]
fn invokevirtual_resolves_print_stream_flush_through_output_stream() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "invokevirtual_resolves_print_stream_flush_through_output_stream"
    )) {
        return;
    }
    let mut program = vm::Program::new();
    let limits = vm::Limits::default();
    for class in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    assert!(program.is_assignable_to("java/io/PrintStream", "java/io/OutputStream"));
    program.add_class(&flush_fixture(), &limits).unwrap();

    let execution = program
        .execute("fixtures/PrintStreamFlush", "run", "()I", limits, false)
        .unwrap();
    assert_eq!(execution.value, Some(vm::Value::Int(42)));
}
