//! Tone and volume control interfaces and implementations.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

pub(super) fn append_controls(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // javax/microedition/media/ToneControlImpl
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
                Some(Constant::Utf8("javax/microedition/media/ToneControlImpl".to_owned())),
                Some(Constant::Utf8("player".to_owned())),
                Some(Constant::Utf8("Ljavax/microedition/media/PlayerImpl;".to_owned())),
                Some(Constant::Class { name_index: 14 }),
                Some(Constant::Utf8("java/lang/IllegalArgumentException".to_owned())),
                Some(Constant::Methodref { class_index: 13, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 17, name_and_type_index: 18 }),
                Some(Constant::Class { name_index: 19 }),
                Some(Constant::NameAndType { name_index: 20, descriptor_index: 21 }),
                Some(Constant::Utf8("javax/microedition/media/PlayerImpl".to_owned())),
                Some(Constant::Utf8("setToneSequence".to_owned())),
                Some(Constant::Utf8("([B)V".to_owned())),
                Some(Constant::Class { name_index: 23 }),
                Some(Constant::Utf8("javax/microedition/media/control/ToneControl".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/media/PlayerImpl;)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("setSequence".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 48,
            this_class: 8,
            super_class: 2,
            interfaces: vec![22],
            fields: vec![
                Member { access_flags: 18, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 0, name_index: 5, descriptor_index: 24, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 25, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 27, descriptor_index: 21, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 25, max_stack: 2, max_locals: 2, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0d, 0x59, 0xb7, 0x00, 0x0f, 0xbf, 0x2a, 0xb4, 0x00, 0x07, 0x2b, 0xb6, 0x00, 0x10,
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // javax/microedition/media/VolumeControlImpl
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
                Some(Constant::Utf8("javax/microedition/media/VolumeControlImpl".to_owned())),
                Some(Constant::Utf8("player".to_owned())),
                Some(Constant::Utf8("Ljavax/microedition/media/PlayerImpl;".to_owned())),
                Some(Constant::Methodref { class_index: 14, name_and_type_index: 15 }),
                Some(Constant::Class { name_index: 16 }),
                Some(Constant::NameAndType { name_index: 17, descriptor_index: 18 }),
                Some(Constant::Utf8("javax/microedition/media/PlayerImpl".to_owned())),
                Some(Constant::Utf8("volumeLevel".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Methodref { class_index: 14, name_and_type_index: 20 }),
                Some(Constant::NameAndType { name_index: 21, descriptor_index: 22 }),
                Some(Constant::Utf8("setVolumeLevel".to_owned())),
                Some(Constant::Utf8("(I)I".to_owned())),
                Some(Constant::Methodref { class_index: 14, name_and_type_index: 24 }),
                Some(Constant::NameAndType { name_index: 25, descriptor_index: 26 }),
                Some(Constant::Utf8("volumeChanged".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/media/control/VolumeControl;)V".to_owned())),
                Some(Constant::Methodref { class_index: 14, name_and_type_index: 28 }),
                Some(Constant::NameAndType { name_index: 29, descriptor_index: 30 }),
                Some(Constant::Utf8("muted".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Methodref { class_index: 14, name_and_type_index: 32 }),
                Some(Constant::NameAndType { name_index: 33, descriptor_index: 34 }),
                Some(Constant::Utf8("setMuted".to_owned())),
                Some(Constant::Utf8("(Z)V".to_owned())),
                Some(Constant::Class { name_index: 36 }),
                Some(Constant::Utf8("javax/microedition/media/control/VolumeControl".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/media/PlayerImpl;)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("setLevel".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("getLevel".to_owned())),
                Some(Constant::Utf8("setMute".to_owned())),
                Some(Constant::Utf8("isMuted".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 48,
            this_class: 8,
            super_class: 2,
            interfaces: vec![35],
            fields: vec![
                Member { access_flags: 18, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 0, name_index: 5, descriptor_index: 37, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 38, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 40, descriptor_index: 22, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 38, max_stack: 2, max_locals: 4, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb6, 0x00, 0x0d, 0x3d, 0x2a, 0xb4, 0x00, 0x07, 0x1b, 0xb6, 0x00, 0x13, 0x3e, 0x1c, 0x1d, 0x9f,
                        0x00, 0x0b, 0x2a, 0xb4, 0x00, 0x07, 0x2a, 0xb6, 0x00, 0x17, 0x1d, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 42, descriptor_index: 18, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 38, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb6, 0x00, 0x0d, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 43, descriptor_index: 34, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 38, max_stack: 2, max_locals: 3, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb6, 0x00, 0x1b, 0x3d, 0x2a, 0xb4, 0x00, 0x07, 0x1b, 0xb6, 0x00, 0x1f, 0x1c, 0x1b, 0x9f, 0x00,
                        0x0b, 0x2a, 0xb4, 0x00, 0x07, 0x2a, 0xb6, 0x00, 0x17, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 44, descriptor_index: 30, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 38, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb6, 0x00, 0x1b, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // javax/microedition/media/control/ToneControl
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Class { name_index: 2 }),
                Some(Constant::Utf8("javax/microedition/media/control/ToneControl".to_owned())),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Class { name_index: 6 }),
                Some(Constant::Utf8("javax/microedition/media/Control".to_owned())),
                Some(Constant::Utf8("VERSION".to_owned())),
                Some(Constant::Utf8("B".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Integer(-2)),
                Some(Constant::Utf8("TEMPO".to_owned())),
                Some(Constant::Integer(-3)),
                Some(Constant::Utf8("RESOLUTION".to_owned())),
                Some(Constant::Integer(-4)),
                Some(Constant::Utf8("BLOCK_START".to_owned())),
                Some(Constant::Integer(-5)),
                Some(Constant::Utf8("BLOCK_END".to_owned())),
                Some(Constant::Integer(-6)),
                Some(Constant::Utf8("PLAY_BLOCK".to_owned())),
                Some(Constant::Integer(-7)),
                Some(Constant::Utf8("SET_VOLUME".to_owned())),
                Some(Constant::Integer(-8)),
                Some(Constant::Utf8("REPEAT".to_owned())),
                Some(Constant::Integer(-9)),
                Some(Constant::Utf8("C4".to_owned())),
                Some(Constant::Integer(60)),
                Some(Constant::Utf8("SILENCE".to_owned())),
                Some(Constant::Integer(-1)),
                Some(Constant::Utf8("setSequence".to_owned())),
                Some(Constant::Utf8("([B)V".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1537,
            this_class: 1,
            super_class: 3,
            interfaces: vec![5],
            fields: vec![
                Member { access_flags: 25, name_index: 7, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x0a,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 11, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x0c,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 13, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x0e,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 15, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x10,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 17, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x12,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 19, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x14,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 21, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x16,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 23, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x18,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 25, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x1a,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 27, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x1c,
                    ] },
                ] },
            ],
            methods: vec![
                Member { access_flags: 1025, name_index: 29, descriptor_index: 30, attributes: Vec::new() },
            ],
            attributes: Vec::new(),
        },
        // javax/microedition/media/control/VolumeControl
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Class { name_index: 2 }),
                Some(Constant::Utf8("javax/microedition/media/control/VolumeControl".to_owned())),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Class { name_index: 6 }),
                Some(Constant::Utf8("javax/microedition/media/Control".to_owned())),
                Some(Constant::Utf8("setLevel".to_owned())),
                Some(Constant::Utf8("(I)I".to_owned())),
                Some(Constant::Utf8("getLevel".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("setMute".to_owned())),
                Some(Constant::Utf8("(Z)V".to_owned())),
                Some(Constant::Utf8("isMuted".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1537,
            this_class: 1,
            super_class: 3,
            interfaces: vec![5],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1025, name_index: 7, descriptor_index: 8, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 9, descriptor_index: 10, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 13, descriptor_index: 14, attributes: Vec::new() },
            ],
            attributes: Vec::new(),
        },
    );
}
