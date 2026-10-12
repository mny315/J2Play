use super::*;

fn speed_contract(literals: &[&str], reader: bool, comparison: bool) -> ClassFile {
    let mut class = class_with_suite_property_literals(literals, reader);
    if comparison {
        let start = u16::try_from(class.constant_pool.len()).unwrap();
        class.constant_pool.extend([
            Some(Constant::Utf8("java/lang/String".into())),
            Some(Constant::Class { name_index: start }),
            Some(Constant::Utf8("equals".into())),
            Some(Constant::Utf8("(Ljava/lang/Object;)Z".into())),
            Some(Constant::NameAndType {
                name_index: start + 2,
                descriptor_index: start + 3,
            }),
            Some(Constant::Methodref {
                class_index: start + 1,
                name_and_type_index: start + 4,
            }),
        ]);
    }
    class
}

fn subclass(name: &str, parent: &str) -> ClassFile {
    let mut class = class_with_suite_property_literals(&[], false);
    class.constant_pool[1] = Some(Constant::Utf8(name.into()));
    class.constant_pool[3] = Some(Constant::Utf8(parent.into()));
    class
}

#[test]
fn neutral_speed_default_requires_the_local_reader_and_sentinel_contract() {
    let class = speed_contract(&["SNAKE_SPEED", "null"], true, true);
    let evidence = static_archive_evidence(&[class]).unwrap();
    assert_eq!(
        evidence.descriptorless_suite_property_defaults,
        BTreeMap::from([("SNAKE_SPEED".into(), "null".into())])
    );
    for class in [
        speed_contract(&["SNAKE_SPEED"], true, true),
        speed_contract(&["SNAKE_SPEED", "null"], false, true),
        speed_contract(&["SNAKE_SPEED", "null"], true, false),
        speed_contract(&["OTHER_SPEED", "null"], true, true),
    ] {
        assert!(
            static_archive_evidence(&[class])
                .unwrap()
                .descriptorless_suite_property_defaults
                .is_empty()
        );
    }
    let separate_classes = [
        speed_contract(&["SNAKE_SPEED"], true, true),
        subclass("example/Other", "java/lang/Object"),
        speed_contract(&["null"], false, false),
    ];
    assert!(
        static_archive_evidence(&separate_classes)
            .unwrap()
            .descriptorless_suite_property_defaults
            .is_empty()
    );
}

#[test]
fn inherited_property_references_resolve_through_midlet_subclasses() {
    let mut reader = speed_contract(&["SNAKE_SPEED", "null"], true, true);
    reader.constant_pool[5] = Some(Constant::Utf8("example/Leaf".into()));
    let evidence = static_archive_evidence(&[
        reader,
        subclass("example/Leaf", "example/Base"),
        subclass("example/Base", MIDLET_CLASS),
    ])
    .unwrap();
    assert_eq!(
        evidence.descriptorless_suite_property_defaults["SNAKE_SPEED"],
        "null"
    );
}

#[test]
fn unrelated_and_cyclic_property_owners_do_not_get_defaults() {
    for parent in ["java/lang/Object", "example/Leaf", "example/Missing"] {
        let mut reader = speed_contract(&["SNAKE_SPEED", "null"], true, true);
        reader.constant_pool[5] = Some(Constant::Utf8("example/Leaf".into()));
        let evidence = static_archive_evidence(&[
            reader,
            subclass("example/Leaf", "example/Base"),
            subclass("example/Base", parent),
        ])
        .unwrap();
        assert!(evidence.descriptorless_suite_property_defaults.is_empty());
    }
}

#[test]
fn a_declared_property_method_does_not_inherit_the_midlet_reader_contract() {
    let mut reader = speed_contract(&["SNAKE_SPEED", "null"], true, true);
    reader.constant_pool[5] = Some(Constant::Utf8("example/Leaf".into()));
    let mut base = subclass("example/Base", MIDLET_CLASS);
    base.methods.push(classfile::Member {
        access_flags: 0x0001,
        name_index: 7,
        descriptor_index: 8,
        attributes: Vec::new(),
    });
    let evidence =
        static_archive_evidence(&[reader, subclass("example/Leaf", "example/Base"), base]).unwrap();
    assert!(evidence.descriptorless_suite_property_defaults.is_empty());
}
