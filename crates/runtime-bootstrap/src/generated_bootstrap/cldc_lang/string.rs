//! Immutable CLDC string declarations.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

pub(super) fn append_string(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/lang/String
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
                Some(Constant::Utf8("initChars".to_owned())),
                Some(Constant::Utf8("([CII)V".to_owned())),
                Some(Constant::Class { name_index: 14 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 13, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 17 }),
                Some(Constant::NameAndType { name_index: 18, descriptor_index: 19 }),
                Some(Constant::Utf8("initString".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 21 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 12 }),
                Some(Constant::Class { name_index: 23 }),
                Some(Constant::Utf8("java/lang/IndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 26 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 27 }),
                Some(Constant::Utf8("([BII)V".to_owned())),
                Some(Constant::String { string_index: 29 }),
                Some(Constant::Utf8("UTF-8".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 31 }),
                Some(Constant::NameAndType { name_index: 32, descriptor_index: 33 }),
                Some(Constant::Utf8("initBytes".to_owned())),
                Some(Constant::Utf8("([BIILjava/lang/String;)Z".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 35 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 36 }),
                Some(Constant::Utf8("([BIILjava/lang/String;)V".to_owned())),
                Some(Constant::Class { name_index: 38 }),
                Some(Constant::Utf8("java/io/UnsupportedEncodingException".to_owned())),
                Some(Constant::Methodref { class_index: 37, name_and_type_index: 40 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 19 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 42 }),
                Some(Constant::NameAndType { name_index: 43, descriptor_index: 44 }),
                Some(Constant::Utf8("length".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Class { name_index: 46 }),
                Some(Constant::Utf8("java/lang/StringIndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 45, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 49 }),
                Some(Constant::NameAndType { name_index: 50, descriptor_index: 51 }),
                Some(Constant::Utf8("charAt0".to_owned())),
                Some(Constant::Utf8("(I)C".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 53 }),
                Some(Constant::NameAndType { name_index: 54, descriptor_index: 51 }),
                Some(Constant::Utf8("charAt".to_owned())),
                Some(Constant::Methodref { class_index: 56, name_and_type_index: 57 }),
                Some(Constant::Class { name_index: 58 }),
                Some(Constant::NameAndType { name_index: 59, descriptor_index: 60 }),
                Some(Constant::Utf8("java/lang/Character".to_owned())),
                Some(Constant::Utf8("toUpperCase".to_owned())),
                Some(Constant::Utf8("(C)C".to_owned())),
                Some(Constant::Methodref { class_index: 56, name_and_type_index: 62 }),
                Some(Constant::NameAndType { name_index: 63, descriptor_index: 60 }),
                Some(Constant::Utf8("toLowerCase".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 65 }),
                Some(Constant::NameAndType { name_index: 66, descriptor_index: 67 }),
                Some(Constant::Utf8("startsWith".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;I)Z".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 69 }),
                Some(Constant::NameAndType { name_index: 70, descriptor_index: 71 }),
                Some(Constant::Utf8("indexOf".to_owned())),
                Some(Constant::Utf8("(II)I".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 73 }),
                Some(Constant::NameAndType { name_index: 74, descriptor_index: 71 }),
                Some(Constant::Utf8("lastIndexOf".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 76 }),
                Some(Constant::NameAndType { name_index: 70, descriptor_index: 77 }),
                Some(Constant::Utf8("(Ljava/lang/String;I)I".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 79 }),
                Some(Constant::NameAndType { name_index: 74, descriptor_index: 77 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 81 }),
                Some(Constant::NameAndType { name_index: 82, descriptor_index: 83 }),
                Some(Constant::Utf8("substring".to_owned())),
                Some(Constant::Utf8("(II)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 85 }),
                Some(Constant::NameAndType { name_index: 86, descriptor_index: 87 }),
                Some(Constant::Utf8("getChars".to_owned())),
                Some(Constant::Utf8("(II[CI)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 89 }),
                Some(Constant::NameAndType { name_index: 90, descriptor_index: 91 }),
                Some(Constant::Utf8("fromChars".to_owned())),
                Some(Constant::Utf8("([C)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 93 }),
                Some(Constant::NameAndType { name_index: 70, descriptor_index: 94 }),
                Some(Constant::Utf8("(I)I".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 96 }),
                Some(Constant::NameAndType { name_index: 97, descriptor_index: 98 }),
                Some(Constant::Utf8("toCharArray".to_owned())),
                Some(Constant::Utf8("()[C".to_owned())),
                Some(Constant::String { string_index: 100 }),
                Some(Constant::Utf8("null".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 102 }),
                Some(Constant::NameAndType { name_index: 103, descriptor_index: 104 }),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::String { string_index: 106 }),
                Some(Constant::Utf8("true".to_owned())),
                Some(Constant::String { string_index: 108 }),
                Some(Constant::Utf8("false".to_owned())),
                Some(Constant::Class { name_index: 110 }),
                Some(Constant::Utf8("java/lang/StringBuffer".to_owned())),
                Some(Constant::Methodref { class_index: 109, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 109, name_and_type_index: 113 }),
                Some(Constant::NameAndType { name_index: 114, descriptor_index: 115 }),
                Some(Constant::Utf8("append".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 109, name_and_type_index: 102 }),
                Some(Constant::Methodref { class_index: 109, name_and_type_index: 118 }),
                Some(Constant::NameAndType { name_index: 114, descriptor_index: 119 }),
                Some(Constant::Utf8("(J)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("([C)V".to_owned())),
                Some(Constant::Class { name_index: 125 }),
                Some(Constant::Utf8("[C".to_owned())),
                Some(Constant::Utf8("([B)V".to_owned())),
                Some(Constant::Class { name_index: 128 }),
                Some(Constant::Utf8("[B".to_owned())),
                Some(Constant::Utf8("([BLjava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("Exceptions".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("compareTo".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)I".to_owned())),
                Some(Constant::Utf8("equalsIgnoreCase".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Z".to_owned())),
                Some(Constant::Utf8("endsWith".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("concat".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("replace".to_owned())),
                Some(Constant::Utf8("(CC)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("trim".to_owned())),
                Some(Constant::Utf8("getBytes".to_owned())),
                Some(Constant::Utf8("()[B".to_owned())),
                Some(Constant::Utf8("valueOf".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("([CII)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("(Z)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("(C)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("(J)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 4, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x03, 0xbc, 0x05, 0x03, 0x03, 0xb7, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 19, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0d, 0x59, 0xb7, 0x00, 0x0f, 0xbf, 0x2a, 0x2b, 0xb7, 0x00,
                        0x10, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 123, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x03, 0x2b, 0xc7, 0x00, 0x07, 0x03, 0xa7, 0x00, 0x05, 0x2b, 0xbe, 0xb7, 0x00, 0x14, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 12, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 4, max_locals: 4, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0d, 0x59, 0xb7, 0x00, 0x0f, 0xbf, 0x1c, 0x9b, 0x00, 0x0f,
                        0x1d, 0x9b, 0x00, 0x0b, 0x1c, 0x2b, 0xbe, 0x1d, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x16, 0x59, 0xb7, 0x00, 0x18, 0xbf,
                        0x2a, 0x2b, 0x1c, 0x1d, 0xb7, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 126, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x03, 0x2b, 0xc7, 0x00, 0x07, 0x03, 0xa7, 0x00, 0x05, 0x2b, 0xbe, 0xb7, 0x00, 0x19, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 27, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 5, max_locals: 4, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0d, 0x59, 0xb7, 0x00, 0x0f, 0xbf, 0x1c, 0x9b, 0x00, 0x0f,
                        0x1d, 0x9b, 0x00, 0x0b, 0x1c, 0x2b, 0xbe, 0x1d, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x16, 0x59, 0xb7, 0x00, 0x18, 0xbf,
                        0x2a, 0x2b, 0x1c, 0x1d, 0x01, 0x00, 0xb7, 0x00, 0x1e, 0x57, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 129, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 5, max_locals: 3, code: vec![
                        0x2a, 0x2b, 0x03, 0x2b, 0xc7, 0x00, 0x07, 0x03, 0xa7, 0x00, 0x05, 0x2b, 0xbe, 0x2c, 0xb7, 0x00, 0x22, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 36, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 5, max_locals: 5, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2b, 0xc6, 0x00, 0x08, 0x19, 0x04, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0d, 0x59, 0xb7, 0x00, 0x0f,
                        0xbf, 0x1c, 0x9b, 0x00, 0x0f, 0x1d, 0x9b, 0x00, 0x0b, 0x1c, 0x2b, 0xbe, 0x1d, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x16,
                        0x59, 0xb7, 0x00, 0x18, 0xbf, 0x2a, 0x2b, 0x1c, 0x1d, 0x19, 0x04, 0xb7, 0x00, 0x1e, 0x9a, 0x00, 0x0d, 0xbb, 0x00, 0x25,
                        0x59, 0x19, 0x04, 0xb7, 0x00, 0x27, 0xbf, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 258, name_index: 18, descriptor_index: 19, attributes: Vec::new() },
                Member { access_flags: 258, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
                Member { access_flags: 258, name_index: 32, descriptor_index: 33, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 90, descriptor_index: 91, attributes: Vec::new() },
                Member { access_flags: 257, name_index: 43, descriptor_index: 44, attributes: Vec::new() },
                Member { access_flags: 1, name_index: 54, descriptor_index: 51, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 2, max_locals: 2, code: vec![
                        0x1b, 0x9b, 0x00, 0x0b, 0x1b, 0x2a, 0xb6, 0x00, 0x29, 0xa1, 0x00, 0x0b, 0xbb, 0x00, 0x2d, 0x59, 0xb7, 0x00, 0x2f, 0xbf,
                        0x2a, 0x1b, 0xb7, 0x00, 0x30, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 258, name_index: 50, descriptor_index: 51, attributes: Vec::new() },
                Member { access_flags: 1, name_index: 86, descriptor_index: 87, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 5, max_locals: 7, code: vec![
                        0x1b, 0x9b, 0x00, 0x10, 0x1c, 0x1b, 0xa1, 0x00, 0x0b, 0x1c, 0x2a, 0xb6, 0x00, 0x29, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x2d,
                        0x59, 0xb7, 0x00, 0x2f, 0xbf, 0x2d, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0d, 0x59, 0xb7, 0x00, 0x0f, 0xbf, 0x1c, 0x1b, 0x64,
                        0x36, 0x05, 0x15, 0x04, 0x9b, 0x00, 0x0d, 0x15, 0x04, 0x2d, 0xbe, 0x15, 0x05, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x16,
                        0x59, 0xb7, 0x00, 0x18, 0xbf, 0x03, 0x36, 0x06, 0x15, 0x06, 0x15, 0x05, 0xa2, 0x00, 0x18, 0x2d, 0x15, 0x04, 0x15, 0x06,
                        0x60, 0x2a, 0x1b, 0x15, 0x06, 0x60, 0xb6, 0x00, 0x34, 0x55, 0x84, 0x06, 0x01, 0xa7, 0xff, 0xe7, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 131, descriptor_index: 132, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 5, code: vec![
                        0x2a, 0x2b, 0xa6, 0x00, 0x05, 0x04, 0xac, 0x2b, 0xc1, 0x00, 0x08, 0x9a, 0x00, 0x05, 0x03, 0xac, 0x2b, 0xc0, 0x00, 0x08,
                        0x4d, 0x2a, 0xb6, 0x00, 0x29, 0x3e, 0x1d, 0x2c, 0xb6, 0x00, 0x29, 0x9f, 0x00, 0x05, 0x03, 0xac, 0x03, 0x36, 0x04, 0x15,
                        0x04, 0x1d, 0xa2, 0x00, 0x1a, 0x2a, 0x15, 0x04, 0xb6, 0x00, 0x34, 0x2c, 0x15, 0x04, 0xb6, 0x00, 0x34, 0x9f, 0x00, 0x05,
                        0x03, 0xac, 0x84, 0x04, 0x01, 0xa7, 0xff, 0xe6, 0x04, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 133, descriptor_index: 44, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 3, code: vec![
                        0x03, 0x3c, 0x03, 0x3d, 0x1c, 0x2a, 0xb6, 0x00, 0x29, 0xa2, 0x00, 0x14, 0x10, 0x1f, 0x1b, 0x68, 0x2a, 0x1c, 0xb6, 0x00,
                        0x34, 0x60, 0x3c, 0x84, 0x02, 0x01, 0xa7, 0xff, 0xea, 0x1b, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 134, descriptor_index: 135, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 7, code: vec![
                        0x2a, 0xb6, 0x00, 0x29, 0x3d, 0x2b, 0xb6, 0x00, 0x29, 0x3e, 0x1c, 0x1d, 0xa2, 0x00, 0x07, 0x1c, 0xa7, 0x00, 0x04, 0x1d,
                        0x36, 0x04, 0x03, 0x36, 0x05, 0x15, 0x05, 0x15, 0x04, 0xa2, 0x00, 0x20, 0x2a, 0x15, 0x05, 0xb6, 0x00, 0x34, 0x2b, 0x15,
                        0x05, 0xb6, 0x00, 0x34, 0x64, 0x36, 0x06, 0x15, 0x06, 0x99, 0x00, 0x06, 0x15, 0x06, 0xac, 0x84, 0x05, 0x01, 0xa7, 0xff,
                        0xdf, 0x1c, 0x1d, 0x64, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 136, descriptor_index: 137, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 2, max_locals: 7, code: vec![
                        0x2b, 0xc6, 0x00, 0x0e, 0x2a, 0xb6, 0x00, 0x29, 0x2b, 0xb6, 0x00, 0x29, 0x9f, 0x00, 0x05, 0x03, 0xac, 0x03, 0x3d, 0x1c,
                        0x2a, 0xb6, 0x00, 0x29, 0xa2, 0x00, 0x3f, 0x2a, 0x1c, 0xb6, 0x00, 0x34, 0x3e, 0x2b, 0x1c, 0xb6, 0x00, 0x34, 0x36, 0x04,
                        0x1d, 0x15, 0x04, 0x9f, 0x00, 0x26, 0x1d, 0xb8, 0x00, 0x37, 0x36, 0x05, 0x15, 0x04, 0xb8, 0x00, 0x37, 0x36, 0x06, 0x15,
                        0x05, 0x15, 0x06, 0x9f, 0x00, 0x12, 0x15, 0x05, 0xb8, 0x00, 0x3d, 0x15, 0x06, 0xb8, 0x00, 0x3d, 0x9f, 0x00, 0x05, 0x03,
                        0xac, 0x84, 0x02, 0x01, 0xa7, 0xff, 0xbf, 0x04, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 66, descriptor_index: 137, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x03, 0xb6, 0x00, 0x40, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 66, descriptor_index: 67, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 4, code: vec![
                        0x1c, 0x9b, 0x00, 0x10, 0x1c, 0x2a, 0xb6, 0x00, 0x29, 0x2b, 0xb6, 0x00, 0x29, 0x64, 0xa4, 0x00, 0x05, 0x03, 0xac, 0x03,
                        0x3e, 0x1d, 0x2b, 0xb6, 0x00, 0x29, 0xa2, 0x00, 0x1a, 0x2a, 0x1c, 0x1d, 0x60, 0xb6, 0x00, 0x34, 0x2b, 0x1d, 0xb6, 0x00,
                        0x34, 0x9f, 0x00, 0x05, 0x03, 0xac, 0x84, 0x03, 0x01, 0xa7, 0xff, 0xe4, 0x04, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 138, descriptor_index: 137, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x2a, 0xb6, 0x00, 0x29, 0x2b, 0xb6, 0x00, 0x29, 0x64, 0xb6, 0x00, 0x40, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 70, descriptor_index: 94, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0x03, 0xb6, 0x00, 0x44, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 70, descriptor_index: 71, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 2, max_locals: 4, code: vec![
                        0x1c, 0x9c, 0x00, 0x05, 0x03, 0x3d, 0x1c, 0x3e, 0x1d, 0x2a, 0xb6, 0x00, 0x29, 0xa2, 0x00, 0x14, 0x2a, 0x1d, 0xb6, 0x00,
                        0x34, 0x1b, 0xa0, 0x00, 0x05, 0x1d, 0xac, 0x84, 0x03, 0x01, 0xa7, 0xff, 0xea, 0x02, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 74, descriptor_index: 94, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0x2a, 0xb6, 0x00, 0x29, 0x04, 0x64, 0xb6, 0x00, 0x48, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 74, descriptor_index: 71, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 2, max_locals: 4, code: vec![
                        0x1c, 0x2a, 0xb6, 0x00, 0x29, 0xa1, 0x00, 0x0a, 0x2a, 0xb6, 0x00, 0x29, 0x04, 0x64, 0x3d, 0x1c, 0x3e, 0x1d, 0x9b, 0x00,
                        0x14, 0x2a, 0x1d, 0xb6, 0x00, 0x34, 0x1b, 0xa0, 0x00, 0x05, 0x1d, 0xac, 0x84, 0x03, 0xff, 0xa7, 0xff, 0xee, 0x02, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 70, descriptor_index: 135, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x03, 0xb6, 0x00, 0x4b, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 70, descriptor_index: 77, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 4, code: vec![
                        0x1c, 0x9c, 0x00, 0x05, 0x03, 0x3d, 0x2b, 0xb6, 0x00, 0x29, 0x9a, 0x00, 0x14, 0x1c, 0x2a, 0xb6, 0x00, 0x29, 0xa2, 0x00,
                        0x07, 0x1c, 0xa7, 0x00, 0x07, 0x2a, 0xb6, 0x00, 0x29, 0xac, 0x1c, 0x3e, 0x1d, 0x2a, 0xb6, 0x00, 0x29, 0x2b, 0xb6, 0x00,
                        0x29, 0x64, 0xa3, 0x00, 0x14, 0x2a, 0x2b, 0x1d, 0xb6, 0x00, 0x40, 0x99, 0x00, 0x05, 0x1d, 0xac, 0x84, 0x03, 0x01, 0xa7,
                        0xff, 0xe5, 0x02, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 74, descriptor_index: 135, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x2a, 0xb6, 0x00, 0x29, 0xb6, 0x00, 0x4e, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 74, descriptor_index: 77, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 5, code: vec![
                        0x2a, 0xb6, 0x00, 0x29, 0x2b, 0xb6, 0x00, 0x29, 0x64, 0x3e, 0x1c, 0x1d, 0xa4, 0x00, 0x05, 0x1d, 0x3d, 0x1c, 0x9c, 0x00,
                        0x05, 0x02, 0xac, 0x1c, 0x36, 0x04, 0x15, 0x04, 0x9b, 0x00, 0x16, 0x2a, 0x2b, 0x15, 0x04, 0xb6, 0x00, 0x40, 0x99, 0x00,
                        0x06, 0x15, 0x04, 0xac, 0x84, 0x04, 0xff, 0xa7, 0xff, 0xeb, 0x02, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 82, descriptor_index: 139, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0x2a, 0xb6, 0x00, 0x29, 0xb6, 0x00, 0x50, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 82, descriptor_index: 83, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 5, max_locals: 4, code: vec![
                        0x1b, 0x9b, 0x00, 0x10, 0x1c, 0x2a, 0xb6, 0x00, 0x29, 0xa3, 0x00, 0x08, 0x1b, 0x1c, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x2d,
                        0x59, 0xb7, 0x00, 0x2f, 0xbf, 0x1b, 0x9a, 0x00, 0x0d, 0x1c, 0x2a, 0xb6, 0x00, 0x29, 0xa0, 0x00, 0x05, 0x2a, 0xb0, 0x1c,
                        0x1b, 0x64, 0xbc, 0x05, 0x4e, 0x2a, 0x1b, 0x1c, 0x2d, 0x03, 0xb6, 0x00, 0x54, 0x2d, 0xb8, 0x00, 0x58, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 140, descriptor_index: 141, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 5, max_locals: 3, code: vec![
                        0x2b, 0xb6, 0x00, 0x29, 0x9a, 0x00, 0x05, 0x2a, 0xb0, 0x2a, 0xb6, 0x00, 0x29, 0x2b, 0xb6, 0x00, 0x29, 0x60, 0xbc, 0x05,
                        0x4d, 0x2a, 0x03, 0x2a, 0xb6, 0x00, 0x29, 0x2c, 0x03, 0xb6, 0x00, 0x54, 0x2b, 0x03, 0x2b, 0xb6, 0x00, 0x29, 0x2c, 0x2a,
                        0xb6, 0x00, 0x29, 0xb6, 0x00, 0x54, 0x2c, 0xb8, 0x00, 0x58, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 142, descriptor_index: 143, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 5, max_locals: 6, code: vec![
                        0x2a, 0x1b, 0xb6, 0x00, 0x5c, 0x3e, 0x1d, 0x9c, 0x00, 0x05, 0x2a, 0xb0, 0x2a, 0xb6, 0x00, 0x29, 0xbc, 0x05, 0x3a, 0x04,
                        0x2a, 0x03, 0x2a, 0xb6, 0x00, 0x29, 0x19, 0x04, 0x03, 0xb6, 0x00, 0x54, 0x1d, 0x36, 0x05, 0x15, 0x05, 0x19, 0x04, 0xbe,
                        0xa2, 0x00, 0x18, 0x19, 0x04, 0x15, 0x05, 0x34, 0x1b, 0xa0, 0x00, 0x09, 0x19, 0x04, 0x15, 0x05, 0x1c, 0x55, 0x84, 0x05,
                        0x01, 0xa7, 0xff, 0xe6, 0x19, 0x04, 0xb8, 0x00, 0x58, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 144, descriptor_index: 104, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 3, code: vec![
                        0x03, 0x3c, 0x2a, 0xb6, 0x00, 0x29, 0x3d, 0x1b, 0x1c, 0xa2, 0x00, 0x13, 0x2a, 0x1b, 0xb6, 0x00, 0x34, 0x10, 0x20, 0xa3,
                        0x00, 0x09, 0x84, 0x01, 0x01, 0xa7, 0xff, 0xee, 0x1c, 0x1b, 0xa4, 0x00, 0x15, 0x2a, 0x1c, 0x04, 0x64, 0xb6, 0x00, 0x34,
                        0x10, 0x20, 0xa3, 0x00, 0x09, 0x84, 0x02, 0xff, 0xa7, 0xff, 0xec, 0x1b, 0x9a, 0x00, 0x0f, 0x1c, 0x2a, 0xb6, 0x00, 0x29,
                        0xa0, 0x00, 0x07, 0x2a, 0xa7, 0x00, 0x09, 0x2a, 0x1b, 0x1c, 0xb6, 0x00, 0x50, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 97, descriptor_index: 98, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 5, max_locals: 2, code: vec![
                        0x2a, 0xb6, 0x00, 0x29, 0xbc, 0x05, 0x4c, 0x2a, 0x03, 0x2a, 0xb6, 0x00, 0x29, 0x2b, 0x03, 0xb6, 0x00, 0x54, 0x2b, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 257, name_index: 145, descriptor_index: 146, attributes: Vec::new() },
                Member { access_flags: 1, name_index: 63, descriptor_index: 104, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 5, code: vec![
                        0x2a, 0xb6, 0x00, 0x5f, 0x4c, 0x03, 0x3d, 0x03, 0x3e, 0x1d, 0x2b, 0xbe, 0xa2, 0x00, 0x20, 0x2b, 0x1d, 0x34, 0xb8, 0x00,
                        0x3d, 0x36, 0x04, 0x15, 0x04, 0x2b, 0x1d, 0x34, 0x9f, 0x00, 0x0a, 0x2b, 0x1d, 0x15, 0x04, 0x55, 0x04, 0x3d, 0x84, 0x03,
                        0x01, 0xa7, 0xff, 0xe0, 0x1c, 0x99, 0x00, 0x0a, 0x2b, 0xb8, 0x00, 0x58, 0xa7, 0x00, 0x04, 0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 59, descriptor_index: 104, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 5, code: vec![
                        0x2a, 0xb6, 0x00, 0x5f, 0x4c, 0x03, 0x3d, 0x03, 0x3e, 0x1d, 0x2b, 0xbe, 0xa2, 0x00, 0x20, 0x2b, 0x1d, 0x34, 0xb8, 0x00,
                        0x37, 0x36, 0x04, 0x15, 0x04, 0x2b, 0x1d, 0x34, 0x9f, 0x00, 0x0a, 0x2b, 0x1d, 0x15, 0x04, 0x55, 0x04, 0x3d, 0x84, 0x03,
                        0x01, 0xa7, 0xff, 0xe0, 0x1c, 0x99, 0x00, 0x0a, 0x2b, 0xb8, 0x00, 0x58, 0xa7, 0x00, 0x04, 0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 103, descriptor_index: 104, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 147, descriptor_index: 148, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xc7, 0x00, 0x08, 0x12, 0x63, 0xa7, 0x00, 0x07, 0x2a, 0xb6, 0x00, 0x65, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 147, descriptor_index: 91, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb8, 0x00, 0x58, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 147, descriptor_index: 149, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 5, max_locals: 5, code: vec![
                        0x2a, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x0d, 0x59, 0xb7, 0x00, 0x0f, 0xbf, 0x1b, 0x9b, 0x00, 0x0f, 0x1c, 0x9b, 0x00, 0x0b,
                        0x1b, 0x2a, 0xbe, 0x1c, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x16, 0x59, 0xb7, 0x00, 0x18, 0xbf, 0x1c, 0xbc, 0x05, 0x4e,
                        0x03, 0x36, 0x04, 0x15, 0x04, 0x1c, 0xa2, 0x00, 0x13, 0x2d, 0x15, 0x04, 0x2a, 0x1b, 0x15, 0x04, 0x60, 0x34, 0x55, 0x84,
                        0x04, 0x01, 0xa7, 0xff, 0xed, 0x2d, 0xb8, 0x00, 0x58, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 147, descriptor_index: 150, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 1, max_locals: 1, code: vec![
                        0x1a, 0x99, 0x00, 0x08, 0x12, 0x69, 0xa7, 0x00, 0x05, 0x12, 0x6b, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 147, descriptor_index: 151, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 4, max_locals: 1, code: vec![
                        0x04, 0xbc, 0x05, 0x59, 0x03, 0x1a, 0x55, 0xb8, 0x00, 0x58, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 147, descriptor_index: 139, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 2, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x6d, 0x59, 0xb7, 0x00, 0x6f, 0x1a, 0xb6, 0x00, 0x70, 0xb6, 0x00, 0x74, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 147, descriptor_index: 152, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 120, max_stack: 3, max_locals: 2, code: vec![
                        0xbb, 0x00, 0x6d, 0x59, 0xb7, 0x00, 0x6f, 0x1e, 0xb6, 0x00, 0x75, 0xb6, 0x00, 0x74, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
