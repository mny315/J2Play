use super::*;
use crate::*;

#[test]
fn cldc_collection_methods_retain_the_specified_monitor_contract() {
    // CLDC 1.0 API specification, Hashtable pp. 229-231 and Vector pp. 245-251.
    // Scalar accessors and wrappers are not all declared synchronized.
    let expected = [
        (
            "java/util/Hashtable",
            vec![
                ("clear", "()V"),
                ("contains", "(Ljava/lang/Object;)Z"),
                ("containsKey", "(Ljava/lang/Object;)Z"),
                ("elements", "()Ljava/util/Enumeration;"),
                ("get", "(Ljava/lang/Object;)Ljava/lang/Object;"),
                ("keys", "()Ljava/util/Enumeration;"),
                (
                    "put",
                    "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                ),
                ("remove", "(Ljava/lang/Object;)Ljava/lang/Object;"),
                ("toString", "()Ljava/lang/String;"),
            ],
        ),
        (
            "java/util/Vector",
            vec![
                ("addElement", "(Ljava/lang/Object;)V"),
                ("copyInto", "([Ljava/lang/Object;)V"),
                ("elementAt", "(I)Ljava/lang/Object;"),
                ("elements", "()Ljava/util/Enumeration;"),
                ("ensureCapacity", "(I)V"),
                ("firstElement", "()Ljava/lang/Object;"),
                ("indexOf", "(Ljava/lang/Object;I)I"),
                ("insertElementAt", "(Ljava/lang/Object;I)V"),
                ("lastElement", "()Ljava/lang/Object;"),
                ("lastIndexOf", "(Ljava/lang/Object;I)I"),
                ("removeAllElements", "()V"),
                ("removeElement", "(Ljava/lang/Object;)Z"),
                ("removeElementAt", "(I)V"),
                ("setElementAt", "(Ljava/lang/Object;I)V"),
                ("setSize", "(I)V"),
                ("toString", "()Ljava/lang/String;"),
                ("trimToSize", "()V"),
            ],
        ),
    ];
    let classes = production_bootstrap_classes();
    for (name, expected) in expected {
        let class = classes
            .iter()
            .find(|class| class.class_name(class.this_class) == Some(name))
            .unwrap();
        let actual = class
            .methods
            .iter()
            .filter(|method| method.access_flags & ACC_SYNCHRONIZED != 0)
            .map(|method| {
                (
                    class.utf8(method.name_index).unwrap(),
                    class.utf8(method.descriptor_index).unwrap(),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected.into_iter().collect(), "{name}");
    }
}

#[test]
fn rust_bootstrap_connects_nokia_direct_graphics_to_lcdui_graphics() {
    let classes = production_bootstrap_classes();
    let direct_graphics = classes
        .iter()
        .find(|class| class.class_name(class.this_class) == Some("com/nokia/mid/ui/DirectGraphics"))
        .unwrap();
    assert_eq!(
        direct_graphics.access_flags,
        ACC_PUBLIC | ACC_INTERFACE | ACC_ABSTRACT
    );
    let interface_surface = direct_graphics
        .methods
        .iter()
        .map(|method| {
            assert_eq!(method.access_flags, ACC_PUBLIC | ACC_ABSTRACT);
            (
                direct_graphics.utf8(method.name_index).unwrap(),
                direct_graphics.utf8(method.descriptor_index).unwrap(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        interface_surface,
        nokia_direct_graphics_methods()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
    );

    let graphics = classes
        .iter()
        .find(|class| {
            class.class_name(class.this_class) == Some("javax/microedition/lcdui/Graphics")
        })
        .unwrap();
    assert!(graphics.interfaces.iter().any(|interface| {
        graphics.class_name(*interface) == Some("com/nokia/mid/ui/DirectGraphics")
    }));
    for (name, descriptor) in nokia_direct_graphics_methods() {
        let method = graphics
            .methods
            .iter()
            .find(|method| {
                graphics.utf8(method.name_index) == Some(name)
                    && graphics.utf8(method.descriptor_index) == Some(descriptor)
            })
            .unwrap();
        assert_eq!(method.access_flags, ACC_PUBLIC | ACC_NATIVE);
    }

    let direct_utils = classes
        .iter()
        .find(|class| class.class_name(class.this_class) == Some("com/nokia/mid/ui/DirectUtils"))
        .unwrap();
    let get_direct = direct_utils
        .methods
        .iter()
        .find(|method| direct_utils.utf8(method.name_index) == Some("getDirectGraphics"))
        .unwrap();
    assert_eq!(
        get_direct.access_flags,
        ACC_PUBLIC | ACC_STATIC | ACC_NATIVE
    );
    assert_eq!(
        direct_utils.utf8(get_direct.descriptor_index),
        Some("(Ljavax/microedition/lcdui/Graphics;)Lcom/nokia/mid/ui/DirectGraphics;")
    );
}

#[test]
fn public_cldc_surface_matches_direct_fixture_inventory() {
    let document: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/java-me/cldc-methods.json")).unwrap();
    let mut expected = document["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|fixture| fixture["signature"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    expected.extend(
        RUST_DIRECT_COMPATIBILITY_FIXTURES
            .iter()
            .map(|signature| (*signature).to_owned()),
    );
    let actual = production_bootstrap_inventory()
        .into_iter()
        .filter(|entry| entry.requirement == BootstrapRequirement::Core)
        .map(|entry| entry.class)
        .filter_map(|class| {
            let name = class.class_name(class.this_class)?.to_owned();
            (class.access_flags & ACC_PUBLIC != 0
                && name.starts_with("java/")
                && !name.contains('$'))
            .then_some((name, class))
        })
        .flat_map(|(name, class)| {
            class
                .methods
                .iter()
                .filter(|method| method.access_flags & ACC_PUBLIC != 0)
                .map(|method| {
                    format!(
                        "{name}::{}{}",
                        class.utf8(method.name_index).unwrap(),
                        class.utf8(method.descriptor_index).unwrap()
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
}
