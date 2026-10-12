use super::*;
use crate::*;

#[test]
fn canonical_inventory_preserves_runtime_ownership_and_native_signatures() {
    use BootstrapOwner::{Bluetooth, Cldc, Gcf, Midp, Mmapi, Rms};
    use BootstrapRequirement::{Core, Jsr, VendorApi};

    let inventory = production_bootstrap_inventory();
    let entry = |name: &str| {
        inventory
            .iter()
            .find(|entry| entry.class.class_name(entry.class.this_class) == Some(name))
            .unwrap_or_else(|| panic!("missing bootstrap class {name}"))
    };
    // Vendor Light/Vibrator, Siemens media, and Nokia DirectUtils already
    // have their complete ownership/capability contracts checked below and
    // in display_audio/profile_surface.
    for (name, owner, requirement) in [
        ("java/lang/String", Cldc, Core),
        ("javax/microedition/lcdui/Display", Midp, Core),
        ("javax/microedition/lcdui/Canvas", Midp, Core),
        ("javax/microedition/lcdui/Image", Midp, Core),
        ("zh/system/GameMIDlet", Midp, VendorApi("zh-system")),
        (
            "javax/microedition/khronos/egl/EGLContext",
            Midp,
            Jsr("239"),
        ),
        (
            "javax/microedition/io/file/FileConnectionImpl$FileInput",
            Gcf,
            Jsr("75"),
        ),
        (
            "javax/microedition/io/file/ConnectionClosedException",
            Gcf,
            Jsr("75"),
        ),
        ("javax/bluetooth/LocalDevice", Bluetooth, Jsr("82")),
    ] {
        let actual = entry(name);
        assert_eq!(
            (actual.owner, actual.requirement),
            (owner, requirement),
            "{name}"
        );
    }

    // Check the runtime declarations directly; a formatted binding string
    // constructed from these same fields cannot verify a native contract.
    for (owner, class_name, name, descriptor) in [
        (Midp, "javax/microedition/lcdui/Display", "numColors", "()I"),
        (Midp, "javax/microedition/lcdui/Display", "vibrate", "(I)Z"),
        (
            Midp,
            "javax/microedition/lcdui/Graphics",
            "arc",
            "(IIIIIIZ)V",
        ),
        (
            Rms,
            "javax/microedition/rms/RecordStore",
            "recordSize0",
            "(JI)I",
        ),
        (
            Midp,
            "javax/microedition/khronos/opengles/GLImpl",
            "glDisableClientState",
            "(I)V",
        ),
        (
            Gcf,
            "javax/microedition/io/file/FileConnectionImpl",
            "outputOffset0",
            "(Ljava/lang/String;J)J",
        ),
        (
            Gcf,
            "javax/microedition/io/file/FileConnectionImpl",
            "revision0",
            "()J",
        ),
        (
            Mmapi,
            "com/siemens/mp/media/PlayerImpl",
            "createLocator0",
            "(Ljava/lang/String;)J",
        ),
        (
            Midp,
            "javax/microedition/lcdui/Display",
            "__setTextInputActive",
            "(Z)V",
        ),
        (
            Bluetooth,
            "javax/bluetooth/LocalDevice",
            "getProperty",
            "(Ljava/lang/String;)Ljava/lang/String;",
        ),
        (Cldc, "java/lang/Thread", "<init>", "()V"),
        (
            Cldc,
            "java/lang/Thread",
            "<init>",
            "(Ljava/lang/Runnable;)V",
        ),
        (Cldc, "java/lang/Thread", "<init>", "(Ljava/lang/String;)V"),
        (
            Cldc,
            "java/lang/Thread",
            "<init>",
            "(Ljava/lang/Runnable;Ljava/lang/String;)V",
        ),
    ] {
        let actual = entry(class_name);
        assert_eq!(actual.owner, owner, "{class_name}");
        assert!(
            actual.class.methods.iter().any(|method| {
                actual.class.utf8(method.name_index) == Some(name)
                    && actual.class.utf8(method.descriptor_index) == Some(descriptor)
                    && method.access_flags & ACC_NATIVE != 0
            }),
            "missing native {class_name}::{name}{descriptor}"
        );
    }
}

#[test]
fn owned_3d_bootstrap_classes_match_their_canonical_inventories() {
    let inventory = production_bootstrap_inventory();
    for (owner, expected) in [
        (BootstrapOwner::M3g, m3g::bootstrap_classes()),
        (BootstrapOwner::Micro3d, micro3d::bootstrap_classes()),
    ] {
        let actual = inventory
            .iter()
            .filter(|entry| entry.owner == owner)
            .map(|entry| &entry.class)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected.iter().collect::<Vec<_>>());
    }
}

