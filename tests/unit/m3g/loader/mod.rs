use super::*;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;

mod binary_values;
mod camera;
mod crop;
mod prefix;
mod shading;

fn object(kind: u8, data: &[u8]) -> Vec<u8> {
    let mut result = vec![kind];
    result.extend_from_slice(&(data.len() as u32).to_le_bytes());
    result.extend_from_slice(data);
    result
}

fn section(payload: &[u8], compressed: bool) -> Vec<u8> {
    let encoded = if compressed {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(payload).unwrap();
        encoder.finish().unwrap()
    } else {
        payload.to_vec()
    };
    let mut result = vec![u8::from(compressed)];
    result.extend_from_slice(&(13_u32 + encoded.len() as u32).to_le_bytes());
    result.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    result.extend_from_slice(&encoded);
    let checksum = adler32(&result);
    result.extend_from_slice(&checksum.to_le_bytes());
    result
}

fn valid_file(compressed_content: bool) -> Vec<u8> {
    file_with_sections(
        false,
        &[section(&object(9, &valid_group_data()), compressed_content)],
    )
}

fn file_with_sections(has_external: bool, content_sections: &[Vec<u8>]) -> Vec<u8> {
    let mut header_data = vec![1, 0, u8::from(has_external)];
    header_data.extend_from_slice(&0_u32.to_le_bytes());
    header_data.extend_from_slice(&123_u32.to_le_bytes());
    header_data.extend_from_slice(b"j2play\0");
    let header_section = section(&object(0, &header_data), false);
    let size = FILE_IDENTIFIER.len()
        + header_section.len()
        + content_sections.iter().map(Vec::len).sum::<usize>();
    header_data[3..7].copy_from_slice(&(size as u32).to_le_bytes());
    let mut file = FILE_IDENTIFIER.to_vec();
    file.extend_from_slice(&section(&object(0, &header_data), false));
    for section in content_sections {
        file.extend_from_slice(section);
    }
    file
}

fn valid_group_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0_u32.to_le_bytes()); // user ID
    data.extend_from_slice(&0_u32.to_le_bytes()); // animation tracks
    data.extend_from_slice(&0_u32.to_le_bytes()); // user parameters
    data.extend_from_slice(&[0, 0]); // component/general transforms
    data.extend_from_slice(&[1, 1, 255]); // rendering, picking, alpha
    data.extend_from_slice(&u32::MAX.to_le_bytes()); // scope
    data.push(0); // alignment
    data.extend_from_slice(&0_u32.to_le_bytes()); // children
    data
}

#[test]
fn parses_uncompressed_and_zlib_sections_with_adler_checksum() {
    for compressed in [false, true] {
        let parsed = M3gFile::parse(&valid_file(compressed), LoaderLimits::default()).unwrap();
        assert_eq!(parsed.header.authoring_field, "j2play");
        assert_eq!(parsed.objects.len(), 2);
        assert_eq!(parsed.objects[1].object_type, ObjectType::Group);
        assert_eq!(parsed.sections[1].compressed, compressed);
    }
}

#[test]
fn serialized_vertex_buffers_can_omit_positions_for_partial_morph_targets() {
    for normals in [0_u32, 2] {
        let mut array = vec![0; 12];
        array.extend_from_slice(&[1, 3, 0, 1, 0, 0, 0, 127]);
        let mut buffer = vec![0; 12];
        buffer.extend_from_slice(&[255, 0, 0, 128]);
        buffer.extend_from_slice(&0_u32.to_le_bytes());
        for value in [0.0_f32, 0.0, 0.0, 1.0] {
            buffer.extend_from_slice(&value.to_le_bytes());
        }
        buffer.extend_from_slice(&normals.to_le_bytes());
        buffer.extend_from_slice(&0_u32.to_le_bytes());
        buffer.extend_from_slice(&0_u32.to_le_bytes());
        let content = [object(20, &array), object(21, &buffer)].concat();
        let bytes = file_with_sections(false, &[section(&content, false)]);
        let file = M3gFile::parse(&bytes, LoaderLimits::default()).unwrap();
        let mut runtime = crate::Runtime::default();
        let loaded = crate::instantiate_file(&file, &mut runtime, &[None; 3], &[None; 3]).unwrap();
        let buffer = runtime
            .resolved_vertex_buffer(loaded.handles[2].unwrap())
            .unwrap();
        assert!(buffer.positions().is_none());
        assert_eq!(buffer.has_normals(), normals != 0);
        assert_eq!(buffer.default_color(), 0x80ff_0000);
    }
}

