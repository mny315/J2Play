use super::*;

#[test]
fn packed_reads_match_individual_bits_at_every_alignment_and_width() {
    let bytes = (0_u8..=255)
        .map(|value| value.wrapping_mul(73).wrapping_add(19))
        .collect::<Vec<_>>();
    for byte_offset in 0..16 {
        for alignment in 0..8 {
            for width in 0..=64 {
                let mut reader = BitReader::new(&bytes, byte_offset).unwrap();
                reader.unsigned(alignment).unwrap();
                let start = byte_offset * 8 + alignment;
                let expected = (0..width).fold(0_u64, |value, shift| {
                    let source = start + shift;
                    value | (u64::from((bytes[source / 8] >> (source % 8)) & 1) << shift)
                });
                assert_eq!(reader.unsigned(width).unwrap(), expected);
                assert_eq!(reader.bit, start + width);
                assert_eq!(reader.aligned_position(), (start + width).div_ceil(8));
            }
        }
    }
}

#[test]
fn truncated_and_invalid_bit_reads_preserve_the_cursor() {
    let bytes = [0xa5; 10];
    for length in 0..=bytes.len() {
        for start in 0..=length * 8 {
            let mut reader = BitReader::new(&bytes[..length], start / 8).unwrap();
            reader.unsigned(start % 8).unwrap();
            let available = length * 8 - start;
            for width in 0..=64 {
                let before = reader.bit;
                let result = reader.unsigned(width);
                if width > available {
                    assert_eq!(result.unwrap_err().code(), "resource-truncated");
                    assert_eq!(reader.bit, before);
                } else {
                    result.unwrap();
                }
                reader.bit = start;
            }
            assert_eq!(
                reader.unsigned(65).unwrap_err().code(),
                "resource-bit-width"
            );
            assert_eq!(reader.bit, start);
            assert_eq!(reader.signed(0).unwrap_err().code(), "resource-bit-width");
            assert_eq!(reader.signed(33).unwrap_err().code(), "resource-bit-width");
            assert_eq!(reader.bit, start);
        }
    }
}

#[test]
fn signed_fields_extend_the_sign_bit_at_all_widths_and_alignments() {
    for width in 1..=32 {
        for alignment in 0..8 {
            for value in [
                0,
                1,
                (1_u64 << (width - 1)) - 1,
                1_u64 << (width - 1),
                (1_u64 << width) - 1,
            ] {
                let bytes = (value << alignment).to_le_bytes();
                let mut reader = BitReader::new(&bytes, 0).unwrap();
                reader.unsigned(alignment).unwrap();
                let expected = if value & (1_u64 << (width - 1)) != 0 {
                    i64::try_from(value).unwrap() - (1_i64 << width)
                } else {
                    i64::try_from(value).unwrap()
                };
                assert_eq!(i64::from(reader.signed(width).unwrap()), expected);
            }
        }
    }
}
