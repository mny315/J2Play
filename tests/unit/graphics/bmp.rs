use crate::Image;

fn bmp_fixture(width: usize, height: usize, depth: u16, top_down: bool) -> (Vec<u8>, Vec<u32>) {
    let palette_len = if depth <= 8 { 1_usize << depth } else { 0 };
    let offset = 54 + palette_len * 4;
    let stride = (width * usize::from(depth)).div_ceil(32) * 4;
    let length = offset + stride * height;
    let mut bytes = vec![0_u8; length];
    bytes[..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&(length as u32).to_le_bytes());
    bytes[10..14].copy_from_slice(&(offset as u32).to_le_bytes());
    bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    let signed_height = if top_down {
        -(height as i32)
    } else {
        height as i32
    };
    bytes[22..26].copy_from_slice(&signed_height.to_le_bytes());
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&depth.to_le_bytes());
    for index in 0..palette_len {
        bytes[54 + index * 4..58 + index * 4].copy_from_slice(&[
            (index * 3) as u8,
            (index * 5) as u8,
            (index * 7) as u8,
            index as u8,
        ]);
    }
    let mut expected = Vec::with_capacity(width * height);
    for y in 0..height {
        let encoded_y = if top_down { y } else { height - 1 - y };
        let row = &mut bytes[offset + encoded_y * stride..offset + (encoded_y + 1) * stride];
        for x in 0..width {
            let pixel = if depth <= 8 {
                let index = ((x + 3 * y) % palette_len) as u8;
                let bit = x * usize::from(depth);
                row[bit / 8] |= index << (8 - usize::from(depth) - bit % 8);
                0xff00_0000
                    | u32::from(index.wrapping_mul(7)) << 16
                    | u32::from(index.wrapping_mul(5)) << 8
                    | u32::from(index.wrapping_mul(3))
            } else if depth == 16 {
                let red = ((3 * x + y) % 32) as u16;
                let green = ((x + 5 * y) % 32) as u16;
                let blue = ((7 * x + 11 * y) % 32) as u16;
                let packed = red * 1024 + green * 32 + blue;
                row[x * 2..x * 2 + 2].copy_from_slice(&packed.to_le_bytes());
                0xff00_0000
                    | (u32::from(red) * 255 / 31) << 16
                    | (u32::from(green) * 255 / 31) << 8
                    | (u32::from(blue) * 255 / 31)
            } else {
                let b = (3 * x + 7 * y) as u8;
                let g = (5 * x + 11 * y) as u8;
                let r = (13 * x + 17 * y) as u8;
                let size = usize::from(depth / 8);
                row[x * size..x * size + 3].copy_from_slice(&[b, g, r]);
                if depth == 32 {
                    row[x * size + 3] = (x + y) as u8;
                }
                0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            };
            expected.push(pixel);
        }
    }
    (bytes, expected)
}

#[test]
fn bmp_rows_preserve_padding_colors_and_both_vertical_orders_at_every_depth() {
    for depth in [1, 4, 8, 16, 24, 32] {
        for top_down in [false, true] {
            for (width, height) in [(1, 1), (13, 7)] {
                let (bytes, expected) = bmp_fixture(width, height, depth, top_down);
                let image = Image::from_encoded(&bytes).unwrap();
                assert_eq!(
                    (image.width(), image.height()),
                    (width as u32, height as u32)
                );
                assert_eq!(
                    image.pixels(),
                    expected,
                    "depth={depth}, top_down={top_down}"
                );
            }
        }
    }
}

#[test]
#[ignore = "manual BMP decoding throughput measurement"]
fn bmp_decode_throughput() {
    for size in [8, 256] {
        for depth in [1, 4, 8, 16, 24, 32] {
            for top_down in [false, true] {
                let (bytes, expected) = bmp_fixture(size, size, depth, top_down);
                let iterations = if size == 8 { 4_096 } else { 128 };
                let started = std::time::Instant::now();
                for _ in 0..iterations {
                    let image = Image::from_encoded(std::hint::black_box(&bytes)).unwrap();
                    assert_eq!(image.pixels(), expected);
                }
                let elapsed = started.elapsed();
                eprintln!("BMP size={size} depth={depth} top_down={top_down} elapsed={elapsed:?}");
            }
        }
    }
}

#[test]
fn encoded_image_dispatches_bottom_up_bmp_with_row_padding() {
    let mut bmp = vec![0_u8; 70];
    bmp[0..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&70_u32.to_le_bytes());
    bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&2_i32.to_le_bytes());
    bmp[22..26].copy_from_slice(&2_i32.to_le_bytes());
    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
    bmp[34..38].copy_from_slice(&16_u32.to_le_bytes());
    // BMP rows are bottom-up and each encoded row is padded to four bytes.
    bmp[54..62].copy_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]);
    bmp[62..70].copy_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);

    let image = Image::from_encoded(&bmp).unwrap();

    assert_eq!((image.width(), image.height()), (2, 2));
    assert_eq!(
        image.pixels(),
        &[0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff]
    );
    assert!(!image.is_mutable());
}

#[test]
fn indexed_top_down_bmp_uses_bgra_palette() {
    let mut bmp = vec![0_u8; 66];
    bmp[0..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&66_u32.to_le_bytes());
    bmp[10..14].copy_from_slice(&62_u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&2_i32.to_le_bytes());
    bmp[22..26].copy_from_slice(&(-1_i32).to_le_bytes());
    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&8_u16.to_le_bytes());
    bmp[34..38].copy_from_slice(&4_u32.to_le_bytes());
    bmp[46..50].copy_from_slice(&2_u32.to_le_bytes());
    bmp[54..62].copy_from_slice(&[0, 0, 0, 0, 0, 0, 255, 0]);
    bmp[62..66].copy_from_slice(&[1, 0, 0, 0]);

    let image = Image::from_midp_encoded(&bmp).unwrap();

    assert_eq!(image.pixels(), &[0xffff_0000, 0xff00_0000]);
}

#[test]
fn bmp_rejects_truncated_pixel_data() {
    let mut bmp = vec![0_u8; 54];
    bmp[0..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&58_u32.to_le_bytes());
    bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&1_i32.to_le_bytes());
    bmp[22..26].copy_from_slice(&1_i32.to_le_bytes());
    bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());

    assert_eq!(
        Image::from_encoded(&bmp).unwrap_err().code(),
        "bmp-structure"
    );
}