#[test]
fn serialized_implicit_strips_bound_the_combined_index_count() {
    for encoding in [0, 1, 2] {
        for (lengths, valid) in [
            (vec![3_u32], true),
            (vec![786_432], true),
            (vec![786_433], false),
            (vec![786_432, 3], false),
            (vec![i32::MAX as u32], false),
            (vec![u32::MAX], false),
        ] {
            let mut data = vec![0; 12]; // Object3D metadata
            data.push(encoding);
            data.extend_from_slice(
                &[0; 4][..match encoding {
                    0 => 4,
                    1 => 1,
                    _ => 2,
                }],
            );
            data.extend_from_slice(&(lengths.len() as u32).to_le_bytes());
            for length in lengths {
                data.extend_from_slice(&length.to_le_bytes());
            }
            let file = file_with_sections(false, &[section(&object(11, &data), false)]);
            let parsed = M3gFile::parse(&file, LoaderLimits::default());
            if valid {
                assert!(parsed.is_ok(), "{parsed:?}");
            } else {
                assert_eq!(parsed.unwrap_err().code(), "invalid-strips");
            }
        }
    }
}

#[test]
fn serialized_explicit_strips_require_enough_indices_for_all_strips() {
    for encoding in [128, 129, 130] {
        for (count, lengths, valid) in [
            (0_u32, &[3_u32][..], false),
            (1, &[3][..], false),
            (2, &[3][..], false),
            (3, &[3][..], true),
            (4, &[3][..], true),
            (4, &[3, 3][..], false),
            (6, &[3, 3][..], true),
            (7, &[3, 3][..], true),
        ] {
            let mut data = vec![0; 12]; // Object3D metadata
            data.push(encoding);
            data.extend_from_slice(&count.to_le_bytes());
            for index in 0..count {
                match encoding {
                    128 => data.extend_from_slice(&index.to_le_bytes()),
                    129 => data.push(index as u8),
                    _ => data.extend_from_slice(&(index as u16).to_le_bytes()),
                }
            }
            data.extend_from_slice(&(lengths.len() as u32).to_le_bytes());
            for length in lengths {
                data.extend_from_slice(&length.to_le_bytes());
            }
            let file = file_with_sections(false, &[section(&object(11, &data), false)]);
            let parsed = M3gFile::parse(&file, LoaderLimits::default());
            if valid {
                assert!(parsed.is_ok(), "{parsed:?}");
            } else {
                assert_eq!(parsed.unwrap_err().code(), "invalid-strips");
            }
        }
    }
}

#[test]
fn serialized_keyframes_accept_wrapped_ranges_but_reject_either_out_of_bounds_index() {
    for (first, last, valid) in [
        (3_u32, 1_u32, true),
        (4, 4, true),
        (5, 1, false),
        (3, 5, false),
    ] {
        let mut data = vec![0; 12]; // Object3D metadata
        data.extend_from_slice(&[176, 192, 0]); // LINEAR, CONSTANT, full float encoding
        for value in [50, first, last, 1, 5] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        for time in [20_u32, 40, 100, 10, 20] {
            data.extend_from_slice(&time.to_le_bytes());
            data.extend_from_slice(&1.0_f32.to_le_bytes());
        }
        for compressed in [false, true] {
            let file = file_with_sections(false, &[section(&object(19, &data), compressed)]);
            let parsed = M3gFile::parse(&file, LoaderLimits::default());
            if valid {
                let parsed = parsed.unwrap();
                assert_eq!(parsed.objects[1].object_type, ObjectType::KeyframeSequence);
                let mut runtime = crate::Runtime::default();
                let instantiated =
                    crate::instantiate_file(&parsed, &mut runtime, &[None, None], &[None, None])
                        .unwrap();
                let crate::ObjectKind::KeyframeSequence(sequence) =
                    runtime.kind(instantiated.handles[1].unwrap()).unwrap()
                else {
                    panic!("expected keyframes");
                };
                assert_eq!(sequence.valid_range(), (first as usize, last as usize));
                assert_eq!(sequence.sample(15.0).unwrap(), [1.0]);
            } else {
                assert_eq!(parsed.unwrap_err().code(), "invalid-keyframes");
            }
        }
    }
}

