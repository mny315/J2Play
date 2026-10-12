use super::*;
use crate::Framebuffer;
use crc32fast::Hasher;

#[path = "png/indexed.rs"]
mod indexed;

// Fixture wire order must be independent of the decoder's pass table.
const FIXTURE_ADAM7: [(u32, u32, u32, u32); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

#[test]
fn stored_png_streams_keep_valid_checksums_across_block_boundaries() {
    use std::io::Read;

    for length in [0, 5_551, 5_552, 5_553, 65_535, 65_536, 131_071] {
        let raw = vec![255; length];
        let compressed = deterministic_zlib_store(&raw);
        let mut decoded = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, raw, "length={length}");
    }
}

#[test]
#[ignore = "manual PNG encoding throughput measurement"]
fn png_encode_throughput() {
    use sha2::{Digest, Sha256};

    for (width, height, iterations) in [(240, 320, 500), (640, 480, 100)] {
        let mut frame = Framebuffer::new(width, height).unwrap();
        for (index, pixel) in frame.image.pixels.iter_mut().enumerate() {
            *pixel = 0xff00_0000 | (index as u32).wrapping_mul(0x010307) & 0xffffff;
        }
        let reference = frame.to_png();
        let started = std::time::Instant::now();
        for _ in 0..iterations {
            let encoded = std::hint::black_box(&frame).to_png();
            assert_eq!(encoded, reference);
        }
        eprintln!(
            "PNG encode {width}x{height} iterations={iterations} elapsed={:?} sha256={:x}",
            started.elapsed(),
            Sha256::digest(&reference),
        );
    }
}

#[test]
fn png_requires_the_complete_zlib_checksum_in_both_decode_modes() {
    let compressed = deterministic_zlib_store(&[0, 12, 34, 56, 255]);
    for missing in 0..=4 {
        let mut encoded = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = Vec::new();
        header.extend_from_slice(&1_u32.to_be_bytes());
        header.extend_from_slice(&1_u32.to_be_bytes());
        header.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(&mut encoded, b"IHDR", &header);
        chunk(
            &mut encoded,
            b"IDAT",
            &compressed[..compressed.len() - missing],
        );
        chunk(&mut encoded, b"IEND", &[]);
        for decoded in [
            Image::from_png(&encoded),
            Image::from_midp_encoded(&encoded),
        ] {
            if missing == 0 {
                assert_eq!(decoded.unwrap().pixels(), &[0xff0c_2238]);
            } else {
                assert!(
                    decoded.is_err(),
                    "accepted zlib stream with {missing} checksum bytes missing"
                );
            }
        }
    }
}

#[test]
fn png_idat_boundaries_preserve_the_stream_and_reject_interrupted_chunks() {
    let compressed = deterministic_zlib_store(&[0, 12, 34, 56, 255, 0, 78, 90, 123, 255]);
    let header = [0, 0, 0, 1, 0, 0, 0, 2, 8, 6, 0, 0, 0];
    for split in 0..=compressed.len() {
        for interrupted in [false, true] {
            let mut encoded = b"\x89PNG\r\n\x1a\n".to_vec();
            chunk(&mut encoded, b"IHDR", &header);
            chunk(&mut encoded, b"IDAT", &[]);
            chunk(&mut encoded, b"IDAT", &compressed[..split]);
            if interrupted {
                chunk(&mut encoded, b"tEXt", b"Author\0J2Play fixture");
            }
            chunk(&mut encoded, b"IDAT", &[]);
            chunk(&mut encoded, b"IDAT", &compressed[split..]);
            chunk(&mut encoded, b"IDAT", &[]);
            chunk(&mut encoded, b"IEND", &[]);
            for decoded in [
                Image::from_png(&encoded),
                Image::from_midp_encoded(&encoded),
            ] {
                if interrupted {
                    assert_eq!(decoded.unwrap_err().code(), "png-structure");
                } else {
                    assert_eq!(decoded.unwrap().pixels(), &[0xff0c_2238, 0xff4e_5a7b]);
                }
            }
        }
    }
    let mut encoded = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut encoded, b"IHDR", &header);
    for byte in &compressed {
        chunk(&mut encoded, b"IDAT", std::slice::from_ref(byte));
    }
    chunk(&mut encoded, b"IEND", &[]);
    assert_eq!(
        Image::from_png(&encoded).unwrap().pixels(),
        &[0xff0c_2238, 0xff4e_5a7b]
    );
}

