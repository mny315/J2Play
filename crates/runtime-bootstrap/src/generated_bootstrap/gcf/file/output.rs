//! `FileConnection` output stream.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

pub(super) fn append_classes(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // javax/microedition/io/file/FileConnectionImpl$FileOutput
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/io/ByteArrayOutputStream".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 12 }),
                Some(Constant::Utf8("javax/microedition/io/file/FileConnectionImpl$FileOutput".to_owned())),
                Some(Constant::Utf8("owner".to_owned())),
                Some(Constant::Utf8("Ljavax/microedition/io/file/FileConnectionImpl;".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 16 }),
                Some(Constant::Utf8("offset".to_owned())),
                Some(Constant::Utf8("J".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 18 }),
                Some(Constant::NameAndType { name_index: 19, descriptor_index: 20 }),
                Some(Constant::Utf8("toByteArray".to_owned())),
                Some(Constant::Utf8("()[B".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 23 }),
                Some(Constant::Class { name_index: 24 }),
                Some(Constant::NameAndType { name_index: 25, descriptor_index: 26 }),
                Some(Constant::Utf8("javax/microedition/io/file/FileConnectionImpl".to_owned())),
                Some(Constant::Utf8("access$000".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/io/file/FileConnectionImpl;)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 28 }),
                Some(Constant::NameAndType { name_index: 29, descriptor_index: 30 }),
                Some(Constant::Utf8("access$100".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;[BZ)V".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 32 }),
                Some(Constant::NameAndType { name_index: 33, descriptor_index: 34 }),
                Some(Constant::Utf8("access$200".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;[BJ)V".to_owned())),
                Some(Constant::Utf8("(Ljavax/microedition/io/file/FileConnectionImpl;J)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("close".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Class { name_index: 41 }),
                Some(Constant::Utf8("[B".to_owned())),
                Some(Constant::Utf8("Exceptions".to_owned())),
                Some(Constant::Class { name_index: 44 }),
                Some(Constant::Utf8("java/io/IOException".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
                Some(Constant::Utf8("InnerClasses".to_owned())),
                Some(Constant::Utf8("FileOutput".to_owned())),
            ],
            access_flags: 48,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
                Member { access_flags: 18, name_index: 15, descriptor_index: 16, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 0, name_index: 5, descriptor_index: 35, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 36, max_stack: 3, max_locals: 4, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb5, 0x00, 0x07, 0x2a, 0x20, 0xb5, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 38, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 36, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xc7, 0x00, 0x04, 0xb1, 0x2a, 0xb6, 0x00, 0x11, 0x4c, 0x2a, 0xb4, 0x00, 0x0d, 0x09, 0x94, 0x9a,
                        0x00, 0x12, 0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x15, 0x2b, 0x03, 0xb8, 0x00, 0x1b, 0xa7, 0x00, 0x12, 0x2a, 0xb4, 0x00,
                        0x07, 0xb8, 0x00, 0x15, 0x2b, 0x2a, 0xb4, 0x00, 0x0d, 0xb8, 0x00, 0x1f, 0x2a, 0x01, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
