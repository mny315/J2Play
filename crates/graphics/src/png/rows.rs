//! Scanline filtering, sample decoding and Adam7 reconstruction.

use super::{EmuError, checked_pixels, error};

fn png_bits_per_pixel(ct: u8, bit_depth: u8) -> usize {
    match ct {
        0 | 3 => usize::from(bit_depth),
        2 => 3 * usize::from(bit_depth),
        4 => 2 * usize::from(bit_depth),
        6 => 4 * usize::from(bit_depth),
        _ => unreachable!(),
    }
}

pub(super) fn png_filter_bpp(ct: u8, bit_depth: u8) -> usize {
    png_bits_per_pixel(ct, bit_depth).div_ceil(8).max(1)
}

pub(super) fn png_row_bytes(width: u32, ct: u8, bit_depth: u8) -> Result<usize, EmuError> {
    (width as usize)
        .checked_mul(png_bits_per_pixel(ct, bit_depth))
        .map(|bits| bits.div_ceil(8))
        .ok_or_else(|| error("png-data", "PNG scanline size overflow"))
}

pub(super) const ADAM7_PASSES: [(u32, u32, u32, u32); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

pub(super) fn adam7_extent(size: u32, start: u32, step: u32) -> u32 {
    if size <= start {
        0
    } else {
        (size - start).div_ceil(step)
    }
}

pub(super) fn adam7_expected_size(
    w: u32,
    h: u32,
    ct: u8,
    bit_depth: u8,
) -> Result<usize, EmuError> {
    let mut expected = 0usize;
    for (x0, y0, dx, dy) in ADAM7_PASSES {
        let pw = adam7_extent(w, x0, dx);
        let ph = adam7_extent(h, y0, dy);
        if pw == 0 || ph == 0 {
            continue;
        }
        let pass = (png_row_bytes(pw, ct, bit_depth)? + 1)
            .checked_mul(ph as usize)
            .ok_or_else(|| error("png-data", "PNG scanline size overflow"))?;
        expected = expected
            .checked_add(pass)
            .ok_or_else(|| error("png-data", "PNG scanline size overflow"))?;
    }
    Ok(expected)
}

pub(super) fn unfilter_png_row(
    filter: u8,
    cur: &mut [u8],
    prior: &[u8],
    filter_bpp: usize,
) -> Result<(), EmuError> {
    match filter {
        0 => {}
        1 => {
            for i in filter_bpp..cur.len() {
                cur[i] = cur[i].wrapping_add(cur[i - filter_bpp]);
            }
        }
        2 => {
            for (value, above) in cur.iter_mut().zip(prior) {
                *value = value.wrapping_add(*above);
            }
        }
        3 => {
            for i in 0..cur.len() {
                let left = if i >= filter_bpp {
                    cur[i - filter_bpp]
                } else {
                    0
                };
                cur[i] = cur[i].wrapping_add(left.midpoint(prior[i]));
            }
        }
        4 => {
            for i in 0..cur.len() {
                let (left, upper_left) = if i >= filter_bpp {
                    (cur[i - filter_bpp], prior[i - filter_bpp])
                } else {
                    (0, 0)
                };
                cur[i] = cur[i].wrapping_add(paeth(left, prior[i], upper_left));
            }
        }
        _ => return Err(error("png-filter", "invalid PNG filter")),
    }
    Ok(())
}

pub(super) fn decode_png_row(
    cur: &[u8],
    width: usize,
    ct: u8,
    bit_depth: u8,
    palette: &[[u8; 3]],
    alpha: &[u8],
    transparent_gray: Option<u16>,
    transparent_rgb: Option<[u16; 3]>,
    recover_invalid_palette_indices: bool,
    pixels: &mut Vec<u32>,
) -> Result<(), EmuError> {
    let start = pixels.len();
    match ct {
        0 => {
            let mask = if bit_depth >= 8 {
                u16::MAX
            } else {
                (1u16 << bit_depth) - 1
            };
            for x in 0..width {
                let raw_sample = match bit_depth {
                    16 => u16::from_be_bytes([cur[x * 2], cur[x * 2 + 1]]),
                    8 => u16::from(cur[x]),
                    _ => {
                        let bit = x * usize::from(bit_depth);
                        let shift = 8 - usize::from(bit_depth) - bit % 8;
                        u16::from(cur[bit / 8] >> shift) & mask
                    }
                };
                let sample = match bit_depth {
                    16 => ((u32::from(raw_sample) * 255 + 32_767) / 65_535) as u8,
                    8 => raw_sample as u8,
                    _ => ((raw_sample * 255) / mask) as u8,
                };
                let alpha = if transparent_gray == Some(raw_sample) {
                    0
                } else {
                    0xff00_0000
                };
                let gray = u32::from(sample);
                pixels.push(alpha | gray << 16 | gray << 8 | gray);
            }
        }
        2 => {
            let bytes_per_pixel = if bit_depth == 16 { 6 } else { 3 };
            for v in cur.chunks_exact(bytes_per_pixel).take(width) {
                let raw = if bit_depth == 16 {
                    (
                        u16::from_be_bytes([v[0], v[1]]),
                        u16::from_be_bytes([v[2], v[3]]),
                        u16::from_be_bytes([v[4], v[5]]),
                    )
                } else {
                    (u16::from(v[0]), u16::from(v[1]), u16::from(v[2]))
                };
                let (red, green, blue) = if bit_depth == 16 {
                    (
                        png_sample_16(v[0], v[1]),
                        png_sample_16(v[2], v[3]),
                        png_sample_16(v[4], v[5]),
                    )
                } else {
                    (v[0], v[1], v[2])
                };
                let alpha = if transparent_rgb == Some([raw.0, raw.1, raw.2]) {
                    0
                } else {
                    0xff00_0000
                };
                pixels.push(alpha | u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue));
            }
        }
        6 => {
            let bytes_per_pixel = if bit_depth == 16 { 8 } else { 4 };
            for v in cur.chunks_exact(bytes_per_pixel).take(width) {
                let (red, green, blue, alpha) = if bit_depth == 16 {
                    (
                        png_sample_16(v[0], v[1]),
                        png_sample_16(v[2], v[3]),
                        png_sample_16(v[4], v[5]),
                        png_sample_16(v[6], v[7]),
                    )
                } else {
                    (v[0], v[1], v[2], v[3])
                };
                pixels.push(
                    u32::from(alpha) << 24
                        | u32::from(red) << 16
                        | u32::from(green) << 8
                        | u32::from(blue),
                );
            }
        }
        4 => {
            let bytes_per_pixel = if bit_depth == 16 { 4 } else { 2 };
            for v in cur.chunks_exact(bytes_per_pixel).take(width) {
                let (gray, alpha) = if bit_depth == 16 {
                    (png_sample_16(v[0], v[1]), png_sample_16(v[2], v[3]))
                } else {
                    (v[0], v[1])
                };
                let gray = u32::from(gray);
                pixels.push(u32::from(alpha) << 24 | gray << 16 | gray << 8 | gray);
            }
        }
        3 => {
            let mask = if bit_depth == 8 {
                u8::MAX
            } else {
                (1u8 << bit_depth) - 1
            };
            for x in 0..width {
                let bit = x * usize::from(bit_depth);
                let shift = 8 - usize::from(bit_depth) - bit % 8;
                let i = cur[bit / 8] >> shift & mask;
                let Some(&[red, green, blue]) = palette.get(usize::from(i)) else {
                    // PNG defines an out-of-range palette index as malformed,
                    // but opaque black is the specified decoder recovery. The
                    // MIDP path uses that bounded recovery for deployed assets;
                    // the strict decoder continues to expose the input error.
                    if recover_invalid_palette_indices {
                        pixels.push(0xff00_0000);
                        continue;
                    }
                    return Err(error("png-palette", "invalid palette index"));
                };
                pixels.push(
                    u32::from(*alpha.get(i as usize).unwrap_or(&255)) << 24
                        | u32::from(red) << 16
                        | u32::from(green) << 8
                        | u32::from(blue),
                );
            }
        }
        _ => unreachable!(),
    }
    if pixels.len() - start != width {
        return Err(error("png-data", "PNG row size mismatch"));
    }
    Ok(())
}