#[test]
#[ignore = "manual PNG decoding throughput measurement"]
fn png_decode_throughput() {
    for (width, height, iterations) in [(64, 64, 2000), (240, 320, 500), (1, 16384, 100)] {
        let mut frame = Framebuffer::new(width, height).unwrap();
        for (index, pixel) in frame.image.pixels.iter_mut().enumerate() {
            *pixel = 0xff00_0000 | (index as u32).wrapping_mul(0x010307) & 0xffffff;
        }
        let encoded = frame.to_png();
        let started = std::time::Instant::now();
        for _ in 0..iterations {
            let image = Image::from_png(std::hint::black_box(&encoded)).unwrap();
            assert_eq!(image.pixels(), frame.pixels());
        }
        eprintln!(
            "PNG {width}x{height} iterations={iterations} elapsed={:?}",
            started.elapsed()
        );
    }
}

#[test]
fn png_round_trip_is_pixel_exact() {
    let mut f = Framebuffer::new(3, 2).unwrap();
    {
        let mut g = f.graphics();
        g.set_color(0x123456);
        g.fill_rect(0, 0, 3, 2);
        g.set_color(0xfedcba);
        g.fill_rect(1, 0, 1, 2);
    }
    let png = f.to_png();
    let decoded = Image::from_png(&png).unwrap();
    assert_eq!(decoded.pixels(), f.pixels());
    assert!(!decoded.is_mutable());
}

#[test]
#[allow(clippy::manual_midpoint)] // Independent encoder; two widened u8s sum to at most 510.
fn png_row_buffers_preserve_all_filters_and_adam7_pass_boundaries() {
    // Encode the filters independently, varying every component on each row.
    // Narrow images exercise empty Adam7 passes. Nonzero first-row filters
    // verify that every pass starts with a zero predictor row.
    for (width, height, bit_depth, first_filter) in [
        (1_u32, 17_u32, 8_u8, 2_usize),
        (17, 1, 8, 3),
        (11, 13, 8, 4),
        (64, 65, 8, 1),
        (1, 17, 16, 2),
        (17, 1, 16, 3),
        (11, 13, 16, 4),
        (64, 65, 16, 1),
    ] {
        let sample_bytes = usize::from(bit_depth / 8);
        let pixel_bytes = 4 * sample_bytes;
        let expected = (0..width * height)
            .map(|i| i.wrapping_mul(0x572d_9307).wrapping_add(0x1020_3040))
            .collect::<Vec<_>>();
        for interlaced in [false, true] {
            let passes: &[(u32, u32, u32, u32)] = if interlaced {
                &FIXTURE_ADAM7
            } else {
                &[(0, 0, 1, 1)]
            };
            let mut raw = Vec::new();
            for &(x0, y0, dx, dy) in passes {
                let xs = (x0..width).step_by(dx as usize).collect::<Vec<_>>();
                if xs.is_empty() {
                    continue;
                }
                let mut prior = vec![0; xs.len() * pixel_bytes];
                for (row_index, y) in (y0..height).step_by(dy as usize).enumerate() {
                    let row = xs
                        .iter()
                        .flat_map(|&x| {
                            let pixel = expected[(y * width + x) as usize];
                            let [a, r, g, b] = pixel.to_be_bytes();
                            [r, g, b, a].into_iter().flat_map(move |value| {
                                // Vary both bytes: sixteen-bit filters predict
                                // complete samples before scaling to 8 bits.
                                [value, value ^ 0xa5].into_iter().take(sample_bytes)
                            })
                        })
                        .collect::<Vec<_>>();
                    let filter = ((row_index + first_filter) % 5) as u8;
                    raw.push(filter);
                    for (i, &value) in row.iter().enumerate() {
                        let a = if i >= pixel_bytes {
                            row[i - pixel_bytes]
                        } else {
                            0
                        };
                        let b = prior[i];
                        let c = if i >= pixel_bytes {
                            prior[i - pixel_bytes]
                        } else {
                            0
                        };
                        let predictor = match filter {
                            0 => 0,
                            1 => a,
                            2 => b,
                            3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
                            4 => {
                                let estimate = i32::from(a) + i32::from(b) - i32::from(c);
                                [a, b, c]
                                    .into_iter()
                                    .min_by_key(|&v| (estimate - i32::from(v)).abs())
                                    .unwrap()
                            }
                            _ => unreachable!(),
                        };
                        raw.push(value.wrapping_sub(predictor));
                    }
                    prior = row;
                }
            }
            let mut encoded = b"\x89PNG\r\n\x1a\n".to_vec();
            let mut header = width.to_be_bytes().to_vec();
            header.extend_from_slice(&height.to_be_bytes());
            header.extend_from_slice(&[bit_depth, 6, 0, 0, u8::from(interlaced)]);
            chunk(&mut encoded, b"IHDR", &header);
            chunk(&mut encoded, b"IDAT", &deterministic_zlib_store(&raw));
            chunk(&mut encoded, b"IEND", &[]);
            let decoded_pixels = expected
                .iter()
                .map(|pixel| {
                    if bit_depth == 8 {
                        *pixel
                    } else {
                        u32::from_be_bytes(pixel.to_be_bytes().map(|high| {
                            let sample = u16::from_be_bytes([high, high ^ 0xa5]);
                            (f64::from(sample) / 257.0).round() as u8
                        }))
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(Image::from_png(&encoded).unwrap().pixels(), decoded_pixels);
            assert_eq!(
                Image::from_midp_encoded(&encoded).unwrap().pixels(),
                decoded_pixels
            );
        }
    }
}

#[test]
fn low_bit_depth_indexed_png_is_unpacked() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[1, 3, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"PLTE", &[0, 0, 0, 255, 0, 0]);
    chunk(
        &mut png,
        b"IDAT",
        &deterministic_zlib_store(&[0, 0b0100_0000]),
    );
    chunk(&mut png, b"IEND", &[]);
    let image = Image::from_png(&png).unwrap();
    assert_eq!(image.pixels(), &[0xff00_0000, 0xffff_0000]);
}

#[test]
fn indexed_transparency_png_decodes_palette_alpha() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[1, 3, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"PLTE", &[0, 0, 0, 255, 0, 0]);
    chunk(&mut png, b"tRNS", &[0]);
    chunk(
        &mut png,
        b"IDAT",
        &deterministic_zlib_store(&[0, 0b0100_0000]),
    );
    chunk(&mut png, b"IEND", &[]);

    let image = Image::from_png(&png).unwrap();

    assert_eq!(image.pixels(), &[0x0000_0000, 0xffff_0000]);
}

#[test]
fn oversized_png_transparency_is_rejected_before_image_data_is_read() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"PLTE", &[0; 768]);
    chunk(&mut png, b"tRNS", &[255; 257]);
    // No image data follows: the palette limit must apply as soon as the
    // oversized chunk arrives, rather than after the complete file is parsed.
    assert_eq!(Image::from_png(&png).unwrap_err().code(), "png-palette");
    assert_eq!(
        Image::from_midp_encoded(&png).unwrap_err().code(),
        "png-palette"
    );
}

