//! Project-owned interpreter workloads for optional compiler profiling.
//! No game archives, host timing or user data are involved.
use classfile::{Attribute, ClassFile, Constant};
use vm::{Limits, Program, Value};

#[allow(clippy::too_many_lines)]
fn readonly_fixture_program(iterations: i32, with_call: bool, mutate: bool) -> Program {
    use classfile::{CodeAttribute, Member};
    let mut caller = vec![0x04, 0xbc, 10, 0x4b, 0x03, 0x3c, 0x03, 0x3d];
    let start = caller.len();
    caller.extend([0x1b, 0x12, 10]);
    let exit = caller.len();
    caller.extend([
        0xa2,
        0,
        0,
        0x2a,
        0x03,
        if mutate { 0x1b } else { 0x03 },
        0x4f,
        0x1c,
        0x2a,
        0x03,
        0xb8,
        0,
        if with_call { 13 } else { 9 },
        0x60,
        0x3d,
        0x84,
        1,
        1,
    ]);
    let back = caller.len();
    caller.extend([0xa7, 0, 0]);
    let finish = caller.len();
    caller.extend([0x1c, 0xac]);
    caller[exit + 1..exit + 3]
        .copy_from_slice(&i16::try_from(finish - exit).unwrap().to_be_bytes());
    caller[back + 1..back + 3]
        .copy_from_slice(&(-i16::try_from(back - start).unwrap()).to_be_bytes());
    let code = |bytes, max_stack, max_locals| {
        Attribute::Code(CodeAttribute {
            name_index: 5,
            max_stack,
            max_locals,
            code: bytes,
            exception_table: vec![],
            attributes: vec![],
        })
    };
    let mut class = ClassFile {
        minor_version: 0,
        major_version: 45,
        constant_pool: vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("ReadProbe".into())),
            Some(Constant::Utf8("main".into())),
            Some(Constant::Utf8("()I".into())),
            Some(Constant::Utf8("Code".into())),
            Some(Constant::Utf8("read".into())),
            Some(Constant::Utf8("([II)I".into())),
            Some(Constant::NameAndType {
                name_index: 6,
                descriptor_index: 7,
            }),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 8,
            }),
            Some(Constant::Integer(iterations)),
            Some(Constant::Utf8("wrapper".into())),
            Some(Constant::NameAndType {
                name_index: 11,
                descriptor_index: 7,
            }),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 12,
            }),
        ],
        access_flags: 0x21,
        this_class: 1,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        attributes: vec![],
        methods: vec![
            Member {
                access_flags: 9,
                name_index: 3,
                descriptor_index: 4,
                attributes: vec![code(caller, 3, 3)],
            },
            Member {
                access_flags: 9,
                name_index: 6,
                descriptor_index: 7,
                attributes: vec![code(vec![0x2a, 0x1b, 0x2e, 0x04, 0x60, 0xac], 2, 2)],
            },
        ],
    };
    if with_call {
        let mut wrapper = Vec::new();
        for _ in 0..3 {
            wrapper.extend([0x1b, 0x10, 7, 0x60, 0x10, 7, 0x64, 0x3c]);
        }
        wrapper.extend([0x2a, 0x1b, 0xb8, 0, 9, 0xac]);
        class.methods.push(Member {
            access_flags: 9,
            name_index: 11,
            descriptor_index: 7,
            attributes: vec![code(wrapper, 2, 2)],
        });
    }
    let mut program = Program::new();
    program.add_class(&class, &Limits::default()).unwrap();
    program
}

fn array_program(array_type: u8) -> Program {
    let load = match array_type {
        10 => 0x2e,
        8 | 4 => 0x33,
        9 => 0x35,
        _ => unreachable!(),
    };
    let store = load + 0x21;
    let mut program_class = ClassFile {
        minor_version: 0,
        major_version: 45,
        constant_pool: vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("ArrayProbe".into())),
            Some(Constant::Utf8("main".into())),
            Some(Constant::Utf8("()I".into())),
            Some(Constant::Utf8("Code".into())),
            Some(Constant::Integer(1_000_000)),
        ],
        access_flags: 0x21,
        this_class: 1,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        attributes: vec![],
        methods: vec![],
    };
    let mut code = vec![0x10, 64, 0xbc, array_type, 0x4b, 0x03, 0x3c, 0x03, 0x3d];
    let loop_pc = code.len();
    code.extend([0x1c, 0x12, 6]);
    let exit_branch = code.len();
    code.extend([0xa2, 0, 0]);
    code.extend([
        0x2a, 0x1c, 0x10, 63, 0x7e, 0x5c, load, 0x04, 0x60, store, 0x1b, 0x1c, 0x82, 0x3c, 0x84, 2,
        1,
    ]);
    let back_branch = code.len();
    code.extend([0xa7, 0, 0]);
    let exit_pc = code.len();
    code.extend([0x2a, 0x03, load, 0x1b, 0x82, 0xac]);
    code[exit_branch + 1..exit_branch + 3]
        .copy_from_slice(&i16::try_from(exit_pc - exit_branch).unwrap().to_be_bytes());
    code[back_branch + 1..back_branch + 3]
        .copy_from_slice(&(-i16::try_from(back_branch - loop_pc).unwrap()).to_be_bytes());
    program_class.methods.push(classfile::Member {
        access_flags: 9,
        name_index: 3,
        descriptor_index: 4,
        attributes: vec![Attribute::Code(classfile::CodeAttribute {
            name_index: 5,
            max_stack: 5,
            max_locals: 3,
            code,
            exception_table: vec![],
            attributes: vec![],
        })],
    });
    let mut program = Program::new();
    program
        .add_class(&program_class, &Limits::default())
        .unwrap();
    program
}

