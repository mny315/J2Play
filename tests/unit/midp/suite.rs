use super::*;

fn info(midlets: Vec<MidletInfo>) -> JarInfo {
    JarInfo {
        sha256: String::new(),
        archive_bytes: 0,
        archive_timestamp_millis: None,
        manifest: BTreeMap::from([
            ("MIDlet-Name".to_owned(), "Jar name".to_owned()),
            ("Custom".to_owned(), "jar".to_owned()),
        ]),
        manifest_sections: Vec::new(),
        midlets,
        resources: Vec::new(),
    }
}

#[test]
fn selects_and_merges_jad_properties() {
    let midlet = MidletInfo {
        index: 1,
        name: "Demo".to_owned(),
        icon: None,
        class_name: "demo.Main".to_owned(),
    };
    let jar = info(vec![midlet.clone()]);
    let jad = JadInfo {
        properties: BTreeMap::from([("CUSTOM".to_owned(), "jad".to_owned())]),
        midlets: vec![midlet],
    };
    let suite = describe_suite(&jar, Some(&jad), None).unwrap();
    assert_eq!(suite.properties["custom"], "jad");
    assert_eq!(suite.properties["midlet-name"], "Jar name");
}

#[test]
fn application_properties_strip_only_outer_midp_whitespace_from_both_sources() {
    let midlet = MidletInfo {
        index: 1,
        name: "Demo".to_owned(),
        icon: None,
        class_name: "demo.Main".to_owned(),
    };
    let mut jar = info(vec![midlet]);
    jar.manifest.extend([
        ("Frame-Count".to_owned(), " 8 \t".to_owned()),
        ("Start-Resource".to_owned(), "\t /start.bin \t".to_owned()),
        ("Text".to_owned(), " \talpha  beta\tgamma \t".to_owned()),
        ("Unicode".to_owned(), " \u{a0}text\u{2003} \t".to_owned()),
        ("Empty".to_owned(), " \t ".to_owned()),
    ]);
    let suite = describe_suite(&jar, None, None).unwrap();
    assert_eq!(suite.properties["frame-count"], "8");
    assert_eq!(suite.properties["start-resource"], "/start.bin");
    assert_eq!(suite.properties["text"], "alpha  beta\tgamma");
    assert_eq!(suite.properties["unicode"], "\u{a0}text\u{2003}");
    assert_eq!(suite.properties["empty"], "");
    assert_eq!(jar.manifest["Frame-Count"], " 8 \t");

    let jad = jar::parse_jad(
        b"MIDlet-1: Demo, , demo.Main\r\nFRAME-COUNT:\t 12 \t\r\nText: \tfirst \r\n  second\t\r\nEmpty:\t \r\n",
    )
    .unwrap();
    let suite = describe_suite(&jar, Some(&jad), None).unwrap();
    assert_eq!(suite.properties["frame-count"], "12");
    assert_eq!(suite.properties["start-resource"], "/start.bin");
    assert_eq!(suite.properties["text"], "first  second");
    assert_eq!(suite.properties["unicode"], "\u{a0}text\u{2003}");
    assert_eq!(suite.properties["empty"], "");
}

#[test]
fn multi_midlet_suite_defaults_to_first_entry() {
    let entry = |index| MidletInfo {
        index,
        name: format!("Demo {index}"),
        icon: None,
        class_name: format!("demo.Main{index}"),
    };
    let jar = info(vec![entry(1), entry(2)]);
    assert_eq!(describe_suite(&jar, None, None).unwrap().midlet.index, 1);
    assert_eq!(describe_suite(&jar, None, Some(2)).unwrap().midlet.index, 2);
}

#[test]
fn suite_without_midlets_has_a_controlled_diagnostic() {
    let jar = info(Vec::new());
    let error = describe_suite(&jar, None, None).unwrap_err();
    assert_eq!(error.code(), "midlet-selection");
    assert_eq!(error.message(), "suite does not declare a MIDlet");
}

#[test]
fn jad_properties_do_not_hide_manifest_midlets() {
    let entry = |index| MidletInfo {
        index,
        name: format!("Demo {index}"),
        icon: None,
        class_name: format!("demo.Main{index}"),
    };
    let jar = info(vec![entry(1), entry(2)]);
    let mut jad = JadInfo {
        properties: BTreeMap::from([("CUSTOM".to_owned(), "jad".to_owned())]),
        midlets: Vec::new(),
    };
    for index in [1, 2] {
        let suite = describe_suite(&jar, Some(&jad), Some(index)).unwrap();
        assert_eq!(suite.midlet, entry(index));
        assert_eq!(suite.properties["custom"], "jad");
    }
    let replacement = MidletInfo {
        name: "Descriptor name".into(),
        ..entry(2)
    };
    jad.midlets = vec![replacement.clone(), entry(3)];
    assert_eq!(
        describe_suite(&jar, Some(&jad), None).unwrap().midlet,
        entry(1)
    );
    assert_eq!(
        describe_suite(&jar, Some(&jad), Some(2)).unwrap().midlet,
        replacement
    );
    assert_eq!(
        describe_suite(&jar, Some(&jad), Some(3)).unwrap().midlet,
        entry(3)
    );
    assert!(describe_suite(&jar, Some(&jad), Some(4)).is_err());
}
