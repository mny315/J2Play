use classfile::{Attribute, ClassFile, CodeAttribute, Constant, Member};

pub fn fixture(iterations: i32) -> ClassFile {
    let code = |bytes: &[u8]| {
        Attribute::Code(CodeAttribute {
            name_index: 9,
            max_stack: 4,
            max_locals: 2,
            code: bytes.to_vec(),
            exception_table: vec![],
            attributes: vec![],
        })
    };
    ClassFile {
        minor_version: 0,
        major_version: 45,
        constant_pool: vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("NumericProbe".into())),
            Some(Constant::Utf8("kernel".into())),
            Some(Constant::Utf8("(I)I".into())),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 6,
            }),
            Some(Constant::NameAndType {
                name_index: 3,
                descriptor_index: 4,
            }),
            Some(Constant::Utf8("run".into())),
            Some(Constant::Utf8("()I".into())),
            Some(Constant::Utf8("Code".into())),
            Some(Constant::Integer(iterations)),
        ],
        access_flags: 0x21,
        this_class: 1,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        methods: vec![
            Member {
                access_flags: 9,
                name_index: 3,
                descriptor_index: 4,
                attributes: vec![code(&[
                    0x03, 0x3c, 0x1a, 0x9e, 0, 16, 0x1b, 0x1a, 0x10, 13, 0x68, 0x60, 0x3c, 0x84, 0,
                    0xff, 0xa7, 0xff, 0xf2, 0x1b, 0xac,
                ])],
            },
            Member {
                access_flags: 9,
                name_index: 7,
                descriptor_index: 8,
                attributes: vec![code(&[
                    0x12, 10, 0x3b, 0x03, 0x3c, 0x1a, 0x9e, 0, 17, 0x10, 32, 0xb8, 0, 5, 0x1b,
                    0x60, 0x3c, 0x84, 0, 0xff, 0xa7, 0xff, 0xf1, 0x1b, 0xac,
                ])],
            },
        ],
        attributes: vec![],
    }
}
