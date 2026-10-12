//! Mutable CLDC string buffer declarations.
//!
//! Stateful methods own the buffer monitor. Conversion-only overloads delegate
//! to those methods after preparing arguments, keeping guest toString callbacks
//! outside the monitor.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

use crate::{ACC_PUBLIC, ACC_SYNCHRONIZED};

pub(super) fn append_string_buffer(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/lang/StringBuffer
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/lang/StringBuffer".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 11 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Class { name_index: 13 }),
                Some(Constant::Utf8("java/lang/NegativeArraySizeException".to_owned())),
                Some(Constant::Methodref { class_index: 12, name_and_type_index: 9 }),
                Some(Constant::Fieldref { class_index: 2, name_and_type_index: 16 }),
                Some(Constant::NameAndType { name_index: 17, descriptor_index: 18 }),
                Some(Constant::Utf8("value".to_owned())),
                Some(Constant::Utf8("[C".to_owned())),
                Some(Constant::Methodref { class_index: 20, name_and_type_index: 21 }),
                Some(Constant::Class { name_index: 22 }),
                Some(Constant::NameAndType { name_index: 23, descriptor_index: 24 }),
                Some(Constant::Utf8("java/lang/String".to_owned())),
                Some(Constant::Utf8("length".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 26 }),
                Some(Constant::NameAndType { name_index: 27, descriptor_index: 28 }),
                Some(Constant::Utf8("append".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Fieldref { class_index: 2, name_and_type_index: 30 }),
                Some(Constant::NameAndType { name_index: 31, descriptor_index: 32 }),
                Some(Constant::Utf8("count".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Class { name_index: 34 }),
                Some(Constant::Utf8("java/lang/StringIndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 33, name_and_type_index: 9 }),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 37 }),
                Some(Constant::NameAndType { name_index: 38, descriptor_index: 6 }),
                Some(Constant::Utf8("ensureCapacity".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 40 }),
                Some(Constant::NameAndType { name_index: 41, descriptor_index: 6 }),
                Some(Constant::Utf8("checkIndex".to_owned())),
                Some(Constant::Class { name_index: 43 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 42, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 46 }),
                Some(Constant::Utf8("java/lang/IndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 45, name_and_type_index: 9 }),
                Some(Constant::String { string_index: 49 }),
                Some(Constant::Utf8("null".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 51 }),
                Some(Constant::NameAndType { name_index: 52, descriptor_index: 53 }),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 20, name_and_type_index: 55 }),
                Some(Constant::NameAndType { name_index: 56, descriptor_index: 57 }),
                Some(Constant::Utf8("charAt".to_owned())),
                Some(Constant::Utf8("(I)C".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 59 }),
                Some(Constant::NameAndType { name_index: 27, descriptor_index: 60 }),
                Some(Constant::Utf8("([CII)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::String { string_index: 62 }),
                Some(Constant::Utf8("true".to_owned())),
                Some(Constant::String { string_index: 64 }),
                Some(Constant::Utf8("false".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 66 }),
                Some(Constant::NameAndType { name_index: 67, descriptor_index: 68 }),
                Some(Constant::Utf8("appendNumber".to_owned())),
                Some(Constant::Utf8("(J)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 70, name_and_type_index: 71 }),
                Some(Constant::Class { name_index: 72 }),
                Some(Constant::NameAndType { name_index: 52, descriptor_index: 73 }),
                Some(Constant::Utf8("java/lang/Float".to_owned())),
                Some(Constant::Utf8("(F)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 75, name_and_type_index: 76 }),
                Some(Constant::Class { name_index: 77 }),
                Some(Constant::NameAndType { name_index: 52, descriptor_index: 78 }),
                Some(Constant::Utf8("java/lang/Double".to_owned())),
                Some(Constant::Utf8("(D)Ljava/lang/String;".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 80 }),
                Some(Constant::NameAndType { name_index: 27, descriptor_index: 81 }),
                Some(Constant::Utf8("(C)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Long(48)),
                None,
                Some(Constant::Long(10)),
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 87 }),
                Some(Constant::NameAndType { name_index: 88, descriptor_index: 89 }),
                Some(Constant::Utf8("reverseRange".to_owned())),
                Some(Constant::Utf8("(II)V".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 91 }),
                Some(Constant::NameAndType { name_index: 92, descriptor_index: 93 }),
                Some(Constant::Utf8("insert".to_owned())),
                Some(Constant::Utf8("(ILjava/lang/String;)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 95 }),
                Some(Constant::NameAndType { name_index: 92, descriptor_index: 96 }),
                Some(Constant::Utf8("(I[C)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 9 }),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 99 }),
                Some(Constant::NameAndType { name_index: 27, descriptor_index: 100 }),
                Some(Constant::Utf8("(I)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 51 }),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 103 }),
                Some(Constant::NameAndType { name_index: 27, descriptor_index: 68 }),
                Some(Constant::Integer(56320)),
                Some(Constant::Integer(57343)),
                Some(Constant::Integer(55296)),
                Some(Constant::Integer(56319)),
                Some(Constant::Methodref { class_index: 20, name_and_type_index: 109 }),
                Some(Constant::NameAndType { name_index: 110, descriptor_index: 111 }),
                Some(Constant::Utf8("fromChars".to_owned())),
                Some(Constant::Utf8("([C)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("capacity".to_owned())),
                Some(Constant::Class { name_index: 18 }),
                Some(Constant::Utf8("setLength".to_owned())),
                Some(Constant::Utf8("setCharAt".to_owned())),
                Some(Constant::Utf8("(IC)V".to_owned())),
                Some(Constant::Utf8("getChars".to_owned())),
                Some(Constant::Utf8("(II[CI)V".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("([C)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("(Z)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("(F)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("(D)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("(ILjava/lang/Object;)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("(IZ)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("(IC)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("(II)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("(IJ)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("(IF)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("(ID)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("reverse".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 49,
            this_class: 2,
            super_class: 8,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 2, name_index: 17, descriptor_index: 18, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 31, descriptor_index: 32, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 11, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x10, 0x10, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x07, 0x1b, 0x9c, 0x00, 0x0b, 0xbb, 0x00, 0x0c, 0x59, 0xb7, 0x00, 0x0e, 0xbf, 0x2a, 0x1b, 0xbc, 0x05,
                        0xb5, 0x00, 0x0f, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 115, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb6, 0x00, 0x13, 0x10, 0x10, 0x60, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb6, 0x00, 0x19, 0x57, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 23, descriptor_index: 24, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x1d, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 116, descriptor_index: 24, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x0f, 0xbe, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 38, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 4, max_locals: 5, code: vec![
                        0x1b, 0x2a, 0xb4, 0x00, 0x0f, 0xbe, 0xa3, 0x00, 0x04, 0xb1, 0x2a, 0xb4, 0x00, 0x0f, 0xbe, 0x05, 0x68, 0x05, 0x60, 0x3d,
                        0x1c, 0x1b, 0xa1, 0x00, 0x07, 0x1c, 0x9c, 0x00, 0x05, 0x1b, 0x3d, 0x1c, 0xbc, 0x05, 0x4e, 0x03, 0x36, 0x04, 0x15, 0x04,
                        0x2a, 0xb4, 0x00, 0x1d, 0xa2, 0x00, 0x14, 0x2d, 0x15, 0x04, 0x2a, 0xb4, 0x00, 0x0f, 0x15, 0x04, 0x34, 0x55, 0x84, 0x04,
                        0x01, 0xa7, 0xff, 0xe9, 0x2a, 0x2d, 0xb5, 0x00, 0x0f, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 118, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 5, max_locals: 2, code: vec![
                        0x1b, 0x9c, 0x00, 0x0b, 0xbb, 0x00, 0x21, 0x59, 0xb7, 0x00, 0x23, 0xbf, 0x2a, 0x1b, 0xb6, 0x00, 0x24, 0x2a, 0xb4, 0x00,
                        0x1d, 0x1b, 0xa2, 0x00, 0x17, 0x2a, 0xb4, 0x00, 0x0f, 0x2a, 0x59, 0xb4, 0x00, 0x1d, 0x5a, 0x04, 0x60, 0xb5, 0x00, 0x1d,
                        0x03, 0x55, 0xa7, 0xff, 0xe7, 0x2a, 0x1b, 0xb5, 0x00, 0x1d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 41, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 2, max_locals: 2, code: vec![
                        0x1b, 0x9b, 0x00, 0x0b, 0x1b, 0x2a, 0xb4, 0x00, 0x1d, 0xa1, 0x00, 0x0b, 0xbb, 0x00, 0x21, 0x59, 0xb7, 0x00, 0x23, 0xbf,
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 56, descriptor_index: 57, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0xb7, 0x00, 0x27, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x34, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 119, descriptor_index: 120, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1b, 0xb7, 0x00, 0x27, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x1c, 0x55, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 121, descriptor_index: 122, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 5, max_locals: 7, code: vec![
                        0x1b, 0x9b, 0x00, 0x10, 0x1c, 0x1b, 0xa1, 0x00, 0x0b, 0x1c, 0x2a, 0xb4, 0x00, 0x1d, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x21,
                        0x59, 0xb7, 0x00, 0x23, 0xbf, 0x2d, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x2a, 0x59, 0xb7, 0x00, 0x2c, 0xbf, 0x1c, 0x1b, 0x64,
                        0x36, 0x05, 0x15, 0x04, 0x9b, 0x00, 0x0d, 0x15, 0x04, 0x2d, 0xbe, 0x15, 0x05, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x2d,
                        0x59, 0xb7, 0x00, 0x2f, 0xbf, 0x03, 0x36, 0x06, 0x15, 0x06, 0x15, 0x05, 0xa2, 0x00, 0x19, 0x2d, 0x15, 0x04, 0x15, 0x06,
                        0x60, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x15, 0x06, 0x60, 0x34, 0x55, 0x84, 0x06, 0x01, 0xa7, 0xff, 0xe6, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 27, descriptor_index: 123, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xc7, 0x00, 0x08, 0x12, 0x30, 0xa7, 0x00, 0x07, 0x2b, 0xb6, 0x00, 0x32, 0xb6, 0x00, 0x19, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 27, descriptor_index: 28, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 5, max_locals: 4, code: vec![
                        0x2b, 0xc7, 0x00, 0x06, 0x12, 0x30, 0x4c, 0x2b, 0xb6, 0x00, 0x13, 0x3d, 0x2a, 0x2a, 0xb4, 0x00, 0x1d, 0x1c, 0x60, 0xb6,
                        0x00, 0x24, 0x03, 0x3e, 0x1d, 0x1c, 0xa2, 0x00, 0x1e, 0x2a, 0xb4, 0x00, 0x0f, 0x2a, 0x59, 0xb4, 0x00, 0x1d, 0x5a, 0x04,
                        0x60, 0xb5, 0x00, 0x1d, 0x2b, 0x1d, 0xb6, 0x00, 0x36, 0x55, 0x84, 0x03, 0x01, 0xa7, 0xff, 0xe3, 0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 27, descriptor_index: 124, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x03, 0x2b, 0xbe, 0xb6, 0x00, 0x3a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 27, descriptor_index: 60, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 5, max_locals: 5, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x2a, 0x59, 0xb7, 0x00, 0x2c, 0xbf, 0x1c, 0x9b, 0x00, 0x0f, 0x1d, 0x9b, 0x00, 0x0b,
                        0x1c, 0x2b, 0xbe, 0x1d, 0x64, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x2d, 0x59, 0xb7, 0x00, 0x2f, 0xbf, 0x2a, 0x2a, 0xb4, 0x00,
                        0x1d, 0x1d, 0x60, 0xb6, 0x00, 0x24, 0x03, 0x36, 0x04, 0x15, 0x04, 0x1d, 0xa2, 0x00, 0x1f, 0x2a, 0xb4, 0x00, 0x0f, 0x2a,
                        0x59, 0xb4, 0x00, 0x1d, 0x5a, 0x04, 0x60, 0xb5, 0x00, 0x1d, 0x2b, 0x1c, 0x15, 0x04, 0x60, 0x34, 0x55, 0x84, 0x04, 0x01,
                        0xa7, 0xff, 0xe1, 0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 27, descriptor_index: 125, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0x99, 0x00, 0x08, 0x12, 0x3d, 0xa7, 0x00, 0x05, 0x12, 0x3f, 0xb6, 0x00, 0x19, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 27, descriptor_index: 81, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 5, max_locals: 2, code: vec![
                        0x2a, 0x2a, 0xb4, 0x00, 0x1d, 0x04, 0x60, 0xb6, 0x00, 0x24, 0x2a, 0xb4, 0x00, 0x0f, 0x2a, 0x59, 0xb4, 0x00, 0x1d, 0x5a,
                        0x04, 0x60, 0xb5, 0x00, 0x1d, 0x1b, 0x55, 0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 27, descriptor_index: 100, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0x85, 0xb7, 0x00, 0x41, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 27, descriptor_index: 68, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1f, 0xb7, 0x00, 0x41, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 27, descriptor_index: 126, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x23, 0xb8, 0x00, 0x45, 0xb6, 0x00, 0x19, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 27, descriptor_index: 127, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x27, 0xb8, 0x00, 0x4a, 0xb6, 0x00, 0x19, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 67, descriptor_index: 68, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 7, max_locals: 5, code: vec![
                        0x1f, 0x09, 0x94, 0x9a, 0x00, 0x0a, 0x2a, 0x10, 0x30, 0xb6, 0x00, 0x4f, 0xb0, 0x1f, 0x09, 0x94, 0x9c, 0x00, 0x07, 0x04,
                        0xa7, 0x00, 0x04, 0x03, 0x3e, 0x1d, 0x9a, 0x00, 0x06, 0x1f, 0x75, 0x40, 0x2a, 0xb4, 0x00, 0x1d, 0x36, 0x04, 0x1f, 0x09,
                        0x94, 0x99, 0x00, 0x1c, 0x2a, 0x14, 0x00, 0x52, 0x1f, 0x14, 0x00, 0x54, 0x71, 0x65, 0x88, 0x92, 0xb6, 0x00, 0x4f, 0x57,
                        0x1f, 0x14, 0x00, 0x54, 0x6d, 0x40, 0xa7, 0xff, 0xe4, 0x1d, 0x99, 0x00, 0x0a, 0x2a, 0x10, 0x2d, 0xb6, 0x00, 0x4f, 0x57,
                        0x2a, 0x15, 0x04, 0x2a, 0xb4, 0x00, 0x1d, 0x04, 0x64, 0xb7, 0x00, 0x56, 0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 92, descriptor_index: 128, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1b, 0x2c, 0xc7, 0x00, 0x08, 0x12, 0x30, 0xa7, 0x00, 0x07, 0x2c, 0xb6, 0x00, 0x32, 0xb6, 0x00, 0x5a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 92, descriptor_index: 93, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 4, max_locals: 5, code: vec![
                        0x1b, 0x9b, 0x00, 0x0b, 0x1b, 0x2a, 0xb4, 0x00, 0x1d, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x21, 0x59, 0xb7, 0x00, 0x23, 0xbf,
                        0x2c, 0xc7, 0x00, 0x06, 0x12, 0x30, 0x4d, 0x2c, 0xb6, 0x00, 0x13, 0x3e, 0x2a, 0x2a, 0xb4, 0x00, 0x1d, 0x1d, 0x60, 0xb6,
                        0x00, 0x24, 0x2a, 0xb4, 0x00, 0x1d, 0x04, 0x64, 0x36, 0x04, 0x15, 0x04, 0x1b, 0xa1, 0x00, 0x19, 0x2a, 0xb4, 0x00, 0x0f,
                        0x15, 0x04, 0x1d, 0x60, 0x2a, 0xb4, 0x00, 0x0f, 0x15, 0x04, 0x34, 0x55, 0x84, 0x04, 0xff, 0xa7, 0xff, 0xe7, 0x03, 0x36,
                        0x04, 0x15, 0x04, 0x1d, 0xa2, 0x00, 0x18, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x15, 0x04, 0x60, 0x2c, 0x15, 0x04, 0xb6, 0x00,
                        0x36, 0x55, 0x84, 0x04, 0x01, 0xa7, 0xff, 0xe8, 0x2a, 0x59, 0xb4, 0x00, 0x1d, 0x1d, 0x60, 0xb5, 0x00, 0x1d, 0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 92, descriptor_index: 96, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 4, max_locals: 4, code: vec![
                        0x2c, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x2a, 0x59, 0xb7, 0x00, 0x2c, 0xbf, 0x1b, 0x9b, 0x00, 0x0b, 0x1b, 0x2a, 0xb4, 0x00,
                        0x1d, 0xa4, 0x00, 0x0b, 0xbb, 0x00, 0x21, 0x59, 0xb7, 0x00, 0x23, 0xbf, 0x2a, 0x2a, 0xb4, 0x00, 0x1d, 0x2c, 0xbe, 0x60,
                        0xb6, 0x00, 0x24, 0x2a, 0xb4, 0x00, 0x1d, 0x04, 0x64, 0x3e, 0x1d, 0x1b, 0xa1, 0x00, 0x18, 0x2a, 0xb4, 0x00, 0x0f, 0x1d,
                        0x2c, 0xbe, 0x60, 0x2a, 0xb4, 0x00, 0x0f, 0x1d, 0x34, 0x55, 0x84, 0x03, 0xff, 0xa7, 0xff, 0xe9, 0x03, 0x3e, 0x1d, 0x2c,
                        0xbe, 0xa2, 0x00, 0x14, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x1d, 0x60, 0x2c, 0x1d, 0x34, 0x55, 0x84, 0x03, 0x01, 0xa7, 0xff,
                        0xec, 0x2a, 0x59, 0xb4, 0x00, 0x1d, 0x2c, 0xbe, 0x60, 0xb5, 0x00, 0x1d, 0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 92, descriptor_index: 129, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1b, 0x1c, 0x99, 0x00, 0x08, 0x12, 0x3d, 0xa7, 0x00, 0x05, 0x12, 0x3f, 0xb6, 0x00, 0x5a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 92, descriptor_index: 130, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 6, max_locals: 3, code: vec![
                        0x2a, 0x1b, 0x04, 0xbc, 0x05, 0x59, 0x03, 0x1c, 0x55, 0xb6, 0x00, 0x5e, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 92, descriptor_index: 131, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 4, max_locals: 3, code: vec![
                        0x2a, 0x1b, 0xbb, 0x00, 0x02, 0x59, 0xb7, 0x00, 0x61, 0x1c, 0xb6, 0x00, 0x62, 0xb6, 0x00, 0x65, 0xb6, 0x00, 0x5a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 92, descriptor_index: 132, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 5, max_locals: 4, code: vec![
                        0x2a, 0x1b, 0xbb, 0x00, 0x02, 0x59, 0xb7, 0x00, 0x61, 0x20, 0xb6, 0x00, 0x66, 0xb6, 0x00, 0x65, 0xb6, 0x00, 0x5a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 92, descriptor_index: 133, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0x1b, 0x24, 0xb8, 0x00, 0x45, 0xb6, 0x00, 0x5a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 92, descriptor_index: 134, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 4, max_locals: 4, code: vec![
                        0x2a, 0x1b, 0x28, 0xb8, 0x00, 0x4a, 0xb6, 0x00, 0x5a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 88, descriptor_index: 89, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 4, max_locals: 4, code: vec![
                        0x1b, 0x1c, 0xa2, 0x00, 0x26, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x34, 0x3e, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x84, 0x01, 0x01,
                        0x2a, 0xb4, 0x00, 0x0f, 0x1c, 0x34, 0x55, 0x2a, 0xb4, 0x00, 0x0f, 0x1c, 0x84, 0x02, 0xff, 0x1d, 0x55, 0xa7, 0xff, 0xdb,
                        0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 135, descriptor_index: 136, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 4, max_locals: 4, code: vec![
                        0x2a, 0x03, 0x2a, 0xb4, 0x00, 0x1d, 0x04, 0x64, 0xb7, 0x00, 0x56, 0x03, 0x3c, 0x1b, 0x2a, 0xb4, 0x00, 0x1d, 0x04, 0x64,
                        0xa2, 0x00, 0x44, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x34, 0x3d, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x04, 0x60, 0x34, 0x3e, 0x1c,
                        0x12, 0x68, 0xa1, 0x00, 0x28, 0x1c, 0x12, 0x69, 0xa3, 0x00, 0x22, 0x1d, 0x12, 0x6a, 0xa1, 0x00, 0x1c, 0x1d, 0x12, 0x6b,
                        0xa3, 0x00, 0x16, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x1d, 0x55, 0x2a, 0xb4, 0x00, 0x0f, 0x1b, 0x04, 0x60, 0x1c, 0x55, 0x84,
                        0x01, 0x01, 0x84, 0x01, 0x01, 0xa7, 0xff, 0xb8, 0x2a, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 52, descriptor_index: 53, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 112, max_stack: 4, max_locals: 3, code: vec![
                        0x2a, 0xb4, 0x00, 0x1d, 0xbc, 0x05, 0x4c, 0x03, 0x3d, 0x1c, 0x2a, 0xb4, 0x00, 0x1d, 0xa2, 0x00, 0x12, 0x2b, 0x1c, 0x2a,
                        0xb4, 0x00, 0x0f, 0x1c, 0x34, 0x55, 0x84, 0x02, 0x01, 0xa7, 0xff, 0xec, 0x2b, 0xb8, 0x00, 0x6c, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
