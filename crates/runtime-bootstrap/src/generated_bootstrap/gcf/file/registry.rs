//! Filesystem registry and listener contract.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

pub(super) fn append_classes(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // javax/microedition/io/file/FileSystemListener
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Class { name_index: 2 }),
                Some(Constant::Utf8("javax/microedition/io/file/FileSystemListener".to_owned())),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Utf8("ROOT_ADDED".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Integer(0)),
                Some(Constant::Utf8("ROOT_REMOVED".to_owned())),
                Some(Constant::Integer(1)),
                Some(Constant::Utf8("rootChanged".to_owned())),
                Some(Constant::Utf8("(ILjava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1537,
            this_class: 1,
            super_class: 3,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Raw { name_index: 7, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x08,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 9, descriptor_index: 6, attributes: vec![
                    Attribute::Raw { name_index: 7, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x0a,
                    ] },
                ] },
            ],
            methods: vec![
                Member { access_flags: 1025, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            attributes: Vec::new(),
        },
        // javax/microedition/io/file/FileSystemRegistry
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
                Some(Constant::String { string_index: 11 }),
                Some(Constant::Utf8("/".to_owned())),
                Some(Constant::Methodref { class_index: 7, name_and_type_index: 13 }),
                Some(Constant::NameAndType { name_index: 14, descriptor_index: 15 }),
                Some(Constant::Utf8("addElement".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)V".to_owned())),
                Some(Constant::Methodref { class_index: 7, name_and_type_index: 17 }),
                Some(Constant::NameAndType { name_index: 18, descriptor_index: 19 }),
                Some(Constant::Utf8("elements".to_owned())),
                Some(Constant::Utf8("()Ljava/util/Enumeration;".to_owned())),
                Some(Constant::Class { name_index: 21 }),
                Some(Constant::Utf8("javax/microedition/io/file/FileSystemRegistry".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("listRoots".to_owned())),
                Some(Constant::Utf8("addFileSystemListener".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/io/file/FileSystemListener;)Z".to_owned())),
                Some(Constant::Utf8("removeFileSystemListener".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 20,
            super_class: 2,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 2, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 22, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 24, descriptor_index: 19, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 22, max_stack: 2, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x07, 0x59, 0xb7, 0x00, 0x09, 0x4b, 0x2a, 0x12, 0x0a, 0xb6, 0x00, 0x0c, 0x2a, 0xb6, 0x00, 0x10, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 25, descriptor_index: 26, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 22, max_stack: 1, max_locals: 1, code: vec![
                        0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 27, descriptor_index: 26, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 22, max_stack: 1, max_locals: 1, code: vec![
                        0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
