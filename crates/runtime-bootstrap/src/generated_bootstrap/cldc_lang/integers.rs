//! Boxed integer values and conversions.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

pub(super) fn append_byte(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/lang/Byte
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/lang/Number".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 12 }),
                Some(Constant::Utf8("java/lang/Byte".to_owned())),
                Some(Constant::Utf8("value".to_owned())),
                Some(Constant::Utf8("B".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 16 }),
                Some(Constant::Utf8("parseByte".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)B".to_owned())),
                Some(Constant::Methodref { class_index: 18, name_and_type_index: 19 }),
                Some(Constant::Class { name_index: 20 }),
                Some(Constant::NameAndType { name_index: 21, descriptor_index: 22 }),
                Some(Constant::Utf8("java/lang/String".to_owned())),
                Some(Constant::Utf8("valueOf".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 24 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 25 }),
                Some(Constant::Utf8("(Ljava/lang/String;I)B".to_owned())),
                Some(Constant::Methodref { class_index: 27, name_and_type_index: 28 }),
                Some(Constant::Class { name_index: 29 }),
                Some(Constant::NameAndType { name_index: 30, descriptor_index: 31 }),
                Some(Constant::Utf8("java/lang/Integer".to_owned())),
                Some(Constant::Utf8("parseInt".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;I)I".to_owned())),
                Some(Constant::Class { name_index: 33 }),
                Some(Constant::Utf8("java/lang/NumberFormatException".to_owned())),
                Some(Constant::Methodref { class_index: 32, name_and_type_index: 35 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 36 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 38 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 39 }),
                Some(Constant::Utf8("(B)V".to_owned())),
                Some(Constant::Utf8("MIN_VALUE".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Integer(-128)),
                Some(Constant::Utf8("MAX_VALUE".to_owned())),
                Some(Constant::Integer(127)),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("byteValue".to_owned())),
                Some(Constant::Utf8("()B".to_owned())),
                Some(Constant::Utf8("shortValue".to_owned())),
                Some(Constant::Utf8("()S".to_owned())),
                Some(Constant::Utf8("intValue".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("longValue".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Utf8("floatValue".to_owned())),
                Some(Constant::Utf8("()F".to_owned())),
                Some(Constant::Utf8("doubleValue".to_owned())),
                Some(Constant::Utf8("()D".to_owned())),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/Byte;".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;I)Ljava/lang/Byte;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 40, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 41, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2a,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 43, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 41, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2c,
                    ] },
                ] },
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 39, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x1b, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 36, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb8, 0x00, 0x0d, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 47, descriptor_index: 48, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 49, descriptor_index: 50, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x93, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 51, descriptor_index: 52, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 53, descriptor_index: 54, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x85, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 55, descriptor_index: 56, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x86, 0xae,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 57, descriptor_index: 58, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x87, 0xaf,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 59, descriptor_index: 60, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x11, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 61, descriptor_index: 52, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 62, descriptor_index: 63, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x15, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x07, 0x2a, 0xb4, 0x00, 0x07, 0xa0, 0x00,
                        0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 15, descriptor_index: 16, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x10, 0x0a, 0xb8, 0x00, 0x17, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 15, descriptor_index: 25, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1b, 0xb8, 0x00, 0x1a, 0x3d, 0x1c, 0x10, 0x80, 0xa1, 0x00, 0x09, 0x1c, 0x10, 0x7f, 0xa4, 0x00, 0x0c, 0xbb, 0x00,
                        0x20, 0x59, 0x2a, 0xb7, 0x00, 0x22, 0xbf, 0x1c, 0x91, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 21, descriptor_index: 65, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 3, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0xb8, 0x00, 0x0d, 0xb7, 0x00, 0x25, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 21, descriptor_index: 66, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 4, max_locals: 2, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0x1b, 0xb8, 0x00, 0x17, 0xb7, 0x00, 0x25, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}

