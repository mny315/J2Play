//! Ordered `java.util` inventory and the random-number generator.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, ExceptionHandler, Member,
    append_bootstrap_classes, build_bootstrap_class,
};

mod calendar;
mod enumeration;
mod hashtable;
mod timers;
mod vector;

pub(super) fn append_generated_cldc_util(classes: &mut Vec<ClassFile>) {
    enumeration::append_array_enumeration(classes);
    calendar::append_calendar(classes);
    calendar::append_date(classes);
    vector::append_empty_stack_exception(classes);
    enumeration::append_enumeration(classes);
    hashtable::append_hashtable(classes);
    enumeration::append_no_such_element_exception(classes);
    append_bootstrap_classes!(classes;
        // java/util/Random
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
                Some(Constant::Utf8("java/util/Random".to_owned())),
                Some(Constant::Utf8("setSeed".to_owned())),
                Some(Constant::Utf8("(J)V".to_owned())),
                Some(Constant::Long(25214903917)),
                None,
                Some(Constant::Long(281474976710655)),
                None,
                Some(Constant::Fieldref { class_index: 8, name_and_type_index: 18 }),
                Some(Constant::NameAndType { name_index: 19, descriptor_index: 20 }),
                Some(Constant::Utf8("seed".to_owned())),
                Some(Constant::Utf8("J".to_owned())),
                Some(Constant::Long(11)),
                None,
                Some(Constant::Methodref { class_index: 8, name_and_type_index: 24 }),
                Some(Constant::NameAndType { name_index: 25, descriptor_index: 26 }),
                Some(Constant::Utf8("next".to_owned())),
                Some(Constant::Utf8("(I)I".to_owned())),
                Some(Constant::Class { name_index: 28 }),
                Some(Constant::Utf8("java/lang/IllegalArgumentException".to_owned())),
                Some(Constant::Methodref { class_index: 27, name_and_type_index: 3 }),
                Some(Constant::Utf8("Code".to_owned())),
                Some(Constant::Utf8("LineNumberTable".to_owned())),
                Some(Constant::Utf8("nextInt".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("StackMapTable".to_owned())),
                Some(Constant::Utf8("nextLong".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 33,
            this_class: 8,
            super_class: 2,
            interfaces: vec![],
            fields: vec![
                Member { access_flags: 2, name_index: 19, descriptor_index: 20, attributes: Vec::new() },
            ],
            methods: vec![
                Member { access_flags: 1, name_index: 5, descriptor_index: 12, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 3, max_locals: 3, code: vec![
                        0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x1f, 0xb6, 0x00, 0x07, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 11, descriptor_index: 12, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 5, max_locals: 3, code: vec![
                        0x2a, 0x1f, 0x14, 0x00, 0x0d, 0x83, 0x14, 0x00, 0x0f, 0x7f, 0xb5, 0x00, 0x11, 0xb1,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 4, name_index: 25, descriptor_index: 26, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 5, max_locals: 2, code: vec![
                        0x2a, 0x2a, 0xb4, 0x00, 0x11, 0x14, 0x00, 0x0d, 0x69, 0x14, 0x00, 0x15, 0x61, 0x14, 0x00, 0x0f, 0x7f, 0xb5, 0x00, 0x11,
                        0x2a, 0xb4, 0x00, 0x11, 0x10, 0x30, 0x1b, 0x64, 0x7d, 0x88, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 32, descriptor_index: 33, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 2, max_locals: 1, code: vec![
                        0x2a, 0x10, 0x20, 0xb6, 0x00, 0x17, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 32, descriptor_index: 26, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 4, max_locals: 4, code: vec![
                        0x1b, 0x9d, 0x00, 0x0b, 0xbb, 0x00, 0x1b, 0x59, 0xb7, 0x00, 0x1d, 0xbf, 0x1b, 0x1b, 0x74, 0x7e, 0x1b, 0xa0, 0x00, 0x12,
                        0x1b, 0x85, 0x2a, 0x10, 0x1f, 0xb6, 0x00, 0x17, 0x85, 0x69, 0x10, 0x1f, 0x7b, 0x88, 0xac, 0x2a, 0x10, 0x1f, 0xb6, 0x00,
                        0x17, 0x3d, 0x1c, 0x1b, 0x70, 0x3e, 0x1c, 0x1d, 0x64, 0x1b, 0x04, 0x64, 0x60, 0x9b, 0xff, 0xee, 0x1d, 0xac,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
                Member { access_flags: 1, name_index: 35, descriptor_index: 36, attributes: vec![
                    Attribute::Code(CodeAttribute { name_index: 30, max_stack: 4, max_locals: 1, code: vec![
                        0x2a, 0x10, 0x20, 0xb6, 0x00, 0x17, 0x85, 0x10, 0x20, 0x79, 0x2a, 0x10, 0x20, 0xb6, 0x00, 0x17, 0x85, 0x61, 0xad,
                    ], exception_table: Vec::new(), attributes: Vec::new() }),
                ] },
            ],
            attributes: Vec::new(),
        },
    );
    vector::append_stack(classes);
    calendar::append_time_zone(classes);
    timers::append_timers(classes);
    vector::append_vector(classes);
}