#[test]
fn rust_bootstrap_contains_nokia_device_control() {
    let classes = production_bootstrap_classes();
    let device_control = classes
        .iter()
        .find(|class| class.class_name(class.this_class) == Some("com/nokia/mid/ui/DeviceControl"))
        .unwrap();

    assert_eq!(
        device_control.class_name(device_control.super_class),
        Some("java/lang/Object")
    );
    assert_eq!(
        device_control.access_flags,
        ACC_PUBLIC | ACC_FINAL | ACC_SUPER
    );

    let constructor = device_control
        .methods
        .iter()
        .find(|method| device_control.utf8(method.name_index) == Some("<init>"))
        .unwrap();
    assert_eq!(constructor.access_flags, ACC_PRIVATE);

    let set_lights = device_control
        .methods
        .iter()
        .find(|method| {
            device_control.utf8(method.name_index) == Some("setLights")
                && device_control.utf8(method.descriptor_index) == Some("(II)V")
        })
        .unwrap();
    assert_eq!(
        set_lights.access_flags,
        ACC_PUBLIC | ACC_STATIC | ACC_NATIVE
    );
    assert!(set_lights.attributes.is_empty());
}

#[test]
fn rust_bootstrap_contains_profile_gated_siemens_light() {
    let inventory = production_bootstrap_inventory();
    let entry = inventory
        .iter()
        .find(|entry| {
            entry.class.class_name(entry.class.this_class) == Some("com/siemens/mp/game/Light")
        })
        .unwrap();

    assert_eq!(entry.owner, BootstrapOwner::Cldc);
    assert_eq!(
        entry.requirement,
        BootstrapRequirement::VendorApi("siemens-game")
    );
    assert_eq!(entry.class.access_flags, ACC_PUBLIC | ACC_SUPER);
    assert_eq!(
        entry.class.class_name(entry.class.super_class),
        Some("java/lang/Object")
    );

    let public_surface = entry
        .class
        .methods
        .iter()
        .map(|method| {
            (
                entry.class.utf8(method.name_index).unwrap(),
                entry.class.utf8(method.descriptor_index).unwrap(),
                method.access_flags,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        public_surface,
        vec![
            ("<init>", "()V", ACC_PUBLIC),
            ("setLightOff", "()V", ACC_PUBLIC | ACC_STATIC | ACC_NATIVE),
            ("setLightOn", "()V", ACC_PUBLIC | ACC_STATIC | ACC_NATIVE),
        ]
    );
    assert!(matches!(
        entry.class.methods[0].attributes.first(),
        Some(Attribute::Code(_))
    ));
    assert!(entry.class.methods[1].attributes.is_empty());
    assert!(entry.class.methods[2].attributes.is_empty());
}

#[test]
fn rust_bootstrap_contains_profile_gated_siemens_vibrator() {
    let inventory = production_bootstrap_inventory();
    let entry = inventory
        .iter()
        .find(|entry| {
            entry.class.class_name(entry.class.this_class) == Some("com/siemens/mp/game/Vibrator")
        })
        .unwrap();

    assert_eq!(entry.owner, BootstrapOwner::Cldc);
    assert_eq!(
        entry.requirement,
        BootstrapRequirement::VendorApi("siemens-game")
    );
    assert_eq!(entry.class.access_flags, ACC_PUBLIC | ACC_SUPER);
    assert_eq!(
        entry.class.class_name(entry.class.super_class),
        Some("java/lang/Object")
    );
    assert_eq!(
        entry
            .class
            .methods
            .iter()
            .map(|method| (
                entry.class.utf8(method.name_index).unwrap(),
                entry.class.utf8(method.descriptor_index).unwrap(),
                method.access_flags,
            ))
            .collect::<Vec<_>>(),
        vec![
            ("<init>", "()V", ACC_PUBLIC),
            ("startVibrator", "()V", ACC_PUBLIC | ACC_STATIC | ACC_NATIVE,),
            ("stopVibrator", "()V", ACC_PUBLIC | ACC_STATIC | ACC_NATIVE,),
            (
                "triggerVibrator",
                "(I)V",
                ACC_PUBLIC | ACC_STATIC | ACC_NATIVE,
            ),
        ]
    );
    assert!(matches!(
        entry.class.methods[0].attributes.first(),
        Some(Attribute::Code(_))
    ));
    assert!(
        entry.class.methods[1..]
            .iter()
            .all(|method| method.attributes.is_empty())
    );
}