#[allow(clippy::too_many_lines)]
fn field_program() -> (Program, i32) {
    let code = |code, max_stack, max_locals| {
        Attribute::Code(classfile::CodeAttribute {
            name_index: 5,
            max_stack,
            max_locals,
            code,
            exception_table: vec![],
            attributes: vec![],
        })
    };
    let mut bytes = vec![
        0xbb, 0, 1, 0x59, 0x06, 0xb5, 0, 10, 0x4b, 0x03, 0x3c, 0x03, 0x3d,
    ];
    let start = bytes.len();
    bytes.extend([0x1c, 0x12, 6]);
    let exit = bytes.len();
    bytes.extend([
        0xa2, 0, 0, 0x2a, 0x59, 0xb4, 0, 10, 0x1c, 0x82, 0xb5, 0, 10, 0xb2, 0, 13, 0x04, 0x60,
        0xb3, 0, 13, 0x1b, 0x2a, 0xb6, 0, 21, 0xb4, 0, 10, 0xb2, 0, 13, 0xb8, 0, 17, 0x60, 0x3c,
        0x84, 2, 1,
    ]);
    let back = bytes.len();
    bytes.extend([0xa7, 0, 0]);
    let finish = bytes.len();
    bytes.extend([0x1b, 0xac]);
    bytes[exit + 1..exit + 3].copy_from_slice(&i16::try_from(finish - exit).unwrap().to_be_bytes());
    bytes[back + 1..back + 3]
        .copy_from_slice(&(-i16::try_from(back - start).unwrap()).to_be_bytes());
    let class = ClassFile {
        minor_version: 0,
        major_version: 45,
        constant_pool: vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("FieldProbe".into())),
            Some(Constant::Utf8("main".into())),
            Some(Constant::Utf8("()I".into())),
            Some(Constant::Utf8("Code".into())),
            Some(Constant::Integer(500_000)),
            Some(Constant::Utf8("value".into())),
            Some(Constant::Utf8("I".into())),
            Some(Constant::NameAndType {
                name_index: 7,
                descriptor_index: 8,
            }),
            Some(Constant::Fieldref {
                class_index: 1,
                name_and_type_index: 9,
            }),
            Some(Constant::Utf8("counter".into())),
            Some(Constant::NameAndType {
                name_index: 11,
                descriptor_index: 8,
            }),
            Some(Constant::Fieldref {
                class_index: 1,
                name_and_type_index: 12,
            }),
            Some(Constant::Utf8("mix".into())),
            Some(Constant::Utf8("(II)I".into())),
            Some(Constant::NameAndType {
                name_index: 14,
                descriptor_index: 15,
            }),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 16,
            }),
            Some(Constant::Utf8("getSelf".into())),
            Some(Constant::Utf8("()LFieldProbe;".into())),
            Some(Constant::NameAndType {
                name_index: 18,
                descriptor_index: 19,
            }),
            Some(Constant::Methodref {
                class_index: 1,
                name_and_type_index: 20,
            }),
        ],
        access_flags: 0x21,
        this_class: 1,
        super_class: 0,
        interfaces: vec![],
        attributes: vec![],
        fields: vec![
            classfile::Member {
                access_flags: 1,
                name_index: 7,
                descriptor_index: 8,
                attributes: vec![],
            },
            classfile::Member {
                access_flags: 9,
                name_index: 11,
                descriptor_index: 8,
                attributes: vec![],
            },
        ],
        methods: vec![
            classfile::Member {
                access_flags: 1,
                name_index: 18,
                descriptor_index: 19,
                attributes: vec![code(vec![0x2a, 0xb0], 1, 1)],
            },
            classfile::Member {
                access_flags: 9,
                name_index: 3,
                descriptor_index: 4,
                attributes: vec![code(bytes, 3, 3)],
            },
            classfile::Member {
                access_flags: 9,
                name_index: 14,
                descriptor_index: 15,
                attributes: vec![code(
                    vec![
                        0x1a, 0x1b, 0x82, 0x3d, 0x1c, 0x04, 0x7e, 0x99, 0, 7, 0x1c, 0x06, 0x7a,
                        0xac, 0x1c, 0x05, 0x78, 0xac,
                    ],
                    2,
                    3,
                )],
            },
        ],
    };
    let mut program = Program::new();
    program.add_class(&class, &Limits::default()).unwrap();
    let mut expected = 0_i32;
    let mut value = 3;
    for index in 0..500_000 {
        value ^= index;
        let mixed: i32 = value ^ (index + 1);
        expected = expected.wrapping_add(if mixed & 1 == 0 {
            mixed.wrapping_shl(2)
        } else {
            mixed >> 3
        });
    }
    (program, expected)
}

