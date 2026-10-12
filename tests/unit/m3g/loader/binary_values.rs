use super::*;

const INVALID_FLOATS: [u32; 11] = [
    0x8000_0000, // Negative zero.
    0x0000_0001, // Positive and negative subnormal endpoints.
    0x007f_ffff,
    0x8000_0001,
    0x807f_ffff,
    0x7f80_0000, // Infinities and quiet/signaling NaNs.
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0000,
    0x7f80_0001,
    0xff80_0001,
];

struct FloatRecord {
    kind: u8,
    data: Vec<u8>,
    float_offsets: Vec<usize>,
}

fn compositing(blending: u8) -> FloatRecord {
    let mut data = vec![0; 12]; // Object3D metadata.
    data.extend_from_slice(&[1, 1, 1, 1, blending, 0]);
    let float_offsets = vec![data.len(), data.len() + 4];
    data.extend_from_slice(&[0; 8]);
    FloatRecord {
        kind: 6,
        data,
        float_offsets,
    }
}

fn keyframes(encoding: u8) -> FloatRecord {
    let mut data = vec![0; 12];
    data.extend_from_slice(&[176, 192, encoding]);
    for value in [10_u32, 0, 1, 2, 2] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    let mut float_offsets = Vec::new();
    if encoding != 0 {
        for _ in 0..4 {
            // Two component biases and scales.
            float_offsets.push(data.len());
            data.extend_from_slice(&1.0_f32.to_le_bytes());
        }
    }
    for time in [0_u32, 10] {
        data.extend_from_slice(&time.to_le_bytes());
        match encoding {
            0 => {
                for _ in 0..2 {
                    float_offsets.push(data.len());
                    data.extend_from_slice(&1.0_f32.to_le_bytes());
                }
            }
            1 => data.extend_from_slice(&[0, 255]),
            _ => data.extend_from_slice(&[0, 0, 255, 255]),
        }
    }
    FloatRecord {
        kind: 19,
        data,
        float_offsets,
    }
}

fn records() -> [FloatRecord; 4] {
    [compositing(68), keyframes(0), keyframes(1), keyframes(2)]
}

fn file_for(record: &FloatRecord, compressed: bool) -> Vec<u8> {
    let content = [
        object(9, &valid_group_data()),
        object(record.kind, &record.data),
    ]
    .concat();
    file_with_sections(false, &[section(&content, compressed)])
}

fn error_code(bits: u32) -> &'static str {
    if f32::from_bits(bits).is_finite() {
        "invalid-float"
    } else {
        "non-finite"
    }
}

#[test]
fn serialized_float_fields_reject_special_encodings_in_every_keyframe_encoding() {
    for mut record in records() {
        for &offset in &record.float_offsets {
            let original: [u8; 4] = record.data[offset..offset + 4].try_into().unwrap();
            for bits in INVALID_FLOATS {
                record.data[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
                for compressed in [false, true] {
                    let error =
                        M3gFile::parse(&file_for(&record, compressed), LoaderLimits::default())
                            .unwrap_err();
                    assert_eq!(
                        error.code(),
                        error_code(bits),
                        "{bits:08x}, offset {offset}"
                    );
                    assert!(error.message().contains("object 3"), "{error}");
                    assert!(error.message().contains("offset"), "{error}");
                }
            }
            record.data[offset..offset + 4].copy_from_slice(&original);
        }
    }
}

#[test]
fn instantiation_rechecks_binary_floats_and_rolls_back_created_objects() {
    for record in records() {
        let valid = M3gFile::parse(&file_for(&record, false), LoaderLimits::default()).unwrap();
        for offset in record.float_offsets {
            for bits in INVALID_FLOATS {
                let mut file = valid.clone();
                // Public parsed records may be changed before instantiation.
                Arc::make_mut(&mut file.objects[2].data)[offset..offset + 4]
                    .copy_from_slice(&bits.to_le_bytes());
                let mut runtime = crate::Runtime::default();
                let existing = runtime.create(None, crate::ObjectKind::Object).unwrap();
                let before = runtime.counters();
                let error = crate::instantiate_file(&file, &mut runtime, &[None; 3], &[None; 3])
                    .unwrap_err();
                assert_eq!(
                    error.code(),
                    error_code(bits),
                    "{bits:08x}, offset {offset}"
                );
                let after = runtime.counters();
                assert_eq!((after.0, after.2), (before.0, before.2));
                assert!(runtime.kind(existing).is_ok());
            }
        }
    }
}

#[test]
fn serialized_float_fields_preserve_normal_values_and_positive_zero() {
    for bits in [
        0_u32,
        0x0080_0000,
        0x8080_0000,
        0x3f80_0000,
        0xbf80_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
    ] {
        let mut record = compositing(68);
        for &offset in &record.float_offsets {
            record.data[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
        }
        for compressed in [false, true] {
            let file =
                M3gFile::parse(&file_for(&record, compressed), LoaderLimits::default()).unwrap();
            let mut runtime = crate::Runtime::default();
            let loaded =
                crate::instantiate_file(&file, &mut runtime, &[None; 3], &[None; 3]).unwrap();
            let crate::ObjectKind::CompositingMode(mode) =
                runtime.kind(loaded.handles[2].unwrap()).unwrap()
            else {
                unreachable!()
            };
            assert_eq!(mode.depth_offset_factor.to_bits(), bits);
            assert_eq!(mode.depth_offset_units.to_bits(), bits);
        }
    }
}

#[test]
fn serialized_compositing_accepts_all_five_blending_modes() {
    for mode in 63..=69 {
        let bytes = file_for(&compositing(mode), false);
        let parsed = M3gFile::parse(&bytes, LoaderLimits::default());
        if (64..=68).contains(&mode) {
            let file = parsed.unwrap();
            let mut runtime = crate::Runtime::default();
            let loaded =
                crate::instantiate_file(&file, &mut runtime, &[None; 3], &[None; 3]).unwrap();
            let crate::ObjectKind::CompositingMode(state) =
                runtime.kind(loaded.handles[2].unwrap()).unwrap()
            else {
                unreachable!()
            };
            assert_eq!(state.blending, i32::from(mode));
        } else {
            assert_eq!(parsed.unwrap_err().code(), "invalid-enum");
        }
    }
}