#[test]
fn compressed_sections_require_the_complete_zlib_checksum() {
    let original = section(&object(9, &valid_group_data()), true);
    for missing in 1..=4 {
        let mut broken = original[..original.len() - 4 - missing].to_vec();
        let length = (broken.len() + 4) as u32;
        broken[1..5].copy_from_slice(&length.to_le_bytes());
        let checksum = adler32(&broken);
        broken.extend_from_slice(&checksum.to_le_bytes());
        let file = file_with_sections(false, &[broken]);
        assert!(
            M3gFile::parse(&file, LoaderLimits::default()).is_err(),
            "accepted zlib stream with {missing} checksum bytes missing"
        );
    }
}

#[test]
fn compressed_sections_reject_bytes_after_the_zlib_stream() {
    let payload = object(9, &valid_group_data());
    let original = section(&payload, true);
    let second = section(&payload, true);
    for trailing in [&[0][..], &[0xde, 0xad], &second[9..second.len() - 4]] {
        let mut broken = original[..original.len() - 4].to_vec();
        broken.extend_from_slice(trailing);
        let length = (broken.len() + 4) as u32;
        broken[1..5].copy_from_slice(&length.to_le_bytes());
        let checksum = adler32(&broken);
        broken.extend_from_slice(&checksum.to_le_bytes());
        let file = file_with_sections(false, &[broken]);
        assert_eq!(
            M3gFile::parse(&file, LoaderLimits::default())
                .expect_err("accepted extra compressed section bytes")
                .code(),
            "invalid-compressed-section"
        );
    }
}

#[test]
fn compressed_section_lengths_are_exact_at_buffer_boundaries() {
    for length in [0, 1, 65_535, 65_536, 65_537, 131_072] {
        let payload = object(9, &vec![0; length]);
        for declared in [0, payload.len() - 1, payload.len(), payload.len() + 1] {
            let mut serialized = section(&payload, true);
            serialized[5..9].copy_from_slice(&(declared as u32).to_le_bytes());
            let checksum_offset = serialized.len() - 4;
            let checksum = adler32(&serialized[..checksum_offset]);
            serialized[checksum_offset..].copy_from_slice(&checksum.to_le_bytes());
            let parsed = parse_section(&serialized, 0, LoaderLimits::default(), &mut 0, 0);
            if declared == payload.len() {
                assert_eq!(parsed.unwrap().objects[0].data.len(), length);
            } else {
                assert!(
                    parsed.is_err(),
                    "accepted {declared} bytes for {}",
                    payload.len()
                );
            }
        }
    }
    assert!(parse_section(&section(&[], true), 0, LoaderLimits::default(), &mut 0, 0).is_ok());
}

#[test]
fn checksum_corruption_is_rejected_before_object_parsing() {
    let mut file = valid_file(false);
    let position = file.len() - 5;
    file[position] ^= 1;
    assert_eq!(
        M3gFile::parse(&file, LoaderLimits::default())
            .unwrap_err()
            .code(),
        "checksum-mismatch"
    );
}

