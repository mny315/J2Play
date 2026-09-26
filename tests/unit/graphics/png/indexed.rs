use super::*;

fn indexed_png(
    width: u32,
    height: u32,
    bit_depth: u8,
    palette_entries: usize,
    alpha_entries: usize,
    interlaced: bool,
) -> (Vec<u8>, Vec<u32>) {
    let colors = (0..palette_entries)
        .map(|index| {
            (index as u32)
                .wrapping_mul(0x0037_5ba3)
                .wrapping_add(0x12_3456)
                & 0xff_ffff
        })
        .collect::<Vec<_>>();
    let alpha = (0..alpha_entries)
        .map(|index| (index * 73) as u8)
        .collect::<Vec<_>>();
    let index_at = |x: u32, y: u32| ((x * 3 + y * 5) as usize % palette_entries) as u8;
    let expected = (0..height)
        .flat_map(|y| {
            let colors = &colors;
            let alpha = &alpha;
            (0..width).map(move |x| {
                let index = usize::from(index_at(x, y));
                (u32::from(alpha.get(index).copied().unwrap_or(255)) << 24) | colors[index]
            })
        })
        .collect();
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
        for y in (y0..height).step_by(dy as usize) {
            raw.push(0);
            let mut row = vec![0; (xs.len() * usize::from(bit_depth)).div_ceil(8)];
            for (position, &x) in xs.iter().enumerate() {
                let bit = position * usize::from(bit_depth);
                row[bit / 8] |= index_at(x, y) << (8 - usize::from(bit_depth) - bit % 8);
            }
            raw.extend(row);
        }
    }
    let mut encoded = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend(width.to_be_bytes());
    header.extend(height.to_be_bytes());
    header.extend([bit_depth, 3, 0, 0, u8::from(interlaced)]);
    chunk(&mut encoded, b"IHDR", &header);
    let palette = colors
        .iter()
        .flat_map(|color| {
            let bytes = color.to_be_bytes();
            [bytes[1], bytes[2], bytes[3]]
        })
        .collect::<Vec<_>>();
    chunk(&mut encoded, b"PLTE", &palette);
    if !alpha.is_empty() {
        chunk(&mut encoded, b"tRNS", &alpha);
    }
    chunk(&mut encoded, b"IDAT", &deterministic_zlib_store(&raw));
    chunk(&mut encoded, b"IEND", &[]);
    (encoded, expected)
}

#[test]
fn indexed_palettes_preserve_all_depths_alpha_lengths_and_adam7_passes() {
    for depth in [1, 2, 4, 8] {
        let entries = 1_usize << depth;
        for alpha_entries in [0, entries / 2, entries] {
            for interlaced in [false, true] {
                let (encoded, expected) =
                    indexed_png(19, 17, depth, entries, alpha_entries, interlaced);
                assert_eq!(Image::from_png(&encoded).unwrap().pixels(), expected);
                assert_eq!(
                    Image::from_midp_encoded(&encoded).unwrap().pixels(),
                    expected
                );
            }
        }
    }
}

#[test]
#[ignore = "manual PNG decoding throughput measurement"]
fn indexed_png_throughput() {
    for size in [1, 8, 256] {
        for (depth, entries) in [(1, 2), (4, 16), (8, 256)] {
            for interlaced in [false, true] {
                let (encoded, expected) =
                    indexed_png(size, size, depth, entries, entries / 2, interlaced);
                let iterations = if size == 256 { 128 } else { 4_096 };
                let started = std::time::Instant::now();
                for _ in 0..iterations {
                    let decoded = Image::from_png(std::hint::black_box(&encoded)).unwrap();
                    assert_eq!(decoded.pixels(), expected);
                }
                let elapsed = started.elapsed();
                eprintln!(
                    "indexed-png size={size} depth={depth} interlaced={interlaced} elapsed={elapsed:?}"
                );
            }
        }
    }
}
