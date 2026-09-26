//! Byte-array input and output streams.

use super::super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

pub(super) fn append_classes(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/io/ByteArrayInputStream
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/io/ByteArrayInputStream".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("([BII)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 11 }),
                Some(Constant::Utf8("java/io/InputStream".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Class { name_index: 13 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 12, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 16 }),
                Some(Constant::Utf8("java/lang/IndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 15, name_and_type_index: 9 }),
                Some(Constant::Fieldref { class_index: 2, name_and_type_index: 19 }),
                Some(Constant::NameAndType { name_index: 20, descriptor_index: 21 }),
                Some(Constant::Utf8("buf".to_owned())),
                Some(Constant::Utf8("[B".to_owned())),
                Some(Constant::Fieldref { class_index: 2, name_and_type_index: 23 }),
                Some(Constant::NameAndType { name_index: 24, descriptor_index: 25 }),
                Some(Constant::Utf8("pos".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Fieldref { class_index: 2, name_and_type_index: 27 }),
                Some(Constant::NameAndType { name_index: 28, descriptor_index: 25 }),
                Some(Constant::Utf8("mark".to_owned())),
                Some(Constant::Fieldref { class_index: 2, name_and_type_index: 30 }),
                Some(Constant::NameAndType { name_index: 31, descriptor_index: 25 }),
                Some(Constant::Utf8("count".to_owned())),
                Some(Constant::Methodref { class_index: 33, name_and_type_index: 34 }),
                Some(Constant::Class { name_index: 35 }),
                Some(Constant::NameAndType { name_index: 36, descriptor_index: 37 }),
                Some(Constant::Utf8("java/lang/System".to_owned())),
                Some(Constant::Utf8("arraycopy".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;ILjava/lang/Object;II)V".to_owned())),
                Some(Constant::Utf8("([B)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Class { name_index: 21 }),
                Some(Constant::Utf8("read".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("([BII)I".to_owned())),
                Some(Constant::Utf8("skip".to_owned())),
                Some(Constant::Utf8("(J)J".to_owned())),
                Some(Constant::Utf8("available".to_owned())),
                Some(Constant::Utf8("markSupported".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Utf8("reset".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 2,
            super_class: 8,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 4, name_index: 20, descriptor_index: 21, attributes: Vec::new() },
                Member { access_flags: 4, name_index: 24, descriptor_index: 25, attributes: Vec::new() },
                Member { access_flags: 4, name_index: 31, descriptor_index: 25, attributes: Vec::new() },
                Member { access_flags: 4, name_index: 28, descriptor_index: 25, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 38, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x03, 0x2b, 0xbe, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 3, max_locals: 4, code: vec![
                        0x2a, 0xb7, 0x00, 0x07, 0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0c, 0x59, 0xb7, 0x00, 0x0e, 0xbf, 0x1c, 0x9b, 0x00, 0x0f,
                        0x1d, 0x9b, 0x00, 0x0b, 0x1c, 0x2b, 0xbe, 0x1d, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x0f, 0x59, 0xb7, 0x00, 0x11, 0xbf,
                        0x2a, 0x2b, 0xb5, 0x00, 0x12, 0x2a, 0x1c, 0xb5, 0x00, 0x16, 0x2a, 0x1c, 0xb5, 0x00, 0x1a, 0x2a, 0x1c, 0x1d, 0x60, 0xb5,
                        0x00, 0x1d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 43, descriptor_index: 44, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 5, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x16, 0x2a, 0xb4, 0x00, 0x1d, 0xa2, 0x00, 0x1a, 0x2a, 0xb4, 0x00, 0x12, 0x2a, 0x59, 0xb4, 0x00, 0x16,
                        0x5a, 0x04, 0x60, 0xb5, 0x00, 0x16, 0x33, 0x11, 0x00, 0xff, 0x7e, 0xa7, 0x00, 0x04, 0x02, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 43, descriptor_index: 45, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 5, max_locals: 5, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0c, 0x59, 0xb7, 0x00, 0x0e, 0xbf, 0x1c, 0x9b, 0x00, 0x0f, 0x1d, 0x9b, 0x00, 0x0b,
                        0x1c, 0x2b, 0xbe, 0x1d, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x0f, 0x59, 0xb7, 0x00, 0x11, 0xbf, 0x2a, 0xb4, 0x00, 0x16,
                        0x2a, 0xb4, 0x00, 0x1d, 0xa1, 0x00, 0x05, 0x02, 0xac, 0x2a, 0xb4, 0x00,
                        0x1d, 0x2a, 0xb4, 0x00, 0x16, 0x64, 0x36, 0x04, 0x1d, 0x15, 0x04, 0xa4, 0x00, 0x06, 0x15, 0x04, 0x3e, 0x2a, 0xb4, 0x00,
                        0x12, 0x2a, 0xb4, 0x00, 0x16, 0x2b, 0x1c, 0x1d, 0xb8, 0x00, 0x20, 0x2a, 0x59, 0xb4, 0x00, 0x16, 0x1d, 0x60, 0xb5, 0x00,
                        0x16, 0x1d, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 46, descriptor_index: 47, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 4, max_locals: 4, code: vec![
                        0x1f, 0x09, 0x94, 0x9d, 0x00, 0x05, 0x09, 0xad, 0x1f, 0x2a, 0xb4, 0x00, 0x1d, 0x2a, 0xb4, 0x00, 0x16, 0x64, 0x85, 0x94,
                        0x9e, 0x00, 0x0f, 0x2a, 0xb4, 0x00, 0x1d, 0x2a, 0xb4, 0x00, 0x16, 0x64, 0xa7, 0x00, 0x05, 0x1f, 0x88, 0x3e, 0x2a, 0x59,
                        0xb4, 0x00, 0x16, 0x1d, 0x60, 0xb5, 0x00, 0x16, 0x1d, 0x85, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 48, descriptor_index: 44, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x1d, 0x2a, 0xb4, 0x00, 0x16, 0x64, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 49, descriptor_index: 50, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 1, max_locals: 1, code: vec![
                        0x04, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 28, descriptor_index: 51, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x2a, 0xb4, 0x00, 0x16, 0xb5, 0x00, 0x1a, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 52, descriptor_index: 11, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 39, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x2a, 0xb4, 0x00, 0x1a, 0xb5, 0x00, 0x16, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // java/io/ByteArrayOutputStream
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
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 11 }),
                Some(Constant::Utf8("java/io/OutputStream".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Class { name_index: 13 }),
                Some(Constant::Utf8("java/lang/IllegalArgumentException".to_owned())),
                Some(Constant::Methodref { class_index: 12, name_and_type_index: 9 }),
                Some(Constant::Fieldref { class_index: 2, name_and_type_index: 16 }),
                Some(Constant::NameAndType { name_index: 17, descriptor_index: 18 }),
                Some(Constant::Utf8("buf".to_owned())),
                Some(Constant::Utf8("[B".to_owned())),
                Some(Constant::Methodref { class_index: 20, name_and_type_index: 21 }),
                Some(Constant::Class { name_index: 22 }),
                Some(Constant::NameAndType { name_index: 23, descriptor_index: 24 }),
                Some(Constant::Utf8("java/lang/Math".to_owned())),
                Some(Constant::Utf8("max".to_owned())),
                Some(Constant::Utf8("(II)I".to_owned())),
                Some(Constant::Fieldref { class_index: 2, name_and_type_index: 26 }),
                Some(Constant::NameAndType { name_index: 27, descriptor_index: 28 }),
                Some(Constant::Utf8("count".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Methodref { class_index: 30, name_and_type_index: 31 }),
                Some(Constant::Class { name_index: 32 }),
                Some(Constant::NameAndType { name_index: 33, descriptor_index: 34 }),
                Some(Constant::Utf8("java/lang/System".to_owned())),
                Some(Constant::Utf8("arraycopy".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;ILjava/lang/Object;II)V".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 36 }),
                Some(Constant::NameAndType { name_index: 37, descriptor_index: 6 }),
                Some(Constant::Utf8("ensureCapacity".to_owned())),
                Some(Constant::Class { name_index: 39 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 38, name_and_type_index: 9 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 42 }),
                Some(Constant::NameAndType { name_index: 43, descriptor_index: 44 }),
                Some(Constant::Utf8("write".to_owned())),
                Some(Constant::Utf8("([BII)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("toByteArray".to_owned())),
                Some(Constant::Utf8("()[B".to_owned())),
                Some(Constant::Utf8("writeTo".to_owned())),
                Some(Constant::Utf8("(Ljava/io/OutputStream;)V".to_owned())),
                Some(Constant::Utf8("Exceptions".to_owned())),
                Some(Constant::Class { name_index: 54 }),
                Some(Constant::Utf8("java/io/IOException".to_owned())),
                Some(Constant::Utf8("size".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("reset".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 2,
            super_class: 8,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 4, name_index: 17, descriptor_index: 18, attributes: Vec::new() },
                Member { access_flags: 4, name_index: 27, descriptor_index: 28, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 11, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x10, 0x20, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x07, 0x1b, 0x9c, 0x00, 0x0b, 0xbb, 0x00, 0x0c, 0x59, 0xb7, 0x00, 0x0e, 0xbf, 0x2a, 0x1b, 0xbc, 0x08,
                        0xb5, 0x00, 0x0f, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 37, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 5, max_locals: 4, code: vec![
                        0x1b, 0x2a, 0xb4, 0x00, 0x0f, 0xbe, 0xa3, 0x00, 0x04, 0xb1, 0x1b, 0x2a, 0xb4, 0x00, 0x0f, 0xbe, 0x05, 0x68, 0x04, 0x60,
                        0xb8, 0x00, 0x13, 0x3d, 0x1c, 0xbc, 0x08, 0x4e, 0x2a, 0xb4, 0x00, 0x0f, 0x03, 0x2d, 0x03, 0x2a, 0xb4, 0x00, 0x19, 0xb8,
                        0x00, 0x1d, 0x2a, 0x2d, 0xb5, 0x00, 0x0f, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 43, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 5, max_locals: 2, code: vec![
                        0x2a, 0x2a, 0xb4, 0x00, 0x19, 0x04, 0x60, 0xb7, 0x00, 0x23, 0x2a, 0xb4, 0x00, 0x0f, 0x2a, 0x59, 0xb4, 0x00, 0x19, 0x5a,
                        0x04, 0x60, 0xb5, 0x00, 0x19, 0x1b, 0x91, 0x54, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 48, descriptor_index: 49, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 5, max_locals: 2, code: vec![
                        0x2a, 0xb4, 0x00, 0x19, 0xbc, 0x08, 0x4c, 0x2a, 0xb4, 0x00, 0x0f, 0x03, 0x2b, 0x03, 0x2a, 0xb4, 0x00, 0x19, 0xb8, 0x00,
                        0x1d, 0x2b, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 50, descriptor_index: 51, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 4, max_locals: 2, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x26, 0x59, 0xb7, 0x00, 0x28, 0xbf, 0x2b, 0x2a, 0xb4, 0x00, 0x0f, 0x03, 0x2a, 0xb4,
                        0x00, 0x19, 0xb6, 0x00, 0x29, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 55, descriptor_index: 56, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x19, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 57, descriptor_index: 11, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x03, 0xb5, 0x00, 0x19, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
