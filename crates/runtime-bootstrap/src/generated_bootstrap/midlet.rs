//! `MIDlet` lifecycle callbacks and state-change errors.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, ExceptionHandler, Member,
    append_bootstrap_classes, build_bootstrap_class,
};

pub(super) fn append_generated_midlet(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // javax/microedition/midlet/MIDlet
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 6 }),
                Some(Constant::Utf8("javax/microedition/midlet/MIDlet".to_owned())),
                Some(Constant::Utf8("startApp".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 13 }),
                Some(Constant::NameAndType { name_index: 14, descriptor_index: 15 }),
                Some(Constant::Utf8("lifecycleCallback".to_owned())),
                Some(Constant::Utf8("(II)V".to_owned())),
                Some(Constant::Class { name_index: 17 }),
                Some(Constant::Utf8("javax/microedition/midlet/MIDletStateChangeException".to_owned())),
                Some(Constant::Class { name_index: 19 }),
                Some(Constant::Utf8("java/lang/RuntimeException".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 21 }),
                Some(Constant::NameAndType { name_index: 22, descriptor_index: 6 }),
                Some(Constant::Utf8("pauseApp".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 24 }),
                Some(Constant::NameAndType { name_index: 25, descriptor_index: 26 }),
                Some(Constant::Utf8("destroyApp".to_owned())),
                Some(Constant::Utf8("(Z)V".to_owned())),
                Some(Constant::Utf8("CALLBACK_START".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Integer(1)),
                Some(Constant::Utf8("CALLBACK_PAUSE".to_owned())),
                Some(Constant::Integer(2)),
                Some(Constant::Utf8("CALLBACK_DESTROY".to_owned())),
                Some(Constant::Integer(3)),
                Some(Constant::Utf8("COMPLETED".to_owned())),
                Some(Constant::Integer(0)),
                Some(Constant::Utf8("REJECTED".to_owned())),
                Some(Constant::Utf8("RUNTIME_FAILURE".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("Exceptions".to_owned())),
                Some(Constant::Utf8("getAppProperty".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("notifyDestroyed".to_owned())),
                Some(Constant::Utf8("notifyPaused".to_owned())),
                Some(Constant::Utf8("resumeRequest".to_owned())),
                Some(Constant::Utf8("platformRequest".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Z".to_owned())),
                Some(Constant::Class { name_index: 50 }),
                Some(Constant::Utf8("javax/microedition/io/ConnectionNotFoundException".to_owned())),
                Some(Constant::Utf8("checkPermission".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)I".to_owned())),
                Some(Constant::Utf8("__amsStartApp".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("__amsPauseApp".to_owned())),
                Some(Constant::Utf8("__amsDestroyApp".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1057,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 26, name_index: 27, descriptor_index: 28, attributes: vec![
                    Attribute::Raw { name_index: 29, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x1e,
                    ] },
                ] },
                Member { access_flags: 26, name_index: 31, descriptor_index: 28, attributes: vec![
                    Attribute::Raw { name_index: 29, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x20,
                    ] },
                ] },
                Member { access_flags: 26, name_index: 33, descriptor_index: 28, attributes: vec![
                    Attribute::Raw { name_index: 29, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x22,
                    ] },
                ] },
                Member { access_flags: 26, name_index: 35, descriptor_index: 28, attributes: vec![
                    Attribute::Raw { name_index: 29, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x24,
                    ] },
                ] },
                Member { access_flags: 26, name_index: 37, descriptor_index: 28, attributes: vec![
                    Attribute::Raw { name_index: 29, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x1e,
                    ] },
                ] },
                Member { access_flags: 26, name_index: 38, descriptor_index: 28, attributes: vec![
                    Attribute::Raw { name_index: 29, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x20,
                    ] },
                ] },
            ],
            methods: vec![
                Member { access_flags: 4, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1028, name_index: 11, descriptor_index: 6, attributes: Vec::new() },
                Member { access_flags: 1028, name_index: 22, descriptor_index: 6, attributes: Vec::new() },
                Member { access_flags: 1028, name_index: 25, descriptor_index: 26, attributes: Vec::new() },
                Member { access_flags: 273, name_index: 42, descriptor_index: 43, attributes: Vec::new() },
                Member { access_flags: 273, name_index: 44, descriptor_index: 6, attributes: Vec::new() },
                Member { access_flags: 273, name_index: 45, descriptor_index: 6, attributes: Vec::new() },
                Member { access_flags: 273, name_index: 46, descriptor_index: 6, attributes: Vec::new() },
                Member { access_flags: 273, name_index: 47, descriptor_index: 48, attributes: Vec::new() },
                Member { access_flags: 273, name_index: 51, descriptor_index: 52, attributes: Vec::new() },
                Member { access_flags: 16, name_index: 53, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0xb6, 0x00, 0x07, 0x2a, 0x04, 0x03, 0xb7, 0x00, 0x0c, 0xa7, 0x00, 0x14, 0x4c, 0x2a, 0x04, 0x04, 0xb7, 0x00, 0x0c,
                        0xa7, 0x00, 0x0a, 0x4c, 0x2a, 0x04, 0x05, 0xb7, 0x00, 0x0c, 0xb1,
                    ], exception_table: vec![
                        ExceptionHandler { start_pc: 0, end_pc: 10, handler_pc: 13, catch_type: 16 },
                        ExceptionHandler { start_pc: 0, end_pc: 10, handler_pc: 23, catch_type: 18 },
                    ], attributes: Vec::new() }),
                ] },
                Member { access_flags: 16, name_index: 55, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0xb6, 0x00, 0x14, 0x2a, 0x05, 0x03, 0xb7, 0x00, 0x0c, 0xa7, 0x00, 0x0a, 0x4c, 0x2a, 0x05, 0x05, 0xb7, 0x00, 0x0c,
                        0xb1,
                    ], exception_table: vec![
                        ExceptionHandler { start_pc: 0, end_pc: 10, handler_pc: 13, catch_type: 18 },
                    ], attributes: Vec::new() }),
                ] },
                Member { access_flags: 16, name_index: 56, descriptor_index: 26, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1b, 0xb6, 0x00, 0x17, 0x2a, 0x06, 0x03, 0xb7, 0x00, 0x0c, 0xa7, 0x00, 0x14, 0x4d, 0x2a, 0x06, 0x04, 0xb7, 0x00,
                        0x0c, 0xa7, 0x00, 0x0a, 0x4d, 0x2a, 0x06, 0x05, 0xb7, 0x00, 0x0c, 0xb1,
                    ], exception_table: vec![
                        ExceptionHandler { start_pc: 0, end_pc: 11, handler_pc: 14, catch_type: 16 },
                        ExceptionHandler { start_pc: 0, end_pc: 11, handler_pc: 24, catch_type: 18 },
                    ], attributes: Vec::new() }),
                ] },
                Member { access_flags: 258, name_index: 14, descriptor_index: 15, attributes: Vec::new() },
            ],
            attributes: Vec::new(),
        },
        // javax/microedition/midlet/MIDletStateChangeException
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/lang/Exception".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 8 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 9 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Class { name_index: 11 }),
                Some(Constant::Utf8("javax/microedition/midlet/MIDletStateChangeException".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 10,
            super_class: 2,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 12, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 9, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 12, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb7, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