#[test]
fn decompression_bomb_is_rejected_before_inflation() {
    let file = valid_file(true);
    let parsed = M3gFile::parse(&file, LoaderLimits::default()).unwrap();
    let total = parsed
        .sections
        .iter()
        .map(|section| section.uncompressed_length as usize)
        .sum::<usize>();
    let limits = LoaderLimits {
        decompressed_bytes: total - 1,
        ..LoaderLimits::default()
    };
    assert_eq!(
        M3gFile::parse(&file, limits).unwrap_err().code(),
        "resource-limit"
    );
}

#[test]
fn reserved_types_and_truncated_sections_are_rejected() {
    let mut reserved = valid_file(false);
    let first_length = read_u32(&reserved, FILE_IDENTIFIER.len() + 1).unwrap() as usize;
    let second_section = FILE_IDENTIFIER.len() + first_length;
    let second_type = second_section + 9;
    reserved[second_type] = 23;
    let section_offset = second_section;
    let checksum_offset = reserved.len() - 4;
    let checksum = adler32(&reserved[section_offset..checksum_offset]);
    reserved[checksum_offset..].copy_from_slice(&checksum.to_le_bytes());
    assert_eq!(
        M3gFile::parse(&reserved, LoaderLimits::default())
            .unwrap_err()
            .code(),
        "unknown-object-type"
    );

    let mut truncated = valid_file(false);
    truncated.pop();
    assert_eq!(
        M3gFile::parse(&truncated, LoaderLimits::default())
            .unwrap_err()
            .code(),
        "truncated-file"
    );
}

#[test]
fn compression_flag_requires_a_zlib_stream() {
    let mut file = valid_file(false);
    file[FILE_IDENTIFIER.len()] = 1;
    let first_length = read_u32(&file, FILE_IDENTIFIER.len() + 1).unwrap() as usize;
    let checksum_offset = FILE_IDENTIFIER.len() + first_length - 4;
    let checksum = adler32(&file[FILE_IDENTIFIER.len()..checksum_offset]);
    file[checksum_offset..checksum_offset + 4].copy_from_slice(&checksum.to_le_bytes());
    assert_eq!(
        M3gFile::parse(&file, LoaderLimits::default())
            .unwrap_err()
            .code(),
        "invalid-compressed-section"
    );
}

#[test]
fn external_references_require_a_declared_dedicated_section() {
    let external = object(255, b"fixture.png\0");
    let scene = object(9, &valid_group_data());
    for compressed in [false, true] {
        for external_first in [false, true] {
            let mixed = if external_first {
                [external.as_slice(), scene.as_slice()].concat()
            } else {
                [scene.as_slice(), external.as_slice()].concat()
            };
            for has_external in [false, true] {
                let file = file_with_sections(has_external, &[section(&mixed, compressed)]);
                assert_eq!(
                    M3gFile::parse(&file, LoaderLimits::default())
                        .unwrap_err()
                        .code(),
                    "external-section-mismatch"
                );
            }
        }
        let file = file_with_sections(
            true,
            &[section(&external, compressed), section(&scene, compressed)],
        );
        let parsed = M3gFile::parse(&file, LoaderLimits::default()).unwrap();
        assert_eq!(
            parsed.objects[1].external_uri().unwrap(),
            Some("fixture.png")
        );
        assert_eq!(parsed.objects[2].object_type, ObjectType::Group);
        let external_only = file_with_sections(true, &[section(&external, compressed)]);
        assert!(M3gFile::parse(&external_only, LoaderLimits::default()).is_ok());
    }
}

#[test]
fn prefix_header_obeys_file_budget_before_checksum_work() {
    let mut file = valid_file(false);
    let section_length = read_u32(&file, FILE_IDENTIFIER.len() + 1).unwrap() as usize;
    file[FILE_IDENTIFIER.len() + section_length - 1] ^= 1;
    let limits = LoaderLimits {
        file_bytes: section_length - 1,
        ..LoaderLimits::default()
    };
    assert_eq!(
        M3gFile::parse_prefix(&file, limits).unwrap_err().code(),
        "resource-limit"
    );
}
