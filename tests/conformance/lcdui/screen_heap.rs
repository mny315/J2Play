use classfile::{Attribute, ClassFile, CodeAttribute, Constant, Member};

fn retained_forms_fixture() -> ClassFile {
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: vec![
            None,
            Some(Constant::Utf8("fixtures/RetainedForms".to_owned())),
            Some(Constant::Class { name_index: 1 }),
            Some(Constant::Utf8("java/lang/Object".to_owned())),
            Some(Constant::Class { name_index: 3 }),
            Some(Constant::Utf8("javax/microedition/lcdui/Form".to_owned())),
            Some(Constant::Class { name_index: 5 }),
            Some(Constant::Utf8("<init>".to_owned())),
            Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
            Some(Constant::Utf8("run".to_owned())),
            Some(Constant::Utf8("()I".to_owned())),
            Some(Constant::Utf8("Code".to_owned())),
            Some(Constant::NameAndType {
                name_index: 7,
                descriptor_index: 8,
            }),
            Some(Constant::Methodref {
                class_index: 6,
                name_and_type_index: 12,
            }),
        ],
        access_flags: 0x0021,
        this_class: 2,
        super_class: 4,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![Member {
            access_flags: 0x0009,
            name_index: 9,
            descriptor_index: 10,
            attributes: vec![Attribute::Code(CodeAttribute {
                name_index: 11,
                max_stack: 5,
                max_locals: 2,
                code: vec![
                    0x10, 0x20, // bipush 32
                    0xbd, 0x00, 0x06, // anewarray Form
                    0x4b, // astore_0
                    0x03, // iconst_0
                    0x3c, // istore_1
                    0x1b, // loop: iload_1
                    0x10, 0x20, // bipush 32
                    0xa2, 0x00, 0x14, // if_icmpge done
                    0x2a, // aload_0
                    0x1b, // iload_1
                    0xbb, 0x00, 0x06, // new Form
                    0x59, // dup
                    0x01, // aconst_null title
                    0xb7, 0x00, 0x0d, // invokespecial Form.<init>(String)
                    0x53, // aastore
                    0x84, 0x01, 0x01, // iinc 1 by 1
                    0xa7, 0xff, 0xec, // goto loop
                    0x2a, // done: aload_0
                    0xbe, // arraylength
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
fn retained_high_level_screens_share_one_bounded_framebuffer() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "retained_high_level_screens_share_one_bounded_framebuffer"
    )) {
        return;
    }
    let limits = vm::Limits {
        max_heap_bytes: 1024 * 1024,
        lcd_width: 240,
        lcd_height: 320,
        lcd_normal_width: 240,
        lcd_normal_height: 266,
        ..vm::Limits::default()
    };
    let mut program = vm::Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display_modes(
        (limits.lcd_width, limits.lcd_height),
        (limits.lcd_normal_width, limits.lcd_normal_height),
    ) {
        program.add_class(&entry.class, &limits).unwrap();
    }
    program
        .add_class(&retained_forms_fixture(), &limits)
        .unwrap();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    midp::register_natives(program.native_registry_mut()).unwrap();

    let execution = program
        .execute("fixtures/RetainedForms", "run", "()I", limits, false)
        .unwrap();

    assert_eq!(execution.value, Some(vm::Value::Int(32)));
    assert!(execution.peak_heap_bytes < 1024 * 1024);
}
