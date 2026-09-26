use super::*;

fn compressed(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn decoding_requires_the_exact_payload_size_including_empty_streams() {
    for payload in [Vec::new(), vec![42], vec![37; 64 * 1024]] {
        let input = compressed(&payload);
        let length = payload.len() as u64;
        assert_eq!(decode(&input, length).unwrap(), payload);
        assert!(decode(&input, length + 1).is_err());
        if length > 0 {
            assert!(decode(&input, length - 1).is_err());
        }
    }
    for length in [save_state::MAX_COMPONENT_BYTES as u64 + 1, u64::MAX] {
        assert!(decode(&compressed(&[]), length).is_err());
    }
}

#[test]
fn decoding_rejects_truncation_corruption_and_bytes_after_the_stream() {
    let payload = vec![29; 64 * 1024];
    let input = compressed(&payload);
    let length = payload.len() as u64;
    for end in 0..input.len() {
        assert!(decode(&input[..end], length).is_err(), "prefix {end}");
    }
    let mut corrupt = input.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(decode(&corrupt, length).is_err());
    for extra in [vec![0], input.clone()] {
        let mut trailing = input.clone();
        trailing.extend_from_slice(&extra);
        assert!(decode(&trailing, length).is_err());
    }
}
