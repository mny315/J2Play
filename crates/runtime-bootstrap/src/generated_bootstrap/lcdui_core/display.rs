use super::{
    Attribute, ClassFile, CodeAttribute, Constant, ExceptionHandler, Member,
    append_bootstrap_classes, build_bootstrap_class,
};

pub(super) fn append_display_classes(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // javax/microedition/lcdui/Display
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
                Some(Constant::Class { name_index: 8 }),
                Some(Constant::Utf8("java/lang/Runnable".to_owned())),
                Some(Constant::Fieldref { class_index: 10, name_and_type_index: 11 }),
                Some(Constant::Class { name_index: 12 }),
                Some(Constant::NameAndType { name_index: 13, descriptor_index: 14 }),
                Some(Constant::Utf8("javax/microedition/lcdui/Display".to_owned())),
                Some(Constant::Utf8("serial".to_owned())),
                Some(Constant::Utf8("[Ljava/lang/Runnable;".to_owned())),
                Some(Constant::Class { name_index: 16 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 15, name_and_type_index: 3 }),
                Some(Constant::Fieldref { class_index: 10, name_and_type_index: 19 }),
                Some(Constant::NameAndType { name_index: 20, descriptor_index: 21 }),
                Some(Constant::Utf8("INSTANCE".to_owned())),
                Some(Constant::Utf8("Ljavax/microedition/lcdui/Display;".to_owned())),
                Some(Constant::Fieldref { class_index: 10, name_and_type_index: 23 }),
                Some(Constant::NameAndType { name_index: 24, descriptor_index: 25 }),
                Some(Constant::Utf8("current".to_owned())),
                Some(Constant::Utf8("Ljavax/microedition/lcdui/Displayable;".to_owned())),
                Some(Constant::Class { name_index: 27 }),
                Some(Constant::Utf8("java/lang/IllegalArgumentException".to_owned())),
                Some(Constant::Methodref { class_index: 26, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 30 }),
                Some(Constant::Utf8("javax/microedition/lcdui/Canvas".to_owned())),
                Some(Constant::Methodref { class_index: 29, name_and_type_index: 32 }),
                Some(Constant::NameAndType { name_index: 33, descriptor_index: 6 }),
                Some(Constant::Utf8("__hide".to_owned())),
                Some(Constant::Class { name_index: 35 }),
                Some(Constant::Utf8("javax/microedition/lcdui/Screen".to_owned())),
                Some(Constant::Methodref { class_index: 34, name_and_type_index: 32 }),
                Some(Constant::Methodref { class_index: 29, name_and_type_index: 38 }),
                Some(Constant::NameAndType { name_index: 39, descriptor_index: 6 }),
                Some(Constant::Utf8("__show".to_owned())),
                Some(Constant::Methodref { class_index: 34, name_and_type_index: 38 }),
                Some(Constant::Methodref { class_index: 42, name_and_type_index: 43 }),
                Some(Constant::Class { name_index: 44 }),
                Some(Constant::NameAndType { name_index: 45, descriptor_index: 46 }),
                Some(Constant::Utf8("javax/microedition/lcdui/Alert".to_owned())),
                Some(Constant::Utf8("__setNext".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/lcdui/Displayable;)V".to_owned())),
                Some(Constant::Methodref { class_index: 10, name_and_type_index: 48 }),
                Some(Constant::NameAndType { name_index: 49, descriptor_index: 46 }),
                Some(Constant::Utf8("setCurrent".to_owned())),
                Some(Constant::Fieldref { class_index: 10, name_and_type_index: 51 }),
                Some(Constant::NameAndType { name_index: 52, descriptor_index: 53 }),
                Some(Constant::Utf8("serialSize".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Class { name_index: 55 }),
                Some(Constant::Utf8("java/lang/IllegalStateException".to_owned())),
                Some(Constant::Methodref { class_index: 54, name_and_type_index: 3 }),
                Some(Constant::Fieldref { class_index: 10, name_and_type_index: 58 }),
                Some(Constant::NameAndType { name_index: 59, descriptor_index: 53 }),
                Some(Constant::Utf8("serialHead".to_owned())),
                Some(Constant::Methodref { class_index: 10, name_and_type_index: 61 }),
                Some(Constant::NameAndType { name_index: 62, descriptor_index: 6 }),
                Some(Constant::Utf8("drainSerial".to_owned())),
                Some(Constant::Fieldref { class_index: 10, name_and_type_index: 64 }),
                Some(Constant::NameAndType { name_index: 65, descriptor_index: 66 }),
                Some(Constant::Utf8("dispatching".to_owned())),
                Some(Constant::Utf8("Z".to_owned())),
                Some(Constant::InterfaceMethodref { class_index: 7, name_and_type_index: 68 }),
                Some(Constant::NameAndType { name_index: 69, descriptor_index: 6 }),
                Some(Constant::Utf8("run".to_owned())),
                Some(Constant::Methodref { class_index: 29, name_and_type_index: 71 }),
                Some(Constant::NameAndType { name_index: 72, descriptor_index: 73 }),
                Some(Constant::Utf8("__hostKeyPressed".to_owned())),
                Some(Constant::Utf8("(II)V".to_owned())),
                Some(Constant::Methodref { class_index: 34, name_and_type_index: 75 }),
                Some(Constant::NameAndType { name_index: 72, descriptor_index: 76 }),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Methodref { class_index: 29, name_and_type_index: 78 }),
                Some(Constant::NameAndType { name_index: 79, descriptor_index: 73 }),
                Some(Constant::Utf8("__hostKeyReleased".to_owned())),
                Some(Constant::Methodref { class_index: 29, name_and_type_index: 81 }),
                Some(Constant::NameAndType { name_index: 82, descriptor_index: 73 }),
                Some(Constant::Utf8("__hostKeyRepeated".to_owned())),
                Some(Constant::Methodref { class_index: 10, name_and_type_index: 84 }),
                Some(Constant::NameAndType { name_index: 85, descriptor_index: 6 }),
                Some(Constant::Utf8("__drainSerial".to_owned())),
                Some(Constant::Methodref { class_index: 29, name_and_type_index: 87 }),
                Some(Constant::NameAndType { name_index: 88, descriptor_index: 6 }),
                Some(Constant::Utf8("__hostIdle".to_owned())),
                Some(Constant::Methodref { class_index: 34, name_and_type_index: 87 }),
                Some(Constant::Methodref { class_index: 91, name_and_type_index: 92 }),
                Some(Constant::Class { name_index: 93 }),
                Some(Constant::NameAndType { name_index: 94, descriptor_index: 95 }),
                Some(Constant::Utf8("java/lang/Thread".to_owned())),
                Some(Constant::Utf8("sleep".to_owned())),
                Some(Constant::Utf8("(J)V".to_owned())),
                Some(Constant::Class { name_index: 97 }),
                Some(Constant::Utf8("java/lang/InterruptedException".to_owned())),
                Some(Constant::Methodref { class_index: 10, name_and_type_index: 87 }),
                Some(Constant::Methodref { class_index: 10, name_and_type_index: 3 }),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("getDisplay".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/midlet/MIDlet;)Ljavax/microedition/lcdui/Display;".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("getCurrent".to_owned())),
                Some(Constant::Utf8("()Ljavax/microedition/lcdui/Displayable;".to_owned())),
                Some(Constant::Utf8("isColor".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("numAlphaLevels".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("vibrate".to_owned())),
                Some(Constant::Utf8("(I)Z".to_owned())),
                Some(Constant::Utf8("flashBacklight".to_owned())),
                Some(Constant::Utf8("__isCurrent".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/lcdui/Displayable;)Z".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/lcdui/Alert;Ljavax/microedition/lcdui/Displayable;)V".to_owned())),
                Some(Constant::Utf8("__returnFromAlert".to_owned())),
                Some(Constant::Utf8("callSerially".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Runnable;)V".to_owned())),
                Some(Constant::Class { name_index: 121 }),
                Some(Constant::Utf8("java/lang/Throwable".to_owned())),
                Some(Constant::Utf8("<clinit>".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 10,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 26, name_index: 20, descriptor_index: 21, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 24, descriptor_index: 25, attributes: Vec::new() },
                Member { access_flags: 18, name_index: 13, descriptor_index: 14, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 59, descriptor_index: 53, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 52, descriptor_index: 53, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 65, descriptor_index: 66, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 2, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x11, 0x10, 0x00, 0xbd, 0x00, 0x07, 0xb5, 0x00, 0x09, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 102, descriptor_index: 103, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0f, 0x59, 0xb7, 0x00, 0x11, 0xbf, 0xb2, 0x00, 0x12, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 105, descriptor_index: 106, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x16, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 107, descriptor_index: 108, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 1, max_locals: 1, code: vec![
                        0x04, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 109, descriptor_index: 110, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 1, max_locals: 1, code: vec![
                        0x11, 0x01, 0x00, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 111, descriptor_index: 112, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 2, code: vec![
                        0x1b, 0x9c, 0x00, 0x0b, 0xbb, 0x00, 0x1a, 0x59, 0xb7, 0x00, 0x1c, 0xbf, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 113, descriptor_index: 112, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 2, code: vec![
                        0x1b, 0x9c, 0x00, 0x0b, 0xbb, 0x00, 0x1a, 0x59, 0xb7, 0x00, 0x1c, 0xbf, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 8, name_index: 114, descriptor_index: 115, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 1, code: vec![
                        0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0x2a, 0xa6, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 49, descriptor_index: 46, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x1d, 0x99, 0x00, 0x10, 0x2a, 0xb4, 0x00, 0x16, 0xc0, 0x00, 0x1d, 0xb6, 0x00, 0x1f,
                        0xa7, 0x00, 0x17, 0x2a, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x22, 0x99, 0x00, 0x0d, 0x2a, 0xb4, 0x00, 0x16, 0xc0, 0x00, 0x22,
                        0xb6, 0x00, 0x24, 0x2a, 0x2b, 0xb5, 0x00, 0x16, 0x2a, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x1d, 0x99, 0x00, 0x10, 0x2a, 0xb4,
                        0x00, 0x16, 0xc0, 0x00, 0x1d, 0xb6, 0x00, 0x25, 0xa7, 0x00, 0x17, 0x2a, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x22, 0x99, 0x00,
                        0x0d, 0x2a, 0xb4, 0x00, 0x16, 0xc0, 0x00, 0x22, 0xb6, 0x00, 0x28, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 49, descriptor_index: 116, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 3, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0f, 0x59, 0xb7, 0x00, 0x11, 0xbf, 0x2b, 0x2c, 0xb6, 0x00, 0x29, 0x2a, 0x2b, 0xb6,
                        0x00, 0x2f, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 8, name_index: 117, descriptor_index: 46, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 1, code: vec![
                        0xb2, 0x00, 0x12, 0x2a, 0xb6, 0x00, 0x2f, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 118, descriptor_index: 119, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 2, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0f, 0x59, 0xb7, 0x00, 0x11, 0xbf, 0x2a, 0xb4, 0x00, 0x32, 0x2a, 0xb4, 0x00, 0x09,
                        0xbe, 0xa0, 0x00, 0x0b, 0xbb, 0x00, 0x36, 0x59, 0xb7, 0x00, 0x38, 0xbf, 0x2a, 0xb4, 0x00, 0x09, 0x2a, 0xb4, 0x00, 0x39,
                        0x2a, 0xb4, 0x00, 0x32, 0x60, 0x2a, 0xb4, 0x00, 0x09, 0xbe, 0x70, 0x2b, 0x53, 0x2a, 0x59, 0xb4, 0x00, 0x32, 0x04, 0x60,
                        0xb5, 0x00, 0x32, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 8, name_index: 85, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 1, max_locals: 0, code: vec![
                        0xb2, 0x00, 0x12, 0xb7, 0x00, 0x3c, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 62, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 4, code: vec![
                        0x2a, 0xb4, 0x00, 0x3f, 0x99, 0x00, 0x04, 0xb1, 0x2a, 0x04, 0xb5, 0x00, 0x3f, 0x2a, 0xb4, 0x00, 0x32, 0x3c, 0x1b, 0x84,
                        0x01, 0xff, 0x9e, 0x00, 0x3a, 0x2a, 0xb4, 0x00, 0x09, 0x2a, 0xb4, 0x00, 0x39, 0x32, 0x4d, 0x2a, 0xb4, 0x00, 0x09, 0x2a,
                        0xb4, 0x00, 0x39, 0x01, 0x53, 0x2a, 0x2a, 0xb4, 0x00, 0x39, 0x04, 0x60, 0x2a, 0xb4, 0x00, 0x09, 0xbe, 0x70, 0xb5, 0x00,
                        0x39, 0x2a, 0x59, 0xb4, 0x00, 0x32, 0x04, 0x64, 0xb5, 0x00, 0x32, 0x2c, 0xb9, 0x00, 0x43, 0x01, 0x00, 0xa7, 0xff, 0xc5,
                        0x2a, 0x03, 0xb5, 0x00, 0x3f, 0xa7, 0x00, 0x0b, 0x4e, 0x2a, 0x03, 0xb5, 0x00, 0x3f, 0x2d, 0xbf, 0xb1,
                    ], exception_table: vec![
                        ExceptionHandler { start_pc: 13, end_pc: 80, handler_pc: 88, catch_type: 0 },
                    ], attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 72, descriptor_index: 73, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 2, code: vec![
                        0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x1d, 0x99, 0x00, 0x14, 0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc0, 0x00,
                        0x1d, 0x1a, 0x1b, 0xb6, 0x00, 0x46, 0xa7, 0x00, 0x1c, 0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x22, 0x99, 0x00,
                        0x10, 0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc0, 0x00, 0x22, 0x1a, 0xb6, 0x00, 0x4a, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 79, descriptor_index: 73, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 2, code: vec![
                        0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x1d, 0x99, 0x00, 0x11, 0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc0, 0x00,
                        0x1d, 0x1a, 0x1b, 0xb6, 0x00, 0x4d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 82, descriptor_index: 73, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 2, code: vec![
                        0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x1d, 0x99, 0x00, 0x14, 0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc0, 0x00,
                        0x1d, 0x1a, 0x1b, 0xb6, 0x00, 0x50, 0xa7, 0x00, 0x1c, 0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x22, 0x99, 0x00,
                        0x10, 0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc0, 0x00, 0x22, 0x1a, 0xb6, 0x00, 0x4a, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 88, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 1, max_locals: 0, code: vec![
                        0xb8, 0x00, 0x53, 0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x1d, 0x99, 0x00, 0x12, 0xb2, 0x00, 0x12, 0xb4, 0x00,
                        0x16, 0xc0, 0x00, 0x1d, 0xb6, 0x00, 0x56, 0xa7, 0x00, 0x1b, 0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc1, 0x00, 0x22, 0x99,
                        0x00, 0x0f, 0xb2, 0x00, 0x12, 0xb4, 0x00, 0x16, 0xc0, 0x00, 0x22, 0xb6, 0x00, 0x59, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 88, descriptor_index: 76, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 2, code: vec![
                        0x1a, 0x85, 0xb8, 0x00, 0x5a, 0xa7, 0x00, 0x04, 0x4c, 0xb8, 0x00, 0x62, 0xb1,
                    ], exception_table: vec![
                        ExceptionHandler { start_pc: 0, end_pc: 5, handler_pc: 8, catch_type: 96 },
                    ], attributes: Vec::new() }),
                ] },
                Member { access_flags: 8, name_index: 122, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 0, code: vec![
                        0xbb, 0x00, 0x0a, 0x59, 0xb7, 0x00, 0x63, 0xb3, 0x00, 0x12, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // javax/microedition/lcdui/Displayable
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
                Some(Constant::Class { name_index: 8 }),
                Some(Constant::Utf8("java/util/Vector".to_owned())),
                Some(Constant::Methodref { class_index: 7, name_and_type_index: 3 }),
                Some(Constant::Fieldref { class_index: 11, name_and_type_index: 12 }),
                Some(Constant::Class { name_index: 13 }),
                Some(Constant::NameAndType { name_index: 14, descriptor_index: 15 }),
                Some(Constant::Utf8("javax/microedition/lcdui/Displayable".to_owned())),
                Some(Constant::Utf8("commands".to_owned())),
                Some(Constant::Utf8("Ljava/util/Vector;".to_owned())),
                Some(Constant::Fieldref { class_index: 11, name_and_type_index: 17 }),
                Some(Constant::NameAndType { name_index: 18, descriptor_index: 19 }),
                Some(Constant::Utf8("title".to_owned())),
                Some(Constant::Utf8("Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 21 }),
                Some(Constant::NameAndType { name_index: 22, descriptor_index: 6 }),
                Some(Constant::Utf8("__requestRepaint".to_owned())),
                Some(Constant::Fieldref { class_index: 11, name_and_type_index: 24 }),
                Some(Constant::NameAndType { name_index: 25, descriptor_index: 26 }),
                Some(Constant::Utf8("ticker".to_owned())),
                Some(Constant::Utf8("Ljavax/microedition/lcdui/Ticker;".to_owned())),
                Some(Constant::Methodref { class_index: 28, name_and_type_index: 29 }),
                Some(Constant::Class { name_index: 30 }),
                Some(Constant::NameAndType { name_index: 31, descriptor_index: 32 }),
                Some(Constant::Utf8("javax/microedition/lcdui/Display".to_owned())),
                Some(Constant::Utf8("__isCurrent".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/lcdui/Displayable;)Z".to_owned())),
                Some(Constant::Class { name_index: 34 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 33, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 7, name_and_type_index: 37 }),
                Some(Constant::NameAndType { name_index: 38, descriptor_index: 39 }),
                Some(Constant::Utf8("contains".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Methodref { class_index: 7, name_and_type_index: 41 }),
                Some(Constant::NameAndType { name_index: 42, descriptor_index: 43 }),
                Some(Constant::Utf8("size".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Methodref { class_index: 7, name_and_type_index: 45 }),
                Some(Constant::NameAndType { name_index: 46, descriptor_index: 47 }),
                Some(Constant::Utf8("elementAt".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/Object;".to_owned())),
                Some(Constant::Class { name_index: 49 }),
                Some(Constant::Utf8("javax/microedition/lcdui/Command".to_owned())),
                Some(Constant::Methodref { class_index: 48, name_and_type_index: 51 }),
                Some(Constant::NameAndType { name_index: 52, descriptor_index: 43 }),
                Some(Constant::Utf8("getPriority".to_owned())),
                Some(Constant::Methodref { class_index: 7, name_and_type_index: 54 }),
                Some(Constant::NameAndType { name_index: 55, descriptor_index: 56 }),
                Some(Constant::Utf8("insertElementAt".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;I)V".to_owned())),
                Some(Constant::Methodref { class_index: 7, name_and_type_index: 58 }),
                Some(Constant::NameAndType { name_index: 59, descriptor_index: 39 }),
                Some(Constant::Utf8("removeElement".to_owned())),
                Some(Constant::Fieldref { class_index: 11, name_and_type_index: 61 }),
                Some(Constant::NameAndType { name_index: 62, descriptor_index: 63 }),
                Some(Constant::Utf8("commandListener".to_owned())),
                Some(Constant::Utf8("Ljavax/microedition/lcdui/CommandListener;".to_owned())),
                Some(Constant::Methodref { class_index: 48, name_and_type_index: 65 }),
                Some(Constant::NameAndType { name_index: 66, descriptor_index: 43 }),
                Some(Constant::Utf8("getCommandType".to_owned())),
                Some(Constant::InterfaceMethodref { class_index: 68, name_and_type_index: 69 }),
                Some(Constant::Class { name_index: 70 }),
                Some(Constant::NameAndType { name_index: 71, descriptor_index: 72 }),
                Some(Constant::Utf8("javax/microedition/lcdui/CommandListener".to_owned())),
                Some(Constant::Utf8("commandAction".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/lcdui/Command;Ljavax/microedition/lcdui/Displayable;)V".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 74 }),
                Some(Constant::NameAndType { name_index: 75, descriptor_index: 76 }),
                Some(Constant::Utf8("__command".to_owned())),
                Some(Constant::Utf8("(Z)Ljavax/microedition/lcdui/Command;".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("getTitle".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("setTitle".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("getTicker".to_owned())),
                Some(Constant::Utf8("()Ljavax/microedition/lcdui/Ticker;".to_owned())),
                Some(Constant::Utf8("setTicker".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/lcdui/Ticker;)V".to_owned())),
                Some(Constant::Utf8("isShown".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("getWidth".to_owned())),
                Some(Constant::Utf8("getHeight".to_owned())),
                Some(Constant::Utf8("addCommand".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/lcdui/Command;)V".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("removeCommand".to_owned())),
                Some(Constant::Utf8("setCommandListener".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/lcdui/CommandListener;)V".to_owned())),
                Some(Constant::Utf8("__dispatchSoftKey".to_owned())),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Utf8("__hasSoftKeyCommand".to_owned())),
                Some(Constant::Utf8("(I)Z".to_owned())),
                Some(Constant::Utf8("__leftCommand".to_owned())),
                Some(Constant::Utf8("()Ljavax/microedition/lcdui/Command;".to_owned())),
                Some(Constant::Utf8("__rightCommand".to_owned())),
                Some(Constant::Utf8("showNotify".to_owned())),
                Some(Constant::Utf8("hideNotify".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1057,
            this_class: 11,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 2, name_index: 18, descriptor_index: 19, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 25, descriptor_index: 26, attributes: Vec::new() },
                Member { access_flags: 18, name_index: 14, descriptor_index: 15, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 62, descriptor_index: 63, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 3, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0xbb, 0x00, 0x07, 0x59, 0xb7, 0x00, 0x09, 0xb5, 0x00, 0x0a, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 79, descriptor_index: 80, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x10, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 81, descriptor_index: 82, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb5, 0x00, 0x10, 0x2a, 0xb6, 0x00, 0x14, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 83, descriptor_index: 84, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x17, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 85, descriptor_index: 86, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb5, 0x00, 0x17, 0x2a, 0xb6, 0x00, 0x14, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 87, descriptor_index: 88, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb8, 0x00, 0x1b, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 89, descriptor_index: 43, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 1, max_locals: 1, code: vec![
                        0x11, crate::LCDUI_WIDTH_MARKER_BYTES[0], crate::LCDUI_WIDTH_MARKER_BYTES[1], 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 90, descriptor_index: 43, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 1, max_locals: 1, code: vec![
                        0x11, crate::LCDUI_HEIGHT_MARKER_BYTES[0], crate::LCDUI_HEIGHT_MARKER_BYTES[1], 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 91, descriptor_index: 92, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 3, max_locals: 3, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x21, 0x59, 0xb7, 0x00, 0x23, 0xbf, 0x2a, 0xb4, 0x00, 0x0a, 0x2b, 0xb6, 0x00, 0x24,
                        0x99, 0x00, 0x04, 0xb1, 0x03, 0x3d, 0x1c, 0x2a, 0xb4, 0x00, 0x0a, 0xb6, 0x00, 0x28, 0xa2, 0x00, 0x1e, 0x2a, 0xb4, 0x00,
                        0x0a, 0x1c, 0xb6, 0x00, 0x2c, 0xc0, 0x00, 0x30, 0xb6, 0x00, 0x32, 0x2b, 0xb6, 0x00, 0x32, 0xa3, 0x00, 0x09, 0x84, 0x02,
                        0x01, 0xa7, 0xff, 0xdd, 0x2a, 0xb4, 0x00, 0x0a, 0x2b, 0x1c, 0xb6, 0x00, 0x35, 0x2a, 0xb6, 0x00, 0x14, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 94, descriptor_index: 92, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb4, 0x00, 0x0a, 0x2b, 0xb6, 0x00, 0x39, 0x57, 0x2a, 0xb6, 0x00, 0x14, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 95, descriptor_index: 96, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb5, 0x00, 0x3c, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 16, name_index: 97, descriptor_index: 98, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 3, max_locals: 6, code: vec![
                        0x2a, 0xb4, 0x00, 0x3c, 0xc6, 0x00, 0x0d, 0x2a, 0xb4, 0x00, 0x0a, 0xb6, 0x00, 0x28, 0x9a, 0x00, 0x04, 0xb1, 0x01, 0x4d,
                        0x03, 0x3e, 0x1d, 0x2a, 0xb4, 0x00, 0x0a, 0xb6, 0x00, 0x28, 0xa2, 0x00, 0x68, 0x2a, 0xb4, 0x00, 0x0a, 0x1d, 0xb6, 0x00,
                        0x2c, 0xc0, 0x00, 0x30, 0x3a, 0x04, 0x19, 0x04, 0xb6, 0x00, 0x40, 0x36, 0x05, 0x1b, 0x10, 0xfa, 0xa0, 0x00, 0x22, 0x15,
                        0x05, 0x07, 0x9f, 0x00, 0x16, 0x15, 0x05, 0x10, 0x08, 0x9f, 0x00, 0x0f, 0x15, 0x05, 0x04, 0x9f, 0x00, 0x09, 0x15, 0x05,
                        0x08, 0xa0, 0x00, 0x09, 0x19, 0x04, 0x4d, 0xa7, 0x00, 0x2f, 0x1b, 0x10, 0xf9, 0xa0, 0x00, 0x23, 0x15, 0x05, 0x05, 0x9f,
                        0x00, 0x17, 0x15, 0x05, 0x06, 0x9f, 0x00, 0x11, 0x15, 0x05, 0x10, 0x07, 0x9f, 0x00, 0x0a, 0x15, 0x05, 0x10, 0x06, 0xa0,
                        0x00, 0x09, 0x19, 0x04, 0x4d, 0xa7, 0x00, 0x09, 0x84, 0x03, 0x01, 0xa7, 0xff, 0x93, 0x2c, 0xc7, 0x00, 0x21, 0x2a, 0xb4,
                        0x00, 0x0a, 0x1b, 0x10, 0xfa, 0xa0, 0x00, 0x07, 0x03, 0xa7, 0x00, 0x0c, 0x2a, 0xb4, 0x00, 0x0a, 0xb6, 0x00, 0x28, 0x04,
                        0x64, 0xb6, 0x00, 0x2c, 0xc0, 0x00, 0x30, 0x4d, 0x2a, 0xb4, 0x00, 0x3c, 0x2c, 0x2a, 0xb9, 0x00, 0x43, 0x03, 0x00, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 16, name_index: 99, descriptor_index: 100, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 2, max_locals: 2, code: vec![
                        0x1b, 0x10, 0xfa, 0x9f, 0x00, 0x09, 0x1b, 0x10, 0xf9, 0xa0, 0x00, 0x18, 0x2a, 0xb4, 0x00, 0x3c, 0xc6, 0x00, 0x11, 0x2a,
                        0xb4, 0x00, 0x0a, 0xb6, 0x00, 0x28, 0x99, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 16, name_index: 101, descriptor_index: 102, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x04, 0xb7, 0x00, 0x49, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 16, name_index: 103, descriptor_index: 102, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x03, 0xb7, 0x00, 0x49, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 75, descriptor_index: 76, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 3, max_locals: 5, code: vec![
                        0x2a, 0xb4, 0x00, 0x0a, 0xb6, 0x00, 0x28, 0x9a, 0x00, 0x05, 0x01, 0xb0, 0x03, 0x3d, 0x1c, 0x2a, 0xb4, 0x00, 0x0a, 0xb6,
                        0x00, 0x28, 0xa2, 0x00, 0x54, 0x2a, 0xb4, 0x00, 0x0a, 0x1c, 0xb6, 0x00, 0x2c, 0xc0, 0x00, 0x30, 0x4e, 0x2d, 0xb6, 0x00,
                        0x40, 0x36, 0x04, 0x1b, 0x99, 0x00, 0x18, 0x15, 0x04, 0x07, 0x9f, 0x00, 0x10, 0x15, 0x04, 0x10, 0x08, 0x9f, 0x00, 0x09,
                        0x15, 0x04, 0x04, 0xa0, 0x00, 0x05, 0x2d, 0xb0, 0x1b, 0x9a, 0x00, 0x1f, 0x15, 0x04, 0x05, 0x9f, 0x00, 0x17, 0x15, 0x04,
                        0x06, 0x9f, 0x00, 0x11, 0x15, 0x04, 0x10, 0x07, 0x9f, 0x00, 0x0a, 0x15, 0x04, 0x10, 0x06, 0xa0, 0x00, 0x05, 0x2d, 0xb0,
                        0x84, 0x02, 0x01, 0xa7, 0xff, 0xa7, 0x2a, 0xb4, 0x00, 0x0a, 0x1b, 0x99, 0x00, 0x07, 0x03, 0xa7, 0x00, 0x0c, 0x2a, 0xb4,
                        0x00, 0x0a, 0xb6, 0x00, 0x28, 0x04, 0x64, 0xb6, 0x00, 0x2c, 0xc0, 0x00, 0x30, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 0, name_index: 22, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 0, max_locals: 1, code: vec![
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 4, name_index: 104, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 0, max_locals: 1, code: vec![
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 4, name_index: 105, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 77, max_stack: 0, max_locals: 1, code: vec![
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // javax/microedition/lcdui/Font
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
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 12 }),
                Some(Constant::Utf8("javax/microedition/lcdui/Font".to_owned())),
                Some(Constant::Utf8("face".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 12 }),
                Some(Constant::Utf8("style".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 17 }),
                Some(Constant::NameAndType { name_index: 18, descriptor_index: 12 }),
                Some(Constant::Utf8("size".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 20 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 21 }),
                Some(Constant::Utf8("(III)V".to_owned())),
                Some(Constant::Class { name_index: 23 }),
                Some(Constant::Utf8("java/lang/IllegalArgumentException".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 26 }),
                Some(Constant::NameAndType { name_index: 27, descriptor_index: 28 }),
                Some(Constant::Utf8("getDefaultFont".to_owned())),
                Some(Constant::Utf8("()Ljavax/microedition/lcdui/Font;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 30 }),
                Some(Constant::NameAndType { name_index: 31, descriptor_index: 32 }),
                Some(Constant::Utf8("getHeight".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Class { name_index: 34 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 33, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 37 }),
                Some(Constant::Utf8("java/lang/ArrayIndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 36, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 40 }),
                Some(Constant::NameAndType { name_index: 41, descriptor_index: 42 }),
                Some(Constant::Utf8("charWidth".to_owned())),
                Some(Constant::Utf8("(C)I".to_owned())),
                Some(Constant::Methodref { class_index: 44, name_and_type_index: 45 }),
                Some(Constant::Class { name_index: 46 }),
                Some(Constant::NameAndType { name_index: 47, descriptor_index: 32 }),
                Some(Constant::Utf8("java/lang/String".to_owned())),
                Some(Constant::Utf8("length".to_owned())),
                Some(Constant::Methodref { class_index: 44, name_and_type_index: 49 }),
                Some(Constant::NameAndType { name_index: 50, descriptor_index: 51 }),
                Some(Constant::Utf8("charAt".to_owned())),
                Some(Constant::Utf8("(I)C".to_owned())),
                Some(Constant::Class { name_index: 53 }),
                Some(Constant::Utf8("java/lang/StringIndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 52, name_and_type_index: 3 }),
                Some(Constant::Utf8("FACE_SYSTEM".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Integer(0)),
                Some(Constant::Utf8("FACE_MONOSPACE".to_owned())),
                Some(Constant::Integer(32)),
                Some(Constant::Utf8("FACE_PROPORTIONAL".to_owned())),
                Some(Constant::Integer(64)),
                Some(Constant::Utf8("STYLE_PLAIN".to_owned())),
                Some(Constant::Utf8("STYLE_BOLD".to_owned())),
                Some(Constant::Integer(1)),
                Some(Constant::Utf8("STYLE_ITALIC".to_owned())),
                Some(Constant::Integer(2)),
                Some(Constant::Utf8("STYLE_UNDERLINED".to_owned())),
                Some(Constant::Integer(4)),
                Some(Constant::Utf8("SIZE_SMALL".to_owned())),
                Some(Constant::Integer(8)),
                Some(Constant::Utf8("SIZE_MEDIUM".to_owned())),
                Some(Constant::Utf8("SIZE_LARGE".to_owned())),
                Some(Constant::Integer(16)),
                Some(Constant::Utf8("FONT_STATIC_TEXT".to_owned())),
                Some(Constant::Utf8("FONT_INPUT_TEXT".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("getFont".to_owned())),
                Some(Constant::Utf8("(I)Ljavax/microedition/lcdui/Font;".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("(III)Ljavax/microedition/lcdui/Font;".to_owned())),
                Some(Constant::Utf8("getBaselinePosition".to_owned())),
                Some(Constant::Utf8("charsWidth".to_owned())),
                Some(Constant::Utf8("([CII)I".to_owned())),
                Some(Constant::Utf8("stringWidth".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)I".to_owned())),
                Some(Constant::Utf8("substringWidth".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;II)I".to_owned())),
                Some(Constant::Utf8("getStyle".to_owned())),
                Some(Constant::Utf8("getSize".to_owned())),
                Some(Constant::Utf8("getFace".to_owned())),
                Some(Constant::Utf8("isPlain".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("isBold".to_owned())),
                Some(Constant::Utf8("isItalic".to_owned())),
                Some(Constant::Utf8("isUnderlined".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 55, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x39,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 58, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x3b,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 60, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x3d,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 62, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x39,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 63, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x40,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 65, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x42,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 67, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x44,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 69, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x46,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 71, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x39,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 72, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x49,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 74, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x39,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 75, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 56, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x40,
                    ] },
                ] },
                Member { access_flags: 18, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
                Member { access_flags: 18, name_index: 18, descriptor_index: 12, attributes: Vec::new() },
                Member { access_flags: 18, name_index: 15, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 2, name_index: 5, descriptor_index: 21, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 2, max_locals: 4, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x1b, 0xb5, 0x00, 0x07, 0x2a, 0x1c, 0xb5, 0x00, 0x0d, 0x2a, 0x1d, 0xb5, 0x00, 0x10, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 27, descriptor_index: 28, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 5, max_locals: 0, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x03, 0x03, 0x03, 0xb7, 0x00, 0x13, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 78, descriptor_index: 79, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 2, max_locals: 1, code: vec![
                        0x1a, 0x99, 0x00, 0x10, 0x1a, 0x04, 0x9f, 0x00, 0x0b, 0xbb, 0x00, 0x16, 0x59, 0xb7, 0x00, 0x18, 0xbf, 0xb8, 0x00, 0x19,
                        0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 78, descriptor_index: 81, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 5, max_locals: 3, code: vec![
                        0x1a, 0x99, 0x00, 0x0f, 0x1a, 0x10, 0x20, 0x9f, 0x00, 0x09, 0x1a, 0x10, 0x40, 0xa0, 0x00, 0x1a, 0x1b, 0x10, 0xf8, 0x7e,
                        0x9a, 0x00, 0x13, 0x1c, 0x10, 0x08, 0x9f, 0x00, 0x15, 0x1c, 0x99, 0x00, 0x11, 0x1c, 0x10, 0x10, 0x9f, 0x00, 0x0b, 0xbb,
                        0x00, 0x16, 0x59, 0xb7, 0x00, 0x18, 0xbf, 0xbb, 0x00, 0x08, 0x59, 0x1a, 0x1b, 0x1c, 0xb7, 0x00, 0x13, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 31, descriptor_index: 32, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x10, 0x10, 0x08, 0xa0, 0x00, 0x08, 0x10, 0x08, 0xa7, 0x00, 0x13, 0x2a, 0xb4, 0x00, 0x10, 0x10, 0x10,
                        0xa0, 0x00, 0x08, 0x10, 0x10, 0xa7, 0x00, 0x05, 0x10, 0x0c, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 82, descriptor_index: 32, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb6, 0x00, 0x1d, 0x04, 0x64, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 41, descriptor_index: 42, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb4, 0x00, 0x10, 0x10, 0x08, 0xa0, 0x00, 0x08, 0x10, 0x06, 0xa7, 0x00, 0x13, 0x2a, 0xb4, 0x00, 0x10, 0x10, 0x10,
                        0xa0, 0x00, 0x08, 0x10, 0x0c, 0xa7, 0x00, 0x05, 0x10, 0x08, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 83, descriptor_index: 84, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 5, max_locals: 6, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x21, 0x59, 0xb7, 0x00, 0x23, 0xbf, 0x1c, 0x9b, 0x00, 0x0f, 0x1d, 0x9b, 0x00, 0x0b,
                        0x1c, 0x2b, 0xbe, 0x1d, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x24, 0x59, 0xb7, 0x00, 0x26, 0xbf, 0x03, 0x36, 0x04, 0x03,
                        0x36, 0x05, 0x15, 0x05, 0x1d, 0xa2, 0x00, 0x18, 0x15, 0x04, 0x2a, 0x2b, 0x1c, 0x15, 0x05, 0x60, 0x34, 0xb6, 0x00, 0x27,
                        0x60, 0x36, 0x04, 0x84, 0x05, 0x01, 0xa7, 0xff, 0xe8, 0x15, 0x04, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 85, descriptor_index: 86, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 4, max_locals: 4, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x21, 0x59, 0xb7, 0x00, 0x23, 0xbf, 0x03, 0x3d, 0x03, 0x3e, 0x1d, 0x2b, 0xb6, 0x00,
                        0x2b, 0xa2, 0x00, 0x15, 0x1c, 0x2a, 0x2b, 0x1d, 0xb6, 0x00, 0x30, 0xb6, 0x00, 0x27, 0x60, 0x3d, 0x84, 0x03, 0x01, 0xa7,
                        0xff, 0xe9, 0x1c, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 87, descriptor_index: 88, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 5, max_locals: 6, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x21, 0x59, 0xb7, 0x00, 0x23, 0xbf, 0x1c, 0x9b, 0x00, 0x11, 0x1d, 0x9b, 0x00, 0x0d,
                        0x1c, 0x2b, 0xb6, 0x00, 0x2b, 0x1d, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x34, 0x59, 0xb7, 0x00, 0x36, 0xbf, 0x03, 0x36,
                        0x04, 0x03, 0x36, 0x05, 0x15, 0x05, 0x1d, 0xa2, 0x00, 0x1a, 0x15, 0x04, 0x2a, 0x2b, 0x1c, 0x15, 0x05, 0x60, 0xb6, 0x00,
                        0x30, 0xb6, 0x00, 0x27, 0x60, 0x36, 0x04, 0x84, 0x05, 0x01, 0xa7, 0xff, 0xe6, 0x15, 0x04, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 89, descriptor_index: 32, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x0d, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 90, descriptor_index: 32, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x10, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 91, descriptor_index: 32, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 92, descriptor_index: 93, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x0d, 0x9a, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 94, descriptor_index: 93, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x0d, 0x04, 0x7e, 0x99, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 95, descriptor_index: 93, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x0d, 0x05, 0x7e, 0x99, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 96, descriptor_index: 93, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 76, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x0d, 0x07, 0x7e, 0x99, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
