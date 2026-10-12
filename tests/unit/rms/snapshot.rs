use super::*;
use crate::{SuiteId, hex, tests::refresh_checksum};

#[test]
fn transactional_state_clones_share_unchanged_record_payloads() {
    let payload: Arc<[u8]> = Arc::from(b"large unchanged record".as_slice());
    let mut state = StoreState::empty([0; 32], "save", 0);
    state.records.insert(1, Arc::clone(&payload));

    let candidate = state.clone();

    assert!(Arc::ptr_eq(
        state.records.get(&1).unwrap(),
        candidate.records.get(&1).unwrap()
    ));
}

#[test]
fn encoded_size_is_exact_without_materializing_a_snapshot() {
    let mut state = StoreState::empty([7; 32], "save", 11);
    state.records.insert(1, Arc::from(b"one".as_slice()));
    state.records.insert(2, Arc::from(b"payload".as_slice()));
    state.next_record_id = 3;

    let limits = Limits::default();
    assert_eq!(
        state.encoded_len(limits).unwrap(),
        state.encode(limits).unwrap().len()
    );

    let too_small = Limits {
        max_store_bytes: state.encoded_len(limits).unwrap() - 1,
        ..limits
    };
    assert_eq!(
        state.encoded_len(too_small).unwrap_err().code(),
        "store-full"
    );
    assert_eq!(state.encode(too_small).unwrap_err().code(), "store-full");
}

#[test]
fn versioned_golden_fixture_round_trips_and_future_version_is_controlled() {
    let suite = SuiteId::new("Golden", "Migration").unwrap();
    let mut state = StoreState::empty(suite.digest(), "save", 1234);
    state.records.insert(1, Arc::from([0, 1, 255]));
    state.next_record_id = 2;
    state.version = 1;
    let bytes = state.encode(Limits::default()).unwrap();
    assert_eq!(
        hex(&bytes),
        "454d553234524d5300017937b1f342d9e990c83365a656cc38bfd87954903309953b4f84c2d542bd21fc000473617665000000020000000100000000000004d20000000100000001000000030001ffd78ca6bb74ab872eff7feaa085e8767ba176bed26a95841eba06265110623a1d"
    );
    assert_eq!(
        decode(&bytes, Limits::default(), suite.digest()).unwrap(),
        state
    );

    let mut future = bytes;
    future[8..10].copy_from_slice(&2_u16.to_be_bytes());
    refresh_checksum(&mut future);
    assert_eq!(
        decode(&future, Limits::default(), suite.digest())
            .unwrap_err()
            .code(),
        "unsupported-format"
    );
}

#[test]
fn decoder_rejects_noncanonical_record_order_and_impossible_next_id() {
    let suite = SuiteId::new("Decode", "Strict").unwrap();
    let mut state = StoreState::empty(suite.digest(), "save", 1);
    state.records.insert(1, Arc::from([1]));
    state.records.insert(2, Arc::from([2]));
    state.next_record_id = 3;
    state.version = 2;
    let mut bytes = state.encode(Limits::default()).unwrap();
    let first_record = 68;
    let second_record = first_record + 9;
    for offset in 0..9 {
        bytes.swap(first_record + offset, second_record + offset);
    }
    refresh_checksum(&mut bytes);
    assert_eq!(
        decode(&bytes, Limits::default(), suite.digest())
            .unwrap_err()
            .code(),
        "corrupt-store"
    );

    let mut impossible_next = state.encode(Limits::default()).unwrap();
    impossible_next[48..52].copy_from_slice(&(MAX_RECORD_ID + 2).to_be_bytes());
    refresh_checksum(&mut impossible_next);
    assert_eq!(
        decode(&impossible_next, Limits::default(), suite.digest())
            .unwrap_err()
            .code(),
        "corrupt-store"
    );
}

#[test]
fn bounded_snapshot_mutations_match_metadata_and_full_decode() {
    let suite = SuiteId::new("J2Play", "Owned snapshot mutations").unwrap();
    let mut state = StoreState::empty(suite.digest(), "save", 123);
    state
        .records
        .insert(1, Arc::from(b"owned payload".as_slice()));
    state.records.insert(3, Arc::from([]));
    state.next_record_id = 4;
    let seed = state.encode(Limits::default()).unwrap();
    let mut random = 0x5d9a_327e_04b6_c812_u64;
    let mut accepted = 0;
    let mut rejected = 0;
    for case in 0..4_096 {
        random = random
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let mut bytes = seed.clone();
        let index = usize::try_from(random % u64::try_from(bytes.len()).unwrap()).unwrap();
        match case % 4 {
            0 => bytes[index] ^= random.to_le_bytes()[2],
            1 => bytes.truncate(index),
            2 => bytes.insert(index, random.to_le_bytes()[3]),
            _ => {
                bytes.remove(index);
            }
        }
        if case % 3 != 0 && bytes.len() >= CHECKSUM_BYTES {
            refresh_checksum(&mut bytes);
        }
        let mut records = BTreeMap::new();
        let inspected = inspect_snapshot(&bytes, Limits::default(), suite.digest(), |id, data| {
            records.insert(id, Arc::<[u8]>::from(data));
        });
        match (inspected, decode(&bytes, Limits::default(), suite.digest())) {
            (Ok(header), Ok(decoded)) => {
                accepted += 1;
                assert_eq!(decoded.encode(Limits::default()).unwrap(), bytes);
                assert_eq!(header.name, decoded.name);
                assert_eq!(header.next_record_id, decoded.next_record_id);
                assert_eq!(header.version, decoded.version);
                assert_eq!(header.last_modified, decoded.last_modified);
                assert_eq!(records, decoded.records);
            }
            (Err(inspected), Err(decoded)) => {
                rejected += 1;
                assert_eq!(inspected.code(), decoded.code());
            }
            result => panic!("metadata/decode mismatch for owned mutation {case}: {result:?}"),
        }
    }
    assert!(accepted > 0 && rejected > 0);
}
