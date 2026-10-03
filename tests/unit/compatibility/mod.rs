use super::*;

mod preflight_work;

fn class_with_descriptor_references() -> ClassFile {
    ClassFile {
        minor_version: 0,
        major_version: 52,
        constant_pool: vec![
            None,
            Some(Constant::Utf8("example/Application".to_owned())),
            Some(Constant::Class { name_index: 1 }),
            Some(Constant::Utf8("java/lang/Object".to_owned())),
            Some(Constant::Class { name_index: 3 }),
            Some(Constant::Utf8("field".to_owned())),
            Some(Constant::Utf8(
                "[Ljavax/microedition/khronos/egl/EGL;".to_owned(),
            )),
            Some(Constant::Utf8("method".to_owned())),
            Some(Constant::Utf8(
                "(Ljava/nio/ByteBuffer;)Ljavax/microedition/m3g/Node;".to_owned(),
            )),
        ],
        access_flags: 0x0021,
        this_class: 2,
        super_class: 4,
        interfaces: Vec::new(),
        fields: vec![classfile::Member {
            access_flags: 0,
            name_index: 5,
            descriptor_index: 6,
            attributes: Vec::new(),
        }],
        methods: vec![classfile::Member {
            access_flags: 0,
            name_index: 7,
            descriptor_index: 8,
            attributes: Vec::new(),
        }],
        attributes: Vec::new(),
    }
}

fn class_with_suite_property_literals(
    literals: &[&str],
    include_property_reader: bool,
) -> ClassFile {
    let mut constant_pool = vec![
        None,
        Some(Constant::Utf8("example/Application".to_owned())),
        Some(Constant::Class { name_index: 1 }),
        Some(Constant::Utf8("java/lang/Object".to_owned())),
        Some(Constant::Class { name_index: 3 }),
        Some(Constant::Utf8(
            "javax/microedition/midlet/MIDlet".to_owned(),
        )),
        Some(Constant::Class { name_index: 5 }),
        Some(Constant::Utf8("getAppProperty".to_owned())),
        Some(Constant::Utf8(
            "(Ljava/lang/String;)Ljava/lang/String;".to_owned(),
        )),
        Some(Constant::NameAndType {
            name_index: 7,
            descriptor_index: 8,
        }),
        include_property_reader.then_some(Constant::Methodref {
            class_index: 6,
            name_and_type_index: 9,
        }),
    ];
    for literal in literals {
        let string_index = u16::try_from(constant_pool.len()).unwrap();
        constant_pool.push(Some(Constant::Utf8((*literal).to_owned())));
        constant_pool.push(Some(Constant::String { string_index }));
    }
    ClassFile {
        minor_version: 0,
        major_version: 50,
        constant_pool,
        access_flags: 0x0021,
        this_class: 2,
        super_class: 4,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: Vec::new(),
        attributes: Vec::new(),
    }
}

#[test]
fn static_evidence_includes_classes_referenced_only_by_member_descriptors() {
    let evidence = static_archive_evidence(&[class_with_descriptor_references()]).unwrap();
    let names = evidence
        .external_classes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert!(names.contains("javax/microedition/khronos/egl/EGL"));
    assert!(names.contains("java/nio/ByteBuffer"));
    assert!(names.contains("javax/microedition/m3g/Node"));
}

#[test]
fn descriptorless_legacy_igp_contract_gets_only_its_neutral_defaults() {
    let evidence = static_archive_evidence(&[class_with_suite_property_literals(
        &["URL-OPERATOR", "URL-SUPPORT"],
        true,
    )])
    .unwrap();
    assert_eq!(
        evidence.descriptorless_suite_property_defaults,
        BTreeMap::from([
            ("URL-OPERATOR".to_owned(), "0".to_owned()),
            ("URL-SUPPORT".to_owned(), "0".to_owned()),
        ])
    );

    let incomplete =
        static_archive_evidence(&[class_with_suite_property_literals(&["URL-OPERATOR"], true)])
            .unwrap();
    assert!(incomplete.descriptorless_suite_property_defaults.is_empty());

    let iap = static_archive_evidence(&[class_with_suite_property_literals(
        &["URL-OPERATOR", "URL-SUPPORT", "IAP-EnableIAP"],
        true,
    )])
    .unwrap();
    assert_eq!(
        iap.descriptorless_suite_property_defaults,
        BTreeMap::from([
            ("IAP-EnableIAP".to_owned(), "0".to_owned()),
            ("URL-OPERATOR".to_owned(), "0".to_owned()),
            ("URL-SUPPORT".to_owned(), "0".to_owned()),
        ])
    );

    let unscoped_iap =
        static_archive_evidence(&[class_with_suite_property_literals(&["IAP-EnableIAP"], true)])
            .unwrap();
    assert!(
        unscoped_iap
            .descriptorless_suite_property_defaults
            .is_empty()
    );
}