fn png_sample_16(high: u8, low: u8) -> u8 {
    let sample = u32::from(u16::from_be_bytes([high, low]));
    ((sample * 255 + 32_767) / 65_535) as u8
}

pub(super) fn decode_adam7(
    mut raw: &mut [u8],
    w: u32,
    h: u32,
    ct: u8,
    bit_depth: u8,
    palette: &[[u8; 3]],
    alpha: &[u8],
    transparent_gray: Option<u16>,
    transparent_rgb: Option<[u16; 3]>,
    recover_invalid_palette_indices: bool,
) -> Result<Vec<u32>, EmuError> {
    let filter_bpp = png_filter_bpp(ct, bit_depth);
    let mut pixels = vec![0u32; checked_pixels(w, h)?];
    for (x0, y0, dx, dy) in ADAM7_PASSES {
        let pw = adam7_extent(w, x0, dx);
        let ph = adam7_extent(h, y0, dy);
        if pw == 0 || ph == 0 {
            continue;
        }
        let stride = png_row_bytes(pw, ct, bit_depth)?;
        let pass_bytes = (stride + 1)
            .checked_mul(ph as usize)
            .ok_or_else(|| error("png-data", "PNG scanline size overflow"))?;
        let (pass, remaining) = raw
            .split_at_mut_checked(pass_bytes)
            .ok_or_else(|| error("png-data", "PNG scanline size mismatch"))?;
        raw = remaining;
        let initial_prior = vec![0u8; stride];
        let mut prior = initial_prior.as_slice();
        let mut pass_pixels = Vec::with_capacity(pw as usize);
        for (py, row) in pass.chunks_exact_mut(stride + 1).enumerate() {
            let filter = row[0];
            let cur = &mut row[1..];
            unfilter_png_row(filter, cur, prior, filter_bpp)?;
            pass_pixels.clear();
            decode_png_row(
                cur,
                pw as usize,
                ct,
                bit_depth,
                palette,
                alpha,
                transparent_gray,
                transparent_rgb,
                recover_invalid_palette_indices,
                &mut pass_pixels,
            )?;
            let y = y0 + py as u32 * dy;
            for (px, &pixel) in pass_pixels.iter().enumerate() {
                let x = x0 + px as u32 * dx;
                pixels[(y * w + x) as usize] = pixel;
            }
            prior = cur;
        }
    }
    if !raw.is_empty() {
        return Err(error("png-data", "PNG scanline size mismatch"));
    }
    Ok(pixels)
}
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i32::from(a) + i32::from(b) - i32::from(c);
    let (pa, pb, pc) = (
        (p - i32::from(a)).abs(),
        (p - i32::from(b)).abs(),
        (p - i32::from(c)).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}
