use super::*;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::hint::black_box;
use std::time::Instant;

#[test]
#[ignore = "manual launch-selection throughput measurement"]
fn selection_throughput() {
    let profiles = profiles();
    for (label, name, properties, classes, overrides) in [
        (
            "fallback",
            None,
            BTreeMap::new(),
            vec![],
            SelectionOverrides::default(),
        ),
        (
            "generic",
            Some("fixture.jar"),
            BTreeMap::new(),
            vec![],
            SelectionOverrides::default(),
        ),
        (
            "model",
            Some("fixture_Nokia6230i.jar"),
            BTreeMap::new(),
            vec![],
            SelectionOverrides::default(),
        ),
        (
            "descriptor",
            Some("fixture.jar"),
            BTreeMap::from([
                (
                    "Build-Configuration".to_owned(),
                    "SONY ERICSSON JP7 240x320".to_owned(),
                ),
                (
                    "MicroEdition-Platform".to_owned(),
                    "SonyEricsson K800".to_owned(),
                ),
            ]),
            vec![],
            SelectionOverrides::default(),
        ),
        (
            "static",
            Some("fixture_240x320.jar"),
            BTreeMap::new(),
            vec![
                "com/nokia/mid/ui/DirectGraphics".to_owned(),
                "javax/microedition/m3g/Graphics3D".to_owned(),
            ],
            SelectionOverrides::default(),
        ),
        (
            "override",
            Some("fixture.jar"),
            BTreeMap::new(),
            vec![],
            SelectionOverrides::default().with_profile_id("se-featurephone"),
        ),
    ] {
        let archive = ArchiveEvidence::new(name.map(str::to_owned), classes);
        let expected =
            resolve_device_selection(profiles, &properties, &archive, overrides).unwrap();
        let mut result = expected.clone();
        let start = Instant::now();
        for _ in 0..128 {
            result = black_box(
                resolve_device_selection(
                    black_box(profiles),
                    black_box(&properties),
                    black_box(&archive),
                    black_box(overrides),
                )
                .unwrap(),
            );
        }
        let elapsed = start.elapsed();
        assert_eq!(result, expected);
        let mut hash = DefaultHasher::new();
        format!("{result:?}").hash(&mut hash);
        println!(
            "selection {label}: elapsed={elapsed:?} checksum={}",
            hash.finish()
        );
    }
}

#[test]
#[ignore = "manual model-alias index throughput measurement"]
fn model_aliases_throughput() {
    let profiles = profiles();
    let start = Instant::now();
    let mut aliases = ModelAliasIndex::new(profiles);
    for _ in 0..128 {
        aliases = black_box(ModelAliasIndex::new(black_box(profiles)));
    }
    let elapsed = start.elapsed();
    let mut hash = DefaultHasher::new();
    for values in [
        aliases.manufacturer,
        aliases.samsung,
        aliases.ambiguous,
        aliases.nokia_code,
        aliases.nokia_numeric,
        aliases.dimension_compacts,
    ] {
        values.into_iter().collect::<BTreeSet<_>>().hash(&mut hash);
    }
    println!(
        "model aliases: elapsed={elapsed:?} checksum={}",
        hash.finish()
    );
}