#[test]
fn midp_indexed_png_without_transparency_preserves_opaque_pixels() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&4_u32.to_be_bytes());
    ihdr.extend_from_slice(&4_u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    let mut palette = vec![1; 768];
    palette[7 * 3..7 * 3 + 3].fill(0);
    palette[8 * 3..8 * 3 + 3].copy_from_slice(&[255, 0, 0]);
    chunk(&mut png, b"PLTE", &palette);
    let rows = [
        0, 7, 7, 8, 7, // top-left, top-right black
        0, 7, 8, 8, 8, 0, 8, 8, 8, 8, // opaque middle
        0, 7, 8, 8, 8, // bottom-left black
    ];
    chunk(&mut png, b"IDAT", &deterministic_zlib_store(&rows));
    chunk(&mut png, b"IEND", &[]);

    let strict = Image::from_png(&png).unwrap();
    let compatible = Image::from_midp_encoded(&png).unwrap();

    assert_eq!(
        strict.pixels(),
        &[
            0xff00_0000,
            0xff00_0000,
            0xffff_0000,
            0xff00_0000,
            0xff00_0000,
            0xffff_0000,
            0xffff_0000,
            0xffff_0000,
            0xffff_0000,
            0xffff_0000,
            0xffff_0000,
            0xffff_0000,
            0xff00_0000,
            0xffff_0000,
            0xffff_0000,
            0xffff_0000,
        ]
    );
    assert_eq!(compatible, strict);
}

#[test]
fn midp_solid_black_indexed_png_stays_opaque() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&4_u32.to_be_bytes());
    ihdr.extend_from_slice(&4_u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    let mut palette = vec![1; 768];
    palette[7 * 3..7 * 3 + 3].fill(0);
    chunk(&mut png, b"PLTE", &palette);
    let rows = [0, 7, 7, 7, 7, 0, 7, 7, 7, 7, 0, 7, 7, 7, 7, 0, 7, 7, 7, 7];
    chunk(&mut png, b"IDAT", &deterministic_zlib_store(&rows));
    chunk(&mut png, b"IEND", &[]);

    let image = Image::from_midp_encoded(&png).unwrap();

    assert_eq!(image.pixels(), &[0xff00_0000; 16]);
}