pub(super) fn append_integer_types(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/lang/Integer
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/lang/Number".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 12 }),
                Some(Constant::Utf8("java/lang/Integer".to_owned())),
                Some(Constant::Utf8("value".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 16 }),
                Some(Constant::Utf8("parseInt".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)I".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 18 }),
                Some(Constant::NameAndType { name_index: 19, descriptor_index: 20 }),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 22 }),
                Some(Constant::NameAndType { name_index: 19, descriptor_index: 23 }),
                Some(Constant::Utf8("(II)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 25 }),
                Some(Constant::NameAndType { name_index: 26, descriptor_index: 23 }),
                Some(Constant::Utf8("toUnsigned".to_owned())),
                Some(Constant::Methodref { class_index: 28, name_and_type_index: 29 }),
                Some(Constant::Class { name_index: 30 }),
                Some(Constant::NameAndType { name_index: 31, descriptor_index: 32 }),
                Some(Constant::Utf8("java/lang/Character".to_owned())),
                Some(Constant::Utf8("forDigit".to_owned())),
                Some(Constant::Utf8("(II)C".to_owned())),
                Some(Constant::Methodref { class_index: 34, name_and_type_index: 35 }),
                Some(Constant::Class { name_index: 36 }),
                Some(Constant::NameAndType { name_index: 37, descriptor_index: 38 }),
                Some(Constant::Utf8("java/lang/String".to_owned())),
                Some(Constant::Utf8("valueOf".to_owned())),
                Some(Constant::Utf8("([CII)Ljava/lang/String;".to_owned())),
                Some(Constant::String { string_index: 40 }),
                Some(Constant::Utf8("0".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 42 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 43 }),
                Some(Constant::Utf8("(Ljava/lang/String;I)I".to_owned())),
                Some(Constant::Class { name_index: 45 }),
                Some(Constant::Utf8("java/lang/NumberFormatException".to_owned())),
                Some(Constant::String { string_index: 47 }),
                Some(Constant::Utf8("null".to_owned())),
                Some(Constant::Methodref { class_index: 44, name_and_type_index: 49 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 50 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Methodref { class_index: 34, name_and_type_index: 52 }),
                Some(Constant::NameAndType { name_index: 53, descriptor_index: 54 }),
                Some(Constant::Utf8("length".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Integer(-2147483647)),
                Some(Constant::Methodref { class_index: 34, name_and_type_index: 57 }),
                Some(Constant::NameAndType { name_index: 58, descriptor_index: 59 }),
                Some(Constant::Utf8("charAt".to_owned())),
                Some(Constant::Utf8("(I)C".to_owned())),
                Some(Constant::Integer(-2147483648)),
                Some(Constant::Methodref { class_index: 28, name_and_type_index: 62 }),
                Some(Constant::NameAndType { name_index: 63, descriptor_index: 64 }),
                Some(Constant::Utf8("digit".to_owned())),
                Some(Constant::Utf8("(CI)I".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 66 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 67 }),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Utf8("MIN_VALUE".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Utf8("MAX_VALUE".to_owned())),
                Some(Constant::Integer(2147483647)),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("byteValue".to_owned())),
                Some(Constant::Utf8("()B".to_owned())),
                Some(Constant::Utf8("shortValue".to_owned())),
                Some(Constant::Utf8("()S".to_owned())),
                Some(Constant::Utf8("intValue".to_owned())),
                Some(Constant::Utf8("longValue".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Utf8("floatValue".to_owned())),
                Some(Constant::Utf8("()F".to_owned())),
                Some(Constant::Utf8("doubleValue".to_owned())),
                Some(Constant::Utf8("()D".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("toHexString".to_owned())),
                Some(Constant::Utf8("toOctalString".to_owned())),
                Some(Constant::Utf8("toBinaryString".to_owned())),
                Some(Constant::Class { name_index: 94 }),
                Some(Constant::Utf8("[C".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/Integer;".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;I)Ljava/lang/Integer;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 68, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 69, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x3c,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 70, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 69, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x47,
                    ] },
                ] },
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 67, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x1b, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 50, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb8, 0x00, 0x0d, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 74, descriptor_index: 75, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x91, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 76, descriptor_index: 77, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x93, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 78, descriptor_index: 54, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 79, descriptor_index: 80, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x85, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 81, descriptor_index: 82, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x86, 0xae,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 83, descriptor_index: 84, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x87, 0xaf,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 19, descriptor_index: 85, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x11, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 86, descriptor_index: 54, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 87, descriptor_index: 88, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x15, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x07, 0x2a, 0xb4, 0x00, 0x07, 0xa0, 0x00,
                        0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 19, descriptor_index: 20, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x1a, 0x10, 0x0a, 0xb8, 0x00, 0x15, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 90, descriptor_index: 20, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x1a, 0x07, 0xb8, 0x00, 0x18, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 91, descriptor_index: 20, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x1a, 0x06, 0xb8, 0x00, 0x18, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 92, descriptor_index: 20, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x1a, 0x04, 0xb8, 0x00, 0x18, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 10, name_index: 26, descriptor_index: 23, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 4, max_locals: 5, code: vec![
                        0x10, 0x20, 0xbc, 0x05, 0x4d, 0x10, 0x20, 0x3e, 0x04, 0x1b, 0x78, 0x04, 0x64, 0x36, 0x04, 0x2c, 0x84, 0x03, 0xff, 0x1d,
                        0x1a, 0x15, 0x04, 0x7e, 0x10, 0x24, 0xb8, 0x00, 0x1b, 0x55, 0x1a, 0x1b, 0x7c, 0x3b, 0x1a, 0x9a, 0xff, 0xec, 0x2c, 0x1d,
                        0x10, 0x20, 0x1d, 0x64, 0xb8, 0x00, 0x21, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 19, descriptor_index: 23, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 4, max_locals: 5, code: vec![
                        0x1b, 0x05, 0xa1, 0x00, 0x09, 0x1b, 0x10, 0x24, 0xa4, 0x00, 0x06, 0x10, 0x0a, 0x3c, 0x1a, 0x9a, 0x00, 0x06, 0x12, 0x27,
                        0xb0, 0x10, 0x21, 0xbc, 0x05, 0x4d, 0x10, 0x21, 0x3e, 0x1a, 0x9c, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0x36, 0x04,
                        0x15, 0x04, 0x9a, 0x00, 0x06, 0x1a, 0x74, 0x3b, 0x1a, 0x99, 0x00, 0x18, 0x2c, 0x84, 0x03, 0xff, 0x1d, 0x1a, 0x1b, 0x70,
                        0x74, 0x1b, 0xb8, 0x00, 0x1b, 0x55, 0x1a, 0x1b, 0x6c, 0x3b, 0xa7, 0xff, 0xea, 0x15, 0x04, 0x99, 0x00, 0x0b, 0x2c, 0x84,
                        0x03, 0xff, 0x1d, 0x10, 0x2d, 0x55, 0x2c, 0x1d, 0x10, 0x21, 0x1d, 0x64, 0xb8, 0x00, 0x21, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 15, descriptor_index: 16, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x10, 0x0a, 0xb8, 0x00, 0x29, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 15, descriptor_index: 43, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 3, max_locals: 9, code: vec![
                        0x2a, 0xc7, 0x00, 0x0d, 0xbb, 0x00, 0x2c, 0x59, 0x12, 0x2e, 0xb7, 0x00, 0x30, 0xbf, 0x1b, 0x05, 0xa1, 0x00, 0x10, 0x1b,
                        0x10, 0x24, 0xa3, 0x00, 0x0a, 0x2a, 0xb6, 0x00, 0x33, 0x9a, 0x00, 0x0c, 0xbb, 0x00, 0x2c, 0x59, 0x2a, 0xb7, 0x00, 0x30,
                        0xbf, 0x03, 0x3d, 0x03, 0x3e, 0x12, 0x37, 0x36, 0x04, 0x2a, 0x03, 0xb6, 0x00, 0x38, 0x36, 0x05, 0x15, 0x05, 0x10, 0x2d,
                        // CLDC accepts a leading minus only; '+' reaches digit validation.
                        0xa0, 0x00, 0x2c, 0x15, 0x05, 0x10, 0x2d, 0xa0, 0x00, 0x07, 0x04, 0xa7, 0x00,
                        0x04, 0x03, 0x3e, 0x1d, 0x99, 0x00, 0x07, 0x12, 0x3c, 0x36, 0x04, 0x2a, 0xb6, 0x00, 0x33, 0x04, 0xa0, 0x00, 0x0c, 0xbb,
                        0x00, 0x2c, 0x59, 0x2a, 0xb7, 0x00, 0x30, 0xbf, 0x84, 0x02, 0x01, 0x03, 0x36, 0x06, 0x15, 0x04, 0x1b, 0x6c, 0x36, 0x07,
                        0x1c, 0x2a, 0xb6, 0x00, 0x33, 0xa2, 0x00, 0x49, 0x2a, 0x1c, 0x84, 0x02, 0x01, 0xb6, 0x00, 0x38, 0x1b, 0xb8, 0x00, 0x3d,
                        0x36, 0x08, 0x15, 0x08, 0x9b, 0x00, 0x0a, 0x15, 0x06, 0x15, 0x07, 0xa2, 0x00, 0x0c, 0xbb, 0x00, 0x2c, 0x59, 0x2a, 0xb7,
                        0x00, 0x30, 0xbf, 0x15, 0x06, 0x1b, 0x68, 0x36, 0x06, 0x15, 0x06, 0x15, 0x04, 0x15, 0x08, 0x60, 0xa2, 0x00, 0x0c, 0xbb,
                        0x00, 0x2c, 0x59, 0x2a, 0xb7, 0x00, 0x30, 0xbf, 0x15, 0x06, 0x15, 0x08, 0x64, 0x36, 0x06, 0xa7, 0xff, 0xb5, 0x1d, 0x99,
                        0x00, 0x08, 0x15, 0x06, 0xa7, 0x00, 0x06, 0x15, 0x06, 0x74, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 37, descriptor_index: 95, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 3, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0xb8, 0x00, 0x0d, 0xb7, 0x00, 0x41, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 37, descriptor_index: 96, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 4, max_locals: 2, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0x1b, 0xb8, 0x00, 0x29, 0xb7, 0x00, 0x41, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // java/lang/Long
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/lang/Number".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 12 }),
                Some(Constant::Utf8("java/lang/Long".to_owned())),
                Some(Constant::Utf8("value".to_owned())),
                Some(Constant::Utf8("J".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 16 }),
                Some(Constant::Utf8("parseLong".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)J".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 18 }),
                Some(Constant::NameAndType { name_index: 19, descriptor_index: 20 }),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("(J)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 22 }),
                Some(Constant::NameAndType { name_index: 19, descriptor_index: 23 }),
                Some(Constant::Utf8("(JI)Ljava/lang/String;".to_owned())),
                Some(Constant::String { string_index: 25 }),
                Some(Constant::Utf8("0".to_owned())),
                Some(Constant::Methodref { class_index: 27, name_and_type_index: 28 }),
                Some(Constant::Class { name_index: 29 }),
                Some(Constant::NameAndType { name_index: 30, descriptor_index: 31 }),
                Some(Constant::Utf8("java/lang/Character".to_owned())),
                Some(Constant::Utf8("forDigit".to_owned())),
                Some(Constant::Utf8("(II)C".to_owned())),
                Some(Constant::Methodref { class_index: 33, name_and_type_index: 34 }),
                Some(Constant::Class { name_index: 35 }),
                Some(Constant::NameAndType { name_index: 36, descriptor_index: 37 }),
                Some(Constant::Utf8("java/lang/String".to_owned())),
                Some(Constant::Utf8("valueOf".to_owned())),
                Some(Constant::Utf8("([CII)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 39 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 40 }),
                Some(Constant::Utf8("(Ljava/lang/String;I)J".to_owned())),
                Some(Constant::Class { name_index: 42 }),
                Some(Constant::Utf8("java/lang/NumberFormatException".to_owned())),
                Some(Constant::String { string_index: 44 }),
                Some(Constant::Utf8("null".to_owned())),
                Some(Constant::Methodref { class_index: 41, name_and_type_index: 46 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 47 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Methodref { class_index: 33, name_and_type_index: 49 }),
                Some(Constant::NameAndType { name_index: 50, descriptor_index: 51 }),
                Some(Constant::Utf8("length".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Long(-9223372036854775807)),
                None,
                Some(Constant::Methodref { class_index: 33, name_and_type_index: 55 }),
                Some(Constant::NameAndType { name_index: 56, descriptor_index: 57 }),
                Some(Constant::Utf8("charAt".to_owned())),
                Some(Constant::Utf8("(I)C".to_owned())),
                Some(Constant::Long(-9223372036854775808)),
                None,
                Some(Constant::Methodref { class_index: 27, name_and_type_index: 61 }),
                Some(Constant::NameAndType { name_index: 62, descriptor_index: 63 }),
                Some(Constant::Utf8("digit".to_owned())),
                Some(Constant::Utf8("(CI)I".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 65 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 66 }),
                Some(Constant::Utf8("(J)V".to_owned())),
                Some(Constant::Utf8("MIN_VALUE".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Utf8("MAX_VALUE".to_owned())),
                Some(Constant::Long(9223372036854775807)),
                None,
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("byteValue".to_owned())),
                Some(Constant::Utf8("()B".to_owned())),
                Some(Constant::Utf8("shortValue".to_owned())),
                Some(Constant::Utf8("()S".to_owned())),
                Some(Constant::Utf8("intValue".to_owned())),
                Some(Constant::Utf8("longValue".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Utf8("floatValue".to_owned())),
                Some(Constant::Utf8("()F".to_owned())),
                Some(Constant::Utf8("doubleValue".to_owned())),
                Some(Constant::Utf8("()D".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Class { name_index: 91 }),
                Some(Constant::Utf8("[C".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/Long;".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;I)Ljava/lang/Long;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 67, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 68, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x3a,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 69, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 68, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x46,
                    ] },
                ] },
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 66, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x1f, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 47, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb8, 0x00, 0x0d, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 74, descriptor_index: 75, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x88, 0x91, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 76, descriptor_index: 77, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x88, 0x93, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 78, descriptor_index: 51, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x88, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 79, descriptor_index: 80, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 81, descriptor_index: 82, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x89, 0xae,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 83, descriptor_index: 84, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x8a, 0xaf,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 19, descriptor_index: 85, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x11, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 86, descriptor_index: 51, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 5, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x2a, 0xb4, 0x00, 0x07, 0x10, 0x20, 0x7d, 0x83, 0x88, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 87, descriptor_index: 88, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 4, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x16, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x07, 0x2a, 0xb4, 0x00, 0x07, 0x94, 0x9a,
                        0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 19, descriptor_index: 20, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 3, max_locals: 2, code: vec![
                        0x1e, 0x10, 0x0a, 0xb8, 0x00, 0x15, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 19, descriptor_index: 23, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 6, max_locals: 6, code: vec![
                        0x1c, 0x05, 0xa1, 0x00, 0x09, 0x1c, 0x10, 0x24, 0xa4, 0x00, 0x06, 0x10, 0x0a, 0x3d, 0x1e, 0x09, 0x94, 0x9a, 0x00, 0x06,
                        0x12, 0x18, 0xb0, 0x10, 0x41, 0xbc, 0x05, 0x4e, 0x10, 0x41, 0x36, 0x04, 0x1e, 0x09, 0x94, 0x9c, 0x00, 0x07, 0x04, 0xa7,
                        0x00, 0x04, 0x03, 0x36, 0x05, 0x15, 0x05, 0x9a, 0x00, 0x06, 0x1e, 0x75, 0x3f, 0x1e, 0x09, 0x94, 0x99, 0x00, 0x1c, 0x2d,
                        0x84, 0x04, 0xff, 0x15, 0x04, 0x1e, 0x1c, 0x85, 0x71, 0x75, 0x88, 0x1c, 0xb8, 0x00, 0x1a, 0x55, 0x1e, 0x1c, 0x85, 0x6d,
                        0x3f, 0xa7, 0xff, 0xe4, 0x15, 0x05, 0x99, 0x00, 0x0c, 0x2d, 0x84, 0x04, 0xff, 0x15, 0x04, 0x10, 0x2d, 0x55, 0x2d, 0x15,
                        0x04, 0x10, 0x41, 0x15, 0x04, 0x64, 0xb8, 0x00, 0x20, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 15, descriptor_index: 16, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x10, 0x0a, 0xb8, 0x00, 0x26, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 15, descriptor_index: 40, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 6, max_locals: 12, code: vec![
                        0x2a, 0xc7, 0x00, 0x0d, 0xbb, 0x00, 0x29, 0x59, 0x12, 0x2b, 0xb7, 0x00, 0x2d, 0xbf, 0x1b, 0x05, 0xa1, 0x00, 0x10, 0x1b,
                        0x10, 0x24, 0xa3, 0x00, 0x0a, 0x2a, 0xb6, 0x00, 0x30, 0x9a, 0x00, 0x0c, 0xbb, 0x00, 0x29, 0x59, 0x2a, 0xb7, 0x00, 0x2d,
                        0xbf, 0x03, 0x3d, 0x03, 0x3e, 0x14, 0x00, 0x34, 0x37, 0x04, 0x2a, 0x03, 0xb6, 0x00, 0x36, 0x36, 0x06, 0x15, 0x06, 0x10,
                        // CLDC accepts a leading minus only; '+' reaches digit validation.
                        0x2d, 0xa0, 0x00, 0x2d, 0x15, 0x06, 0x10, 0x2d, 0xa0, 0x00, 0x07, 0x04, 0xa7,
                        0x00, 0x04, 0x03, 0x3e, 0x1d, 0x99, 0x00, 0x08, 0x14, 0x00, 0x3a, 0x37, 0x04, 0x2a, 0xb6, 0x00, 0x30, 0x04, 0xa0, 0x00,
                        0x0c, 0xbb, 0x00, 0x29, 0x59, 0x2a, 0xb7, 0x00, 0x2d, 0xbf, 0x84, 0x02, 0x01, 0x09, 0x37, 0x07, 0x16, 0x04, 0x1b, 0x85,
                        0x6d, 0x37, 0x09, 0x1c, 0x2a, 0xb6, 0x00, 0x30, 0xa2, 0x00, 0x4e, 0x2a, 0x1c, 0x84, 0x02, 0x01, 0xb6, 0x00, 0x36, 0x1b,
                        0xb8, 0x00, 0x3c, 0x36, 0x0b, 0x15, 0x0b, 0x9b, 0x00, 0x0b, 0x16, 0x07, 0x16, 0x09, 0x94, 0x9c, 0x00, 0x0c, 0xbb, 0x00,
                        0x29, 0x59, 0x2a, 0xb7, 0x00, 0x2d, 0xbf, 0x16, 0x07, 0x1b, 0x85, 0x69, 0x37, 0x07, 0x16, 0x07, 0x16, 0x04, 0x15, 0x0b,
                        0x85, 0x61, 0x94, 0x9c, 0x00, 0x0c, 0xbb, 0x00, 0x29, 0x59, 0x2a, 0xb7, 0x00, 0x2d, 0xbf, 0x16, 0x07, 0x15, 0x0b, 0x85,
                        0x65, 0x37, 0x07, 0xa7, 0xff, 0xb0, 0x1d, 0x99, 0x00, 0x08, 0x16, 0x07, 0xa7, 0x00, 0x06, 0x16, 0x07, 0x75, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 36, descriptor_index: 92, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 4, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0xb8, 0x00, 0x0d, 0xb7, 0x00, 0x40, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 36, descriptor_index: 93, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 72, max_stack: 4, max_locals: 2, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0x1b, 0xb8, 0x00, 0x26, 0xb7, 0x00, 0x40, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // java/lang/Short
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/lang/Number".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 12 }),
                Some(Constant::Utf8("java/lang/Short".to_owned())),
                Some(Constant::Utf8("value".to_owned())),
                Some(Constant::Utf8("S".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 16 }),
                Some(Constant::Utf8("parseShort".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)S".to_owned())),
                Some(Constant::Methodref { class_index: 18, name_and_type_index: 19 }),
                Some(Constant::Class { name_index: 20 }),
                Some(Constant::NameAndType { name_index: 21, descriptor_index: 22 }),
                Some(Constant::Utf8("java/lang/String".to_owned())),
                Some(Constant::Utf8("valueOf".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 24 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 25 }),
                Some(Constant::Utf8("(Ljava/lang/String;I)S".to_owned())),
                Some(Constant::Methodref { class_index: 27, name_and_type_index: 28 }),
                Some(Constant::Class { name_index: 29 }),
                Some(Constant::NameAndType { name_index: 30, descriptor_index: 31 }),
                Some(Constant::Utf8("java/lang/Integer".to_owned())),
                Some(Constant::Utf8("parseInt".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;I)I".to_owned())),
                Some(Constant::Class { name_index: 33 }),
                Some(Constant::Utf8("java/lang/NumberFormatException".to_owned())),
                Some(Constant::Methodref { class_index: 32, name_and_type_index: 35 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 36 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 38 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 39 }),
                Some(Constant::Utf8("(S)V".to_owned())),
                Some(Constant::Utf8("MIN_VALUE".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Integer(-32768)),
                Some(Constant::Utf8("MAX_VALUE".to_owned())),
                Some(Constant::Integer(32767)),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("byteValue".to_owned())),
                Some(Constant::Utf8("()B".to_owned())),
                Some(Constant::Utf8("shortValue".to_owned())),
                Some(Constant::Utf8("()S".to_owned())),
                Some(Constant::Utf8("intValue".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("longValue".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Utf8("floatValue".to_owned())),
                Some(Constant::Utf8("()F".to_owned())),
                Some(Constant::Utf8("doubleValue".to_owned())),
                Some(Constant::Utf8("()D".to_owned())),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/Short;".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;I)Ljava/lang/Short;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 40, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 41, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2a,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 43, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 41, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2c,
                    ] },
                ] },
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 39, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x1b, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 36, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb8, 0x00, 0x0d, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 47, descriptor_index: 48, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x91, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 49, descriptor_index: 50, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 51, descriptor_index: 52, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 53, descriptor_index: 54, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x85, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 55, descriptor_index: 56, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x86, 0xae,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 57, descriptor_index: 58, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x87, 0xaf,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 59, descriptor_index: 60, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x11, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 61, descriptor_index: 52, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 62, descriptor_index: 63, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x15, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x07, 0x2a, 0xb4, 0x00, 0x07, 0xa0, 0x00,
                        0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 15, descriptor_index: 16, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x10, 0x0a, 0xb8, 0x00, 0x17, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 15, descriptor_index: 25, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1b, 0xb8, 0x00, 0x1a, 0x3d, 0x1c, 0x11, 0x80, 0x00, 0xa1, 0x00, 0x0a, 0x1c, 0x11, 0x7f, 0xff, 0xa4, 0x00, 0x0c,
                        0xbb, 0x00, 0x20, 0x59, 0x2a, 0xb7, 0x00, 0x22, 0xbf, 0x1c, 0x93, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 21, descriptor_index: 65, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 3, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0xb8, 0x00, 0x0d, 0xb7, 0x00, 0x25, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 21, descriptor_index: 66, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 45, max_stack: 4, max_locals: 2, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0x1b, 0xb8, 0x00, 0x17, 0xb7, 0x00, 0x25, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
