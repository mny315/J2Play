//! CLDC key/value collection declarations.

use crate::{ACC_PUBLIC, ACC_SYNCHRONIZED};

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

pub(super) fn append_hashtable(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/util/Hashtable
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
                Some(Constant::Utf8("java/lang/IllegalArgumentException".to_owned())),
                Some(Constant::Methodref { class_index: 7, name_and_type_index: 3 }),
                Some(Constant::Fieldref { class_index: 11, name_and_type_index: 12 }),
                Some(Constant::Class { name_index: 13 }),
                Some(Constant::NameAndType { name_index: 14, descriptor_index: 15 }),
                Some(Constant::Utf8("java/util/Hashtable".to_owned())),
                Some(Constant::Utf8("keys".to_owned())),
                Some(Constant::Utf8("[Ljava/lang/Object;".to_owned())),
                Some(Constant::Fieldref { class_index: 11, name_and_type_index: 17 }),
                Some(Constant::NameAndType { name_index: 18, descriptor_index: 15 }),
                Some(Constant::Utf8("values".to_owned())),
                Some(Constant::Float(1061158912)),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 21 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 22 }),
                Some(Constant::Utf8("(IF)V".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 24 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 25 }),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Fieldref { class_index: 11, name_and_type_index: 27 }),
                Some(Constant::NameAndType { name_index: 28, descriptor_index: 29 }),
                Some(Constant::Utf8("count".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Class { name_index: 31 }),
                Some(Constant::Utf8("java/util/ArrayEnumeration".to_owned())),
                Some(Constant::Methodref { class_index: 30, name_and_type_index: 33 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 34 }),
                Some(Constant::Utf8("([Ljava/lang/Object;I)V".to_owned())),
                Some(Constant::Class { name_index: 36 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 35, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 39 }),
                Some(Constant::NameAndType { name_index: 40, descriptor_index: 41 }),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 43 }),
                Some(Constant::NameAndType { name_index: 44, descriptor_index: 45 }),
                Some(Constant::Utf8("find".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)I".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 47 }),
                Some(Constant::NameAndType { name_index: 48, descriptor_index: 25 }),
                Some(Constant::Utf8("ensureCapacity".to_owned())),
                Some(Constant::Class { name_index: 50 }),
                Some(Constant::Utf8("java/lang/StringBuffer".to_owned())),
                Some(Constant::String { string_index: 52 }),
                Some(Constant::Utf8("{".to_owned())),
                Some(Constant::Methodref { class_index: 49, name_and_type_index: 54 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 55 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::String { string_index: 57 }),
                Some(Constant::Utf8(", ".to_owned())),
                Some(Constant::Methodref { class_index: 49, name_and_type_index: 59 }),
                Some(Constant::NameAndType { name_index: 60, descriptor_index: 61 }),
                Some(Constant::Utf8("append".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::String { string_index: 63 }),
                Some(Constant::Utf8("(this Map)".to_owned())),
                Some(Constant::Methodref { class_index: 49, name_and_type_index: 65 }),
                Some(Constant::NameAndType { name_index: 60, descriptor_index: 66 }),
                Some(Constant::Utf8("(Ljava/lang/Object;)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 49, name_and_type_index: 68 }),
                Some(Constant::NameAndType { name_index: 60, descriptor_index: 69 }),
                Some(Constant::Utf8("(C)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 49, name_and_type_index: 71 }),
                Some(Constant::NameAndType { name_index: 72, descriptor_index: 73 }),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("size".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("isEmpty".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("()Ljava/util/Enumeration;".to_owned())),
                Some(Constant::Utf8("elements".to_owned())),
                Some(Constant::Utf8("contains".to_owned())),
                Some(Constant::Utf8("containsKey".to_owned())),
                Some(Constant::Utf8("get".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Ljava/lang/Object;".to_owned())),
                Some(Constant::Utf8("rehash".to_owned())),
                Some(Constant::Class { name_index: 15 }),
                Some(Constant::Utf8("put".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;".to_owned())),
                Some(Constant::Utf8("remove".to_owned())),
                Some(Constant::Utf8("clear".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 11,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 2, name_index: 14, descriptor_index: 15, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 18, descriptor_index: 15, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 28, descriptor_index: 29, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 22, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 2, max_locals: 3, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x1b, 0x9b, 0x00, 0x0f, 0x24, 0x0b, 0x96, 0x9e, 0x00, 0x09, 0x24, 0x24, 0x95, 0x99, 0x00, 0x0b,
                        0xbb, 0x00, 0x07, 0x59, 0xb7, 0x00, 0x09, 0xbf, 0x2a, 0x1b, 0x9a, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x1b, 0xbd, 0x00,
                        0x02, 0xb5, 0x00, 0x0a, 0x2a, 0x2a, 0xb4, 0x00, 0x0a, 0xbe, 0xbd, 0x00, 0x02, 0xb5, 0x00, 0x10, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 25, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0x12, 0x13, 0xb7, 0x00, 0x14, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x10, 0x0b, 0xb7, 0x00, 0x17, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 77, descriptor_index: 78, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x1a, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 79, descriptor_index: 80, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x1a, 0x9a, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 14, descriptor_index: 81, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 4, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x1e, 0x59, 0x2a, 0xb4, 0x00, 0x0a, 0x2a, 0xb4, 0x00, 0x1a, 0xb7, 0x00, 0x20, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 82, descriptor_index: 81, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 4, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x1e, 0x59, 0x2a, 0xb4, 0x00, 0x10, 0x2a, 0xb4, 0x00, 0x1a, 0xb7, 0x00, 0x20, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 83, descriptor_index: 41, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 3, max_locals: 3, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x23, 0x59, 0xb7, 0x00, 0x25, 0xbf, 0x03, 0x3d, 0x1c, 0x2a, 0xb4, 0x00, 0x1a, 0xa2,
                        0x00, 0x18, 0x2b, 0x2a, 0xb4, 0x00, 0x10, 0x1c, 0x32, 0xb6, 0x00, 0x26, 0x99, 0x00, 0x05, 0x04, 0xac, 0x84, 0x02, 0x01,
                        0xa7, 0xff, 0xe6, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 84, descriptor_index: 41, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb7, 0x00, 0x2a, 0x9b, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 85, descriptor_index: 86, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 2, max_locals: 3, code: vec![
                        0x2a, 0x2b, 0xb7, 0x00, 0x2a, 0x3d, 0x1c, 0x9c, 0x00, 0x07, 0x01, 0xa7, 0x00, 0x09, 0x2a, 0xb4, 0x00, 0x10, 0x1c, 0x32,
                        0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 44, descriptor_index: 45, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 3, max_locals: 3, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x23, 0x59, 0xb7, 0x00, 0x25, 0xbf, 0x03, 0x3d, 0x1c, 0x2a, 0xb4, 0x00, 0x1a, 0xa2,
                        0x00, 0x18, 0x2b, 0x2a, 0xb4, 0x00, 0x0a, 0x1c, 0x32, 0xb6, 0x00, 0x26, 0x99, 0x00, 0x05, 0x1c, 0xac, 0x84, 0x02, 0x01,
                        0xa7, 0xff, 0xe6, 0x02, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 4, name_index: 87, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 3, max_locals: 1, code: vec![
                        0x2a, 0x2a, 0xb4, 0x00, 0x1a, 0x04, 0x60, 0xb7, 0x00, 0x2e, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 2, name_index: 48, descriptor_index: 25, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 4, max_locals: 6, code: vec![
                        0x1b, 0x2a, 0xb4, 0x00, 0x0a, 0xbe, 0xa3, 0x00, 0x04, 0xb1, 0x2a, 0xb4, 0x00, 0x0a, 0xbe, 0x05, 0x68, 0x04, 0x60, 0x3d,
                        0x1c, 0x1b, 0xa1, 0x00, 0x07, 0x1c, 0x9c, 0x00, 0x05, 0x1b, 0x3d, 0x1c, 0xbd, 0x00, 0x02, 0x4e, 0x1c, 0xbd, 0x00, 0x02,
                        0x3a, 0x04, 0x03, 0x36, 0x05, 0x15, 0x05, 0x2a, 0xb4, 0x00, 0x1a, 0xa2, 0x00, 0x20, 0x2d, 0x15, 0x05, 0x2a, 0xb4, 0x00,
                        0x0a, 0x15, 0x05, 0x32, 0x53, 0x19, 0x04, 0x15, 0x05, 0x2a, 0xb4, 0x00, 0x10, 0x15, 0x05, 0x32, 0x53, 0x84, 0x05, 0x01,
                        0xa7, 0xff, 0xdd, 0x2a, 0x2d, 0xb5, 0x00, 0x0a, 0x2a, 0x19, 0x04, 0xb5, 0x00, 0x10, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 89, descriptor_index: 90, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 5, max_locals: 5, code: vec![
                        0x2c, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x23, 0x59, 0xb7, 0x00, 0x25, 0xbf, 0x2a, 0x2b, 0xb7, 0x00, 0x2a, 0x3e, 0x1d, 0x9b,
                        0x00, 0x15, 0x2a, 0xb4, 0x00, 0x10, 0x1d, 0x32, 0x3a, 0x04, 0x2a, 0xb4, 0x00, 0x10, 0x1d, 0x2c, 0x53, 0x19, 0x04, 0xb0,
                        0x2a, 0x2a, 0xb4, 0x00, 0x1a, 0x04, 0x60, 0xb7, 0x00, 0x2e, 0x2a, 0xb4, 0x00, 0x0a, 0x2a, 0xb4, 0x00, 0x1a, 0x2b, 0x53,
                        0x2a, 0xb4, 0x00, 0x10, 0x2a, 0x59, 0xb4, 0x00, 0x1a, 0x5a, 0x04, 0x60, 0xb5, 0x00, 0x1a, 0x2c, 0x53, 0x01, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 91, descriptor_index: 86, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 5, max_locals: 5, code: vec![
                        0x2a, 0x2b, 0xb7, 0x00, 0x2a, 0x3d, 0x1c, 0x9c, 0x00, 0x05, 0x01, 0xb0, 0x2a, 0xb4, 0x00, 0x10, 0x1c, 0x32, 0x4e, 0x1c,
                        0x36, 0x04, 0x15, 0x04, 0x2a, 0xb4, 0x00, 0x1a, 0x04, 0x64, 0xa2, 0x00, 0x29, 0x2a, 0xb4, 0x00, 0x0a, 0x15, 0x04, 0x2a,
                        0xb4, 0x00, 0x0a, 0x15, 0x04, 0x04, 0x60, 0x32, 0x53, 0x2a, 0xb4, 0x00, 0x10, 0x15, 0x04, 0x2a, 0xb4, 0x00, 0x10, 0x15,
                        0x04, 0x04, 0x60, 0x32, 0x53, 0x84, 0x04, 0x01, 0xa7, 0xff, 0xd2, 0x2a, 0xb4, 0x00, 0x0a, 0x2a, 0x59, 0xb4, 0x00, 0x1a,
                        0x04, 0x64, 0x5a, 0xb5, 0x00, 0x1a, 0x01, 0x53, 0x2a, 0xb4, 0x00, 0x10, 0x2a, 0xb4, 0x00, 0x1a, 0x01, 0x53, 0x2d, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 92, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 4, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x1a, 0x9e, 0x00, 0x21, 0x2a, 0xb4, 0x00, 0x0a, 0x2a, 0x59, 0xb4, 0x00, 0x1a, 0x04, 0x64, 0x5a, 0xb5,
                        0x00, 0x1a, 0x01, 0x53, 0x2a, 0xb4, 0x00, 0x10, 0x2a, 0xb4, 0x00, 0x1a, 0x01, 0x53, 0xa7, 0xff, 0xde, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 72, descriptor_index: 73, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 74, max_stack: 3, max_locals: 3, code: vec![
                        0xbb, 0x00, 0x31, 0x59, 0x12, 0x33, 0xb7, 0x00, 0x35, 0x4c, 0x03, 0x3d, 0x1c, 0x2a, 0xb4, 0x00, 0x1a, 0xa2, 0x00, 0x4f,
                        0x1c, 0x99, 0x00, 0x0a, 0x2b, 0x12, 0x38, 0xb6, 0x00, 0x3a, 0x57, 0x2b, 0x2a, 0xb4, 0x00, 0x0a, 0x1c, 0x32, 0x2a, 0xa6,
                        0x00, 0x08, 0x12, 0x3e, 0xa7, 0x00, 0x09, 0x2a, 0xb4, 0x00, 0x0a, 0x1c, 0x32, 0xb6, 0x00, 0x40, 0x57, 0x2b, 0x10, 0x3d,
                        0xb6, 0x00, 0x43, 0x57, 0x2b, 0x2a, 0xb4, 0x00, 0x10, 0x1c, 0x32, 0x2a, 0xa6, 0x00, 0x08, 0x12, 0x3e, 0xa7, 0x00, 0x09,
                        0x2a, 0xb4, 0x00, 0x10, 0x1c, 0x32, 0xb6, 0x00, 0x40, 0x57, 0x84, 0x02, 0x01, 0xa7, 0xff, 0xaf, 0x2b, 0x10, 0x7d, 0xb6,
                        0x00, 0x43, 0xb6, 0x00, 0x46, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
