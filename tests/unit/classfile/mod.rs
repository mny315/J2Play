use super::*;

mod linkage_names;
mod parameter_slots;
mod throughput;

pub(crate) fn class_with_pool(count: u16, pool: &[u8]) -> Vec<u8> {
    let mut bytes = CLASS_MAGIC.to_be_bytes().to_vec();
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&48_u16.to_be_bytes());
    bytes.extend_from_slice(&count.to_be_bytes());
    bytes.extend_from_slice(pool);
    bytes
}

pub(crate) fn minimal_class() -> Vec<u8> {
    let mut pool = Vec::new();
    for value in ["Test", "java/lang/Object", "main", "()V", "Code"] {
        pool.push(1);
        pool.extend_from_slice(&u16::try_from(value.len()).unwrap().to_be_bytes());
        pool.extend_from_slice(value.as_bytes());
        if value == "Test" {
            pool.extend_from_slice(&[7, 0, 1]);
        } else if value == "java/lang/Object" {
            pool.extend_from_slice(&[7, 0, 3]);
        }
    }
    let mut bytes = class_with_pool(8, &pool);
    bytes.extend_from_slice(&0x0021_u16.to_be_bytes());
    bytes.extend_from_slice(&2_u16.to_be_bytes());
    bytes.extend_from_slice(&4_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&0x0009_u16.to_be_bytes());
    bytes.extend_from_slice(&5_u16.to_be_bytes());
    bytes.extend_from_slice(&6_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&7_u16.to_be_bytes());
    bytes.extend_from_slice(&13_u32.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.push(0xB1);
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes
}

#[test]
fn field_declarations_and_references_accept_angle_brackets() {
    for name in ["<", ">", "a<b>", "<init>", "<clinit>"] {
        let mut pool = Vec::new();
        for (value, class_index) in [
            ("Test", Some(1_u16)),
            ("java/lang/Object", Some(3)),
            (name, None),
            ("I", None),
        ] {
            pool.push(1);
            pool.extend_from_slice(&u16::try_from(value.len()).unwrap().to_be_bytes());
            pool.extend_from_slice(value.as_bytes());
            if let Some(index) = class_index {
                pool.push(7);
                pool.extend_from_slice(&index.to_be_bytes());
            }
        }
        // NameAndType and Fieldref point to the same declared field.
        pool.extend_from_slice(&[12, 0, 5, 0, 6, 9, 0, 2, 0, 7]);
        let mut bytes = class_with_pool(9, &pool);
        for value in [0x0021_u16, 2, 4, 0, 1, 1, 5, 6, 0, 0, 0] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        let parsed = parse(&bytes).unwrap_or_else(|error| panic!("field {name:?}: {error}"));
        assert_eq!(parsed.fields.len(), 1);
        assert_eq!(parsed.fields[0].name_index, 5);
    }
}

#[test]
fn rejects_class_versions_outside_target_range() {
    let mut bytes = minimal_class();
    bytes[6..8].copy_from_slice(&51_u16.to_be_bytes());
    assert_eq!(
        parse(&bytes).unwrap_err().code(),
        "unsupported-class-version"
    );
}

#[test]
fn accepts_legacy_minor_stamp_on_cldc_major_version() {
    let mut bytes = minimal_class();
    bytes[4..6].copy_from_slice(&3_u16.to_be_bytes());
    bytes[6..8].copy_from_slice(&47_u16.to_be_bytes());

    let parsed = parse(&bytes).unwrap();

    assert_eq!((parsed.major_version, parsed.minor_version), (47, 3));
}

#[test]
fn parses_complete_class_and_code_attribute() {
    let parsed = parse(&minimal_class()).unwrap();
    assert_eq!(parsed.this_class, 2);
    assert_eq!(parsed.super_class, 4);
    assert_eq!(parsed.methods.len(), 1);
    let Attribute::Code(code) = &parsed.methods[0].attributes[0] else {
        panic!("expected Code attribute");
    };
    assert_eq!(code.code, [0xB1]);
    assert_eq!(code.max_stack, 0);
}

#[test]
fn batch_parsing_charges_each_class_and_restores_budget_after_failure() {
    let bytes = minimal_class();
    let expected = parse(&bytes).unwrap();
    let mut unlimited = usize::MAX;
    assert_eq!(parse_with_budget(&bytes, &mut unlimited).unwrap(), expected);
    let charged = usize::MAX - unlimited;
    assert!(charged > bytes.len());

    let mut remaining = 2 * charged - 1;
    assert_eq!(parse_with_budget(&bytes, &mut remaining).unwrap(), expected);
    assert_eq!(remaining, charged - 1);
    assert_eq!(
        parse_with_budget(&bytes, &mut remaining)
            .unwrap_err()
            .code(),
        "class-materialization-limit",
    );
    assert_eq!(remaining, charged - 1);

    let mut malformed = bytes.clone();
    malformed.push(0);
    let mut remaining = 2 * charged;
    assert_eq!(
        parse_with_budget(&malformed, &mut remaining)
            .unwrap_err()
            .code(),
        "trailing-data",
    );
    assert_eq!(remaining, 2 * charged);
}

#[test]
fn rejects_wrong_constant_type_with_offset() {
    let mut bytes = minimal_class();
    let prefix = parse_prefix(&bytes).unwrap();
    bytes[prefix.next_offset + 2..prefix.next_offset + 4].copy_from_slice(&1_u16.to_be_bytes());
    let error = parse(&bytes).unwrap_err();
    assert_eq!(error.code(), "invalid-constant-type");
    assert!(
        error
            .message()
            .contains(&format!("offset {}", prefix.next_offset + 2))
    );
}

#[test]
fn rejects_trailing_class_data() {
    let mut bytes = minimal_class();
    bytes.push(0);
    assert_eq!(parse(&bytes).unwrap_err().code(), "trailing-data");
}

#[test]
fn many_fields_can_share_one_long_reference_descriptor() {
    let descriptor = format!("L{};", "X".repeat(60_000));
    let mut pool = vec![None, Some(Constant::Utf8(descriptor))];
    let mut bytes = 4096_u16.to_be_bytes().to_vec();
    for number in 0..4096_u16 {
        pool.push(Some(Constant::Utf8(format!("field{number}"))));
        bytes.extend_from_slice(&1_u16.to_be_bytes());
        bytes.extend_from_slice(&(number + 2).to_be_bytes());
        bytes.extend_from_slice(&1_u16.to_be_bytes());
        bytes.extend_from_slice(&0_u16.to_be_bytes());
    }
    let mut reader = Reader::new(&bytes);
    let mut materialization = MaterializationBudget::for_input(128 * 1024);
    let fields = parse_members(
        &mut reader,
        &pool,
        MemberKind::Field,
        &mut materialization,
        &mut ConstantValidationCache::new(pool.len()),
    )
    .unwrap();
    assert_eq!(fields.len(), 4096);
    assert_eq!(reader.remaining(), 0);
    assert!(fields.iter().all(|field| field.descriptor_index == 1));
}

#[test]
fn shared_text_does_not_skip_later_member_flags_or_constructor_checks() {
    for (kind, descriptor, later_name, later_flags, expected) in [
        (
            MemberKind::Field,
            "I",
            "second",
            0x0051,
            "invalid-access-flags",
        ),
        (
            MemberKind::Method,
            "()I",
            "<init>",
            0x0101,
            "invalid-constructor",
        ),
        (
            MemberKind::Method,
            "()V",
            "<clinit>",
            0x0101,
            "invalid-class-initializer",
        ),
    ] {
        let pool = vec![
            None,
            Some(Constant::Utf8("first".to_owned())),
            Some(Constant::Utf8(descriptor.to_owned())),
            Some(Constant::Utf8(later_name.to_owned())),
        ];
        let first_flags = if kind == MemberKind::Field {
            0x0001_u16
        } else {
            0x0101
        };
        let mut bytes = 2_u16.to_be_bytes().to_vec();
        for (flags, name_index) in [(first_flags, 1_u16), (later_flags, 3)] {
            for value in [flags, name_index, 2, 0] {
                bytes.extend_from_slice(&value.to_be_bytes());
            }
        }
        let mut reader = Reader::new(&bytes);
        let mut materialization = MaterializationBudget::for_input(256);
        assert_eq!(
            parse_members(
                &mut reader,
                &pool,
                kind,
                &mut materialization,
                &mut ConstantValidationCache::new(pool.len()),
            )
            .unwrap_err()
            .code(),
            expected
        );
    }
}
