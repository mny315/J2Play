//! Base byte streams, text output and I/O exceptions.

mod byte_arrays;
mod data_input;
mod data_output;

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, ExceptionHandler, Member,
    append_bootstrap_classes, build_bootstrap_class,
};

pub(super) fn append_generated_cldc_io(classes: &mut Vec<ClassFile>) {
    byte_arrays::append_classes(classes);
    data_input::append_classes(classes);
    data_output::append_classes(classes);
    append_bootstrap_classes!(classes;
        // java/io/EOFException
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/io/IOException".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 8 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 9 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Class { name_index: 11 }),
                Some(Constant::Utf8("java/io/EOFException".to_owned())),
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
        // java/io/IOException
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
                Some(Constant::Utf8("java/io/IOException".to_owned())),
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
        // java/io/InputStream
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
                Some(Constant::Utf8("java/io/InputStream".to_owned())),
                Some(Constant::Utf8("read".to_owned())),
                Some(Constant::Utf8("([BII)I".to_owned())),
                Some(Constant::Class { name_index: 14 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 13, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 17 }),
                Some(Constant::Utf8("java/lang/IndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 16, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 20 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 21 }),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Class { name_index: 23 }),
                Some(Constant::Utf8("java/io/IOException".to_owned())),
                Some(Constant::String { string_index: 25 }),
                Some(Constant::Utf8("mark/reset not supported".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 27 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 28 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("Exceptions".to_owned())),
                Some(Constant::Utf8("([B)I".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("available".to_owned())),
                Some(Constant::Utf8("skip".to_owned())),
                Some(Constant::Utf8("(J)J".to_owned())),
                Some(Constant::Utf8("mark".to_owned())),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Utf8("reset".to_owned())),
                Some(Constant::Utf8("markSupported".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("close".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1057,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 29, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1025, name_index: 11, descriptor_index: 21, attributes: Vec::new() },
                Member { access_flags: 1, name_index: 11, descriptor_index: 32, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 29, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x03, 0x2b, 0xbe, 0xb6, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 11, descriptor_index: 12, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 29, max_stack: 3, max_locals: 8, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0d, 0x59, 0xb7, 0x00, 0x0f, 0xbf, 0x1c, 0x9b, 0x00, 0x15, 0x1d, 0x9b, 0x00, 0x11,
                        0x1c, 0x2b, 0xbe, 0xa3, 0x00, 0x0b, 0x1d, 0x2b, 0xbe, 0x1c, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x10, 0x59, 0xb7, 0x00,
                        0x12, 0xbf, 0x1d, 0x9a, 0x00, 0x05, 0x03, 0xac, 0x2a, 0xb6, 0x00, 0x13, 0x36, 0x04, 0x15, 0x04, 0x9c, 0x00, 0x05, 0x02,
                        0xac, 0x2b, 0x1c, 0x15, 0x04, 0x91, 0x54, 0x04, 0x36, 0x05, 0x15, 0x05, 0x1d, 0xa2, 0x00, 0x28, 0x2a, 0xb6, 0x00, 0x13,
                        0x36, 0x06, 0xa7, 0x00, 0x08, 0x3a, 0x07, 0xa7, 0x00, 0x1a, 0x15, 0x06, 0x9c, 0x00, 0x06, 0xa7, 0x00, 0x12, 0x2b, 0x1c,
                        0x15, 0x05, 0x60, 0x15, 0x06, 0x91, 0x54, 0x84, 0x05, 0x01, 0xa7, 0xff, 0xd8, 0x15, 0x05, 0xac,
                    ], exception_table: vec![
                        ExceptionHandler { start_pc: 76, end_pc: 82, handler_pc: 85, catch_type: 22 },
                    ], attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 34, descriptor_index: 21, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 29, max_stack: 1, max_locals: 1, code: vec![
                        0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 35, descriptor_index: 36, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 29, max_stack: 4, max_locals: 5, code: vec![
                        0x1f, 0x09, 0x94, 0x9d, 0x00, 0x05, 0x09, 0xad, 0x09, 0x42, 0x21, 0x1f, 0x94, 0x9c, 0x00, 0x11, 0x2a, 0xb6, 0x00, 0x13,
                        0x9b, 0x00, 0x0a, 0x21, 0x0a, 0x61, 0x42, 0xa7, 0xff, 0xef, 0x21, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 37, descriptor_index: 38, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 29, max_stack: 0, max_locals: 2, code: vec![
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 39, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 29, max_stack: 3, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x16, 0x59, 0x12, 0x18, 0xb7, 0x00, 0x1a, 0xbf,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 40, descriptor_index: 41, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 29, max_stack: 1, max_locals: 1, code: vec![
                        0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 42, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 29, max_stack: 0, max_locals: 1, code: vec![
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // java/io/OutputStream
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
                Some(Constant::Utf8("java/io/OutputStream".to_owned())),
                Some(Constant::Utf8("write".to_owned())),
                Some(Constant::Utf8("([BII)V".to_owned())),
                Some(Constant::Class { name_index: 14 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 13, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 17 }),
                Some(Constant::Utf8("java/lang/IndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 16, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 20 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 21 }),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("Exceptions".to_owned())),
                Some(Constant::Class { name_index: 26 }),
                Some(Constant::Utf8("java/io/IOException".to_owned())),
                Some(Constant::Utf8("([B)V".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("flush".to_owned())),
                Some(Constant::Utf8("close".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1057,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 22, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1025, name_index: 11, descriptor_index: 21, attributes: Vec::new() },
                Member { access_flags: 1, name_index: 11, descriptor_index: 27, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 22, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x03, 0x2b, 0xbe, 0xb6, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 11, descriptor_index: 12, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 22, max_stack: 4, max_locals: 5, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0d, 0x59, 0xb7, 0x00, 0x0f, 0xbf, 0x1c, 0x9b, 0x00, 0x15, 0x1d, 0x9b, 0x00, 0x11,
                        0x1c, 0x2b, 0xbe, 0xa3, 0x00, 0x0b, 0x1d, 0x2b, 0xbe, 0x1c, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x10, 0x59, 0xb7, 0x00,
                        0x12, 0xbf, 0x03, 0x36, 0x04, 0x15, 0x04, 0x1d, 0xa2, 0x00, 0x13, 0x2a, 0x2b, 0x1c, 0x15, 0x04, 0x60, 0x33, 0xb6, 0x00,
                        0x13, 0x84, 0x04, 0x01, 0xa7, 0xff, 0xed, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 29, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 22, max_stack: 0, max_locals: 1, code: vec![
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 30, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 22, max_stack: 0, max_locals: 1, code: vec![
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // java/io/PrintStream
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
                Some(Constant::Utf8("java/lang/String".to_owned())),
                Some(Constant::Utf8("valueOf".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 14, name_and_type_index: 15 }),
                Some(Constant::Class { name_index: 16 }),
                Some(Constant::NameAndType { name_index: 17, descriptor_index: 18 }),
                Some(Constant::Utf8("java/io/PrintStream".to_owned())),
                Some(Constant::Utf8("write0".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;Z)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 20 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 21 }),
                Some(Constant::Utf8("(I)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 23 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 24 }),
                Some(Constant::Utf8("(J)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 26 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 27 }),
                Some(Constant::Utf8("(Z)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 29 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 30 }),
                Some(Constant::Utf8("(C)Ljava/lang/String;".to_owned())),
                Some(Constant::String { string_index: 32 }),
                Some(Constant::Utf8("".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("print".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)V".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Utf8("(J)V".to_owned())),
                Some(Constant::Utf8("(Z)V".to_owned())),
                Some(Constant::Utf8("(C)V".to_owned())),
                Some(Constant::Utf8("println".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 14,
            super_class: 2,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 258, name_index: 17, descriptor_index: 18, attributes: Vec::new() },
                Member { access_flags: 1, name_index: 35, descriptor_index: 36, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb8, 0x00, 0x07, 0x03, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 35, descriptor_index: 37, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb8, 0x00, 0x07, 0x03, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 35, descriptor_index: 38, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0xb8, 0x00, 0x13, 0x03, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 35, descriptor_index: 39, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1f, 0xb8, 0x00, 0x16, 0x03, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 35, descriptor_index: 40, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0xb8, 0x00, 0x19, 0x03, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 35, descriptor_index: 41, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0xb8, 0x00, 0x1c, 0x03, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 42, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 1, code: vec![
                        0x2a, 0x12, 0x1f, 0x04, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 42, descriptor_index: 36, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb8, 0x00, 0x07, 0x04, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 42, descriptor_index: 37, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb8, 0x00, 0x07, 0x04, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 42, descriptor_index: 38, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0xb8, 0x00, 0x13, 0x04, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 42, descriptor_index: 39, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1f, 0xb8, 0x00, 0x16, 0x04, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 42, descriptor_index: 40, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0xb8, 0x00, 0x19, 0x04, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 42, descriptor_index: 41, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0xb8, 0x00, 0x1c, 0x04, 0xb7, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // java/io/UTFDataFormatException
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/io/IOException".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 8 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 9 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Class { name_index: 11 }),
                Some(Constant::Utf8("java/io/UTFDataFormatException".to_owned())),
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
        // java/io/UnsupportedEncodingException
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/io/IOException".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 8 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 9 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Class { name_index: 11 }),
                Some(Constant::Utf8("java/io/UnsupportedEncodingException".to_owned())),
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
