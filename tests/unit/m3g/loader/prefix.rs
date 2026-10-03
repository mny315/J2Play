use super::*;

#[test]
fn prefix_parser_counts_the_header_once_in_all_resource_budgets() {
    for compressed in [false, true] {
        let bytes = valid_file(compressed);
        let expected = M3gFile::parse(&bytes, LoaderLimits::default()).unwrap();
        let exact = LoaderLimits {
            file_bytes: bytes.len(),
            decompressed_bytes: expected
                .sections
                .iter()
                .map(|section| section.uncompressed_length as usize)
                .sum(),
            sections: expected.sections.len(),
            objects: expected.objects.len(),
            object_bytes: expected
                .objects
                .iter()
                .map(|object| object.data.len())
                .max()
                .unwrap(),
        };
        let mut packed = bytes.clone();
        packed.extend_from_slice(&[0; 8192]);
        assert_eq!(
            M3gFile::parse_prefix(&packed, exact).unwrap(),
            (expected.clone(), bytes.len())
        );
        assert_eq!(M3gFile::parse(&bytes, exact).unwrap(), expected);
        assert!(M3gFile::parse(&packed, exact).is_err());
        for dimension in 0..5 {
            let mut limited = exact;
            match dimension {
                0 => limited.file_bytes -= 1,
                1 => limited.decompressed_bytes -= 1,
                2 => limited.sections -= 1,
                3 => limited.objects -= 1,
                _ => limited.object_bytes -= 1,
            }
            assert_eq!(
                M3gFile::parse_prefix(&packed, limited).unwrap_err().code(),
                "resource-limit"
            );
        }
        for limit in [
            LoaderLimits {
                sections: 0,
                ..exact
            },
            LoaderLimits {
                objects: 0,
                ..exact
            },
        ] {
            assert_eq!(
                M3gFile::parse_prefix(&packed, limit).unwrap_err().code(),
                "resource-limit"
            );
        }
    }
}

#[test]
fn header_file_boundaries_cannot_hide_header_bytes_or_partial_sections() {
    let original = valid_file(false);
    let header_start = FILE_IDENTIFIER.len();
    let header_length = read_u32(&original, header_start + 1).unwrap() as usize;
    let header_end = header_start + header_length;
    let file_size_offset = header_start + 9 + 5 + 3;
    for file_size in [
        0,
        header_start,
        header_end - 1,
        header_end,
        original.len() - 1,
        original.len() + 1,
    ] {
        let mut bytes = original.clone();
        bytes[file_size_offset..file_size_offset + 4]
            .copy_from_slice(&(file_size as u32).to_le_bytes());
        let checksum = adler32(&bytes[header_start..header_end - 4]);
        bytes[header_end - 4..header_end].copy_from_slice(&checksum.to_le_bytes());
        assert!(M3gFile::parse(&bytes, LoaderLimits::default()).is_err());
        assert!(M3gFile::parse_prefix(&bytes, LoaderLimits::default()).is_err());
    }
    for end in 0..original.len() {
        assert!(M3gFile::parse(&original[..end], LoaderLimits::default()).is_err());
        assert!(M3gFile::parse_prefix(&original[..end], LoaderLimits::default()).is_err());
    }
}