#[test]
fn damaged_first_png_filter_uses_midp_none_compatibility() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&2_u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(
        &mut png,
        b"IDAT",
        &deterministic_zlib_store(&[0x78, 1, 2, 3, 4, 0, 5, 6, 7, 8]),
    );
    chunk(&mut png, b"IEND", &[]);

    assert_eq!(Image::from_png(&png).unwrap_err().code(), "png-filter");
    let image = Image::from_midp_encoded(&png).unwrap();
    assert_eq!(image.pixels(), &[0x0401_0203, 0x0805_0607]);
    assert!(unfilter_png_row(5, &mut [0], &[0], 1).is_err());
}

#[test]
fn sixteen_bit_rgba_png_is_downsampled() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&[16, 6, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(
        &mut png,
        b"IDAT",
        &deterministic_zlib_store(&[0, 0xff, 0xff, 0x80, 0x00, 0, 0, 0x40, 0x00]),
    );
    chunk(&mut png, b"IEND", &[]);

    let image = Image::from_png(&png).unwrap();
    assert_eq!(image.pixels(), &[0x40ff_8000]);
}

#[test]
fn adam7_indexed_png_is_deinterlaced() {
    let (width, height) = (9u32, 9u32);
    let mut raw = Vec::new();
    for (x0, y0, dx, dy) in FIXTURE_ADAM7 {
        if x0 >= width || y0 >= height {
            continue;
        }
        for y in (y0..height).step_by(dy as usize) {
            raw.push(0);
            for x in (x0..width).step_by(dx as usize) {
                raw.push(((x + y) & 1) as u8);
            }
        }
    }

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 1]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"PLTE", &[0, 0, 0, 255, 0, 0]);
    chunk(&mut png, b"IDAT", &deterministic_zlib_store(&raw));
    chunk(&mut png, b"IEND", &[]);

    let image = Image::from_png(&png).unwrap();
    assert_eq!(image.width(), width);
    assert_eq!(image.height(), height);
    for y in 0..height {
        for x in 0..width {
            let expected = if (x + y) & 1 == 0 {
                0xff00_0000
            } else {
                0xffff_0000
            };
            assert_eq!(image.pixels()[(y * width + x) as usize], expected);
        }
    }
}

#[test]
fn grayscale_png_is_decoded() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&3u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[2, 0, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(
        &mut png,
        b"IDAT",
        &deterministic_zlib_store(&[0, 0b00_01_10_00]),
    );
    chunk(&mut png, b"IEND", &[]);
    let image = Image::from_png(&png).unwrap();
    assert_eq!(image.pixels(), &[0xff00_0000, 0xff55_5555, 0xffaa_aaaa]);
}

#[test]
fn grayscale_transparency_png_is_decoded() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"tRNS", &[0, 0x40]);
    chunk(
        &mut png,
        b"IDAT",
        &deterministic_zlib_store(&[0, 0x40, 0xc0]),
    );
    chunk(&mut png, b"IEND", &[]);
    let image = Image::from_png(&png).unwrap();
    assert_eq!(image.pixels(), &[0x0040_4040, 0xffc0_c0c0]);
}

#[test]
fn grayscale_png_transparency_masks_unused_sample_bits() {
    for (bit_depth, row) in [
        (1, &[0, 0b0100_0000][..]),
        (2, &[0, 0b0011_0000][..]),
        (4, &[0, 0b0000_1111][..]),
        (8, &[0, 0, 255][..]),
        (16, &[0, 0, 0, 255, 255][..]),
    ] {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&2_u32.to_be_bytes());
        ihdr.extend_from_slice(&1_u32.to_be_bytes());
        ihdr.extend_from_slice(&[bit_depth, 0, 0, 0, 0]);
        chunk(&mut png, b"IHDR", &ihdr);
        chunk(&mut png, b"tRNS", &[255, 255]);
        chunk(&mut png, b"IDAT", &deterministic_zlib_store(row));
        chunk(&mut png, b"IEND", &[]);

        let strict = Image::from_png(&png).unwrap();
        assert_eq!(
            strict.pixels(),
            &[0xff00_0000, 0x00ff_ffff],
            "depth {bit_depth}"
        );
        assert_eq!(Image::from_midp_encoded(&png).unwrap(), strict);
    }
}

#[test]
fn truecolor_transparency_png_is_decoded_before_sample_rescaling() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"tRNS", &[0, 0xbb, 0, 0, 0, 0xbb]);
    chunk(
        &mut png,
        b"IDAT",
        &deterministic_zlib_store(&[0, 0xbb, 0, 0xbb, 0xbb, 0, 0xbc]),
    );
    chunk(&mut png, b"IEND", &[]);

    let image = Image::from_png(&png).unwrap();
    assert_eq!(image.pixels(), &[0x00bb_00bb, 0xffbb_00bc]);
}

