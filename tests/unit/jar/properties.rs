use super::*;

#[test]
fn manifest_case_index_preserves_first_spelling_and_replacement_rules() {
    let parsed = parse_manifest(
        b"Manifest-Version: 1.0\nMIDlet-Vendor: First\nmidlet-vendor: Second\n value\n",
    )
    .unwrap();

    assert_eq!(parsed.main["MIDlet-Vendor"], "Secondvalue");
    assert!(!parsed.main.contains_key("midlet-vendor"));
    assert_eq!(
        parsed
            .main
            .keys()
            .filter(|key| key.eq_ignore_ascii_case("MIDlet-Vendor"))
            .count(),
        1
    );
}

#[test]
fn parses_jad_with_continuation() {
    let jad =
        parse_jad(b"MIDlet-Name: Demo\r\nMIDlet-1: Demo, /icon.png, demo.\r\n Main\r\n").unwrap();
    assert_eq!(jad.midlets[0].class_name, "demo.Main");
    assert_eq!(jad.properties["MIDlet-Name"], "Demo");
}

#[test]
fn property_continuations_reassemble_utf8_before_decoding() {
    for character in ["é", "€", "😀"] {
        for split in 1..character.len() {
            let prefix = "x".repeat(72 - b"MIDlet-Name: ".len() - split);
            let mut source = format!("MIDlet-Name: {prefix}").into_bytes();
            source.extend_from_slice(&character.as_bytes()[..split]);
            assert_eq!(source.len(), 72);
            source.extend_from_slice(b"\r\n ");
            source.extend_from_slice(&character.as_bytes()[split..]);
            source.extend_from_slice(b"\r\nMIDlet-1: Demo, , demo.Main\r\n");

            let expected = format!("{prefix}{character}");
            assert_eq!(
                parse_manifest(&source).unwrap().main["MIDlet-Name"],
                expected
            );
            assert_eq!(
                parse_jad(&source).unwrap().properties["MIDlet-Name"],
                expected
            );
        }
    }
}

#[test]
fn jad_continuations_still_require_a_field_and_valid_complete_utf8() {
    for source in [
        &b"MIDlet-Name: \xc3\r\n x\r\n"[..],
        &b"MIDlet-Name: \xf0\r\n \x9f\x98\r\n"[..],
    ] {
        assert_eq!(parse_jad(source).unwrap_err().code(), "property-encoding");
    }
    for source in [
        &b" MIDlet-Name: Demo\n"[..],
        &b"MIDlet-Name: Demo\n\n detached\n"[..],
    ] {
        assert_eq!(
            parse_jad(source).unwrap_err().code(),
            "manifest-continuation"
        );
    }
}

#[test]
fn manifest_line_endings_preserve_properties_continuations_and_sections() {
    for newline in ["\n", "\r\n", "\r"] {
        let source = [
            "Manifest-Version: 1.0",
            "MIDlet-Name: Demo",
            "MIDlet-1: Demo, , demo.",
            " Main",
            "",
            "Name: demo/Main.class",
            "X-Attribute: section value",
            "",
        ]
        .join(newline);
        let parsed = parse_manifest(source.as_bytes()).unwrap();
        assert_eq!(parsed.main["MIDlet-Name"], "Demo");
        assert_eq!(
            parse_midlets(&parsed.main).unwrap()[0].class_name,
            "demo.Main"
        );
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].name, "demo/Main.class");
        assert_eq!(
            parsed.sections[0].attributes["X-Attribute"],
            "section value"
        );
        assert!(!parsed.main.contains_key("X-Attribute"));
    }
}

#[test]
fn long_property_names_do_not_amplify_continuation_work() {
    let key = "X".repeat(512 * 1024);
    let continuations = 160_000;
    let source = format!("{key}: v\n{}", " x\n".repeat(continuations));
    assert!(source.len() as u64 <= MAX_JAD_BYTES);
    let jad = parse_jad(source.as_bytes()).unwrap();
    assert_eq!(jad.properties.len(), 1);
    assert_eq!(
        jad.properties[&key],
        format!("v{}", "x".repeat(continuations))
    );
}

#[test]
fn parses_midlet_attributes_case_insensitively() {
    let jad = parse_jad(b"midlet-1: Demo, , demo.Main\n").unwrap();
    assert_eq!(jad.midlets[0].class_name, "demo.Main");
}

#[test]
fn rejects_duplicate_jad_properties_case_insensitively() {
    let error = parse_jad(b"MIDlet-Name: A\nmidlet-name: B\n").unwrap_err();
    assert_eq!(error.code(), "manifest-field");
}
