use super::*;
use crate::ManagedHeapRecoveryCandidate;

#[test]
fn recovery_variants_match_explicit_selection_with_the_same_canvas_and_evidence() {
    let profiles = profiles();
    let failed = resolve_device_selection(
        profiles,
        &BTreeMap::new(),
        &ArchiveEvidence::default(),
        SelectionOverrides::default()
            .with_profile_id("nokia-featurephone")
            .with_canvas_dimensions(240, 320),
    )
    .unwrap();
    for (properties, name, classes) in [
        (vec![], None, vec![]),
        (vec![], Some("Sony Ericsson K800 320x240.jar"), vec![]),
        (
            vec![("Nokia-MIDlet-Target-Display-Size", "128,160")],
            Some("nokia-s40v1.jar"),
            vec![],
        ),
        (
            vec![
                ("MIDlet-Touch-Support", "true"),
                ("Nokia-MIDlet-App-Orientation", "portrait"),
            ],
            Some("samsung-f480-240x320.jar"),
            vec![],
        ),
        (
            vec![("Nokia-MIDlet-App-Orientation", "landscape")],
            None,
            vec![],
        ),
        (
            vec![],
            Some("nokia-s40v3.jar"),
            vec!["com/mascotcapsule/micro3d/v3/Graphics3D"],
        ),
        (
            vec![("Build-Configuration", "Nokia240x320_S40")],
            None,
            vec!["com/nokia/mid/ui/DirectUtils", "java/nio/ByteBuffer"],
        ),
    ] {
        let properties = properties
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect();
        let archive = ArchiveEvidence::new(
            name.map(str::to_owned),
            classes.into_iter().map(str::to_owned),
        );
        for pointer_override in [false, true] {
            let candidates = managed_heap_recovery_candidates(
                profiles,
                &properties,
                &archive,
                &failed,
                16 * 1024 * 1024,
                pointer_override,
            )
            .unwrap();
            for profile in profiles {
                let actual = candidates
                    .iter()
                    .find(|candidate| candidate.selection().profile_id() == profile.profile_id());
                if profile.profile_id() == failed.selection().profile_id()
                    || profile
                        .limits()
                        .heap_bytes()
                        .value()
                        .is_some_and(|heap| *heap <= 2 * 1024 * 1024)
                {
                    assert!(actual.is_none());
                    continue;
                }
                let mut overrides = SelectionOverrides::default()
                    .with_profile_id(profile.profile_id())
                    .with_canvas_dimensions(240, 320);
                if pointer_override {
                    overrides = overrides.with_pointer_events();
                }
                match resolve_device_selection(profiles, &properties, &archive, overrides) {
                    Ok(expected) => assert_eq!(
                        actual.map(ManagedHeapRecoveryCandidate::selection),
                        Some(expected.selection()),
                        "{} with {properties:?}, {archive:?}, pointer={pointer_override}",
                        profile.profile_id(),
                    ),
                    Err(error) => {
                        assert_eq!(error.code(), "device-selection-no-match");
                        assert!(actual.is_none());
                    }
                }
            }
        }
    }
}

#[test]
fn managed_heap_recovery_keeps_canvas_and_offers_only_larger_or_unknown_heaps() {
    let profiles = profiles();
    let properties = BTreeMap::new();
    let archive = ArchiveEvidence::new(Some("240320s40v3a.jar".to_owned()), Vec::<String>::new());
    let failed = resolve_device_selection(
        profiles,
        &properties,
        &archive,
        SelectionOverrides::default().with_profile_id("nokia-featurephone"),
    )
    .unwrap();
    assert_eq!(failed.selection().profile_id(), "nokia-featurephone");

    let candidates = managed_heap_recovery_candidates(
        profiles,
        &properties,
        &archive,
        &failed,
        16 * 1024 * 1024,
        false,
    )
    .unwrap();
    assert!(!candidates.is_empty());
    assert_eq!(candidates[0].selection().profile_id(), "nokia-asha-touch");
    assert_eq!(candidates[0].heap_bytes(), Some(3 * 1024 * 1024));
    assert!(candidates.iter().all(|candidate| {
        candidate.selection().profile_id() != "nokia-featurephone"
            && candidate.selection().canvas_dimensions() == (240, 320)
            && candidate
                .heap_bytes()
                .is_none_or(|heap| heap > 2 * 1024 * 1024)
    }));
    assert!(candidates.iter().any(|candidate| {
        candidate.selection().profile_id() == "se-featurephone"
            && candidate.heap_bytes() == Some(6 * 1024 * 1024)
    }));

    let unknown_heap_failure = resolve_device_selection(
        profiles,
        &properties,
        &archive,
        SelectionOverrides::default()
            .with_profile_id("nokia-s60-keypad")
            .with_canvas_dimensions(240, 320),
    )
    .unwrap();
    let candidates = managed_heap_recovery_candidates(
        profiles,
        &properties,
        &archive,
        &unknown_heap_failure,
        16 * 1024 * 1024,
        false,
    )
    .unwrap();
    assert!(!candidates.is_empty());
    assert!(candidates.iter().all(|candidate| {
        candidate
            .heap_bytes()
            .is_some_and(|heap| heap > 16 * 1024 * 1024)
    }));

    let automatic_failure = resolve_device_selection(
        profiles,
        &properties,
        &archive,
        SelectionOverrides::default(),
    )
    .unwrap();
    assert_eq!(
        automatic_failure.selection().profile_id(),
        "nokia-s60-keypad"
    );
    let candidates = managed_heap_recovery_candidates(
        profiles,
        &properties,
        &archive,
        &automatic_failure,
        1,
        false,
    )
    .unwrap();
    assert!(!candidates.is_empty());
    assert!(candidates.iter().all(|candidate| {
        candidate
            .heap_bytes()
            .is_some_and(|heap| heap > 16 * 1024 * 1024)
    }));
}
