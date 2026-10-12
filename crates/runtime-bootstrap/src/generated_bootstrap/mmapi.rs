//! Common media control interfaces and the shared time base.

mod controls;
mod manager;
mod player;

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, ExceptionHandler, Member,
    append_bootstrap_classes, build_bootstrap_class,
};

pub(super) fn append_generated_mmapi(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // javax/microedition/media/Control
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Class { name_index: 2 }),
                Some(Constant::Utf8("javax/microedition/media/Control".to_owned())),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1537,
            this_class: 1,
            super_class: 3,
            interfaces: vec![],
            fields: Vec::new(),
            methods: Vec::new(),
            attributes: Vec::new(),
        },
        // javax/microedition/media/Controllable
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Class { name_index: 2 }),
                Some(Constant::Utf8("javax/microedition/media/Controllable".to_owned())),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Utf8("getControls".to_owned())),
                Some(Constant::Utf8("()[Ljavax/microedition/media/Control;".to_owned())),
                Some(Constant::Utf8("getControl".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljavax/microedition/media/Control;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1537,
            this_class: 1,
            super_class: 3,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1025, name_index: 5, descriptor_index: 6, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 7, descriptor_index: 8, attributes: Vec::new() },
            ],
            attributes: Vec::new(),
        },
    );
    manager::append_manager(classes);
    player::append_player(classes);
    append_bootstrap_classes!(classes;
        // javax/microedition/media/SystemTimeBase
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
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 12 }),
                Some(Constant::Utf8("java/lang/System".to_owned())),
                Some(Constant::Utf8("currentTimeMillis".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Long(1000)),
                None,
                Some(Constant::Class { name_index: 16 }),
                Some(Constant::Utf8("javax/microedition/media/SystemTimeBase".to_owned())),
                Some(Constant::Methodref { class_index: 15, name_and_type_index: 3 }),
                Some(Constant::Fieldref { class_index: 15, name_and_type_index: 19 }),
                Some(Constant::NameAndType { name_index: 20, descriptor_index: 21 }),
                Some(Constant::Utf8("INSTANCE".to_owned())),
                Some(Constant::Utf8("Ljavax/microedition/media/SystemTimeBase;".to_owned())),
                Some(Constant::Class { name_index: 23 }),
                Some(Constant::Utf8("javax/microedition/media/TimeBase".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("getTime".to_owned())),
                Some(Constant::Utf8("<clinit>".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 48,
            this_class: 15,
            super_class: 2,
            interfaces: vec![22],
            fields: vec![
                Member { access_flags: 24, name_index: 20, descriptor_index: 21, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 2, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 24, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 26, descriptor_index: 12, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 24, max_stack: 4, max_locals: 1, code: vec![
                        0xb8, 0x00, 0x07, 0x14, 0x00, 0x0d, 0x69, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 8, name_index: 27, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 24, max_stack: 2, max_locals: 0, code: vec![
                        0xbb, 0x00, 0x0f, 0x59, 0xb7, 0x00, 0x11, 0xb3, 0x00, 0x12, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // javax/microedition/media/TimeBase
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Class { name_index: 2 }),
                Some(Constant::Utf8("javax/microedition/media/TimeBase".to_owned())),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Utf8("getTime".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1537,
            this_class: 1,
            super_class: 3,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1025, name_index: 5, descriptor_index: 6, attributes: Vec::new() },
            ],
            attributes: Vec::new(),
        },
    );
    controls::append_controls(classes);
}
