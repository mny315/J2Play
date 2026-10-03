//! Calendar fields, dates and time zones.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, ExceptionHandler, Member,
    append_bootstrap_classes, build_bootstrap_class,
};

pub(super) fn append_calendar(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/util/Calendar
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/util/TimeZone".to_owned())),
                Some(Constant::Utf8("getDefault".to_owned())),
                Some(Constant::Utf8("()Ljava/util/TimeZone;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 12 }),
                Some(Constant::Utf8("java/util/Calendar".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("(Ljava/util/TimeZone;)V".to_owned())),
                Some(Constant::Methodref { class_index: 14, name_and_type_index: 15 }),
                Some(Constant::Class { name_index: 16 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 17 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 19 }),
                Some(Constant::NameAndType { name_index: 20, descriptor_index: 21 }),
                Some(Constant::Utf8("setFields".to_owned())),
                Some(Constant::Utf8("[Z".to_owned())),
                Some(Constant::Class { name_index: 23 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 15 }),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 26 }),
                Some(Constant::NameAndType { name_index: 27, descriptor_index: 28 }),
                Some(Constant::Utf8("zone".to_owned())),
                Some(Constant::Utf8("Ljava/util/TimeZone;".to_owned())),
                Some(Constant::Methodref { class_index: 30, name_and_type_index: 31 }),
                Some(Constant::Class { name_index: 32 }),
                Some(Constant::NameAndType { name_index: 33, descriptor_index: 34 }),
                Some(Constant::Utf8("java/lang/System".to_owned())),
                Some(Constant::Utf8("currentTimeMillis".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 36 }),
                Some(Constant::NameAndType { name_index: 37, descriptor_index: 38 }),
                Some(Constant::Utf8("millis".to_owned())),
                Some(Constant::Utf8("J".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 40 }),
                Some(Constant::NameAndType { name_index: 41, descriptor_index: 17 }),
                Some(Constant::Utf8("markAllSet".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 15 }),
                Some(Constant::Class { name_index: 44 }),
                Some(Constant::Utf8("java/util/Date".to_owned())),
                Some(Constant::Methodref { class_index: 43, name_and_type_index: 46 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 47 }),
                Some(Constant::Utf8("(J)V".to_owned())),
                Some(Constant::Methodref { class_index: 43, name_and_type_index: 49 }),
                Some(Constant::NameAndType { name_index: 50, descriptor_index: 34 }),
                Some(Constant::Utf8("getTime".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 52 }),
                Some(Constant::NameAndType { name_index: 53, descriptor_index: 34 }),
                Some(Constant::Utf8("toLocalMillis".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 55 }),
                Some(Constant::NameAndType { name_index: 56, descriptor_index: 57 }),
                Some(Constant::Utf8("fieldValue".to_owned())),
                Some(Constant::Utf8("(JI)I".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 59 }),
                Some(Constant::NameAndType { name_index: 60, descriptor_index: 61 }),
                Some(Constant::Utf8("getRawOffset".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Class { name_index: 63 }),
                Some(Constant::Utf8("java/lang/Long".to_owned())),
                Some(Constant::Long(9223372036854775807)),
                None,
                Some(Constant::Long(-9223372036854775808)),
                None,
                Some(Constant::Class { name_index: 69 }),
                Some(Constant::Utf8("java/lang/IllegalArgumentException".to_owned())),
                Some(Constant::String { string_index: 71 }),
                Some(Constant::Utf8("Calendar timezone overflow".to_owned())),
                Some(Constant::Methodref { class_index: 68, name_and_type_index: 73 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 74 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 76 }),
                Some(Constant::NameAndType { name_index: 77, descriptor_index: 78 }),
                Some(Constant::Utf8("replaceField".to_owned())),
                Some(Constant::Utf8("(JII)J".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 80 }),
                Some(Constant::NameAndType { name_index: 81, descriptor_index: 82 }),
                Some(Constant::Utf8("fromLocalMillis".to_owned())),
                Some(Constant::Utf8("(J)J".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 84 }),
                Some(Constant::NameAndType { name_index: 85, descriptor_index: 86 }),
                Some(Constant::Utf8("get".to_owned())),
                Some(Constant::Utf8("(I)I".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 88 }),
                Some(Constant::NameAndType { name_index: 89, descriptor_index: 90 }),
                Some(Constant::Utf8("setDateTime".to_owned())),
                Some(Constant::Utf8("(IIIIII)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 92 }),
                Some(Constant::NameAndType { name_index: 93, descriptor_index: 94 }),
                Some(Constant::Utf8("replaceDateTime".to_owned())),
                Some(Constant::Utf8("(JIIIIII)J".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 96 }),
                Some(Constant::NameAndType { name_index: 97, descriptor_index: 78 }),
                Some(Constant::Utf8("addField".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 99 }),
                Some(Constant::NameAndType { name_index: 100, descriptor_index: 78 }),
                Some(Constant::Utf8("rollField".to_owned())),
                Some(Constant::Class { name_index: 102 }),
                Some(Constant::Utf8("java/lang/ArrayIndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 101, name_and_type_index: 104 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 105 }),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 107 }),
                Some(Constant::NameAndType { name_index: 108, descriptor_index: 109 }),
                Some(Constant::Utf8("set".to_owned())),
                Some(Constant::Utf8("(II)V".to_owned())),
                Some(Constant::Utf8("ERA".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::Integer(0)),
                Some(Constant::Utf8("YEAR".to_owned())),
                Some(Constant::Integer(1)),
                Some(Constant::Utf8("MONTH".to_owned())),
                Some(Constant::Integer(2)),
                Some(Constant::Utf8("WEEK_OF_YEAR".to_owned())),
                Some(Constant::Integer(3)),
                Some(Constant::Utf8("WEEK_OF_MONTH".to_owned())),
                Some(Constant::Integer(4)),
                Some(Constant::Utf8("DATE".to_owned())),
                Some(Constant::Integer(5)),
                Some(Constant::Utf8("DAY_OF_MONTH".to_owned())),
                Some(Constant::Utf8("DAY_OF_YEAR".to_owned())),
                Some(Constant::Integer(6)),
                Some(Constant::Utf8("DAY_OF_WEEK".to_owned())),
                Some(Constant::Integer(7)),
                Some(Constant::Utf8("DAY_OF_WEEK_IN_MONTH".to_owned())),
                Some(Constant::Integer(8)),
                Some(Constant::Utf8("AM_PM".to_owned())),
                Some(Constant::Integer(9)),
                Some(Constant::Utf8("HOUR".to_owned())),
                Some(Constant::Integer(10)),
                Some(Constant::Utf8("HOUR_OF_DAY".to_owned())),
                Some(Constant::Integer(11)),
                Some(Constant::Utf8("MINUTE".to_owned())),
                Some(Constant::Integer(12)),
                Some(Constant::Utf8("SECOND".to_owned())),
                Some(Constant::Integer(13)),
                Some(Constant::Utf8("MILLISECOND".to_owned())),
                Some(Constant::Integer(14)),
                Some(Constant::Utf8("SUNDAY".to_owned())),
                Some(Constant::Utf8("JANUARY".to_owned())),
                Some(Constant::Utf8("AM".to_owned())),
                Some(Constant::Utf8("PM".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("getInstance".to_owned())),
                Some(Constant::Utf8("()Ljava/util/Calendar;".to_owned())),
                Some(Constant::Utf8("(Ljava/util/TimeZone;)Ljava/util/Calendar;".to_owned())),
                Some(Constant::Utf8("()Ljava/util/Date;".to_owned())),
                Some(Constant::Utf8("setTime".to_owned())),
                Some(Constant::Utf8("(Ljava/util/Date;)V".to_owned())),
                Some(Constant::Utf8("getTimeInMillis".to_owned())),
                Some(Constant::Utf8("setTimeInMillis".to_owned())),
                Some(Constant::Utf8("(III)V".to_owned())),
                Some(Constant::Utf8("(IIIII)V".to_owned())),
                Some(Constant::Utf8("add".to_owned())),
                Some(Constant::Utf8("roll".to_owned())),
                Some(Constant::Utf8("(IZ)V".to_owned())),
                Some(Constant::Utf8("clear".to_owned())),
                Some(Constant::Utf8("isSet".to_owned())),
                Some(Constant::Utf8("(I)Z".to_owned())),
                Some(Constant::Utf8("getTimeZone".to_owned())),
                Some(Constant::Utf8("setTimeZone".to_owned())),
                Some(Constant::Utf8("before".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("after".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 8,
            super_class: 14,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 25, name_index: 110, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x71,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 114, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x73,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 116, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x75,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 118, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x77,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 120, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x79,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 122, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x7b,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 124, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x7b,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 125, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x7e,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 127, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x80,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 129, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x82,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 131, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x84,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 133, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x86,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 135, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x88,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 137, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x8a,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 139, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x8c,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 141, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x8e,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 143, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x73,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 144, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x71,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 145, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x71,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 146, descriptor_index: 111, attributes: vec![
                    Attribute::Raw { name_index: 112, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x73,
                    ] },
                ] },
                Member { access_flags: 2, name_index: 37, descriptor_index: 38, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 27, descriptor_index: 28, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 20, descriptor_index: 21, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 4, name_index: 11, descriptor_index: 17, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb8, 0x00, 0x01, 0xb7, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 4, name_index: 11, descriptor_index: 12, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x0d, 0x2a, 0x10, 0x0f, 0xbc, 0x04, 0xb5, 0x00, 0x12, 0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x16, 0x59,
                        0xb7, 0x00, 0x18, 0xbf, 0x2a, 0x2b, 0xb5, 0x00, 0x19, 0x2a, 0xb8, 0x00, 0x1d, 0xb5, 0x00, 0x23, 0x2a, 0xb7, 0x00, 0x27,
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 41, descriptor_index: 17, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 3, max_locals: 2, code: vec![
                        0x03, 0x3c, 0x1b, 0x2a, 0xb4, 0x00, 0x12, 0xbe, 0xa2, 0x00, 0x10, 0x2a, 0xb4, 0x00, 0x12, 0x1b, 0x04, 0x54, 0x84, 0x01,
                        0x01, 0xa7, 0xff, 0xed, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 150, descriptor_index: 151, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 2, max_locals: 0, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0xb7, 0x00, 0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 150, descriptor_index: 152, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 3, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x2a, 0xb7, 0x00, 0x07, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 17, name_index: 50, descriptor_index: 153, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 4, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x2b, 0x59, 0x2a, 0xb4, 0x00, 0x23, 0xb7, 0x00, 0x2d, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 17, name_index: 154, descriptor_index: 155, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 3, max_locals: 2, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x16, 0x59, 0xb7, 0x00, 0x18, 0xbf, 0x2a, 0x2b, 0xb6, 0x00, 0x30, 0xb5, 0x00, 0x23,
                        0x2a, 0xb7, 0x00, 0x27, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 156, descriptor_index: 34, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x23, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 157, descriptor_index: 47, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1f, 0xb5, 0x00, 0x23, 0x2a, 0xb7, 0x00, 0x27, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 85, descriptor_index: 86, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0xb7, 0x00, 0x33, 0x1b, 0xb8, 0x00, 0x36, 0x3d, 0x2a, 0xb4, 0x00, 0x12, 0x1b, 0x04, 0x54, 0x1c, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 266, name_index: 56, descriptor_index: 57, attributes: Vec::new() },
                Member { access_flags: 266, name_index: 77, descriptor_index: 78, attributes: Vec::new() },
                Member { access_flags: 266, name_index: 93, descriptor_index: 94, attributes: Vec::new() },
                Member { access_flags: 266, name_index: 97, descriptor_index: 78, attributes: Vec::new() },
                Member { access_flags: 266, name_index: 100, descriptor_index: 78, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 53, descriptor_index: 34, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 6, max_locals: 2, code: vec![
                        0x2a, 0xb4, 0x00, 0x19, 0xb6, 0x00, 0x3a, 0x3c, 0x1b, 0x9e, 0x00, 0x11, 0x2a, 0xb4, 0x00, 0x23, 0x14, 0x00, 0x40, 0x1b,
                        0x85, 0x65, 0x94, 0x9d, 0x00, 0x15, 0x1b, 0x9c, 0x00, 0x1b, 0x2a, 0xb4, 0x00, 0x23, 0x14, 0x00, 0x42, 0x1b, 0x85, 0x65,
                        0x94, 0x9c, 0x00, 0x0d, 0xbb, 0x00, 0x44, 0x59, 0x12, 0x46, 0xb7, 0x00, 0x48, 0xbf, 0x2a, 0xb4, 0x00, 0x23, 0x1b, 0x85,
                        0x61, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 81, descriptor_index: 82, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 6, max_locals: 4, code: vec![
                        0x2a, 0xb4, 0x00, 0x19, 0xb6, 0x00, 0x3a, 0x3e, 0x1d, 0x9e, 0x00, 0x0e, 0x1f, 0x14, 0x00, 0x42, 0x1d, 0x85, 0x61, 0x94,
                        0x9b, 0x00, 0x12, 0x1d, 0x9c, 0x00, 0x18, 0x1f, 0x14, 0x00, 0x40, 0x1d, 0x85, 0x61, 0x94, 0x9e, 0x00, 0x0d, 0xbb, 0x00,
                        0x44, 0x59, 0x12, 0x46, 0xb7, 0x00, 0x48, 0xbf, 0x1f, 0x1d, 0x85, 0x65, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 108, descriptor_index: 109, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 6, max_locals: 5, code: vec![
                        0x2a, 0xb7, 0x00, 0x33, 0x42, 0x2a, 0x2a, 0x21, 0x1b, 0x1c, 0xb8, 0x00, 0x4b, 0xb7, 0x00, 0x4f, 0xb5, 0x00, 0x23, 0x2a,
                        0xb4, 0x00, 0x12, 0x1b, 0x04, 0x54, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 17, name_index: 108, descriptor_index: 158, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 8, max_locals: 4, code: vec![
                        0x2a, 0x1b, 0x1c, 0x1d, 0x2a, 0x10, 0x0b, 0xb6, 0x00, 0x53, 0x2a, 0x10, 0x0c, 0xb6, 0x00, 0x53, 0x2a, 0x10, 0x0d, 0xb6,
                        0x00, 0x53, 0xb7, 0x00, 0x57, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 17, name_index: 108, descriptor_index: 159, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 8, max_locals: 6, code: vec![
                        0x2a, 0x1b, 0x1c, 0x1d, 0x15, 0x04, 0x15, 0x05, 0x2a, 0x10, 0x0d, 0xb6, 0x00, 0x53, 0xb7, 0x00, 0x57, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 17, name_index: 108, descriptor_index: 90, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 7, max_locals: 7, code: vec![
                        0x2a, 0x1b, 0x1c, 0x1d, 0x15, 0x04, 0x15, 0x05, 0x15, 0x06, 0xb7, 0x00, 0x57, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 89, descriptor_index: 90, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 10, max_locals: 9, code: vec![
                        0x2a, 0xb7, 0x00, 0x33, 0x37, 0x07, 0x2a, 0x2a, 0x16, 0x07, 0x1b, 0x1c, 0x1d, 0x15, 0x04, 0x15, 0x05, 0x15, 0x06, 0xb8,
                        0x00, 0x5b, 0xb7, 0x00, 0x4f, 0xb5, 0x00, 0x23, 0x2a, 0xb7, 0x00, 0x27, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 160, descriptor_index: 109, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 6, max_locals: 5, code: vec![
                        0x2a, 0xb7, 0x00, 0x33, 0x42, 0x2a, 0x2a, 0x21, 0x1b, 0x1c, 0xb8, 0x00, 0x5f, 0xb7, 0x00, 0x4f, 0xb5, 0x00, 0x23, 0x2a,
                        0xb7, 0x00, 0x27, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 161, descriptor_index: 162, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 6, max_locals: 5, code: vec![
                        0x2a, 0xb7, 0x00, 0x33, 0x42, 0x2a, 0x2a, 0x21, 0x1b, 0x1c, 0x99, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x02, 0xb8, 0x00,
                        0x62, 0xb7, 0x00, 0x4f, 0xb5, 0x00, 0x23, 0x2a, 0xb7, 0x00, 0x27, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 17, name_index: 163, descriptor_index: 17, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x09, 0xb5, 0x00, 0x23, 0x03, 0x3c, 0x1b, 0x2a, 0xb4, 0x00, 0x12, 0xbe, 0xa2, 0x00, 0x10, 0x2a, 0xb4, 0x00, 0x12,
                        0x1b, 0x03, 0x54, 0x84, 0x01, 0x01, 0xa7, 0xff, 0xed, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 17, name_index: 163, descriptor_index: 105, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 3, max_locals: 2, code: vec![
                        0x1b, 0x9b, 0x00, 0x0c, 0x1b, 0x2a, 0xb4, 0x00, 0x12, 0xbe, 0xa1, 0x00, 0x0c, 0xbb, 0x00, 0x65, 0x59, 0x1b, 0xb7, 0x00,
                        0x67, 0xbf, 0x1b, 0x04, 0xa0, 0x00, 0x0e, 0x2a, 0x1b, 0x11, 0x07, 0xb2, 0xb6, 0x00, 0x6a, 0xa7, 0x00, 0x49, 0x1b, 0x05,
                        0xa0, 0x00, 0x0c, 0x2a, 0x1b, 0x03, 0xb6, 0x00, 0x6a, 0xa7, 0x00, 0x3b, 0x1b, 0x08, 0xa0, 0x00, 0x0c, 0x2a, 0x1b, 0x04,
                        0xb6, 0x00, 0x6a, 0xa7, 0x00, 0x2d, 0x1b, 0x10, 0x09, 0x9f, 0x00, 0x21, 0x1b, 0x10, 0x0a, 0x9f, 0x00, 0x1b, 0x1b, 0x10,
                        0x0b, 0x9f, 0x00, 0x15, 0x1b, 0x10, 0x0c, 0x9f, 0x00, 0x0f, 0x1b, 0x10, 0x0d, 0x9f, 0x00, 0x09, 0x1b, 0x10, 0x0e, 0xa0,
                        0x00, 0x09, 0x2a, 0x1b, 0x03, 0xb6, 0x00, 0x6a, 0x2a, 0xb4, 0x00, 0x12, 0x1b, 0x03, 0x54, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 17, name_index: 164, descriptor_index: 165, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb4, 0x00, 0x12, 0x1b, 0x33, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 166, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x19, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 167, descriptor_index: 12, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 2, max_locals: 2, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x16, 0x59, 0xb7, 0x00, 0x18, 0xbf, 0x2a, 0x2b, 0xb5, 0x00, 0x19, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 168, descriptor_index: 169, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 4, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x16, 0x2a, 0xb4, 0x00, 0x23, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x23, 0x94, 0x9c,
                        0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 170, descriptor_index: 169, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 4, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x16, 0x2a, 0xb4, 0x00, 0x23, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x23, 0x94, 0x9e,
                        0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 171, descriptor_index: 169, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 4, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x2a, 0x2a, 0xb4, 0x00, 0x23, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x23, 0x94, 0x9a,
                        0x00, 0x1b, 0x2a, 0xb4, 0x00, 0x19, 0xb6, 0x00, 0x3a, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x19, 0xb6, 0x00, 0x3a, 0xa0,
                        0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 172, descriptor_index: 61, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 147, max_stack: 5, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x23, 0x2a, 0xb4, 0x00, 0x23, 0x10, 0x20, 0x7d, 0x83, 0x88, 0x2a, 0xb4, 0x00, 0x19, 0xb6, 0x00, 0x3a,
                        0x82, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}

pub(super) fn append_date(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/util/Date
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/lang/System".to_owned())),
                Some(Constant::Utf8("currentTimeMillis".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 12 }),
                Some(Constant::Utf8("java/util/Date".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("(J)V".to_owned())),
                Some(Constant::Methodref { class_index: 14, name_and_type_index: 15 }),
                Some(Constant::Class { name_index: 16 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 17 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 19 }),
                Some(Constant::NameAndType { name_index: 20, descriptor_index: 21 }),
                Some(Constant::Utf8("millis".to_owned())),
                Some(Constant::Utf8("J".to_owned())),
                Some(Constant::Class { name_index: 23 }),
                Some(Constant::Utf8("java/lang/StringBuffer".to_owned())),
                Some(Constant::String { string_index: 25 }),
                Some(Constant::Utf8("Date(".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 27 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 28 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 30 }),
                Some(Constant::NameAndType { name_index: 31, descriptor_index: 32 }),
                Some(Constant::Utf8("append".to_owned())),
                Some(Constant::Utf8("(J)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 34 }),
                Some(Constant::NameAndType { name_index: 31, descriptor_index: 35 }),
                Some(Constant::Utf8("(C)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 37 }),
                Some(Constant::NameAndType { name_index: 38, descriptor_index: 39 }),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("getTime".to_owned())),
                Some(Constant::Utf8("setTime".to_owned())),
                Some(Constant::Utf8("before".to_owned())),
                Some(Constant::Utf8("(Ljava/util/Date;)Z".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("after".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Utf8("hashCode".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 8,
            super_class: 14,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 2, name_index: 20, descriptor_index: 21, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 11, descriptor_index: 17, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 40, max_stack: 3, max_locals: 1, code: vec![
                        0x2a, 0xb8, 0x00, 0x01, 0xb7, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 11, descriptor_index: 12, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 40, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0xb7, 0x00, 0x0d, 0x2a, 0x1f, 0xb5, 0x00, 0x12, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 42, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 40, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x12, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 43, descriptor_index: 12, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 40, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1f, 0xb5, 0x00, 0x12, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 44, descriptor_index: 45, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 40, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0xb4, 0x00, 0x12, 0x2b, 0xb4, 0x00, 0x12, 0x94, 0x9c, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 47, descriptor_index: 45, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 40, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0xb4, 0x00, 0x12, 0x2b, 0xb4, 0x00, 0x12, 0x94, 0x9e, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 48, descriptor_index: 49, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 40, max_stack: 4, max_locals: 2, code: vec![
                        0x2b, 0xc1, 0x00, 0x08, 0x99, 0x00, 0x16, 0x2b, 0xc0, 0x00, 0x08, 0xb4, 0x00, 0x12, 0x2a, 0xb4, 0x00, 0x12, 0x94, 0x9a,
                        0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 50, descriptor_index: 51, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 40, max_stack: 5, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x12, 0x2a, 0xb4, 0x00, 0x12, 0x10, 0x20, 0x7d, 0x83, 0x88, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 38, descriptor_index: 39, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 40, max_stack: 3, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x16, 0x59, 0x12, 0x18, 0xb7, 0x00, 0x1a, 0x2a, 0xb4, 0x00, 0x12, 0xb6, 0x00, 0x1d, 0x10, 0x29, 0xb6, 0x00,
                        0x21, 0xb6, 0x00, 0x24, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}

pub(super) fn append_time_zone(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/util/TimeZone
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
                Some(Constant::Utf8("java/util/TimeZone".to_owned())),
                Some(Constant::Utf8("id".to_owned())),
                Some(Constant::Utf8("Ljava/lang/String;".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 16 }),
                Some(Constant::Utf8("rawOffset".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::String { string_index: 18 }),
                Some(Constant::Utf8("GMT".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 20 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 21 }),
                Some(Constant::Utf8("(Ljava/lang/String;I)V".to_owned())),
                Some(Constant::Class { name_index: 23 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 22, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 26, name_and_type_index: 27 }),
                Some(Constant::Class { name_index: 28 }),
                Some(Constant::NameAndType { name_index: 29, descriptor_index: 30 }),
                Some(Constant::Utf8("java/lang/String".to_owned())),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::String { string_index: 32 }),
                Some(Constant::Utf8("UTC".to_owned())),
                Some(Constant::String { string_index: 34 }),
                Some(Constant::Utf8("GMT+".to_owned())),
                Some(Constant::Methodref { class_index: 26, name_and_type_index: 36 }),
                Some(Constant::NameAndType { name_index: 37, descriptor_index: 38 }),
                Some(Constant::Utf8("startsWith".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Z".to_owned())),
                Some(Constant::String { string_index: 40 }),
                Some(Constant::Utf8("GMT-".to_owned())),
                Some(Constant::Methodref { class_index: 26, name_and_type_index: 42 }),
                Some(Constant::NameAndType { name_index: 43, descriptor_index: 44 }),
                Some(Constant::Utf8("charAt".to_owned())),
                Some(Constant::Utf8("(I)C".to_owned())),
                Some(Constant::Methodref { class_index: 26, name_and_type_index: 46 }),
                Some(Constant::NameAndType { name_index: 47, descriptor_index: 48 }),
                Some(Constant::Utf8("indexOf".to_owned())),
                Some(Constant::Utf8("(II)I".to_owned())),
                Some(Constant::Methodref { class_index: 26, name_and_type_index: 50 }),
                Some(Constant::NameAndType { name_index: 51, descriptor_index: 52 }),
                Some(Constant::Utf8("substring".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 54, name_and_type_index: 55 }),
                Some(Constant::Class { name_index: 56 }),
                Some(Constant::NameAndType { name_index: 57, descriptor_index: 58 }),
                Some(Constant::Utf8("java/lang/Integer".to_owned())),
                Some(Constant::Utf8("parseInt".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)I".to_owned())),
                Some(Constant::Methodref { class_index: 26, name_and_type_index: 60 }),
                Some(Constant::NameAndType { name_index: 51, descriptor_index: 61 }),
                Some(Constant::Utf8("(II)Ljava/lang/String;".to_owned())),
                Some(Constant::Integer(60000)),
                Some(Constant::Class { name_index: 64 }),
                Some(Constant::Utf8("java/lang/NumberFormatException".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("getDefault".to_owned())),
                Some(Constant::Utf8("()Ljava/util/TimeZone;".to_owned())),
                Some(Constant::Utf8("getTimeZone".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/util/TimeZone;".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("getID".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("setID".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("getRawOffset".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("getOffset".to_owned())),
                Some(Constant::Utf8("(IIIIII)I".to_owned())),
                Some(Constant::Utf8("useDaylightTime".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 15, descriptor_index: 16, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 2, name_index: 5, descriptor_index: 21, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 65, max_stack: 2, max_locals: 3, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb5, 0x00, 0x07, 0x2a, 0x1c, 0xb5, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 67, descriptor_index: 68, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 65, max_stack: 4, max_locals: 0, code: vec![
                        0xbb, 0x00, 0x08, 0x59, 0x12, 0x11, 0x03, 0xb7, 0x00, 0x13, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 9, name_index: 69, descriptor_index: 70, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 65, max_stack: 4, max_locals: 6, code: vec![
                        0x2a, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x16, 0x59, 0xb7, 0x00, 0x18, 0xbf, 0x2a, 0x12, 0x11, 0xb6, 0x00, 0x19, 0x9a, 0x00,
                        0x0c, 0x2a, 0x12, 0x1f, 0xb6, 0x00, 0x19, 0x99, 0x00, 0x0d, 0xbb, 0x00, 0x08, 0x59, 0x2a, 0x03, 0xb7, 0x00, 0x13, 0xb0,
                        0x2a, 0x12, 0x21, 0xb6, 0x00, 0x23, 0x9a, 0x00, 0x0c, 0x2a, 0x12, 0x27, 0xb6, 0x00, 0x23, 0x99, 0x00, 0x7f, 0x2a, 0x06,
                        0xb6, 0x00, 0x29, 0x10, 0x2d, 0xa0, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0x3c, 0x2a, 0x10, 0x3a, 0x07, 0xb6, 0x00,
                        0x2d, 0x3d, 0x1c, 0x9c, 0x00, 0x12, 0x2a, 0x07, 0xb6, 0x00, 0x31, 0xb8, 0x00, 0x35, 0x3e, 0x03, 0x36, 0x04, 0xa7, 0x00,
                        0x19, 0x2a, 0x07, 0x1c, 0xb6, 0x00, 0x3b, 0xb8, 0x00, 0x35, 0x3e, 0x2a, 0x1c, 0x04, 0x60, 0xb6, 0x00, 0x31, 0xb8, 0x00,
                        0x35, 0x36, 0x04, 0x1d, 0x9b, 0x00, 0x36, 0x1d, 0x10, 0x17, 0xa3, 0x00, 0x30, 0x15, 0x04, 0x10, 0x3b, 0xa3, 0x00, 0x29,
                        0x15, 0x04, 0x9b, 0x00, 0x24, 0x1d, 0x10, 0x3c, 0x68, 0x15, 0x04, 0x60, 0x12, 0x3e, 0x68, 0x36, 0x05, 0xbb, 0x00, 0x08,
                        0x59, 0x2a, 0x1b, 0x99, 0x00, 0x09, 0x15, 0x05, 0x74, 0xa7, 0x00, 0x05, 0x15, 0x05, 0xb7, 0x00, 0x13, 0xb0, 0xa7, 0x00,
                        0x04, 0x4e, 0xbb, 0x00, 0x08, 0x59, 0x12, 0x11, 0x03, 0xb7, 0x00, 0x13, 0xb0,
                    ], exception_table: vec![
                        ExceptionHandler { start_pc: 82, end_pc: 177, handler_pc: 181, catch_type: 63 },
                    ], attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 72, descriptor_index: 73, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 65, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x07, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 74, descriptor_index: 75, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 65, max_stack: 2, max_locals: 2, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x16, 0x59, 0xb7, 0x00, 0x18, 0xbf, 0x2a, 0x2b, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 76, descriptor_index: 77, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 65, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x0d, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 78, descriptor_index: 79, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 65, max_stack: 1, max_locals: 7, code: vec![
                        0x2a, 0xb4, 0x00, 0x0d, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 80, descriptor_index: 81, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 65, max_stack: 1, max_locals: 1, code: vec![
                        0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
