use super::*;
use crate::archive_name::{NormalizedArchiveName, archive_model_matches};
use crate::{
    ArchiveEvidence, SelectionOverrides, builtin_device_profiles, resolve_device_selection,
};

#[test]
fn samsung_aliases_merge_catalog_synonyms_and_reject_longer_codes() {
    let profiles = builtin_device_profiles().unwrap();
    let aliases = ModelAliasIndex::new(profiles);
    assert!(aliases.samsung.contains("e380"));
    assert!(aliases.samsung.contains("e810"));
    assert!(aliases.samsung.contains("sghe380"));
    assert!(aliases.samsung.contains("d900"));
    assert!(!aliases.samsung.contains("z500"));
    assert!(aliases.ambiguous.contains("z500"));

    for archive in ["D9001.jar", "Z500.jar"] {
        assert!(
            archive_model_matches(profiles, &NormalizedArchiveName::new(archive))
                .iter()
                .all(|model_match| model_match.models.is_empty()),
            "{archive}"
        );
    }
    for (archive, expected) in [
        ("E380.jar", "samsung-midp2-keypad"),
        ("SamsungE380.jar", "samsung-midp2-keypad"),
        ("SGHE380.jar", "samsung-midp2-keypad"),
        ("E810.jar", "samsung-midp2-cldc10-keypad"),
        ("SGHE810.jar", "samsung-midp2-cldc10-keypad"),
        ("SGHZ500.jar", "samsung-featurephone"),
        ("SonyEricssonZ500.jar", "se-jp3-keypad"),
    ] {
        let matched = archive_model_matches(profiles, &NormalizedArchiveName::new(archive));
        assert_eq!(
            matched
                .iter()
                .position(|model_match| !model_match.models.is_empty())
                .map(|index| profiles[index].profile_id()),
            Some(expected),
            "{archive}"
        );
    }
}

#[test]
fn samsung_touch_model_survives_audioclip_requirement() {
    let profiles = builtin_device_profiles().unwrap();
    let decision = resolve_device_selection(
        profiles,
        &BTreeMap::new(),
        &ArchiveEvidence::new(
            Some("SGHE898_AudioClip.jar".to_owned()),
            ["com/samsung/util/AudioClip".to_owned()],
        ),
        SelectionOverrides::default(),
    )
    .unwrap();
    assert_eq!(
        decision.target_selection().profile_id(),
        "samsung-midp20-touch"
    );
    assert_eq!(decision.selection().profile_id(), "samsung-touch");
    assert_eq!(decision.selection().canvas_dimensions(), (240, 320));
}
