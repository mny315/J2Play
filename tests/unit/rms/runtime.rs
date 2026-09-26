use super::*;
use crate::tests::repository;
use std::{collections::BTreeMap, fs};

#[test]
fn reading_open_store_metadata_does_not_scan_the_suite_directory() {
    let (root, repository, suite) = repository();
    let directory = repository.suite_path(suite.digest());
    let mut runtime = Runtime::new(
        &root.0,
        suite,
        Limits {
            max_stores_per_suite: 1,
            ..Limits::default()
        },
    );
    let handle = runtime.open("save", true, 10).unwrap();
    runtime.add(handle, b"value", 20).unwrap();
    for index in 0..4 {
        fs::write(directory.join(format!("leftover-{index}.tmp")), b"").unwrap();
    }
    let size = runtime.metadata(handle, RmsMetadataField::Size).unwrap();
    let fields = [
        RmsMetadataField::Version,
        RmsMetadataField::RecordCount,
        RmsMetadataField::Size,
        RmsMetadataField::NextRecordId,
        RmsMetadataField::LastModified,
    ];
    let values = fields.map(|field| runtime.metadata(handle, field));
    let available = runtime.metadata(handle, RmsMetadataField::SizeAvailable);

    assert_eq!(values.map(Result::unwrap), [1, 1, size, 2, 20]);
    assert_eq!(available.unwrap_err().code(), "store-limit");
}

#[test]
fn exhausted_handle_space_does_not_create_an_unopened_store() {
    let (root, _, suite) = repository();
    let mut runtime = Runtime::new(&root.0, suite, Limits::default());
    let existing = runtime.open("existing", true, 1).unwrap();
    runtime.next_handle = u64::try_from(i64::MAX).unwrap();

    assert_eq!(runtime.open("existing", false, 2).unwrap(), existing);
    assert_eq!(
        runtime.open("new", true, 3).unwrap_err().code(),
        "open-limit"
    );
    assert_eq!(runtime.list_stores().unwrap(), ["existing"]);
    runtime.close(existing).unwrap();
    runtime.close(existing).unwrap();
}

#[test]
fn runtime_reports_the_newest_persisted_guest_timestamp() {
    let (root, _, suite) = repository();
    let mut runtime = Runtime::new(&root.0, suite, Limits::default());
    assert_eq!(runtime.latest_modified().unwrap(), None);

    let first = runtime.open("first", true, 10).unwrap();
    runtime.add(first, b"record", 20).unwrap();
    runtime.close(first).unwrap();
    let second = runtime.open("second", true, 15).unwrap();
    runtime.close(second).unwrap();

    assert_eq!(runtime.latest_modified().unwrap(), Some(20));
}

#[test]
fn runtime_balances_duplicate_opens_and_rejects_open_store_deletion() {
    let (root, _repository, suite) = repository();
    let mut runtime = Runtime::new(&root.0, suite, Limits::default());
    let first = runtime.open("save", true, 1).unwrap();
    let second = runtime.open("save", false, 2).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        runtime.delete_store("save").unwrap_err().code(),
        "store-open"
    );
    runtime.add(first, b"state", 3).unwrap();
    runtime.close(first).unwrap();
    assert_eq!(runtime.get(second, 1).unwrap(), b"state");
    runtime.close(second).unwrap();
    assert_eq!(
        runtime
            .metadata(first, RmsMetadataField::Version)
            .unwrap_err()
            .code(),
        "store-not-open"
    );
    let reopened = runtime.open("save", false, 4).unwrap();
    assert_ne!(reopened, first);
    runtime.close(reopened).unwrap();
    runtime.delete_store("save").unwrap();
    assert!(runtime.list_stores().unwrap().is_empty());
}