#[test]
fn descriptorless_legacy_content_flag_gets_its_neutral_default() {
    for name in ["Has_blood", "HAS-BLOOD"] {
        let evidence =
            static_archive_evidence(&[class_with_suite_property_literals(&[name], true)]).unwrap();
        assert_eq!(
            evidence.descriptorless_suite_property_defaults,
            BTreeMap::from([(name.to_owned(), "0".to_owned())])
        );
    }

    let both = static_archive_evidence(&[class_with_suite_property_literals(
        &["Has_blood", "HAS-BLOOD"],
        true,
    )])
    .unwrap();
    assert_eq!(
        both.descriptorless_suite_property_defaults,
        BTreeMap::from([
            ("HAS-BLOOD".to_owned(), "0".to_owned()),
            ("Has_blood".to_owned(), "0".to_owned()),
        ])
    );

    let unrelated =
        static_archive_evidence(&[class_with_suite_property_literals(&["Unrelated"], true)])
            .unwrap();
    assert!(unrelated.descriptorless_suite_property_defaults.is_empty());

    let no_reader = static_archive_evidence(&[class_with_suite_property_literals(
        &["Has_blood", "HAS-BLOOD"],
        false,
    )])
    .unwrap();
    assert!(no_reader.descriptorless_suite_property_defaults.is_empty());
}

#[test]
fn descriptorless_c2m_language_list_comes_from_bundled_metadata() {
    let application = [class_with_suite_property_literals(
        &[LEGACY_C2M_LANGUAGE_LIST_PROPERTY],
        true,
    )];
    let mut contracts = recognized_descriptorless_suite_property_contracts(&application).unwrap();
    add_c2m_language_list_default(
        &mut contracts,
        Some(b"3\r\nen-GB\r\nfr-FR\r\npt_BR\r\nEnglish\r\nFran\xe7ais\r\n"),
    );
    let evidence = static_archive_evidence_with_contracts(&application, contracts).unwrap();

    assert_eq!(
        evidence.descriptorless_suite_property_defaults,
        BTreeMap::from([(
            LEGACY_C2M_LANGUAGE_LIST_PROPERTY.to_owned(),
            "en-GB,fr-FR,pt_BR".to_owned(),
        )])
    );
}

#[test]
fn c2m_language_default_requires_the_reader_contract_and_valid_metadata() {
    for metadata in [
        &b"0\r\n"[..],
        &b"2\r\nen-GB\r\n"[..],
        &b"1\r\n../en-GB\r\n"[..],
        &b"2\r\nen-GB\r\nen-GB\r\n"[..],
    ] {
        assert_eq!(c2m_language_list_default(metadata), None);
    }

    let application = [class_with_suite_property_literals(
        &[LEGACY_C2M_LANGUAGE_LIST_PROPERTY],
        false,
    )];
    let mut contracts = recognized_descriptorless_suite_property_contracts(&application).unwrap();
    add_c2m_language_list_default(&mut contracts, Some(b"1\r\nen-GB\r\n"));
    assert!(contracts.defaults.is_empty());

    let unrelated = [class_with_suite_property_literals(&["Unrelated"], true)];
    let mut contracts = recognized_descriptorless_suite_property_contracts(&unrelated).unwrap();
    add_c2m_language_list_default(&mut contracts, Some(b"1\r\nen-GB\r\n"));
    assert!(contracts.defaults.is_empty());
}

#[test]
fn oversized_optional_language_metadata_preserves_other_static_evidence() {
    let application = [class_with_suite_property_literals(
        &[LEGACY_C2M_LANGUAGE_LIST_PROPERTY, "Has_blood"],
        true,
    )];
    let expected = static_archive_evidence(&application).unwrap();
    let evidence = static_archive_evidence_from_classes(&application, |name, maximum_bytes| {
        assert_eq!(name, LEGACY_C2M_LANGUAGE_METADATA_RESOURCE);
        assert_eq!(maximum_bytes, 64 * 1024);
        Err(EmuError::new(
            Category::Jar,
            "entry-too-large",
            "optional language table exceeds its read limit",
        ))
    })
    .unwrap();

    assert_eq!(evidence, expected);
    assert_eq!(
        evidence.descriptorless_suite_property_defaults["Has_blood"],
        "0"
    );
}

#[test]
fn optional_language_metadata_preserves_read_errors_and_skips_unrequested_reads() {
    let application = [class_with_suite_property_literals(
        &[LEGACY_C2M_LANGUAGE_LIST_PROPERTY],
        true,
    )];
    for code in ["invalid-zip", "entry-size-mismatch", "io"] {
        let error = static_archive_evidence_from_classes(&application, |_, _| {
            Err(EmuError::new(Category::Jar, code, "unreadable metadata"))
        })
        .unwrap_err();
        assert_eq!(error.code(), code);
    }

    let unrelated = [class_with_suite_property_literals(&["Unrelated"], true)];
    let evidence = static_archive_evidence_from_classes(&unrelated, |_, _| {
        panic!("unrequested language metadata must not be opened");
    })
    .unwrap();
    assert_eq!(evidence, static_archive_evidence(&unrelated).unwrap());
}