fn fill_program() -> Program {
    use classfile::{CodeAttribute, Member};
    let mut bytes = vec![
        0x11, 2, 0, 0xbc, 10, 0xb3, 0, 9, 0x11, 2, 0, 0x3c, 0x10, 7, 0x3d, 0x11, 3, 0xe8, 0x3e,
    ];
    let outer = bytes.len();
    bytes.extend([0x03, 0x3b]);
    let start = bytes.len();
    bytes.extend([
        0x1a, 0x1b, 0xa2, 0, 0, 0xb2, 0, 9, 0x1a, 0x1c, 0x4f, 0x84, 0, 1, 0xa7, 0, 0,
    ]);
    let exit = bytes.len();
    bytes[start + 3..start + 5]
        .copy_from_slice(&i16::try_from(exit - start - 2).unwrap().to_be_bytes());
    bytes[start + 15..start + 17].copy_from_slice(&(-14_i16).to_be_bytes());
    bytes.extend([0x84, 3, 0xff, 0x1d]);
    let repeat = bytes.len();
    bytes.extend([0x9d, 0, 0]);
    bytes[repeat + 1..repeat + 3]
        .copy_from_slice(&(-i16::try_from(repeat - outer).unwrap()).to_be_bytes());
    bytes.extend([0xb2, 0, 9, 0x11, 1, 0xff, 0x2e, 0xac]);
    let class = ClassFile {
        minor_version: 0,
        major_version: 45,
        access_flags: 0x21,
        this_class: 1,
        super_class: 0,
        interfaces: vec![],
        attributes: vec![],
        constant_pool: vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("FillProbe".into())),
            Some(Constant::Utf8("main".into())),
            Some(Constant::Utf8("()I".into())),
            Some(Constant::Utf8("Code".into())),
            Some(Constant::Utf8("pixels".into())),
            Some(Constant::Utf8("[I".into())),
            Some(Constant::NameAndType {
                name_index: 6,
                descriptor_index: 7,
            }),
            Some(Constant::Fieldref {
                class_index: 1,
                name_and_type_index: 8,
            }),
        ],
        fields: vec![Member {
            access_flags: 9,
            name_index: 6,
            descriptor_index: 7,
            attributes: vec![],
        }],
        methods: vec![Member {
            access_flags: 9,
            name_index: 3,
            descriptor_index: 4,
            attributes: vec![Attribute::Code(CodeAttribute {
                name_index: 5,
                max_stack: 3,
                max_locals: 4,
                code: bytes,
                exception_table: vec![],
                attributes: vec![],
            })],
        }],
    };
    let mut program = Program::new();
    program.add_class(&class, &Limits::default()).unwrap();
    program
}

fn main() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            for round in 0..4 {
                let (fields, field_result) = field_program();
                let (array_type, array_result) = [(10, 15_625), (8, 9), (9, 15_625), (4, 1)][round];
                for (name, program, expected) in [
                    ("ArrayProbe", array_program(array_type), array_result),
                    ("FillProbe", fill_program(), 7),
                    ("FieldProbe", fields, field_result),
                    (
                        "ReadProbe",
                        readonly_fixture_program(200_000, false, true),
                        -1_474_736_480,
                    ),
                    (
                        "ReadProbe",
                        readonly_fixture_program(200_000, true, true),
                        -1_474_736_480,
                    ),
                    (
                        "ReadProbe",
                        readonly_fixture_program(200_000, true, false),
                        200_000,
                    ),
                ] {
                    let result = program
                        .execute(
                            name,
                            "main",
                            "()I",
                            Limits {
                                max_instructions: 30_000_000,
                                ..Limits::default()
                            },
                            false,
                        )
                        .unwrap();
                    assert_eq!(result.value, Some(Value::Int(expected)));
                    println!("round={round} {name} instructions={}", result.instructions);
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
