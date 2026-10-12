use super::*;

#[test]
fn preserves_binary_float_values_and_non_string_map_keys() {
    use std::collections::BTreeMap;

    let values = (
        f32::from_bits(0x7fc0_1234),
        BTreeMap::from([(vec![1_u16, 2], u64::MAX)]),
    );
    let bytes = encode(&values).unwrap();
    let restored: (f32, BTreeMap<Vec<u16>, u64>) = decode(&bytes).unwrap();
    assert_eq!(restored.0.to_bits(), values.0.to_bits());
    assert_eq!(restored.1, values.1);
}

#[test]
fn rejects_oversized_output_truncation_and_trailing_bytes() {
    assert!(encode_bounded(&vec![1_u8; 100], 10).is_err());
    let mut bytes = encode(&vec![1_u8; 100]).unwrap();
    assert!(decode::<Vec<u8>>(&bytes[..bytes.len() - 1]).is_err());
    bytes.push(0);
    assert!(decode::<Vec<u8>>(&bytes).is_err());
}

#[test]
fn checkpoint_encoding_polls_cancellation_and_streams_identically() {
    let value = vec![12_u8; 256 * 1024];
    let polls = std::cell::Cell::new(0);
    assert!(
        encode_cancellable(&value, MAX_COMPONENT_BYTES, &|| {
            polls.set(polls.get() + 1);
            polls.get() > 2
        })
        .is_err()
    );
    assert_eq!(polls.get(), 3);
    let mut streamed = Vec::new();
    let length = write(&value, &mut streamed, MAX_COMPONENT_BYTES, &|| false).unwrap();
    assert_eq!(length, streamed.len());
    assert_eq!(streamed, encode(&value).unwrap());
}

#[test]
fn memory_encoding_keeps_the_wire_format_and_exact_byte_limits() {
    let value = (
        vec![i16::MIN, -8192, -64, -1, 0, 1, 63, 8191, i16::MAX],
        vec![u32::MAX; 32 * 1024],
        (f32::from_bits(0x7fc0_1234), f64::NEG_INFINITY),
        Some("checkpoint"),
    );
    let expected = postcard::to_stdvec(&value).unwrap();
    assert_eq!(encode_bounded(&value, expected.len()).unwrap(), expected);
    assert!(encode_bounded(&value, expected.len() - 1).is_err());
    let mut streamed = Vec::new();
    write(&value, &mut streamed, expected.len(), &|| false).unwrap();
    assert_eq!(streamed, expected);
}

#[test]
fn large_byte_blocks_remain_cancellable_in_memory_and_streamed() {
    #[derive(Serialize)]
    struct Bytes<'a>(#[serde(serialize_with = "serialize_bytes")] &'a [u8]);

    let value = Bytes(&vec![42; 512 * 1024]);
    for stream in [false, true] {
        let polls = std::cell::Cell::new(0);
        let cancelled = || {
            polls.set(polls.get() + 1);
            polls.get() >= 3
        };
        if stream {
            let mut output = Vec::new();
            assert!(write(&value, &mut output, MAX_COMPONENT_BYTES, &cancelled).is_err());
            assert!(output.len() < 256 * 1024);
        } else {
            assert!(encode_cancellable(&value, MAX_COMPONENT_BYTES, &cancelled).is_err());
        }
        assert_eq!(polls.get(), 3);
    }
}

#[test]
fn byte_blocks_read_previous_vectors_and_reject_invalid_lengths() {
    #[derive(serde::Deserialize)]
    struct Bytes(#[serde(deserialize_with = "deserialize_bytes")] Vec<u8>);

    for value in [Vec::new(), (0..=255).cycle().take(256 * 1024).collect()] {
        let encoded = postcard::to_stdvec(&value).unwrap();
        assert_eq!(decode::<Bytes>(&encoded).unwrap().0, value);
        assert!(decode::<Bytes>(&encoded[..encoded.len() - 1]).is_err());
    }
    // A claimed huge block is rejected by the input reader before allocation.
    assert!(decode::<Bytes>(&postcard::to_stdvec(&usize::MAX).unwrap()).is_err());
}

#[test]
fn optional_byte_blocks_preserve_legacy_values_and_reject_invalid_payloads() {
    #[derive(Serialize, Deserialize)]
    struct Bytes(
        #[serde(
            serialize_with = "serialize_optional_bytes",
            deserialize_with = "deserialize_optional_bytes"
        )]
        Option<Vec<u8>>,
    );

    for value in [
        None,
        Some(Vec::new()),
        Some((0..=255).cycle().take(256 * 1024).collect()),
    ] {
        let encoded = postcard::to_stdvec(&value).unwrap();
        let decoded: Bytes = decode(&encoded).unwrap();
        assert_eq!(decoded.0, value);
        assert_eq!(encode(&decoded).unwrap(), encoded);
        assert!(decode::<Bytes>(&encoded[..encoded.len() - 1]).is_err());
    }
    assert!(decode::<Bytes>(&[2]).is_err());
    let mut oversized = vec![1];
    oversized.extend(postcard::to_stdvec(&usize::MAX).unwrap());
    assert!(decode::<Bytes>(&oversized).is_err());
}

#[test]
fn memory_encoding_bounds_allocated_capacity_for_scalars_and_byte_blocks() {
    #[derive(Serialize)]
    struct Bytes<'a>(#[serde(serialize_with = "serialize_bytes")] &'a [u8]);

    for length in [0, 1, 7, 8, 15, 16, 17, 1023, 1024, 65_537, 196_609] {
        let value = vec![42_u8; length];
        let expected = postcard::to_stdvec(&value).unwrap();
        for spare in [0, 1, 7] {
            let limit = expected.len() + spare;
            for encoded in [
                encode_bounded(&value, limit).unwrap(),
                encode_bounded(&Bytes(&value), limit).unwrap(),
            ] {
                assert_eq!(encoded, expected);
                assert!(
                    encoded.capacity() <= limit,
                    "length={length}, limit={limit}, capacity={}",
                    encoded.capacity()
                );
            }
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn checkpoint_encoding_throughput() {
    use std::{hint::black_box, time::Instant};
    #[derive(Serialize)]
    struct Bytes<'a>(#[serde(serialize_with = "serialize_bytes")] &'a [u8]);

    let scalar = vec![u32::MAX; 65_536];
    for (length, binary) in [
        (65_536, false),
        (64, true),
        (65_536, true),
        (3 * 1024 * 1024, true),
    ] {
        let bytes = vec![42; length];
        let expected = if binary {
            postcard::to_stdvec(&Bytes(&bytes)).unwrap()
        } else {
            postcard::to_stdvec(&scalar).unwrap()
        };
        let started = Instant::now();
        let mut checksum = 0;
        let mut capacity = 0;
        for _ in 0..32 {
            let encoded = if binary {
                encode_bounded(&Bytes(black_box(&bytes)), expected.len()).unwrap()
            } else {
                encode_bounded(black_box(&scalar), expected.len()).unwrap()
            };
            checksum +=
                encoded.len() + usize::from(encoded[0]) + usize::from(*encoded.last().unwrap());
            capacity = encoded.capacity();
            black_box(encoded);
        }
        eprintln!(
            "checkpoint bytes={length} binary={binary} ns={} capacity={capacity} checksum={checksum}",
            started.elapsed().as_nanos()
        );
    }
}