#[test]
fn metadata_size_and_available_follow_store_and_suite_quotas() {
    let (root, _repository, suite) = repository();
    let limits = Limits {
        max_record_bytes: 128,
        max_store_bytes: 512,
        max_suite_bytes: 700,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(&root.0, suite, limits);
    let handle = runtime.open("save", true, 1).unwrap();
    let empty_size = runtime.metadata(handle, RmsMetadataField::Size).unwrap();
    let empty_available = runtime
        .metadata(handle, RmsMetadataField::SizeAvailable)
        .unwrap();
    assert_eq!(empty_available, 512 - empty_size);
    runtime.add(handle, &[7; 40], 2).unwrap();
    let read = |field| runtime.metadata(handle, field).unwrap();
    let occupied = empty_size + 8 + 40;
    assert_eq!(read(RmsMetadataField::Size), occupied);
    assert_eq!(read(RmsMetadataField::SizeAvailable), 512 - occupied);
    for (field, expected) in [
        (RmsMetadataField::Version, 1),
        (RmsMetadataField::RecordCount, 1),
        (RmsMetadataField::NextRecordId, 2),
        (RmsMetadataField::LastModified, 2),
    ] {
        assert_eq!(read(field), expected, "{field:?}");
    }
    // A second store makes the suite quota tighter than the per-store quota.
    let second = runtime.open("other", true, 3).unwrap();
    runtime.add(second, &[9; 128], 4).unwrap();
    let second_size = runtime.metadata(second, RmsMetadataField::Size).unwrap();
    let suite_room = 700 - occupied - second_size;
    assert!(suite_room < 512 - occupied);
    assert_eq!(
        runtime
            .metadata(handle, RmsMetadataField::SizeAvailable)
            .unwrap(),
        suite_room
    );
    runtime.close(second).unwrap();
    runtime.delete_store("other").unwrap();
    assert_eq!(
        runtime
            .metadata(handle, RmsMetadataField::SizeAvailable)
            .unwrap(),
        512 - occupied
    );
    runtime.close(handle).unwrap();
}

#[test]
fn deterministic_property_sequences_match_an_in_memory_model_across_reopens() {
    let (root, _repository, suite) = repository();
    let mut runtime = Runtime::new(&root.0, suite, Limits::default());
    let mut handle = runtime.open("property", true, 0).unwrap();
    let mut model = BTreeMap::<i32, Vec<u8>>::new();
    let mut next_id = 1_i32;
    let mut seed = 0x05ee_d240_u64;
    let mut observed_operations = 0_u8;
    for step in 1_i64..=128 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let operation = (seed >> 32) % 4;
        let data_length = usize::from(seed.to_be_bytes()[7] % 8);
        let data = seed.to_be_bytes()[..data_length].to_vec();
        if operation == 0 || model.is_empty() {
            observed_operations |= 1;
            let id = runtime.add(handle, &data, step).unwrap();
            assert_eq!(id, next_id);
            model.insert(id, data);
            next_id += 1;
        } else {
            let model_length = u64::try_from(model.len()).unwrap();
            let index = usize::try_from(seed % model_length).unwrap();
            let id = *model.keys().nth(index).unwrap();
            match operation {
                1 => {
                    observed_operations |= 2;
                    runtime.set(handle, id, &data, step).unwrap();
                    model.insert(id, data);
                }
                2 => {
                    observed_operations |= 4;
                    runtime.delete(handle, id, step).unwrap();
                    model.remove(&id);
                }
                _ => {
                    observed_operations |= 8;
                    assert_eq!(runtime.get(handle, id).unwrap(), model[&id]);
                }
            }
        }
        if step % 37 == 0 {
            runtime.close(handle).unwrap();
            handle = runtime.open("property", false, step).unwrap();
            assert_eq!(
                runtime.record_ids(handle).unwrap(),
                model.keys().copied().collect::<Vec<_>>()
            );
            for (&id, expected) in &model {
                assert_eq!(runtime.get(handle, id).unwrap(), expected.as_slice());
            }
            assert_eq!(
                runtime
                    .metadata(handle, RmsMetadataField::NextRecordId)
                    .unwrap(),
                i64::from(next_id)
            );
        }
    }
    assert_eq!(observed_operations, 0b1111);
    runtime.close(handle).unwrap();
}
