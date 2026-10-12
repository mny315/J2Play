use super::*;
use crate::*;

#[test]
fn rust_bootstrap_contains_mmapi_owned_samsung_audio_clip() {
    let inventory = production_bootstrap_inventory();
    let audio_clip = inventory
        .iter()
        .find(|entry| {
            entry.class.class_name(entry.class.this_class) == Some("com/samsung/util/AudioClip")
        })
        .unwrap();
    assert_eq!(audio_clip.owner, BootstrapOwner::Mmapi);
    assert_eq!(audio_clip.class.access_flags, ACC_PUBLIC | ACC_SUPER);
    for (name, expected) in [("TYPE_MMF", 1), ("TYPE_MP3", 2), ("TYPE_MIDI", 3)] {
        let field = audio_clip
            .class
            .fields
            .iter()
            .find(|field| audio_clip.class.utf8(field.name_index) == Some(name))
            .unwrap();
        assert_eq!(field.access_flags, ACC_PUBLIC | ACC_STATIC | ACC_FINAL);
        assert_eq!(audio_clip.class.utf8(field.descriptor_index), Some("I"));
        let Attribute::Raw { bytes, .. } = &field.attributes[0] else {
            panic!("{name} must have a ConstantValue attribute");
        };
        let value_index = u16::from_be_bytes([bytes[0], bytes[1]]);
        assert_eq!(
            audio_clip
                .class
                .constant_pool
                .get(usize::from(value_index))
                .and_then(Option::as_ref),
            Some(&Constant::Integer(expected))
        );
    }
    let public_surface = audio_clip
        .class
        .methods
        .iter()
        .filter(|method| method.access_flags & ACC_PUBLIC != 0)
        .map(|method| {
            (
                audio_clip.class.utf8(method.name_index).unwrap(),
                audio_clip.class.utf8(method.descriptor_index).unwrap(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        public_surface,
        BTreeSet::from([
            ("<init>", "()V"),
            ("<init>", "(I[BII)V"),
            ("<init>", "(ILjava/lang/String;)V"),
            ("isSupported", "()Z"),
            ("pause", "()V"),
            ("play", "(II)V"),
            ("resume", "()V"),
            ("stop", "()V"),
        ])
    );
}

#[test]
fn siemens_media_namespace_uses_the_complete_mmapi_adapter() {
    let inventory = production_bootstrap_inventory();
    let standard_names = inventory
        .iter()
        .filter_map(|entry| {
            entry
                .class
                .class_name(entry.class.this_class)
                .filter(|name| name.starts_with("javax/microedition/media/"))
        })
        .map(|name| name.replace("javax/microedition/media/", "com/siemens/mp/media/"))
        .collect::<BTreeSet<_>>();
    let siemens = inventory
        .iter()
        .filter(|entry| {
            entry
                .class
                .class_name(entry.class.this_class)
                .is_some_and(|name| name.starts_with("com/siemens/mp/media/"))
        })
        .collect::<Vec<_>>();
    let siemens_names = siemens
        .iter()
        .map(|entry| {
            entry
                .class
                .class_name(entry.class.this_class)
                .unwrap()
                .to_owned()
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(siemens_names, standard_names);
    assert!(siemens.iter().all(|entry| {
        entry.owner == BootstrapOwner::Mmapi
            && entry.requirement == BootstrapRequirement::VendorApi("siemens-extension")
            && entry.class.constant_pool.iter().flatten().all(|constant| {
                !matches!(
                    constant,
                    Constant::Utf8(value)
                        if value.contains("javax/microedition/media")
                            || value.contains("javax.microedition.media")
                )
            })
    }));

    let manager = &siemens
        .iter()
        .find(|entry| {
            entry.class.class_name(entry.class.this_class) == Some("com/siemens/mp/media/Manager")
        })
        .unwrap()
        .class;
    let create_player = manager
        .methods
        .iter()
        .find(|method| {
            manager.utf8(method.name_index) == Some("createPlayer")
                && manager.utf8(method.descriptor_index)
                    == Some("(Ljava/lang/String;)Lcom/siemens/mp/media/Player;")
        })
        .unwrap();
    assert_eq!(create_player.access_flags, ACC_PUBLIC | ACC_STATIC);
    assert!(matches!(
        create_player.attributes.first(),
        Some(Attribute::Code(_))
    ));

    let player = &siemens
        .iter()
        .find(|entry| {
            entry.class.class_name(entry.class.this_class) == Some("com/siemens/mp/media/Player")
        })
        .unwrap()
        .class;
    assert_eq!(
        player.access_flags,
        ACC_PUBLIC | ACC_INTERFACE | ACC_ABSTRACT
    );
    for (name, descriptor) in [
        ("close", "()V"),
        ("setMediaTime", "(J)J"),
        ("start", "()V"),
        ("stop", "()V"),
    ] {
        assert!(player.methods.iter().any(|method| {
            player.utf8(method.name_index) == Some(name)
                && player.utf8(method.descriptor_index) == Some(descriptor)
        }));
    }

    let implementation = &siemens
        .iter()
        .find(|entry| {
            entry.class.class_name(entry.class.this_class)
                == Some("com/siemens/mp/media/PlayerImpl")
        })
        .unwrap()
        .class;
    assert!(implementation.fields.iter().any(|field| {
        implementation.utf8(field.name_index) == Some("handle")
            && implementation.utf8(field.descriptor_index) == Some("J")
    }));
}

#[test]
fn rust_bootstrap_contains_mmapi_owned_nokia_sound() {
    let inventory = production_bootstrap_inventory();
    let sound = inventory
        .iter()
        .find(|entry| {
            entry.class.class_name(entry.class.this_class) == Some("com/nokia/mid/sound/Sound")
        })
        .unwrap();
    assert_eq!(sound.owner, BootstrapOwner::Mmapi);
    let public_surface = sound
        .class
        .methods
        .iter()
        .filter(|method| method.access_flags & ACC_PUBLIC != 0)
        .map(|method| {
            (
                sound.class.utf8(method.name_index).unwrap(),
                sound.class.utf8(method.descriptor_index).unwrap(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        public_surface,
        BTreeSet::from([
            ("<init>", "([BI)V"),
            ("<init>", "(IJ)V"),
            ("getGain", "()I"),
            ("getState", "()I"),
            ("init", "([BI)V"),
            ("init", "(IJ)V"),
            ("play", "(I)V"),
            ("release", "()V"),
            ("resume", "()V"),
            ("setGain", "(I)V"),
            ("stop", "()V"),
        ])
    );
}
