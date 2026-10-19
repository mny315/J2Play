use super::*;

#[test]
fn resolution_support_requires_a_supported_3d_api_reference() {
    for class in [
        "javax/microedition/m3g/Graphics3D",
        "com/mascotcapsule/micro3d/v3/Graphics3D",
    ] {
        assert!(ArchiveEvidence::new(None, [class.to_owned()]).supports_render_resolution());
    }
    for class in [
        "javax/microedition/lcdui/Graphics",
        "javax/microedition/lcdui/game/Sprite",
        "javax/microedition/khronos/opengles/GL10",
        "game/Graphics3D",
    ] {
        assert!(
            !ArchiveEvidence::new(Some("game-m3g.jar".into()), [class.to_owned()])
                .supports_render_resolution()
        );
    }
}

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
            ("SNAKE_SPEED".to_owned(), "null".to_owned()),
        ]),
    };
    let mut properties = BTreeMap::from([
        ("c2m-langlist".to_owned(), "de-DE".to_owned()),
        ("has-blood".to_owned(), "1".to_owned()),
        ("has_blood".to_owned(), "1".to_owned()),
        ("url-operator".to_owned(), "carrier".to_owned()),
        ("unrelated".to_owned(), "preserved".to_owned()),
        ("snake_speed".to_owned(), "98304".to_owned()),
    ]);

    let applied = evidence.apply_descriptorless_suite_property_defaults(&mut properties);

    assert_eq!(applied, vec![("URL-SUPPORT".to_owned(), "0".to_owned())]);
    assert_eq!(properties["c2m-langlist"], "de-DE");
    assert_eq!(properties["has-blood"], "1");
    assert_eq!(properties["has_blood"], "1");
    assert_eq!(properties["url-operator"], "carrier");
    assert_eq!(properties["url-support"], "0");
    assert_eq!(properties["unrelated"], "preserved");
    assert_eq!(properties["snake_speed"], "98304");
}