#[test]
fn grayscale_alpha_png_is_decoded() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 4, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(
        &mut png,
        b"IDAT",
        &deterministic_zlib_store(&[0, 0x20, 0xff, 0xc0, 0x80]),
    );
    chunk(&mut png, b"IEND", &[]);
    let image = Image::from_png(&png).unwrap();
    assert_eq!(image.pixels(), &[0xff20_2020, 0x80c0_c0c0]);
}

#[test]
fn png_accepts_trailing_bytes_but_requires_complete_structure() {
    let framebuffer = Framebuffer::new(1, 1).unwrap();
    let valid = framebuffer.to_png();
    assert!(Image::from_png(&valid[..valid.len() - 12]).is_err());
    let mut trailing = valid.clone();
    trailing.extend_from_slice(&[0, 0xff, b'P', b'K']);
    assert!(Image::from_png(&trailing).is_ok());
    let mut critical = valid;
    critical.splice(8..8, [0, 0, 0, 0, b'B', b'A', b'D', b'!']);
    assert!(Image::from_png(&critical).is_err());
}

#[test]
fn midp_out_of_range_palette_index_recovers_as_opaque_black() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&2_u32.to_be_bytes());
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"PLTE", &[255, 0, 0]);
    chunk(&mut png, b"tRNS", &[0]);
    chunk(&mut png, b"IDAT", &deterministic_zlib_store(&[0, 0, 255]));
    chunk(&mut png, b"IEND", &[]);

    assert_eq!(Image::from_png(&png).unwrap_err().code(), "png-palette");
    assert_eq!(
        Image::from_midp_encoded(&png).unwrap().pixels(),
        &[0x00ff_0000, 0xff00_0000]
    );
}

#[test]
fn midp_palette_replacement_may_leave_original_plte_crc() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"PLTE", &[255, 0, 0]);
    chunk(&mut png, b"IDAT", &deterministic_zlib_store(&[0, 0]));
    chunk(&mut png, b"IEND", &[]);

    let palette = png.windows(4).position(|bytes| bytes == b"PLTE").unwrap() + 4;
    png[palette..palette + 3].copy_from_slice(&[0, 255, 0]);

    assert_eq!(Image::from_png(&png).unwrap_err().code(), "png-crc");
    assert_eq!(
        Image::from_midp_encoded(&png).unwrap().pixels(),
        &[0xff00_ff00]
    );
}

#[test]
fn midp_extended_indexed_transparency_may_leave_one_entry_trns_crc() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(
        &mut png,
        b"PLTE",
        &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
    );
    chunk(&mut png, b"tRNS", &[0xff, 0xff, 0xff, 0]);
    let trns_crc = png.len() - 4;
    let mut hasher = Hasher::new();
    hasher.update(b"tRNS");
    hasher.update(&[0]);
    png[trns_crc..].copy_from_slice(&hasher.finalize().to_be_bytes());
    chunk(&mut png, b"IDAT", &deterministic_zlib_store(&[0, 3]));
    chunk(&mut png, b"IEND", &[]);

    assert_eq!(Image::from_png(&png).unwrap_err().code(), "png-crc");
    assert_eq!(
        Image::from_midp_encoded(&png).unwrap().pixels(),
        &[0x00ff_ffff]
    );
}

#[test]
fn sixteen_bit_grayscale_transparency_compares_before_rescaling() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&2_u32.to_be_bytes());
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&[16, 0, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"tRNS", &[255, 254]);
    chunk(
        &mut png,
        b"IDAT",
        &deterministic_zlib_store(&[0, 255, 254, 255, 255]),
    );
    chunk(&mut png, b"IEND", &[]);

    let strict = Image::from_png(&png).unwrap();
    assert_eq!(strict.pixels(), &[0x00ff_ffff, 0xffff_ffff]);
    assert_eq!(Image::from_midp_encoded(&png).unwrap(), strict);
}

#[test]
fn midp_rejects_unrecognized_trns_crc_mismatch() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"PLTE", &[255, 0, 0, 0, 255, 0]);
    chunk(&mut png, b"tRNS", &[0xff, 0]);
    let trns_crc = png.len() - 4;
    png[trns_crc..].copy_from_slice(&0_u32.to_be_bytes());
    chunk(&mut png, b"IDAT", &deterministic_zlib_store(&[0, 1]));
    chunk(&mut png, b"IEND", &[]);

    assert_eq!(
        Image::from_midp_encoded(&png).unwrap_err().code(),
        "png-crc"
    );
}
