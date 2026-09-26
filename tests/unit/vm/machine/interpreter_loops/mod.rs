use super::*;
use bytecode::decode;
use classfile::{Attribute, ClassFile, CodeAttribute, Constant, Member};

fn int_array_field_constants() -> Vec<Option<Constant>> {
    vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("Fixture".into())),
        Some(Constant::Fieldref {
            class_index: 1,
            name_and_type_index: 4,
        }),
        Some(Constant::NameAndType {
            name_index: 5,
            descriptor_index: 6,
        }),
        Some(Constant::Utf8("pixels".into())),
        Some(Constant::Utf8("[I".into())),
    ]
}

#[test]
fn structural_counted_fill_links_without_suite_identity() {
    // while (local1-- >= 0) Fixture.pixels[local2++] = local3;
    let code = [
        0x1b, 0x59, 0x04, 0x64, 0x3c, 0x9b, 0x00, 0x0f, 0xb2, 0x00, 0x03, 0x1c, 0x84, 0x02, 0x01,
        0x1d, 0x4f, 0xa7, 0xff, 0xef,
    ];
    let instructions = decode(&code).unwrap();
    let index = super::super::build_instruction_index(code.len(), &instructions);
    let mut runtime = super::super::build_runtime_instructions(&instructions, &index).unwrap();
    let fills =
        link_counted_array_fills(&instructions, &int_array_field_constants(), &mut runtime, 0);
    assert_eq!(fills.len(), 1);
    assert_eq!(runtime[0].counted_array_fill, 1);
    assert_eq!(fills[0].access, ArrayAccessKind::Int);
    assert_eq!(fills[0].start_pc, 0);
    assert_eq!(fills[0].exit_pc, 20);
}

#[test]
fn counted_fill_rejects_changed_index_semantics() {
    let code = [
        0x1b, 0x59, 0x04, 0x64, 0x3c, 0x9b, 0x00, 0x0f, 0xb2, 0x00, 0x03, 0x1c, 0x84, 0x02, 0x02,
        0x1d, 0x4f, 0xa7, 0xff, 0xef,
    ];
    let instructions = decode(&code).unwrap();
    let index = super::super::build_instruction_index(code.len(), &instructions);
    let mut runtime = super::super::build_runtime_instructions(&instructions, &index).unwrap();
    assert!(
        link_counted_array_fills(&instructions, &int_array_field_constants(), &mut runtime, 0,)
            .is_empty()
    );
    assert_eq!(runtime[0].counted_array_fill, 0);
}

fn fill_fixture(max_stack: u16) -> ClassFile {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("FillFixture".into())),
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
    ];
    let code = vec![
        0x07, 0xbc, 0x0a, 0xb3, 0x00, 0x09, 0x05, 0x3b, 0x03, 0x3c, 0x10, 0x07, 0x3d, 0x1a, 0x59,
        0x04, 0x64, 0x3b, 0x9b, 0x00, 0x0f, 0xb2, 0x00, 0x09, 0x1b, 0x84, 0x01, 0x01, 0x1c, 0x4f,
        0xa7, 0xff, 0xef, 0xb2, 0x00, 0x09, 0x05, 0x2e, 0xac,
    ];
    ClassFile {
        minor_version: 0,
        major_version: 50,
        constant_pool: constants,
        access_flags: 0x0021,
        this_class: 1,
        super_class: 0,
        interfaces: Vec::new(),
        fields: vec![Member {
            access_flags: 0x0008,
            name_index: 6,
            descriptor_index: 7,
            attributes: Vec::new(),
        }],
        methods: vec![Member {
            access_flags: 0x0009,
            name_index: 3,
            descriptor_index: 4,
            attributes: vec![Attribute::Code(CodeAttribute {
                name_index: 5,
                max_stack,
                max_locals: 3,
                code,
                exception_table: Vec::new(),
                attributes: Vec::new(),
            })],
        }],
        attributes: Vec::new(),
    }
}

#[test]
fn linked_fill_executes_with_exact_guest_instruction_count() {
    let class = fill_fixture(4);
    let mut program = super::super::Program::new();
    program
        .add_class(&class, &super::super::Limits::default())
        .unwrap();
    assert_eq!(program.counted_array_fills.len(), 1);
    let execution = program
        .execute(
            "FillFixture",
            "main",
            "()I",
            super::super::Limits::default(),
            false,
        )
        .unwrap();
    assert_eq!(execution.value, Some(Value::Int(7)));
    assert_eq!(execution.instructions, 55);
}

