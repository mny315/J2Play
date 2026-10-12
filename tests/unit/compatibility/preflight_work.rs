use super::*;

#[test]
fn array_member_owners_only_contribute_external_component_classes() {
    for (owner, external_component) in [
        ("[I", None),
        ("[[I", None),
        ("[Lexample/Application;", None),
        ("[[Lvendor/Element;", Some("vendor/Element")),
    ] {
        let mut class = class_with_descriptor_references();
        class.fields.clear();
        class.methods.clear();
        let start = u16::try_from(class.constant_pool.len()).unwrap();
        class.constant_pool.extend([
            Some(Constant::Utf8(owner.into())),
            Some(Constant::Class { name_index: start }),
            Some(Constant::Utf8("clone".into())),
            Some(Constant::Utf8("()Ljava/lang/Object;".into())),
            Some(Constant::NameAndType {
                name_index: start + 2,
                descriptor_index: start + 3,
            }),
            Some(Constant::Methodref {
                class_index: start + 1,
                name_and_type_index: start + 4,
            }),
        ]);
        let evidence = static_archive_evidence(&[class]).unwrap();
        let expected: Vec<_> = std::iter::once("java/lang/Object")
            .chain(external_component)
            .collect();
        assert_eq!(evidence.external_classes, expected, "owner {owner}");
    }
}

#[test]
fn static_analysis_shares_decoded_memory_budget_across_classes() {
    let archive = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/java-me/conformance.jar",
    ));
    let resource = jar::read_class_entries_bytes(archive)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let mut remaining = MAX_ANALYSIS_CLASS_BYTES;
    let expected = parse_entries_with_budget(vec![resource.clone()], &mut remaining).unwrap();
    let charged = MAX_ANALYSIS_CLASS_BYTES - remaining;
    assert!(charged > 0);

    let mut remaining = 2 * charged;
    let parsed =
        parse_entries_with_budget(vec![resource.clone(), resource.clone()], &mut remaining)
            .unwrap();
    assert_eq!(parsed, [expected[0].clone(), expected[0].clone()]);
    assert_eq!(remaining, 0);

    let mut remaining = 2 * charged - 1;
    assert_eq!(
        parse_entries_with_budget(vec![resource.clone(), resource], &mut remaining)
            .unwrap_err()
            .code(),
        "compatibility-class-parse",
    );
}

#[test]
fn many_members_sharing_long_names_produce_only_unique_external_classes() {
    let mut class = class_with_descriptor_references();
    let owner_name = format!("vendor/{}", "a".repeat(60_000));
    class.constant_pool[6] = Some(Constant::Utf8(format!("L{owner_name};")));
    let owner_name_index = u16::try_from(class.constant_pool.len()).unwrap();
    class
        .constant_pool
        .push(Some(Constant::Utf8(owner_name.clone())));
    let owner = u16::try_from(class.constant_pool.len()).unwrap();
    class.constant_pool.push(Some(Constant::Class {
        name_index: owner_name_index,
    }));
    for index in 0..600 {
        let name_index = u16::try_from(class.constant_pool.len()).unwrap();
        class
            .constant_pool
            .push(Some(Constant::Utf8(format!("field{index}"))));
        let name_and_type_index = u16::try_from(class.constant_pool.len()).unwrap();
        class.constant_pool.push(Some(Constant::NameAndType {
            name_index,
            descriptor_index: 6,
        }));
        class.constant_pool.push(Some(Constant::Fieldref {
            class_index: owner,
            name_and_type_index,
        }));
    }

    let evidence = static_archive_evidence(&[class]).unwrap();
    assert_eq!(
        evidence.external_classes,
        [
            "java/lang/Object".to_owned(),
            "java/nio/ByteBuffer".to_owned(),
            "javax/microedition/m3g/Node".to_owned(),
            owner_name,
        ]
    );
}

#[test]
fn repeated_constant_pool_references_preserve_bounded_preflight_results() {
    let mut class = class_with_descriptor_references();
    let external = format!("vendor/{}", "a".repeat(60_000));
    class.constant_pool[6] = Some(Constant::Utf8(format!("L{external};")));
    let owner_name = u16::try_from(class.constant_pool.len()).unwrap();
    class
        .constant_pool
        .push(Some(Constant::Utf8(external.clone())));
    let owner = u16::try_from(class.constant_pool.len()).unwrap();
    class.constant_pool.push(Some(Constant::Class {
        name_index: owner_name,
    }));
    let name_and_type = u16::try_from(class.constant_pool.len()).unwrap();
    class.constant_pool.push(Some(Constant::NameAndType {
        name_index: 5,
        descriptor_index: 6,
    }));
    let reference = Constant::Fieldref {
        class_index: owner,
        name_and_type_index: name_and_type,
    };
    class.constant_pool.push(Some(reference.clone()));
    let expected = static_archive_evidence(std::slice::from_ref(&class)).unwrap();
    assert!(expected.external_classes.contains(&external));
    for index in 1..4_096 {
        let name_index = u16::try_from(class.constant_pool.len()).unwrap();
        class
            .constant_pool
            .push(Some(Constant::Utf8(format!("field{index}"))));
        class.fields.push(classfile::Member {
            access_flags: 0,
            name_index,
            descriptor_index: 6,
            attributes: vec![],
        });
        class.constant_pool.push(Some(Constant::Class {
            name_index: owner_name,
        }));
        class.constant_pool.push(Some(reference.clone()));
    }
    assert_eq!(
        static_archive_evidence(std::slice::from_ref(&class)).unwrap(),
        expected
    );
}

#[test]
fn descriptor_memoization_is_scoped_to_each_class_and_retains_errors() {
    let first = class_with_descriptor_references();
    let mut second = class_with_descriptor_references();
    second.constant_pool[1] = Some(Constant::Utf8("example/Second".into()));
    second.constant_pool[6] = Some(Constant::Utf8("Lvendor/SecondType;".into()));
    let evidence = static_archive_evidence(&[first.clone(), second.clone()]).unwrap();
    assert!(
        evidence
            .external_classes
            .iter()
            .any(|name| name == "vendor/SecondType")
    );
    assert!(
        evidence
            .external_classes
            .iter()
            .any(|name| name == "javax/microedition/khronos/egl/EGL")
    );
    second.constant_pool[6] = Some(Constant::Utf8("Lunterminated".into()));
    assert_eq!(
        static_archive_evidence(&[first, second])
            .unwrap_err()
            .code(),
        "compatibility-model"
    );
}
