use super::*;

fn single_mr122_frame() -> Vec<u8> {
    let mut data = FILE_MAGIC.to_vec();
    data.push(0x3c);
    data.extend_from_slice(&[0; 31]);
    data
}

#[test]
fn decodes_one_bounded_storage_frame() {
    let samples = decode_storage_file(&single_mr122_frame(), SAMPLES_PER_FRAME).unwrap();
    assert_eq!(samples.len(), SAMPLES_PER_FRAME);
}

#[test]
fn mixed_frame_sizes_and_quality_preserve_decoder_state_and_sample_limit() {
    let mut data = FILE_MAGIC.to_vec();
    let mut expected = Vec::new();
    let mut decoder = ffi::Decoder::new().unwrap();
    // Include every speech mode, SID, and the one-byte NO_DATA frame.
    for (frame_type, length) in [
        (0, 13),
        (1, 14),
        (2, 16),
        (3, 18),
        (4, 20),
        (5, 21),
        (6, 27),
        (7, 32),
        (8, 6),
        (15, 1),
    ] {
        for good in [true, false] {
            let mut frame = vec![0; length];
            frame[0] = (frame_type << 3) | if good { 0x04 } else { 0 };
            let mut pcm = [0; SAMPLES_PER_FRAME];
            decoder.decode(&frame, &mut pcm, !good);
            expected.extend_from_slice(&pcm);
            data.extend_from_slice(&frame);
        }
    }
    assert_eq!(
        decode_storage_file(&data, expected.len()).unwrap(),
        expected
    );
    assert_eq!(
        decode_storage_file(&data, expected.len() - 1),
        Err(DecodeError::SampleLimit)
    );
}

#[test]
fn cancellation_is_checked_during_validation_and_native_frame_decoding() {
    let mut data = FILE_MAGIC.to_vec();
    for _ in 0..8 {
        data.extend_from_slice(&single_mr122_frame()[FILE_MAGIC.len()..]);
    }
    for cancel_at in [2, 11] {
        let checks = std::cell::Cell::new(0);
        let result = decode_storage_file_with_cancellation(&data, 8 * SAMPLES_PER_FRAME, &|| {
            checks.set(checks.get() + 1);
            checks.get() == cancel_at
        });
        assert_eq!(result, Err(DecodeError::Cancelled));
        assert_eq!(checks.get(), cancel_at);
    }
    assert_eq!(
        decode_storage_file(&data, 8 * SAMPLES_PER_FRAME)
            .unwrap()
            .len(),
        8 * SAMPLES_PER_FRAME
    );
}

#[test]
fn rejects_malformed_reserved_truncated_and_oversized_streams() {
    assert_eq!(
        decode_storage_file(b"not-amr", SAMPLES_PER_FRAME),
        Err(DecodeError::InvalidHeader)
    );

    let mut reserved = FILE_MAGIC.to_vec();
    reserved.push(9 << 3);
    assert_eq!(
        decode_storage_file(&reserved, SAMPLES_PER_FRAME),
        Err(DecodeError::ReservedFrameType(9))
    );

    let mut truncated = FILE_MAGIC.to_vec();
    truncated.extend_from_slice(&[0x3c, 0]);
    assert_eq!(
        decode_storage_file(&truncated, SAMPLES_PER_FRAME),
        Err(DecodeError::TruncatedFrame)
    );
    assert_eq!(
        decode_storage_file(&single_mr122_frame(), SAMPLES_PER_FRAME - 1),
        Err(DecodeError::SampleLimit)
    );
}

#[test]
fn maps_the_storage_quality_bit_to_the_decoder_bad_frame_flag() {
    assert!(!frame_is_bad(&[0x3c]));
    assert!(frame_is_bad(&[0x38]));
}
