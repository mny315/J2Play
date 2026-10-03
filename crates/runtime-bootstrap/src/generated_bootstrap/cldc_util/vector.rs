//! Resizable vectors, live enumeration and the derived stack.

use crate::{ACC_PUBLIC, ACC_SYNCHRONIZED};

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, Member, append_bootstrap_classes,
    build_bootstrap_class,
};

pub(super) fn append_empty_stack_exception(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/util/EmptyStackException
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
                Some(Constant::Class { name_index: 8 }),
                Some(Constant::Utf8("java/util/EmptyStackException".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 7,
            super_class: 2,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 9, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}

pub(super) fn append_stack(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/util/Stack
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 3 }),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 6 }),
                Some(Constant::Utf8("java/util/Vector".to_owned())),
                Some(Constant::Utf8("<init>".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 9 }),
                Some(Constant::Class { name_index: 10 }),
                Some(Constant::NameAndType { name_index: 11, descriptor_index: 12 }),
                Some(Constant::Utf8("java/util/Stack".to_owned())),
                Some(Constant::Utf8("size".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Class { name_index: 14 }),
                Some(Constant::Utf8("java/util/EmptyStackException".to_owned())),
                Some(Constant::Methodref { class_index: 13, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 17 }),
                Some(Constant::NameAndType { name_index: 18, descriptor_index: 19 }),
                Some(Constant::Utf8("elementAt".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/Object;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 21 }),
                Some(Constant::NameAndType { name_index: 22, descriptor_index: 23 }),
                Some(Constant::Utf8("peek".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/Object;".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 25 }),
                Some(Constant::NameAndType { name_index: 26, descriptor_index: 27 }),
                Some(Constant::Utf8("removeElementAt".to_owned())),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 29 }),
                Some(Constant::NameAndType { name_index: 30, descriptor_index: 31 }),
                Some(Constant::Utf8("addElement".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)V".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 33 }),
                Some(Constant::NameAndType { name_index: 34, descriptor_index: 35 }),
                Some(Constant::Utf8("lastIndexOf".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)I".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("empty".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("pop".to_owned())),
                Some(Constant::Utf8("push".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Ljava/lang/Object;".to_owned())),
                Some(Constant::Utf8("search".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 36, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 33, name_index: 38, descriptor_index: 39, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 36, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb6, 0x00, 0x07, 0x9a, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 33, name_index: 22, descriptor_index: 23, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 36, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0xb6, 0x00, 0x07, 0x3c, 0x1b, 0x9a, 0x00, 0x0b, 0xbb, 0x00, 0x0d, 0x59, 0xb7, 0x00, 0x0f, 0xbf, 0x2a, 0x1b, 0x04,
                        0x64, 0xb6, 0x00, 0x10, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 33, name_index: 41, descriptor_index: 23, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 36, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0xb6, 0x00, 0x14, 0x4c, 0x2a, 0x2a, 0xb6, 0x00, 0x07, 0x04, 0x64, 0xb6, 0x00, 0x18, 0x2b, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 33, name_index: 42, descriptor_index: 43, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 36, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0xb6, 0x00, 0x1c, 0x2b, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 33, name_index: 44, descriptor_index: 35, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 36, max_stack: 2, max_locals: 3, code: vec![
                        0x2a, 0x2b, 0xb6, 0x00, 0x20, 0x3d, 0x1c, 0x9c, 0x00, 0x07, 0x02, 0xa7, 0x00, 0x09, 0x2a, 0xb6, 0x00, 0x07, 0x1c, 0x64,
                        0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}

pub(super) fn append_vector(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // java/util/Vector
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
                Some(Constant::Utf8("java/util/Vector".to_owned())),
                Some(Constant::Utf8("elementData".to_owned())),
                Some(Constant::Utf8("[Ljava/lang/Object;".to_owned())),
                Some(Constant::Fieldref { class_index: 11, name_and_type_index: 17 }),
                Some(Constant::NameAndType { name_index: 18, descriptor_index: 19 }),
                Some(Constant::Utf8("capacityIncrement".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 21 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 22 }),
                Some(Constant::Utf8("(II)V".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 24 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 25 }),
                Some(Constant::Utf8("(I)V".to_owned())),
                Some(Constant::Class { name_index: 27 }),
                Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
                Some(Constant::Methodref { class_index: 26, name_and_type_index: 3 }),
                Some(Constant::Fieldref { class_index: 11, name_and_type_index: 30 }),
                Some(Constant::NameAndType { name_index: 31, descriptor_index: 19 }),
                Some(Constant::Utf8("elementCount".to_owned())),
                Some(Constant::Class { name_index: 33 }),
                Some(Constant::Utf8("java/lang/ArrayIndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 32, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 36 }),
                Some(Constant::NameAndType { name_index: 37, descriptor_index: 38 }),
                Some(Constant::Utf8("copyInto".to_owned())),
                Some(Constant::Utf8("([Ljava/lang/Object;)V".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 40 }),
                Some(Constant::NameAndType { name_index: 41, descriptor_index: 25 }),
                Some(Constant::Utf8("ensureCapacity".to_owned())),
                Some(Constant::Class { name_index: 43 }),
                Some(Constant::Utf8("java/util/VectorEnumeration".to_owned())),
                Some(Constant::Methodref { class_index: 42, name_and_type_index: 45 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 46 }),
                Some(Constant::Utf8("(Ljava/util/Vector;)V".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 48 }),
                Some(Constant::NameAndType { name_index: 49, descriptor_index: 50 }),
                Some(Constant::Utf8("indexOf".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;I)I".to_owned())),
                Some(Constant::Class { name_index: 52 }),
                Some(Constant::Utf8("java/lang/IndexOutOfBoundsException".to_owned())),
                Some(Constant::Methodref { class_index: 51, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 2, name_and_type_index: 55 }),
                Some(Constant::NameAndType { name_index: 56, descriptor_index: 57 }),
                Some(Constant::Utf8("equals".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)Z".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 59 }),
                Some(Constant::NameAndType { name_index: 60, descriptor_index: 50 }),
                Some(Constant::Utf8("lastIndexOf".to_owned())),
                Some(Constant::Methodref { class_index: 32, name_and_type_index: 24 }),
                Some(Constant::Class { name_index: 63 }),
                Some(Constant::Utf8("java/util/NoSuchElementException".to_owned())),
                Some(Constant::Methodref { class_index: 62, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 66 }),
                Some(Constant::NameAndType { name_index: 67, descriptor_index: 68 }),
                Some(Constant::Utf8("insertElementAt".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;I)V".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 70 }),
                Some(Constant::NameAndType { name_index: 49, descriptor_index: 71 }),
                Some(Constant::Utf8("(Ljava/lang/Object;)I".to_owned())),
                Some(Constant::Methodref { class_index: 11, name_and_type_index: 73 }),
                Some(Constant::NameAndType { name_index: 74, descriptor_index: 25 }),
                Some(Constant::Utf8("removeElementAt".to_owned())),
                Some(Constant::Class { name_index: 76 }),
                Some(Constant::Utf8("java/lang/StringBuffer".to_owned())),
                Some(Constant::String { string_index: 78 }),
                Some(Constant::Utf8("[".to_owned())),
                Some(Constant::Methodref { class_index: 75, name_and_type_index: 80 }),
                Some(Constant::NameAndType { name_index: 5, descriptor_index: 81 }),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::String { string_index: 83 }),
                Some(Constant::Utf8(", ".to_owned())),
                Some(Constant::Methodref { class_index: 75, name_and_type_index: 85 }),
                Some(Constant::NameAndType { name_index: 86, descriptor_index: 87 }),
                Some(Constant::Utf8("append".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::String { string_index: 89 }),
                Some(Constant::Utf8("(this Vector)".to_owned())),
                Some(Constant::Methodref { class_index: 75, name_and_type_index: 91 }),
                Some(Constant::NameAndType { name_index: 86, descriptor_index: 92 }),
                Some(Constant::Utf8("(Ljava/lang/Object;)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 75, name_and_type_index: 94 }),
                Some(Constant::NameAndType { name_index: 86, descriptor_index: 95 }),
                Some(Constant::Utf8("(C)Ljava/lang/StringBuffer;".to_owned())),
                Some(Constant::Methodref { class_index: 75, name_and_type_index: 97 }),
                Some(Constant::NameAndType { name_index: 98, descriptor_index: 99 }),
                Some(Constant::Utf8("toString".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("trimToSize".to_owned())),
                Some(Constant::Utf8("setSize".to_owned())),
                Some(Constant::Utf8("capacity".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("size".to_owned())),
                Some(Constant::Utf8("isEmpty".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("elements".to_owned())),
                Some(Constant::Utf8("()Ljava/util/Enumeration;".to_owned())),
                Some(Constant::Utf8("contains".to_owned())),
                Some(Constant::Utf8("elementAt".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/Object;".to_owned())),
                Some(Constant::Utf8("firstElement".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/Object;".to_owned())),
                Some(Constant::Utf8("lastElement".to_owned())),
                Some(Constant::Utf8("setElementAt".to_owned())),
                Some(Constant::Utf8("addElement".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/Object;)V".to_owned())),
                Some(Constant::Utf8("removeElement".to_owned())),
                Some(Constant::Utf8("removeAllElements".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 11,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 4, name_index: 14, descriptor_index: 15, attributes: Vec::new() },
                Member { access_flags: 4, name_index: 31, descriptor_index: 19, attributes: Vec::new() },
                Member { access_flags: 4, name_index: 18, descriptor_index: 19, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 22, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 3, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x1b, 0x9c, 0x00, 0x0b, 0xbb, 0x00, 0x07, 0x59, 0xb7, 0x00, 0x09, 0xbf, 0x2a, 0x1b, 0xbd, 0x00,
                        0x02, 0xb5, 0x00, 0x0a, 0x2a, 0x1c, 0xb5, 0x00, 0x10, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 25, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x1b, 0x03, 0xb7, 0x00, 0x14, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 5, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x10, 0x0a, 0xb7, 0x00, 0x17, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 37, descriptor_index: 38, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 4, max_locals: 3, code: vec![
                        0x2b, 0xc7, 0x00, 0x0b, 0xbb, 0x00, 0x1a, 0x59, 0xb7, 0x00, 0x1c, 0xbf, 0x2b, 0xbe, 0x2a, 0xb4, 0x00, 0x1d, 0xa2, 0x00,
                        0x0b, 0xbb, 0x00, 0x20, 0x59, 0xb7, 0x00, 0x22, 0xbf, 0x03, 0x3d, 0x1c, 0x2a, 0xb4, 0x00, 0x1d, 0xa2, 0x00, 0x12, 0x2b,
                        0x1c, 0x2a, 0xb4, 0x00, 0x0a, 0x1c, 0x32, 0x53, 0x84, 0x02, 0x01, 0xa7, 0xff, 0xec, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 103, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb4, 0x00, 0x0a, 0xbe, 0x2a, 0xb4, 0x00, 0x1d, 0xa0, 0x00, 0x04, 0xb1, 0x2a, 0xb4, 0x00, 0x1d, 0xbd, 0x00, 0x02,
                        0x4c, 0x2a, 0x2b, 0xb6, 0x00, 0x23, 0x2a, 0x2b, 0xb5, 0x00, 0x0a, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 41, descriptor_index: 25, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 4, code: vec![
                        0x1b, 0x2a, 0xb4, 0x00, 0x0a, 0xbe, 0xa3, 0x00, 0x04, 0xb1, 0x2a, 0xb4, 0x00, 0x10, 0x9e, 0x00, 0x10, 0x2a, 0xb4, 0x00,
                        0x0a, 0xbe, 0x2a, 0xb4, 0x00, 0x10, 0x60, 0xa7, 0x00, 0x0a, 0x2a, 0xb4, 0x00, 0x0a, 0xbe, 0x05, 0x68, 0x3d, 0x1c, 0x1b,
                        0xa1, 0x00, 0x07, 0x1c, 0x9c, 0x00, 0x05, 0x1b, 0x3d, 0x1c, 0xbd, 0x00, 0x02, 0x4e, 0x2a, 0x2d, 0xb6, 0x00, 0x23, 0x2a,
                        0x2d, 0xb5, 0x00, 0x0a, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 104, descriptor_index: 25, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 5, max_locals: 2, code: vec![
                        0x1b, 0x9c, 0x00, 0x0b, 0xbb, 0x00, 0x20, 0x59, 0xb7, 0x00, 0x22, 0xbf, 0x2a, 0x1b, 0xb6, 0x00, 0x27, 0x2a, 0xb4, 0x00,
                        0x1d, 0x1b, 0xa4, 0x00, 0x17, 0x2a, 0xb4, 0x00, 0x0a, 0x2a, 0x59, 0xb4, 0x00, 0x1d, 0x04, 0x64, 0x5a, 0xb5, 0x00, 0x1d,
                        0x01, 0x53, 0xa7, 0xff, 0xe7, 0x2a, 0xb4, 0x00, 0x1d, 0x1b, 0xa2, 0x00, 0x17, 0x2a, 0xb4, 0x00, 0x0a, 0x2a, 0x59, 0xb4,
                        0x00, 0x1d, 0x5a, 0x04, 0x60, 0xb5, 0x00, 0x1d, 0x01, 0x53, 0xa7, 0xff, 0xe7, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 105, descriptor_index: 106, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x0a, 0xbe, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 107, descriptor_index: 106, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x1d, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 108, descriptor_index: 109, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 1, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x1d, 0x9a, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 110, descriptor_index: 111, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 1, code: vec![
                        0xbb, 0x00, 0x2a, 0x59, 0x2a, 0xb7, 0x00, 0x2c, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 112, descriptor_index: 57, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x03, 0xb6, 0x00, 0x2f, 0x9b, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 49, descriptor_index: 71, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x03, 0xb6, 0x00, 0x2f, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 49, descriptor_index: 50, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 4, code: vec![
                        0x1c, 0x9c, 0x00, 0x0b, 0xbb, 0x00, 0x33, 0x59, 0xb7, 0x00, 0x35, 0xbf, 0x2b, 0xc7, 0x00, 0x1e, 0x1c, 0x3e, 0x1d, 0x2a,
                        0xb4, 0x00, 0x1d, 0xa2, 0x00, 0x33, 0x2a, 0xb4, 0x00, 0x0a, 0x1d, 0x32, 0xc7, 0x00, 0x05, 0x1d, 0xac, 0x84, 0x03, 0x01,
                        0xa7, 0xff, 0xea, 0x1c, 0x3e, 0x1d, 0x2a, 0xb4, 0x00, 0x1d, 0xa2, 0x00, 0x18, 0x2b, 0x2a, 0xb4, 0x00, 0x0a, 0x1d, 0x32,
                        0xb6, 0x00, 0x36, 0x99, 0x00, 0x05, 0x1d, 0xac, 0x84, 0x03, 0x01, 0xa7, 0xff, 0xe6, 0x02, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 60, descriptor_index: 71, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 4, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x2a, 0xb4, 0x00, 0x1d, 0x04, 0x64, 0xb6, 0x00, 0x3a, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 60, descriptor_index: 50, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 4, code: vec![
                        0x1c, 0x2a, 0xb4, 0x00, 0x1d, 0xa1, 0x00, 0x0b, 0xbb, 0x00, 0x33, 0x59, 0xb7, 0x00, 0x35, 0xbf, 0x2b, 0xc7, 0x00, 0x1a,
                        0x1c, 0x3e, 0x1d, 0x9b, 0x00, 0x2f, 0x2a, 0xb4, 0x00, 0x0a, 0x1d, 0x32, 0xc7, 0x00, 0x05, 0x1d, 0xac, 0x84, 0x03, 0xff,
                        0xa7, 0xff, 0xee, 0x1c, 0x3e, 0x1d, 0x9b, 0x00, 0x18, 0x2b, 0x2a, 0xb4, 0x00, 0x0a, 0x1d, 0x32, 0xb6, 0x00, 0x36, 0x99,
                        0x00, 0x05, 0x1d, 0xac, 0x84, 0x03, 0xff, 0xa7, 0xff, 0xea, 0x02, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 113, descriptor_index: 114, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 2, code: vec![
                        0x1b, 0x9b, 0x00, 0x0b, 0x1b, 0x2a, 0xb4, 0x00, 0x1d, 0xa1, 0x00, 0x0c, 0xbb, 0x00, 0x20, 0x59, 0x1b, 0xb7, 0x00, 0x3d,
                        0xbf, 0x2a, 0xb4, 0x00, 0x0a, 0x1b, 0x32, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 115, descriptor_index: 116, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x1d, 0x9a, 0x00, 0x0b, 0xbb, 0x00, 0x3e, 0x59, 0xb7, 0x00, 0x40, 0xbf, 0x2a, 0xb4, 0x00, 0x0a, 0x03,
                        0x32, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 117, descriptor_index: 116, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x1d, 0x9a, 0x00, 0x0b, 0xbb, 0x00, 0x3e, 0x59, 0xb7, 0x00, 0x40, 0xbf, 0x2a, 0xb4, 0x00, 0x0a, 0x2a,
                        0xb4, 0x00, 0x1d, 0x04, 0x64, 0x32, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 118, descriptor_index: 68, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 3, code: vec![
                        0x1c, 0x9b, 0x00, 0x0b, 0x1c, 0x2a, 0xb4, 0x00, 0x1d, 0xa1, 0x00, 0x0c, 0xbb, 0x00, 0x20, 0x59, 0x1c, 0xb7, 0x00, 0x3d,
                        0xbf, 0x2a, 0xb4, 0x00, 0x0a, 0x1c, 0x2b, 0x53, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 74, descriptor_index: 25, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 5, max_locals: 3, code: vec![
                        0x1b, 0x9b, 0x00, 0x0b, 0x1b, 0x2a, 0xb4, 0x00, 0x1d, 0xa1, 0x00, 0x0c, 0xbb, 0x00, 0x20, 0x59, 0x1b, 0xb7, 0x00, 0x3d,
                        0xbf, 0x1b, 0x3d, 0x1c, 0x2a, 0xb4, 0x00, 0x1d, 0x04, 0x64, 0xa2, 0x00, 0x17, 0x2a, 0xb4, 0x00, 0x0a, 0x1c, 0x2a, 0xb4,
                        0x00, 0x0a, 0x1c, 0x04, 0x60, 0x32, 0x53, 0x84, 0x02, 0x01, 0xa7, 0xff, 0xe5, 0x2a, 0xb4, 0x00, 0x0a, 0x2a, 0x59, 0xb4,
                        0x00, 0x1d, 0x04, 0x64, 0x5a, 0xb5, 0x00, 0x1d, 0x01, 0x53, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 67, descriptor_index: 68, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 5, max_locals: 4, code: vec![
                        0x1c, 0x9b, 0x00, 0x0b, 0x1c, 0x2a, 0xb4, 0x00, 0x1d, 0xa4, 0x00, 0x0c, 0xbb, 0x00, 0x20, 0x59, 0x1c, 0xb7, 0x00, 0x3d,
                        0xbf, 0x2a, 0x2a, 0xb4, 0x00, 0x1d, 0x04, 0x60, 0xb6, 0x00, 0x27, 0x2a, 0xb4, 0x00, 0x1d, 0x3e, 0x1d, 0x1c, 0xa4, 0x00,
                        0x17, 0x2a, 0xb4, 0x00, 0x0a, 0x1d, 0x2a, 0xb4, 0x00, 0x0a, 0x1d, 0x04, 0x64, 0x32, 0x53, 0x84, 0x03, 0xff, 0xa7, 0xff,
                        0xea, 0x2a, 0xb4, 0x00, 0x0a, 0x1c, 0x2b, 0x53, 0x2a, 0x59, 0xb4, 0x00, 0x1d, 0x04, 0x60, 0xb5, 0x00, 0x1d, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 119, descriptor_index: 120, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 2, code: vec![
                        0x2a, 0x2b, 0x2a, 0xb4, 0x00, 0x1d, 0xb6, 0x00, 0x41, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 121, descriptor_index: 57, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 2, max_locals: 3, code: vec![
                        0x2a, 0x2b, 0xb6, 0x00, 0x45, 0x3d, 0x1c, 0x9c, 0x00, 0x05, 0x03, 0xac, 0x2a, 0x1c, 0xb6, 0x00, 0x48, 0x04, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 122, descriptor_index: 6, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 4, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x1d, 0x9e, 0x00, 0x17, 0x2a, 0xb4, 0x00, 0x0a, 0x2a, 0x59, 0xb4, 0x00, 0x1d, 0x04, 0x64, 0x5a, 0xb5,
                        0x00, 0x1d, 0x01, 0x53, 0xa7, 0xff, 0xe8, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: ACC_PUBLIC | ACC_SYNCHRONIZED, name_index: 98, descriptor_index: 99, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 100, max_stack: 3, max_locals: 4, code: vec![
                        0xbb, 0x00, 0x4b, 0x59, 0x12, 0x4d, 0xb7, 0x00, 0x4f, 0x4c, 0x03, 0x3d, 0x1c, 0x2a, 0xb4, 0x00, 0x1d, 0xa2, 0x00, 0x2b,
                        0x1c, 0x99, 0x00, 0x0a, 0x2b, 0x12, 0x52, 0xb6, 0x00, 0x54, 0x57, 0x2a, 0xb4, 0x00, 0x0a, 0x1c, 0x32, 0x4e, 0x2b, 0x2d,
                        0x2a, 0xa6, 0x00, 0x08, 0x12, 0x58, 0xa7, 0x00, 0x04, 0x2d, 0xb6, 0x00, 0x5a, 0x57, 0x84, 0x02, 0x01, 0xa7, 0xff, 0xd3,
                        0x2b, 0x10, 0x5d, 0xb6, 0x00, 0x5d, 0xb6, 0x00, 0x60, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
        // java/util/VectorEnumeration
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
                Some(Constant::Utf8("java/util/VectorEnumeration".to_owned())),
                Some(Constant::Utf8("vector".to_owned())),
                Some(Constant::Utf8("Ljava/util/Vector;".to_owned())),
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 14 }),
                Some(Constant::NameAndType { name_index: 15, descriptor_index: 16 }),
                Some(Constant::Utf8("index".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Methodref { class_index: 18, name_and_type_index: 19 }),
                Some(Constant::Class { name_index: 20 }),
                Some(Constant::NameAndType { name_index: 21, descriptor_index: 22 }),
                Some(Constant::Utf8("java/util/Vector".to_owned())),
                Some(Constant::Utf8("size".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 24 }),
                Some(Constant::NameAndType { name_index: 25, descriptor_index: 26 }),
                Some(Constant::Utf8("hasMoreElements".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Class { name_index: 28 }),
                Some(Constant::Utf8("java/util/NoSuchElementException".to_owned())),
                Some(Constant::Methodref { class_index: 27, name_and_type_index: 3 }),
                Some(Constant::Methodref { class_index: 18, name_and_type_index: 31 }),
                Some(Constant::NameAndType { name_index: 32, descriptor_index: 33 }),
                Some(Constant::Utf8("elementAt".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/Object;".to_owned())),
                Some(Constant::Class { name_index: 35 }),
                Some(Constant::Utf8("java/util/Enumeration".to_owned())),
                Some(Constant::Utf8("(Ljava/util/Vector;)V".to_owned())),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("nextElement".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/Object;".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 48,
            this_class: 8,
            super_class: 2,
            interfaces: vec![34],
            fields: vec![
                Member { access_flags: 2, name_index: 11, descriptor_index: 12, attributes: Vec::new() },
                Member { access_flags: 2, name_index: 15, descriptor_index: 16, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 0, name_index: 5, descriptor_index: 36, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 37, max_stack: 2, max_locals: 2, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb5, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 25, descriptor_index: 26, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 37, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0xb4, 0x00, 0x0d, 0x2a, 0xb4, 0x00, 0x07, 0xb6, 0x00, 0x11, 0xa2, 0x00, 0x07, 0x04, 0xa7, 0x00, 0x04, 0x03, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 40, descriptor_index: 41, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 37, max_stack: 5, max_locals: 1, code: vec![
                        0x2a, 0xb6, 0x00, 0x17, 0x9a, 0x00, 0x0b, 0xbb, 0x00, 0x1b, 0x59, 0xb7, 0x00, 0x1d, 0xbf, 0x2a, 0xb4, 0x00, 0x07, 0x2a,
                        0x59, 0xb4, 0x00, 0x0d, 0x5a, 0x04, 0x60, 0xb5, 0x00, 0x0d, 0xb6, 0x00, 0x1e, 0xb0,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
}