#[test]
fn counted_fill_preserves_operand_stack_limits() {
    for (method_stack, total_stack) in [(2, 65_536), (4, 2)] {
        let limits = super::super::Limits {
            max_stack_slots: total_stack,
            ..super::super::Limits::default()
        };
        let mut program = super::super::Program::new();
        program
            .add_class(&fill_fixture(method_stack), &limits)
            .unwrap();
        for tracing in [false, true] {
            assert_eq!(
                program
                    .execute("FillFixture", "main", "()I", limits.clone(), tracing)
                    .unwrap_err()
                    .code(),
                "operand-stack-overflow"
            );
        }
    }
}

#[test]
fn limit_fill_matches_bytecodes_at_bounds_and_poll_boundaries() {
    for (initial, finish) in [
        (-1_i16, 4_i16),
        (0, -1),
        (0, 0),
        (0, 512),
        (1, 511),
        (510, 513),
        (513, 514),
    ] {
        for max_stack in [2, 3, 4] {
            for null_array in [false, true] {
                let mut class = fill_fixture(max_stack);
                let Attribute::Code(code) = &mut class.methods[0].attributes[0] else {
                    unreachable!()
                };
                let mut bytes = if null_array {
                    vec![0x01]
                } else {
                    vec![0x11, 2, 0, 0xbc, 10]
                };
                bytes.extend([0xb3, 0, 9, 0x11]);
                bytes.extend(initial.to_be_bytes());
                bytes.extend([0x3b, 0x11]);
                bytes.extend(finish.to_be_bytes());
                bytes.extend([0x3c, 0x10, 7, 0x3d]);
                let start = bytes.len();
                bytes.extend([
                    0x1a, 0x1b, 0xa2, 0, 0, 0xb2, 0, 9, 0x1a, 0x1c, 0x4f, 0x84, 0, 1, 0xa7, 0, 0,
                ]);
                let exit = bytes.len();
                bytes[start + 3..start + 5]
                    .copy_from_slice(&i16::try_from(exit - start - 2).unwrap().to_be_bytes());
                bytes[start + 15..start + 17].copy_from_slice(&(-14_i16).to_be_bytes());
                bytes.extend([0xb2, 0, 9, 0x11, 1, 0xff, 0x2e, 0x1a, 0x60, 0xac]);
                code.code = bytes;
                let mut program = super::super::Program::new();
                program
                    .add_class(&class, &super::super::Limits::default())
                    .unwrap();
                assert_eq!(program.counted_array_fills.len(), 1);
                for limit in [
                    1, 8, 15, 16, 20, 21, 22, 23, 24, 25, 26, 1022, 1023, 1024, 1025, 9000,
                ] {
                    let limits = super::super::Limits {
                        max_instructions: limit,
                        ..super::super::Limits::default()
                    };
                    let expected =
                        program.execute("FillFixture", "main", "()I", limits.clone(), true);
                    let actual = program.execute("FillFixture", "main", "()I", limits, false);
                    match (actual, expected) {
                        (Ok(a), Ok(b)) => {
                            assert_eq!(a.value, b.value);
                            assert_eq!(a.instructions, b.instructions);
                        }
                        (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
                        (a, b) => panic!(
                            "limit fill mismatch {initial}..{finish} budget {limit}: {a:?} / {b:?}"
                        ),
                    }
                }
            }
        }
    }
}

#[test]
fn length_fill_matches_bytecodes_at_limits_and_invalid_indices() {
    for initial in [-1_i8, 0, 1, 127] {
        for max_stack in [2, 3, 4] {
            let mut class = fill_fixture(max_stack);
            let Attribute::Code(code) = &mut class.methods[0].attributes[0] else {
                unreachable!()
            };
            code.code = vec![
                0x11,
                0x02,
                0x00,
                0xbc,
                0x0a,
                0xb3,
                0x00,
                0x09,
                0x10,
                initial as u8,
                0x3b,
                // for (; index < pixels.length; index++) pixels[index] = 7;
                0x1a,
                0xb2,
                0x00,
                0x09,
                0xbe,
                0xa2,
                0x00,
                0x10,
                0xb2,
                0x00,
                0x09,
                0x1a,
                0x10,
                0x07,
                0x4f,
                0x84,
                0x00,
                0x01,
                0xa7,
                0xff,
                0xee,
                0xb2,
                0x00,
                0x09,
                0x11,
                0x01,
                0xff,
                0x2e,
                0x1a,
                0x60,
                0xac,
            ];
            let mut program = super::super::Program::new();
            program
                .add_class(&class, &super::super::Limits::default())
                .unwrap();
            assert_eq!(program.counted_array_fills.len(), 1);
            for limit in [1, 8, 16, 30, 1022, 1023, 1024, 1025, 9000] {
                let limits = super::super::Limits {
                    max_instructions: limit,
                    ..super::super::Limits::default()
                };
                let expected = program.execute("FillFixture", "main", "()I", limits.clone(), true);
                let actual = program.execute("FillFixture", "main", "()I", limits, false);
                match (actual, expected) {
                    (Ok(actual), Ok(expected)) => {
                        assert_eq!(actual.value, expected.value);
                        assert_eq!(actual.instructions, expected.instructions);
                    }
                    (Err(actual), Err(expected)) => {
                        assert_eq!(actual.to_string(), expected.to_string())
                    }
                    (actual, expected) => {
                        panic!("fill execution mismatch: {actual:?} vs {expected:?}")
                    }
                }
            }
        }
    }
}

#[test]
fn reverse_copy_matches_bytecodes_with_aliases_bounds_and_polling() {
    for (limit, destination, source, alias, null) in [
        (128_i16, 0_i16, 128_i16, false, false),
        (128, 0, 128, true, false),
        (128, 0, 128, false, true),
        (0, -1, -1, false, true),
        (128, -1, 128, false, false),
        (128, 0, 129, false, false),
        (128, 0, 127, false, false),
    ] {
        let mut class = fill_fixture(4);
        let Attribute::Code(code) = &mut class.methods[0].attributes[0] else {
            unreachable!()
        };
        code.max_locals = 5;
        let mut bytes = vec![0x11, 0x00, 0x80, 0xbc, 0x0a, 0x4b];
        for (index, value) in [(0_u16, 13_u8), (63, 42), (127, 77)] {
            bytes.extend([0x2a, 0x11]);
            bytes.extend(index.to_be_bytes());
            bytes.extend([0x10, value, 0x4f]);
        }
        if alias {
            bytes.push(0x2a);
        } else {
            bytes.extend([0x11, 0x00, 0x80, 0xbc, 0x0a]);
        }
        bytes.extend([0xb3, 0x00, 0x09]);
        for (slot, value) in [(1, limit), (2, destination), (3, source)] {
            bytes.push(0x11);
            bytes.extend(value.to_be_bytes());
            bytes.extend([0x36, slot]);
        }
        if null {
            bytes.extend([0x01, 0x4b]);
        }
        bytes.extend([0x03, 0x36, 0x04]);
        let start = bytes.len();
        bytes.extend([0x15, 0x04, 0x1b]);
        let exit = bytes.len();
        bytes.extend([
            0xa2, 0, 0, 0xb2, 0x00, 0x09, 0x1c, 0x84, 0x02, 0x01, 0x2a, 0x84, 0x03, 0xff, 0x1d,
            0x2e, 0x4f, 0x84, 0x04, 0x01,
        ]);
        let back = bytes.len();
        bytes.extend([0xa7, 0, 0]);
        let finish = bytes.len();
        bytes[exit + 1..exit + 3]
            .copy_from_slice(&i16::try_from(finish - exit).unwrap().to_be_bytes());
        bytes[back + 1..back + 3]
            .copy_from_slice(&(-i16::try_from(back - start).unwrap()).to_be_bytes());
        bytes.push(0x03);
        for index in [0_u16, 64, 127] {
            bytes.extend([0xb2, 0x00, 0x09, 0x11]);
            bytes.extend(index.to_be_bytes());
            bytes.extend([0x2e, 0x60]);
        }
        bytes.extend([0x1c, 0x60, 0x1d, 0x60, 0x15, 0x04, 0x60, 0xac]);
        code.code = bytes;
        let mut program = super::super::Program::new();
        program
            .add_class(&class, &super::super::Limits::default())
            .unwrap();
        assert_eq!(program.counted_array_fills.len(), 1);
        for instruction_limit in [40, 45, 50, 70, 1022, 1023, 1024, 1025, 4000] {
            let limits = super::super::Limits {
                max_instructions: instruction_limit,
                ..super::super::Limits::default()
            };
            let expected = program.execute("FillFixture", "main", "()I", limits.clone(), true);
            let actual = program.execute("FillFixture", "main", "()I", limits, false);
            match (actual, expected) {
                (Ok(actual), Ok(expected)) => {
                    assert_eq!(actual.value, expected.value);
                    assert_eq!(actual.instructions, expected.instructions);
                }
                (Err(actual), Err(expected)) => {
                    assert_eq!(actual.to_string(), expected.to_string())
                }
                (actual, expected) => {
                    panic!("copy execution mismatch: {actual:?} vs {expected:?}")
                }
            }
        }
    }
}
