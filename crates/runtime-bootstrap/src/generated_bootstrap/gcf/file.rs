//! `FileConnection` API declarations and implementation inventory.

use super::{
    Attribute, ClassFile, CodeAttribute, Constant, ExceptionHandler, Member,
    append_bootstrap_classes, build_bootstrap_class,
};

mod connection;
mod filter;
mod output;
mod registry;

pub(super) fn append_classes(classes: &mut Vec<ClassFile>) {
    append_file_connection(classes);
    output::append_classes(classes);
    connection::append_classes(classes);
    registry::append_classes(classes);
    append_illegal_mode_exception(classes);
}

fn append_file_connection(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // javax/microedition/io/file/FileConnection
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Class { name_index: 2 }),
                Some(Constant::Utf8("javax/microedition/io/file/FileConnection".to_owned())),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Class { name_index: 6 }),
                Some(Constant::Utf8("javax/microedition/io/StreamConnection".to_owned())),
                Some(Constant::Utf8("availableSize".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Utf8("canRead".to_owned())),
                Some(Constant::Utf8("()Z".to_owned())),
                Some(Constant::Utf8("canWrite".to_owned())),
                Some(Constant::Utf8("create".to_owned())),
                Some(Constant::Utf8("()V".to_owned())),
                Some(Constant::Utf8("Exceptions".to_owned())),
                Some(Constant::Class { name_index: 16 }),
                Some(Constant::Utf8("java/io/IOException".to_owned())),
                Some(Constant::Utf8("delete".to_owned())),
                Some(Constant::Utf8("directorySize".to_owned())),
                Some(Constant::Utf8("(Z)J".to_owned())),
                Some(Constant::Utf8("exists".to_owned())),
                Some(Constant::Utf8("fileSize".to_owned())),
                Some(Constant::Utf8("getName".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("getPath".to_owned())),
                Some(Constant::Utf8("getURL".to_owned())),
                Some(Constant::Utf8("isDirectory".to_owned())),
                Some(Constant::Utf8("isHidden".to_owned())),
                Some(Constant::Utf8("lastModified".to_owned())),
                Some(Constant::Utf8("list".to_owned())),
                Some(Constant::Utf8("()Ljava/util/Enumeration;".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;Z)Ljava/util/Enumeration;".to_owned())),
                Some(Constant::Utf8("mkdir".to_owned())),
                Some(Constant::Utf8("openOutputStream".to_owned())),
                Some(Constant::Utf8("(J)Ljava/io/OutputStream;".to_owned())),
                Some(Constant::Utf8("rename".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("setFileConnection".to_owned())),
                Some(Constant::Utf8("setHidden".to_owned())),
                Some(Constant::Utf8("(Z)V".to_owned())),
                Some(Constant::Utf8("setReadable".to_owned())),
                Some(Constant::Utf8("setWritable".to_owned())),
                Some(Constant::Utf8("totalSize".to_owned())),
                Some(Constant::Utf8("truncate".to_owned())),
                Some(Constant::Utf8("(J)V".to_owned())),
                Some(Constant::Utf8("usedSize".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1537,
            this_class: 1,
            super_class: 3,
            interfaces: vec![5],
            fields: Vec::new(),
            methods: vec![
                Member { access_flags: 1025, name_index: 7, descriptor_index: 8, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 9, descriptor_index: 10, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 11, descriptor_index: 10, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 12, descriptor_index: 13, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 17, descriptor_index: 13, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 18, descriptor_index: 19, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 20, descriptor_index: 10, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 21, descriptor_index: 8, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 22, descriptor_index: 23, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 24, descriptor_index: 23, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 25, descriptor_index: 23, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 26, descriptor_index: 10, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 27, descriptor_index: 10, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 28, descriptor_index: 8, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 29, descriptor_index: 30, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 29, descriptor_index: 31, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 32, descriptor_index: 13, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 33, descriptor_index: 34, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 35, descriptor_index: 36, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 37, descriptor_index: 36, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 38, descriptor_index: 39, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 40, descriptor_index: 39, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 41, descriptor_index: 39, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 42, descriptor_index: 8, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 43, descriptor_index: 44, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 45, descriptor_index: 8, attributes: Vec::new() },
            ],
            attributes: Vec::new(),
        },
    );
}

fn append_illegal_mode_exception(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // javax/microedition/io/file/IllegalModeException
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
                Some(Constant::Utf8("javax/microedition/io/file/IllegalModeException".to_owned())),
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
