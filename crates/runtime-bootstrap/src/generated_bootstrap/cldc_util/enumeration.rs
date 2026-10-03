//! Enumeration interface, array snapshots and exhaustion errors.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

pub(super) fn append_array_enumeration(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/util/ArrayEnumeration
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
                Some(Constant::Utf8("java/util/ArrayEnumeration".to_owned())),
                Some(Constant::Utf8("values".to_owned())),
                Some(Constant::Utf8("[Ljava/lang/Object;".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 16 }),
                Some(Constant::Utf8("size".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 18 }),
                Some(Constant::NameAndType { name_index: 19, descriptor_index: 16 }),
                Some(Constant::Utf8("index".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 21 }),
                Some(Constant::NameAndType { name_index: 22, descriptor_index: 23 }),
                Some(Constant::Utf8("hasMoreElements".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Class { name_index: 25 }),
                Some(Constant::Utf8("java/util/NoSuchElementException".to_owned())),
                Some(Constant::Methodref { class_index: 24, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 28 }),
                Some(Constant::Utf8("java/util/Enumeration".to_owned())),
                Some(Constant::Utf8("([Ljava/lang/Object;I)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("nextElement".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/Object;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
                Some(Constant::Methodref { class_index: 38, name_and_type_index: 39 }),
                Some(Constant::Class { name_index: 40 }),
                Some(Constant::NameAndType { name_index: 41, descriptor_index: 42 }),
                Some(Constant::Utf8("java/lang/System".to_owned())),
                Some(Constant::Utf8("arraycopy".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;ILjava/lang/Object;II)V".to_owned())),
            ],
            access_flags: 48,
            this_class: 8,
            super_class: 2,
            interfaces: vec![27],
            fields: vec![
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 15, descriptor_index: 16, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 19, descriptor_index: 16, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 0, name_index: 5, descriptor_index: 29, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 5, max_locals: 3, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x1c, 0xbd, 0x00, 0x02, 0xb5, 0x00, 0x07, 0x2b, 0x03, 0x2a, 0xb4, 0x00, 0x07, 0x03,
                        0x1c, 0xb8, 0x00, 0x25, 0x2a, 0x1c, 0xb5, 0x00, 0x0d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 22, descriptor_index: 23, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x11, 0x2a, 0xb4, 0x00, 0x0d, 0xa2, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 33, descriptor_index: 34, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 5, max_locals: 1, code: vec![
                        0x2a, 0xb6, 0x00, 0x14, 0x9a, 0x00, 0x0b, 0xbb, 0x00, 0x18, 0x59, 0xb7, 0x00, 0x1a, 0xbf, 0x2a, 0xb4, 0x00, 0x07, 0x2a,
                        0x59, 0xb4, 0x00, 0x11, 0x5a, 0x04, 0x60, 0xb5, 0x00, 0x11, 0x32, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}

pub(super) fn append_enumeration(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/util/Enumeration
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Class { name_index: 2 }),
                Some(Constant::Utf8("java/util/Enumeration".to_owned())),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Utf8("hasMoreElements".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("nextElement".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/Object;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1537,
            this_class: 1,
            super_class: 3,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1025, name_index: 5, descriptor_index: 6, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 7, descriptor_index: 8, attributes: Vec::new() },
            ],
            attributes: Vec::new(),
        },
    );
}

pub(super) fn append_no_such_element_exception(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/util/NoSuchElementException
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/lang/RuntimeException".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 8 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 9 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Class { name_index: 11 }),
                Some(Constant::Utf8("java/util/NoSuchElementException".to_owned())),
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
