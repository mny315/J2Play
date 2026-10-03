use super::*;

#[test]
fn descriptorless_defaults_never_replace_explicit_suite_values() {
    let evidence = ArchiveEvidence {
        archive_name: None,
        external_classes: Vec::new(),
        descriptorless_suite_property_defaults: BTreeMap::from([
            ("C2M-LangList".to_owned(), "en-GB,fr-FR".to_owned()),
            ("HAS-BLOOD".to_owned(), "0".to_owned()),
            ("Has_blood".to_owned(), "0".to_owned()),
            ("URL-OPERATOR".to_owned(), "0".to_owned()),
            ("URL-SUPPORT".to_owned(), "0".to_owned()),
        ]),
    };
    let mut properties = BTreeMap::from([
        ("c2m-langlist".to_owned(), "de-DE".to_owned()),
        ("has-blood".to_owned(), "1".to_owned()),
        ("has_blood".to_owned(), "1".to_owned()),
        ("url-operator".to_owned(), "carrier".to_owned()),
        ("unrelated".to_owned(), "preserved".to_owned()),
    ]);

    let applied = evidence.apply_descriptorless_suite_property_defaults(&mut properties);

    assert_eq!(applied, vec![("URL-SUPPORT".to_owned(), "0".to_owned())]);
    assert_eq!(properties["c2m-langlist"], "de-DE");
    assert_eq!(properties["has-blood"], "1");
    assert_eq!(properties["has_blood"], "1");
    assert_eq!(properties["url-operator"], "carrier");
    assert_eq!(properties["url-support"], "0");
    assert_eq!(properties["unrelated"], "preserved");
}
