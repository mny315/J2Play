use super::*;
use crate::{MAX_CLASS_MATERIALIZED_BYTES, tests::class_with_pool};

#[test]
fn parameter_slots_in_references_include_constructor_and_interface_receivers() {
    for count in [254, 255, 256] {
        for name in ["method", "<init>"] {
            let pool = vec![
                None,
                Some(Constant::Utf8("Owner".into())),
                Some(Constant::Class { name_index: 1 }),
                Some(Constant::Utf8(name.into())),
                Some(Constant::Utf8(format!("({})V", "I".repeat(count)))),
                Some(Constant::NameAndType {
                    name_index: 3,
                    descriptor_index: 4,
                }),
            ];
            let mut cache = ConstantValidationCache::new(pool.len());
            let method = Constant::Methodref {
                class_index: 2,
                name_and_type_index: 5,
            };
            let valid = count + usize::from(name == "<init>") <= 255;
            assert_eq!(
                validate_constant(&method, &pool, 10, &mut cache).is_ok(),
                valid
            );
            if valid && name == "method" {
                // The same descriptor was already cached as a potentially static call.
                let interface = Constant::InterfaceMethodref {
                    class_index: 2,
                    name_and_type_index: 5,
                };
                let result = validate_constant(&interface, &pool, 20, &mut cache);
                assert_eq!(result.is_ok(), count < 255);
                if let Err(error) = result {
                    assert_eq!(error.code(), "invalid-descriptor");
                    assert!(error.message().contains("offset 20"));
                }
            }
        }
    }
}

#[test]
fn parses_header_and_modified_utf8() {
    let bytes = class_with_pool(2, &[1, 0, 6, b'A', 0xC0, 0x80, 0xED, 0xA0, 0xBD]);
    let parsed = parse_prefix(&bytes).unwrap();
    let literal = parsed.constant_pool[1].as_ref().unwrap();
    assert_eq!(literal.utf8_text(), Some("A\0�"));
    assert_eq!(literal.utf16_units().unwrap().as_ref(), &[65, 0, 0xd83d]);

    let bytes = class_with_pool(2, &[1, 0, 4, b'A', 0xC0, 0x80, b'B']);
    let parsed = parse_prefix(&bytes).unwrap();
    assert_eq!(parsed.major_version, 48);
    assert_eq!(parsed.constant_pool[1], Some(Constant::Utf8("A\0B".into())));
    assert_eq!(parsed.next_offset, bytes.len());

    let bytes = class_with_pool(2, &[1, 0, 4, 0xF0, 0x9F, 0x98, 0x80]);
    assert_eq!(
        parse_prefix(&bytes).unwrap().constant_pool[1],
        Some(Constant::Utf8("😀".into()))
    );
}

