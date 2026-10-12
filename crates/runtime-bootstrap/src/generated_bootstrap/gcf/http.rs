//! HTTP connection interface and constants.

mod connection;

use super::{
    Attribute, ClassFile, Constant, Member, append_bootstrap_classes, build_bootstrap_class,
};

pub(super) fn append_classes(classes: &mut Vec<ClassFile>) {
    append_bootstrap_classes!(classes;
        // javax/microedition/io/HttpConnection
        ClassFile {
            minor_version: 0,
            major_version: 48,
            constant_pool: vec![
                None,
                Some(Constant::Class { name_index: 2 }),
                Some(Constant::Utf8("javax/microedition/io/HttpConnection".to_owned())),
                Some(Constant::Class { name_index: 4 }),
                Some(Constant::Utf8("java/lang/Object".to_owned())),
                Some(Constant::Class { name_index: 6 }),
                Some(Constant::Utf8("javax/microedition/io/ContentConnection".to_owned())),
                Some(Constant::Utf8("HEAD".to_owned())),
                Some(Constant::Utf8("Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("ConstantValue".to_owned())),
                Some(Constant::String { string_index: 7 }),
                Some(Constant::Utf8("GET".to_owned())),
                Some(Constant::String { string_index: 11 }),
                Some(Constant::Utf8("POST".to_owned())),
                Some(Constant::String { string_index: 13 }),
                Some(Constant::Utf8("HTTP_OK".to_owned())),
                Some(Constant::Utf8("I".to_owned())),
                Some(Constant::Integer(200)),
                Some(Constant::Utf8("HTTP_CREATED".to_owned())),
                Some(Constant::Integer(201)),
                Some(Constant::Utf8("HTTP_ACCEPTED".to_owned())),
                Some(Constant::Integer(202)),
                Some(Constant::Utf8("HTTP_NOT_AUTHORITATIVE".to_owned())),
                Some(Constant::Integer(203)),
                Some(Constant::Utf8("HTTP_NO_CONTENT".to_owned())),
                Some(Constant::Integer(204)),
                Some(Constant::Utf8("HTTP_RESET".to_owned())),
                Some(Constant::Integer(205)),
                Some(Constant::Utf8("HTTP_PARTIAL".to_owned())),
                Some(Constant::Integer(206)),
                Some(Constant::Utf8("HTTP_MULT_CHOICE".to_owned())),
                Some(Constant::Integer(300)),
                Some(Constant::Utf8("HTTP_MOVED_PERM".to_owned())),
                Some(Constant::Integer(301)),
                Some(Constant::Utf8("HTTP_MOVED_TEMP".to_owned())),
                Some(Constant::Integer(302)),
                Some(Constant::Utf8("HTTP_SEE_OTHER".to_owned())),
                Some(Constant::Integer(303)),
                Some(Constant::Utf8("HTTP_NOT_MODIFIED".to_owned())),
                Some(Constant::Integer(304)),
                Some(Constant::Utf8("HTTP_USE_PROXY".to_owned())),
                Some(Constant::Integer(305)),
                Some(Constant::Utf8("HTTP_TEMP_REDIRECT".to_owned())),
                Some(Constant::Integer(307)),
                Some(Constant::Utf8("HTTP_BAD_REQUEST".to_owned())),
                Some(Constant::Integer(400)),
                Some(Constant::Utf8("HTTP_UNAUTHORIZED".to_owned())),
                Some(Constant::Integer(401)),
                Some(Constant::Utf8("HTTP_PAYMENT_REQUIRED".to_owned())),
                Some(Constant::Integer(402)),
                Some(Constant::Utf8("HTTP_FORBIDDEN".to_owned())),
                Some(Constant::Integer(403)),
                Some(Constant::Utf8("HTTP_NOT_FOUND".to_owned())),
                Some(Constant::Integer(404)),
                Some(Constant::Utf8("HTTP_BAD_METHOD".to_owned())),
                Some(Constant::Integer(405)),
                Some(Constant::Utf8("HTTP_NOT_ACCEPTABLE".to_owned())),
                Some(Constant::Integer(406)),
                Some(Constant::Utf8("HTTP_PROXY_AUTH".to_owned())),
                Some(Constant::Integer(407)),
                Some(Constant::Utf8("HTTP_CLIENT_TIMEOUT".to_owned())),
                Some(Constant::Integer(408)),
                Some(Constant::Utf8("HTTP_CONFLICT".to_owned())),
                Some(Constant::Integer(409)),
                Some(Constant::Utf8("HTTP_GONE".to_owned())),
                Some(Constant::Integer(410)),
                Some(Constant::Utf8("HTTP_LENGTH_REQUIRED".to_owned())),
                Some(Constant::Integer(411)),
                Some(Constant::Utf8("HTTP_PRECON_FAILED".to_owned())),
                Some(Constant::Integer(412)),
                Some(Constant::Utf8("HTTP_ENTITY_TOO_LARGE".to_owned())),
                Some(Constant::Integer(413)),
                Some(Constant::Utf8("HTTP_REQ_TOO_LONG".to_owned())),
                Some(Constant::Integer(414)),
                Some(Constant::Utf8("HTTP_UNSUPPORTED_TYPE".to_owned())),
                Some(Constant::Integer(415)),
                Some(Constant::Utf8("HTTP_INTERNAL_ERROR".to_owned())),
                Some(Constant::Integer(500)),
                Some(Constant::Utf8("HTTP_NOT_IMPLEMENTED".to_owned())),
                Some(Constant::Integer(501)),
                Some(Constant::Utf8("HTTP_BAD_GATEWAY".to_owned())),
                Some(Constant::Integer(502)),
                Some(Constant::Utf8("HTTP_UNAVAILABLE".to_owned())),
                Some(Constant::Integer(503)),
                Some(Constant::Utf8("HTTP_GATEWAY_TIMEOUT".to_owned())),
                Some(Constant::Integer(504)),
                Some(Constant::Utf8("HTTP_VERSION".to_owned())),
                Some(Constant::Integer(505)),
                Some(Constant::Utf8("getURL".to_owned())),
                Some(Constant::Utf8("()Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("getProtocol".to_owned())),
                Some(Constant::Utf8("getHost".to_owned())),
                Some(Constant::Utf8("getFile".to_owned())),
                Some(Constant::Utf8("getRef".to_owned())),
                Some(Constant::Utf8("getQuery".to_owned())),
                Some(Constant::Utf8("getPort".to_owned())),
                Some(Constant::Utf8("()I".to_owned())),
                Some(Constant::Utf8("getRequestMethod".to_owned())),
                Some(Constant::Utf8("setRequestMethod".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("Exceptions".to_owned())),
                Some(Constant::Class { name_index: 102 }),
                Some(Constant::Utf8("java/io/IOException".to_owned())),
                Some(Constant::Utf8("getRequestProperty".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("setRequestProperty".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;Ljava/lang/String;)V".to_owned())),
                Some(Constant::Utf8("getResponseCode".to_owned())),
                Some(Constant::Utf8("getResponseMessage".to_owned())),
                Some(Constant::Utf8("getExpiration".to_owned())),
                Some(Constant::Utf8("()J".to_owned())),
                Some(Constant::Utf8("getDate".to_owned())),
                Some(Constant::Utf8("getLastModified".to_owned())),
                Some(Constant::Utf8("getHeaderField".to_owned())),
                Some(Constant::Utf8("getHeaderFieldInt".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;I)I".to_owned())),
                Some(Constant::Utf8("getHeaderFieldDate".to_owned())),
                Some(Constant::Utf8("(Ljava/lang/String;J)J".to_owned())),
                Some(Constant::Utf8("(I)Ljava/lang/String;".to_owned())),
                Some(Constant::Utf8("getHeaderFieldKey".to_owned())),
                Some(Constant::Utf8("SourceFile".to_owned())),
                Some(Constant::Utf8("generated_bootstrap.rs".to_owned())),
            ],
            access_flags: 1537,
            this_class: 1,
            super_class: 3,
            interfaces: vec![5],
            fields: vec![
                Member { access_flags: 25, name_index: 7, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x0a,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 11, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x0c,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 13, descriptor_index: 8, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x0e,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 15, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x11,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 18, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x13,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 20, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x15,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 22, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x17,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 24, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x19,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 26, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x1b,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 28, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x1d,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 30, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x1f,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 32, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x21,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 34, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x23,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 36, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x25,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 38, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x27,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 40, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x29,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 42, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2b,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 44, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2d,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 46, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x2f,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 48, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x31,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 50, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x33,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 52, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x35,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 54, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x37,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 56, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x39,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 58, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x3b,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 60, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x3d,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 62, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x3f,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 64, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x41,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 66, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x43,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 68, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x45,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 70, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x47,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 72, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x49,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 74, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x4b,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 76, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x4d,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 78, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x4f,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 80, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x51,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 82, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x53,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 84, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x55,
                    ] },
                ] },
                Member { access_flags: 25, name_index: 86, descriptor_index: 16, attributes: vec![
                    Attribute::Raw { name_index: 9, name: "ConstantValue".to_owned(), bytes: vec![
                        0x00, 0x57,
                    ] },
                ] },
            ],
            methods: vec![
                Member { access_flags: 1025, name_index: 88, descriptor_index: 89, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 90, descriptor_index: 89, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 91, descriptor_index: 89, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 92, descriptor_index: 89, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 93, descriptor_index: 89, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 94, descriptor_index: 89, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 95, descriptor_index: 96, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 97, descriptor_index: 89, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 98, descriptor_index: 99, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 103, descriptor_index: 104, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 105, descriptor_index: 106, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 107, descriptor_index: 96, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 108, descriptor_index: 89, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 109, descriptor_index: 110, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 111, descriptor_index: 110, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 112, descriptor_index: 110, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 113, descriptor_index: 104, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 114, descriptor_index: 115, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 116, descriptor_index: 117, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 113, descriptor_index: 118, attributes: Vec::new() },
                Member { access_flags: 1025, name_index: 119, descriptor_index: 118, attributes: Vec::new() },
            ],
            attributes: Vec::new(),
        },
    );
    connection::append_classes(classes);
}
