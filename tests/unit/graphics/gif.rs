use super::read_gif_code;
use crate::Image;

#[test]
fn lzw_codes_cross_byte_boundaries_without_consuming_truncated_codes() {
    // Consecutive least-significant-bit-first codes of widths 3 through 12.
    let packed = [0x4d, 0x2f, 0x54, 0xff, 0x59, 0x02, 0x58, 0x55, 0x55, 0x05];
    let mut position = 0;
    for (width, expected) in [
        (3, 5),
        (4, 9),
        (5, 30),
        (6, 2),
        (7, 85),
        (8, 255),
        (9, 300),
        (10, 512),
        (11, 1365),
        (12, 2730),
    ] {
        assert_eq!(read_gif_code(&packed, &mut position, width), Some(expected));
    }
    assert_eq!(position, 75);

    for (bytes, start, width) in [
        (&packed[..0], 0, 3),
        (&packed[..9], 63, 12),
        (&packed[..], 75, 12),
        (&packed[..], usize::MAX, 12),
    ] {
        let mut position = start;
        assert_eq!(read_gif_code(bytes, &mut position, width), None);
        assert_eq!(position, start);
    }
}

#[test]
fn encoded_image_dispatches_bounded_gif_first_frame() {
    const GIF_HEX: &str =
        "47494638396101000100800000000000ffffff21f90401000001002c00000000010001000002024401003b";
    let gif = GIF_HEX
        .as_bytes()
        .chunks_exact(2)
        .map(|digits| {
            let nibble = |digit| match digit {
                b'0'..=b'9' => digit - b'0',
                b'a'..=b'f' => digit - b'a' + 10,
                _ => unreachable!(),
            };
            nibble(digits[0]) << 4 | nibble(digits[1])
        })
        .collect::<Vec<_>>();

    let image = Image::from_encoded(&gif).unwrap();

    assert_eq!((image.width(), image.height()), (1, 1));
    assert_eq!(image.pixels(), &[0xff00_0000]);
    assert!(!image.is_mutable());
}

#[test]
fn gif_opaque_frame_uses_the_global_background_outside_its_rectangle() {
    for local_palette in [false, true] {
        for transparent in [false, true] {
            let mut gif = b"GIF89a".to_vec();
            // Logical screen 3x2, global black/red palette, red background.
            gif.extend_from_slice(&[3, 0, 2, 0, 0x80, 1, 0, 0, 0, 0, 255, 0, 0]);
            if transparent {
                gif.extend_from_slice(&[0x21, 0xf9, 4, 1, 0, 0, 1, 0]);
            }
            // One pixel at (1, 1), optionally with a local blue/green palette.
            gif.extend_from_slice(&[0x2c, 1, 0, 1, 0, 1, 0, 1, 0]);
            gif.push(if local_palette { 0x80 } else { 0 });
            if local_palette {
                gif.extend_from_slice(&[0, 0, 255, 0, 255, 0]);
            }
            // LZW clear, literal 0, end: three three-bit codes.
            gif.extend_from_slice(&[2, 2, 0x44, 0x01, 0, 0x3b]);
            let image = Image::from_encoded(&gif).unwrap();
            let background = if transparent { 0 } else { 0xffff_0000 };
            let foreground = if local_palette {
                0xff00_00ff
            } else {
                0xff00_0000
            };
            assert_eq!(
                image.pixels(),
                &[
                    background, background, background, background, foreground, background,
                ]
            );
        }
    }
}

#[test]
fn gif_deinterlaces_offset_frames_with_empty_passes() {
    for height in 1..=9_u16 {
        for width in [1_u16, 2, 7] {
            for interlaced in [false, true] {
                let mut gif = b"GIF89a".to_vec();
                gif.extend_from_slice(&(width + 2).to_le_bytes());
                gif.extend_from_slice(&(height + 2).to_le_bytes());
                gif.extend_from_slice(&[0, 0, 0, 0x2c]);
                for value in [1_u16, 1, width, height] {
                    gif.extend_from_slice(&value.to_le_bytes());
                }
                gif.push(0x83 | if interlaced { 0x40 } else { 0 });
                for index in 0..16_u8 {
                    gif.extend_from_slice(&[index * 13, 0, 0]);
                }
                let row_order = if interlaced {
                    [0, 8, 4, 2, 6, 1, 3, 5, 7]
                } else {
                    [0, 1, 2, 3, 4, 5, 6, 7, 8]
                };
                // A clear code before each literal keeps every code five bits wide.
                let codes = row_order
                    .into_iter()
                    .filter(|row| *row < height)
                    .flat_map(|row| (0..width).flat_map(move |column| [16, row + column]))
                    .chain([17]);
                let count = usize::from(width) * usize::from(height);
                let mut compressed = vec![0_u8; ((count * 2 + 1) * 5).div_ceil(8)];
                for (index, code) in codes.enumerate() {
                    for bit in 0..5 {
                        let offset = index * 5 + bit;
                        compressed[offset / 8] |= (((code >> bit) & 1) as u8) << (offset % 8);
                    }
                }
                gif.extend_from_slice(&[4, u8::try_from(compressed.len()).unwrap()]);
                gif.extend_from_slice(&compressed);
                gif.extend_from_slice(&[0, 0x3b]);

                let image = Image::from_encoded(&gif).unwrap();
                let mut expected = vec![0; usize::from(height + 2) * usize::from(width + 2)];
                for row in 0..height {
                    for column in 0..width {
                        expected[usize::from(row + 1) * usize::from(width + 2)
                            + usize::from(column + 1)] =
                            0xff00_0000 | u32::from((row + column) * 13) << 16;
                    }
                }
                assert_eq!(
                    image.pixels(),
                    expected,
                    "{width}×{height}, interlaced={interlaced}"
                );
            }
        }
    }
}
