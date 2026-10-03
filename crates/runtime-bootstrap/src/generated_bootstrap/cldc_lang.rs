//! Ordered `java.lang` inventory and boolean/character values.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

mod floating_point;
mod integers;
mod string;
mod string_buffer;
mod thread;

pub(super) fn append_generated_cldc_lang(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/lang/Boolean
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
                Some(Constant::Utf8("java/lang/Boolean".to_owned())),
                Some(Constant::Utf8("value".to_owned())),
                Some(Constant::Utf8("Z".to_owned())),
                Some(Constant::String { string_index: 14 }),
                Some(Constant::Utf8("true".to_owned())),
                Some(Constant::Methodref { class_index: 16, name_and_type_index: 17 }),
                Some(Constant::Class { name_index: 18 }),
                Some(Constant::NameAndType { name_index: 19, descriptor_index: 20 }),
                Some(Constant::Utf8("java/lang/String".to_owned())),
                Some(Constant::Utf8("equalsIgnoreCase".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Z".to_owned())),
                Some(Constant::String { string_index: 22 }),
                Some(Constant::Utf8("false".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 24 }),
                Some(Constant::NameAndType { name_index: 25, descriptor_index: 26 }),
                Some(Constant::Utf8("TRUE".to_owned())),
                Some(Constant::Utf8("Ljava/lang/Boolean;".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 28 }),
                Some(Constant::NameAndType { name_index: 29, descriptor_index: 26 }),
                Some(Constant::Utf8("FALSE".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 31 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 32 }),
                Some(Constant::Utf8("(Z)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("booleanValue".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("valueOf".to_owned())),
                Some(Constant::Utf8("(Z)Ljava/lang/Boolean;".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/Boolean;".to_owned())),
                Some(Constant::Utf8("(Z)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("<clinit>".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 25, descriptor_index: 26, attributes: Vec::new() },
                Member { access_flags: 25, name_index: 29, descriptor_index: 26, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 32, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x1b, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 35, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xc6, 0x00, 0x10, 0x2b, 0x12, 0x0d, 0xb6, 0x00, 0x0f, 0x99, 0x00, 0x07, 0x04, 0xa7,
                        0x00, 0x04, 0x03, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 37, descriptor_index: 38, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 39, descriptor_index: 40, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x99, 0x00, 0x08, 0x12, 0x0d, 0xa7, 0x00, 0x05, 0x12, 0x15, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 41, descriptor_index: 42, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x99, 0x00, 0x09, 0x11, 0x04, 0xcf, 0xa7, 0x00, 0x06, 0x11, 0x04, 0xd5, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 43, descriptor_index: 44, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 2, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x15, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x07, 0x2a, 0xb4, 0x00, 0x07, 0xa0, 0x00,
                        0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 45, descriptor_index: 46, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 1, max_locals: 1, code: vec![
                        0x1a, 0x99, 0x00, 0x09, 0xb2, 0x00, 0x17, 0xa7, 0x00, 0x06, 0xb2, 0x00, 0x1b, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 45, descriptor_index: 47, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xc6, 0x00, 0x12, 0x2a, 0x12, 0x0d, 0xb6, 0x00, 0x0f, 0x99, 0x00, 0x09, 0xb2, 0x00, 0x17, 0xa7, 0x00, 0x06, 0xb2,
                        0x00, 0x1b, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 39, descriptor_index: 48, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 1, max_locals: 1, code: vec![
                        0x1a, 0x99, 0x00, 0x08, 0x12, 0x0d, 0xa7, 0x00, 0x05, 0x12, 0x15, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 8, name_index: 49, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 33, max_stack: 3, max_locals: 0, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x04, 0xb7, 0x00, 0x1e, 0xb3, 0x00, 0x17, 0xbb, 0x00, 0x08, 0x59, 0x03, 0xb7, 0x00, 0x1e, 0xb3,
                        0x00, 0x1b, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
    integers::append_byte(classes);
    append_bootstrap_classes!(classes;
        // java/lang/Character
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
                Some(Constant::Utf8("java/lang/Character".to_owned())),
                Some(Constant::Utf8("value".to_owned())),
                Some(Constant::Utf8("C".to_owned())),
                Some(Constant::Methodref { class_index: 14, name_and_type_index: 15 }),
                Some(Constant::Class { name_index: 16 }),
                Some(Constant::NameAndType { name_index: 17, descriptor_index: 18 }),
                Some(Constant::Utf8("java/lang/String".to_owned())),
                Some(Constant::Utf8("valueOf".to_owned())),
                Some(Constant::Utf8("(C)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("MIN_VALUE".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Integer(0)),
                Some(Constant::Utf8("MAX_VALUE".to_owned())),
                Some(Constant::Integer(65535)),
                Some(Constant::Utf8("MIN_RADIX".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Integer(2)),
                Some(Constant::Utf8("MAX_RADIX".to_owned())),
                Some(Constant::Integer(36)),
                Some(Constant::Utf8("(C)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("charValue".to_owned())),
                Some(Constant::Utf8("()C".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("digit".to_owned())),
                Some(Constant::Utf8("(CI)I".to_owned())),
                Some(Constant::Utf8("forDigit".to_owned())),
                Some(Constant::Utf8("(II)C".to_owned())),
                Some(Constant::Utf8("isDigit".to_owned())),
                Some(Constant::Utf8("(C)Z".to_owned())),
                Some(Constant::Utf8("isLowerCase".to_owned())),
                Some(Constant::Utf8("isUpperCase".to_owned())),
                Some(Constant::Utf8("toLowerCase".to_owned())),
                Some(Constant::Utf8("(C)C".to_owned())),
                Some(Constant::Utf8("toUpperCase".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 19, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 20, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x15,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 22, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 20, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x17,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 24, descriptor_index: 25, attributes: vec![
                    Attribute::Raw { name_index: 20, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x1a,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 27, descriptor_index: 25, attributes: vec![
                    Attribute::Raw { name_index: 20, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x1c,
                    ] },
                ] },
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 29, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x1b, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 32, descriptor_index: 33, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 34, descriptor_index: 35, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 36, descriptor_index: 37, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 2, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x15, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x07, 0x2a, 0xb4, 0x00, 0x07, 0xa0, 0x00,
                        0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 39, descriptor_index: 40, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x0d, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 265, name_index: 41, descriptor_index: 42, attributes: Vec::new() },
                Member { access_flags: 9, name_index: 43, descriptor_index: 44, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 2, max_locals: 2, code: vec![
                        0x1a, 0x9b, 0x00, 0x13, 0x1a, 0x1b, 0xa2, 0x00, 0x0e, 0x1b, 0x05, 0xa1, 0x00, 0x09, 0x1b, 0x10, 0x24, 0xa4, 0x00, 0x05,
                        0x03, 0xac, 0x1a, 0x10, 0x0a, 0xa2, 0x00, 0x0a, 0x10, 0x30, 0x1a, 0x60, 0xa7, 0x00, 0x0a, 0x10, 0x61, 0x1a, 0x60, 0x10,
                        0x0a, 0x64, 0x92, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 265, name_index: 45, descriptor_index: 46, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 47, descriptor_index: 46, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 48, descriptor_index: 46, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 49, descriptor_index: 50, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 51, descriptor_index: 50, attributes: Vec::new() },
            ],
            attributes: Vec::new(),
        },
    );
    floating_point::append_floating_point(classes);
    integers::append_integer_types(classes);
    string::append_string(classes);
    string_buffer::append_string_buffer(classes);
    thread::append_thread(classes);
}
