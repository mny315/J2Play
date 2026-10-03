use super::*;
use crate::archive_name::{NormalizedArchiveName, archive_model_specificity};
use crate::catalog::{builtin_device_profile, builtin_device_profiles};
use crate::decision::CanvasOrientation;
use crate::model_aliases::ModelAliasIndex;
use std::collections::{BTreeSet, HashSet};
use std::ffi::OsStr;

fn profiles() -> &'static [DeviceProfile] {
    builtin_device_profiles().unwrap()
}

#[test]
fn builtin_catalog_contains_every_profile_document() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let mut document_ids = BTreeSet::new();
    for directory in std::fs::read_dir(root).unwrap() {
        let directory = directory.unwrap();
        if !directory.file_type().unwrap().is_dir() || directory.file_name() == "schema" {
            continue;
        }
        for entry in std::fs::read_dir(directory.path()).unwrap() {
            let path = entry.unwrap().path();
            if path.extension() == Some(OsStr::new("json")) {
                document_ids.insert(
                    DeviceProfile::from_path(path)
                        .unwrap()
                        .profile_id()
                        .to_owned(),
                );
            }
        }
    }
    let builtin_ids = profiles()
        .iter()
        .map(|profile| profile.profile_id().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(builtin_ids, document_ids);
}

fn resolve(properties: &BTreeMap<String, String>, archive: &ArchiveEvidence) -> DeviceDecision {
    resolve_device_selection(
        profiles(),
        properties,
        archive,
        SelectionOverrides::default(),
    )
    .unwrap()
}

mod archive_screens;
mod catalog_and_hosts;
mod evidence_priority;
mod nokia_names;
mod recovery;
mod throughput;
mod vendor_names;