#[test]
fn combines_surrogate_pairs() {
    let bytes = class_with_pool(2, &[1, 0, 6, 0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
    assert_eq!(
        parse_prefix(&bytes).unwrap().constant_pool[1],
        Some(Constant::Utf8("😀".into()))
    );
}

#[test]
fn parses_every_constant_tag_in_target_format() {
    let pool = [
        1, 0, 1, b'x', 3, 0, 0, 0, 1, 4, 0x3f, 0x80, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 2, 6, 0x3f,
        0xf0, 0, 0, 0, 0, 0, 0, 7, 0, 1, 8, 0, 1, 9, 0, 8, 0, 13, 10, 0, 8, 0, 13, 11, 0, 8, 0, 13,
        12, 0, 1, 0, 1,
    ];
    let parsed = parse_prefix(&class_with_pool(14, &pool)).unwrap();
    assert_eq!(
        parsed.constant_pool,
        vec![
            None,
            Some(Constant::Utf8("x".into())),
            Some(Constant::Integer(1)),
            Some(Constant::Float(1.0_f32.to_bits())),
            Some(Constant::Long(2)),
            None,
            Some(Constant::Double(1.0_f64.to_bits())),
            None,
            Some(Constant::Class { name_index: 1 }),
            Some(Constant::String { string_index: 1 }),
            Some(Constant::Fieldref {
                class_index: 8,
                name_and_type_index: 13
            }),
            Some(Constant::Methodref {
                class_index: 8,
                name_and_type_index: 13
            }),
            Some(Constant::InterfaceMethodref {
                class_index: 8,
                name_and_type_index: 13
            }),
            Some(Constant::NameAndType {
                name_index: 1,
                descriptor_index: 1
            }),
        ]
    );
}

#[test]
fn shared_name_and_type_text_is_validated_for_each_member_kind() {
    let pool = vec![
        None,
        Some(Constant::Utf8("field".into())),
        Some(Constant::Utf8("I".into())),
        Some(Constant::NameAndType {
            name_index: 1,
            descriptor_index: 2,
        }),
        Some(Constant::NameAndType {
            name_index: 1,
            descriptor_index: 2,
        }),
    ];
    let mut cache = ConstantValidationCache::new(pool.len());

    validate_name_and_type_reference(&pool, 3, 10, MemberKind::Field, &mut cache).unwrap();
    validate_name_and_type_reference(&pool, 4, 20, MemberKind::Field, &mut cache).unwrap();

    // The same pool entry can be referenced in another role; field text does
    // not become a valid method descriptor merely because it was cached.
    assert_eq!(
        validate_name_and_type_reference(&pool, 3, 30, MemberKind::Method, &mut cache)
            .unwrap_err()
            .code(),
        "invalid-descriptor",
    );
}

#[test]
fn rejects_constant_tags_newer_than_target_class_version() {
    let prefix = ClassPrefix {
        minor_version: 0,
        major_version: 50,
        constant_pool: vec![
            None,
            Some(Constant::MethodType {
                descriptor_index: 1,
            }),
        ],
        constant_offsets: vec![None, Some(10)],
        next_offset: 13,
    };
    assert_eq!(
        validate_target_version(&prefix).unwrap_err().code(),
        "unsupported-constant-version"
    );
}

#[test]
fn shared_text_keeps_class_field_and_method_name_rules_separate() {
    for (name, first, second) in [
        (
            "nested/Type",
            Constant::Class { name_index: 1 },
            Constant::Fieldref {
                class_index: 2,
                name_and_type_index: 5,
            },
        ),
        (
            "angle<field",
            Constant::Fieldref {
                class_index: 2,
                name_and_type_index: 5,
            },
            Constant::Methodref {
                class_index: 2,
                name_and_type_index: 7,
            },
        ),
    ] {
        let pool = vec![
            None,
            Some(Constant::Utf8(name.into())),
            Some(Constant::Class { name_index: 3 }),
            Some(Constant::Utf8("Owner".into())),
            Some(Constant::Utf8("I".into())),
            Some(Constant::NameAndType {
                name_index: 1,
                descriptor_index: 4,
            }),
            Some(Constant::Utf8("()V".into())),
            Some(Constant::NameAndType {
                name_index: 1,
                descriptor_index: 6,
            }),
        ];
        let mut cache = ConstantValidationCache::new(pool.len());
        validate_constant(&first, &pool, 10, &mut cache).unwrap();
        let error = validate_constant(&second, &pool, 20, &mut cache).unwrap_err();
        assert_eq!(error.code(), "invalid-member-name");
        assert!(error.message().contains("offset 20"));
    }
}

#[test]
fn constant_pool_text_is_subject_to_the_class_materialization_limit() {
    let text = vec![b'x'; u16::MAX as usize];
    let count = u16::try_from(MAX_CLASS_MATERIALIZED_BYTES / text.len() + 2).unwrap();
    let mut bytes = class_with_pool(count, &[]);
    for _ in 1..count {
        bytes.push(1);
        bytes.extend_from_slice(&u16::MAX.to_be_bytes());
        bytes.extend_from_slice(&text);
    }

    let result = parse_prefix(&bytes);
    assert!(matches!(result, Err(error) if error.code() == "class-materialization-limit"));
}

#[test]
fn reserves_two_slots_for_wide_constants() {
    let mut pool = vec![5];
    pool.extend_from_slice(&42_i64.to_be_bytes());
    pool.extend_from_slice(&[3, 0, 0, 0, 7]);
    let parsed = parse_prefix(&class_with_pool(4, &pool)).unwrap();
    assert_eq!(
        parsed.constant_pool,
        vec![
            None,
            Some(Constant::Long(42)),
            None,
            Some(Constant::Integer(7))
        ]
    );
}

#[test]
fn truncated_input_reports_exact_offset() {
    let error = parse_prefix(&[0xCA, 0xFE]).unwrap_err();
    assert_eq!(error.code(), "truncated");
    assert!(error.message().contains("offset 0"));
}

#[test]
fn rejects_unknown_tag_at_its_offset() {
    let error = parse_prefix(&class_with_pool(2, &[99])).unwrap_err();
    assert_eq!(error.code(), "unknown-constant-tag");
    assert!(error.message().contains("offset 10"));
}

#[test]
fn rejects_wide_constant_in_last_slot() {
    let mut pool = vec![6];
    pool.extend_from_slice(&0_u64.to_be_bytes());
    assert_eq!(
        parse_prefix(&class_with_pool(2, &pool)).unwrap_err().code(),
        "invalid-wide-constant"
    );
}
