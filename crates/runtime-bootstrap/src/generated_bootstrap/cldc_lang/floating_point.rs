//! Boxed floating-point values and conversions.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

pub(super) fn append_floating_point(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/lang/Double
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
                Some(Constant::Utf8("java/lang/Double".to_owned())),
                Some(Constant::Utf8("value".to_owned())),
                Some(Constant::Utf8("D".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 16 }),
                Some(Constant::Utf8("parseDouble".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)D".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 18 }),
                Some(Constant::NameAndType { name_index: 19, descriptor_index: 20 }),
                Some(Constant::Utf8("isNaN".to_owned())),
                Some(Constant::Utf8("(D)Z".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 22 }),
                Some(Constant::NameAndType { name_index: 23, descriptor_index: 20 }),
                Some(Constant::Utf8("isInfinite".to_owned())),
                Some(Constant::Double(9218868437227405312)),
                None,
                Some(Constant::Double(18442240474082181120)),
                None,
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 29 }),
                Some(Constant::NameAndType { name_index: 30, descriptor_index: 31 }),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("(D)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 33 }),
                Some(Constant::NameAndType { name_index: 34, descriptor_index: 35 }),
                Some(Constant::Utf8("doubleToLongBits".to_owned())),
                Some(Constant::Utf8("(D)J".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 37 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 38 }),
                Some(Constant::Utf8("(D)V".to_owned())),
                Some(Constant::Utf8("POSITIVE_INFINITY".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Utf8("NEGATIVE_INFINITY".to_owned())),
                Some(Constant::Utf8("NaN".to_owned())),
                Some(Constant::Double(9221120237041090560)),
                None,
                Some(Constant::Utf8("MAX_VALUE".to_owned())),
                Some(Constant::Double(9218868437227405311)),
                None,
                Some(Constant::Utf8("MIN_VALUE".to_owned())),
                Some(Constant::Double(1)),
                None,
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
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
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("longBitsToDouble".to_owned())),
                Some(Constant::Utf8("(J)D".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("valueOf".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/Double;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 39, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 40, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x18,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 41, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 40, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x1a,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 42, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 40, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2b,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 45, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 40, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2e,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 48, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 40, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x31,
                    ] },
                ] },
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 38, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x27, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 53, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb8, 0x00, 0x0d, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 54, descriptor_index: 55, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x8e, 0x91, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 56, descriptor_index: 57, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x8e, 0x93, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 58, descriptor_index: 59, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x8e, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 60, descriptor_index: 61, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x8f, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 62, descriptor_index: 63, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x90, 0xae,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 64, descriptor_index: 65, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xaf,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 19, descriptor_index: 66, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x11, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 19, descriptor_index: 20, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 4, max_locals: 2, code: vec![
                        0x26, 0x26, 0x97, 0x99, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 23, descriptor_index: 66, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x15, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 23, descriptor_index: 20, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 4, max_locals: 2, code: vec![
                        0x26, 0x14, 0x00, 0x18, 0x97, 0x99, 0x00, 0x0b, 0x26, 0x14, 0x00, 0x1a, 0x97, 0x9a, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04,
                        0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 30, descriptor_index: 68, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x1c, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 265, name_index: 30, descriptor_index: 31, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 15, descriptor_index: 16, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 34, descriptor_index: 35, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 69, descriptor_index: 70, attributes: Vec::new() },
                Member { access_flags: 1, name_index: 71, descriptor_index: 59, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 5, max_locals: 3, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x20, 0x40, 0x1f, 0x1f, 0x10, 0x20, 0x7d, 0x83, 0x88, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 72, descriptor_index: 73, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 4, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x1c, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x20, 0x2a, 0xb4, 0x00,
                        0x07, 0xb8, 0x00, 0x20, 0x94, 0x9a, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 74, descriptor_index: 75, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 51, max_stack: 4, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0xb8, 0x00, 0x0d, 0xb7, 0x00, 0x24, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // java/lang/Float
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
                Some(Constant::Utf8("java/lang/Float".to_owned())),
                Some(Constant::Utf8("value".to_owned())),
                Some(Constant::Utf8("F".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 16 }),
                Some(Constant::Utf8("parseFloat".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)F".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 18 }),
                Some(Constant::NameAndType { name_index: 19, descriptor_index: 20 }),
                Some(Constant::Utf8("isNaN".to_owned())),
                Some(Constant::Utf8("(F)Z".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 22 }),
                Some(Constant::NameAndType { name_index: 23, descriptor_index: 20 }),
                Some(Constant::Utf8("isInfinite".to_owned())),
                Some(Constant::Float(2139095040)),
                Some(Constant::Float(4286578688)),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 27 }),
                Some(Constant::NameAndType { name_index: 28, descriptor_index: 29 }),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("(F)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 31 }),
                Some(Constant::NameAndType { name_index: 32, descriptor_index: 33 }),
                Some(Constant::Utf8("floatToIntBits".to_owned())),
                Some(Constant::Utf8("(F)I".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 35 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 36 }),
                Some(Constant::Utf8("(F)V".to_owned())),
                Some(Constant::Utf8("POSITIVE_INFINITY".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Utf8("NEGATIVE_INFINITY".to_owned())),
                Some(Constant::Utf8("NaN".to_owned())),
                Some(Constant::Float(2143289344)),
                Some(Constant::Utf8("MAX_VALUE".to_owned())),
                Some(Constant::Float(2139095039)),
                Some(Constant::Utf8("MIN_VALUE".to_owned())),
                Some(Constant::Float(1)),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("(D)V".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
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
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("intBitsToFloat".to_owned())),
                Some(Constant::Utf8("(I)F".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("valueOf".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/Float;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 37, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 38, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x18,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 39, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 38, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x19,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 40, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 38, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x29,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 42, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 38, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2b,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 44, descriptor_index: 12, attributes: vec![
                    Attribute::Raw { name_index: 38, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2d,
                    ] },
                ] },
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 36, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x23, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 48, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x27, 0x90, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 49, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb8, 0x00, 0x0d, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 50, descriptor_index: 51, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x8b, 0x91, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 52, descriptor_index: 53, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x8b, 0x93, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 54, descriptor_index: 55, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x8b, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 56, descriptor_index: 57, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x8c, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 58, descriptor_index: 59, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xae,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 60, descriptor_index: 61, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0x8d, 0xaf,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 19, descriptor_index: 62, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x11, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 19, descriptor_index: 20, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 2, max_locals: 1, code: vec![
                        0x22, 0x22, 0x95, 0x99, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 23, descriptor_index: 62, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x15, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 23, descriptor_index: 20, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 2, max_locals: 1, code: vec![
                        0x22, 0x12, 0x18, 0x95, 0x99, 0x00, 0x0a, 0x22, 0x12, 0x19, 0x95, 0x9a, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 28, descriptor_index: 64, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x1a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 265, name_index: 28, descriptor_index: 29, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 15, descriptor_index: 16, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 32, descriptor_index: 33, attributes: Vec::new() },
                Member { access_flags: 265, name_index: 65, descriptor_index: 66, attributes: Vec::new() },
                Member { access_flags: 1, name_index: 67, descriptor_index: 55, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x1e, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 68, descriptor_index: 69, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 2, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x1b, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x07, 0xb8, 0x00, 0x1e, 0x2a, 0xb4, 0x00,
                        0x07, 0xb8, 0x00, 0x1e, 0xa0, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 70, descriptor_index: 71, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 46, max_stack: 3, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0xb8, 0x00, 0x0d, 0xb7, 0x00, 0x22, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
