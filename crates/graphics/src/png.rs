//! PNG container validation, bounded inflation and deterministic encoding.

use super::{Category, EmuError, Image, checked_pixels, error};
use crc32fast::Hasher;
use flate2::{Decompress, FlushDecompress, Status};
use std::borrow::Cow;

mod rows;
use rows::{
    adam7_expected_size, decode_adam7, decode_png_row, png_filter_bpp, png_row_bytes,
    unfilter_png_row,
};

pub(super) fn decode_png(bytes: &[u8]) -> Result<Image, EmuError> {
    decode_png_inner(bytes, false)
}

pub(super) fn decode_png_with_midp_compatibility(bytes: &[u8]) -> Result<Image, EmuError> {
    decode_png_inner(bytes, true)
}

fn read_png_u32(bytes: &[u8], offset: usize) -> Result<u32, EmuError> {
    let value = bytes
        .get(offset..)
        .and_then(|tail| tail.first_chunk::<4>())
        .ok_or_else(|| error("png-chunk", "truncated PNG chunk"))?;
    Ok(u32::from_be_bytes(*value))
}

fn has_legacy_extended_trns_crc(data: &[u8], expected: u32) -> bool {
    // Palette reordering can extend tRNS with opaque entries while retaining
    // the CRC of its original [0] payload. Accept only that exact stale form.
    if data.len() <= 1
        || data.last() != Some(&0)
        || !data[..data.len() - 1].iter().all(|alpha| *alpha == 0xff)
    {
        return false;
    }
    let mut hasher = Hasher::new();
    hasher.update(b"tRNS");
    hasher.update(&[0]);
    hasher.finalize() == expected
}

fn decode_png_inner(bytes: &[u8], midp_compatibility: bool) -> Result<Image, EmuError> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(error("png-signature", "invalid PNG signature"));
    }
    let mut p = 8;
    let (mut w, mut h, mut ct, mut bit_depth, mut interlace) = (0, 0, 0, 0, 0);
    let mut palette: &[[u8; 3]] = &[];
    let mut alpha: &[u8] = &[];
    let mut transparent_gray = None;
    let mut transparent_rgb = None;
    let mut compressed = Cow::Borrowed(&[][..]);
    let mut seen_ihdr = false;
    let mut seen_idat = false;
    let mut ended_idat = false;
    let mut seen_iend = false;
    while bytes.get(p..).is_some_and(|tail| tail.len() >= 12) {
        let len = read_png_u32(bytes, p)? as usize;
        let kind_start = p
            .checked_add(4)
            .ok_or_else(|| error("png-chunk", "PNG chunk offset overflow"))?;
        let data_start = kind_start
            .checked_add(4)
            .ok_or_else(|| error("png-chunk", "PNG chunk offset overflow"))?;
        let data_end = data_start
            .checked_add(len)
            .ok_or_else(|| error("png-chunk", "PNG chunk length overflow"))?;
        let chunk_end = data_end
            .checked_add(4)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| error("png-chunk", "truncated PNG chunk"))?;
        let kind = bytes
            .get(kind_start..data_start)
            .ok_or_else(|| error("png-chunk", "truncated PNG chunk kind"))?;
        let data = bytes
            .get(data_start..data_end)
            .ok_or_else(|| error("png-chunk", "truncated PNG chunk data"))?;
        let expected = read_png_u32(bytes, data_end)?;
        p = chunk_end;
        let mut hasher = Hasher::new();
        hasher.update(kind);
        hasher.update(data);
        // MIDP assets can replace PLTE without updating its CRC. All chunk
        // bounds, palette structure and compressed image data remain checked.
        let compatible_crc = midp_compatibility
            && (kind == b"PLTE"
                || (kind == b"tRNS"
                    && seen_ihdr
                    && !seen_idat
                    && ct == 3
                    && has_legacy_extended_trns_crc(data, expected)));
        if hasher.finalize() != expected && !compatible_crc {
            return Err(error("png-crc", "PNG chunk CRC mismatch"));
        }
        match kind {
            b"IHDR" if len == 13 && !seen_ihdr && p == 33 => {
                w = read_png_u32(data, 0)?;
                h = read_png_u32(data, 4)?;
                if !matches!(
                    (data[9], data[8]),
                    (0, 1 | 2 | 4 | 8 | 16) | (3, 1 | 2 | 4 | 8) | (2 | 4 | 6, 8 | 16)
                ) || data[10] != 0
                    || data[11] != 0
                    || data[12] > 1
                {
                    return Err(EmuError::new(
                        Category::Api,
                        "png-format",
                        format!(
                            "unsupported PNG format: bit_depth={} color_type={} compression={} filter={} interlace={}",
                            data[8], data[9], data[10], data[11], data[12]
                        ),
                    ));
                }
                ct = data[9];
                bit_depth = data[8];
                interlace = data[12];
                checked_pixels(w, h)?;
                seen_ihdr = true;
            }
            b"IHDR" => return Err(error("png-structure", "invalid or repeated PNG IHDR")),
            b"PLTE" if seen_ihdr && !seen_idat && !palette.is_empty() => {
                return Err(error("png-structure", "repeated PNG palette"));
            }
            b"PLTE"
                if seen_ihdr && !seen_idat && len > 0 && len <= 768 && len.is_multiple_of(3) =>
            {
                palette = data.as_chunks::<3>().0;
            }
            b"PLTE" => return Err(error("png-palette", "invalid PNG palette")),
            b"tRNS"
                if seen_ihdr && !seen_idat && ct == 0 && transparent_gray.is_none() && len == 2 =>
            {
                transparent_gray = Some(png_transparency_sample([data[0], data[1]], bit_depth));
            }
            b"tRNS"
                if seen_ihdr && !seen_idat && ct == 2 && transparent_rgb.is_none() && len == 6 =>
            {
                transparent_rgb = Some([
                    png_transparency_sample([data[0], data[1]], bit_depth),
                    png_transparency_sample([data[2], data[3]], bit_depth),
                    png_transparency_sample([data[4], data[5]], bit_depth),
                ]);
            }
            b"tRNS" if seen_ihdr && !seen_idat && ct == 3 && alpha.is_empty() => {
                // An indexed palette has at most 256 entries. Apply that
                // limit even if IDAT/IEND is missing and the final palette
                // validation is never reached.
                if len > 256 {
                    return Err(error(
                        "png-palette",
                        "PNG transparency exceeds palette limit",
                    ));
                }
                alpha = data;
            }
            b"tRNS" => return Err(error("png-format", "unsupported PNG transparency chunk")),
            b"IDAT" if seen_ihdr && !ended_idat => {
                seen_idat = true;
                if compressed.len().saturating_add(data.len()) > 32 * 1_024 * 1_024 {
                    return Err(error("png-data", "PNG compressed data exceeds limit"));
                }
                if compressed.is_empty() {
                    compressed = Cow::Borrowed(data);
                } else if !data.is_empty() {
                    compressed.to_mut().extend_from_slice(data);
                }
            }
            b"IDAT" => return Err(error("png-structure", "non-contiguous PNG IDAT chunks")),
            b"IEND" if seen_ihdr && seen_idat && len == 0 => {
                seen_iend = true;
                break;
            }
            b"IEND" => return Err(error("png-structure", "invalid PNG IEND")),
            _ if kind[0] & 0x20 == 0 => {
                return Err(error("png-format", "unsupported critical PNG chunk"));
            }
            _ => {}
        }
        if seen_idat && kind != b"IDAT" {
            ended_idat = true;
        }
    }
    // Packed image slices may extend beyond IEND. Ignore that tail while
    // requiring a complete PNG with validated chunks and compressed data.
    if !seen_ihdr || !seen_idat || !seen_iend {
        return Err(error("png-structure", "PNG has missing structural data"));
    }
    if ct == 3 && (palette.is_empty() || alpha.len() > palette.len()) {
        return Err(error("png-palette", "indexed PNG lacks a valid palette"));
    }
    let filter_bpp = png_filter_bpp(ct, bit_depth);
    let pixel_count = checked_pixels(w, h)?;
    let expected = if interlace == 0 {
        (png_row_bytes(w, ct, bit_depth)? + 1)
            .checked_mul(h as usize)
            .ok_or_else(|| error("png-data", "PNG scanline size overflow"))?
    } else {
        adam7_expected_size(w, h, ct, bit_depth)?
    };
    let mut raw = Vec::new();
    raw.try_reserve_exact(expected)
        .map_err(|_| error("png-data", "cannot reserve PNG scanline buffer"))?;
    // Require the zlib trailer as well as the scanlines; a Read EOF alone does
    // not distinguish a complete stream from a truncated checksum.
    let status = Decompress::new(true)
        .decompress_vec(&compressed, &mut raw, FlushDecompress::Finish)
        .map_err(|_| error("png-deflate", "invalid PNG compressed data"))?;
    if status != Status::StreamEnd {
        return Err(error(
            "png-deflate",
            "PNG compressed data is incomplete or exceeds its declared size",
        ));
    }
    if raw.len() != expected {
        return Err(error("png-data", "PNG scanline size mismatch"));
    }

    let pixels = if interlace == 0 {
        let stride = png_row_bytes(w, ct, bit_depth)?;
        let initial_prior = vec![0u8; stride];
        let mut prior = initial_prior.as_slice();
        let mut pixels = Vec::with_capacity(pixel_count);
        for (row_index, row) in raw.chunks_exact_mut(stride + 1).enumerate() {
            // MIDP tolerates an invalid first filter as None. Later rows stay
            // strict because their filters depend on the preceding scanline.
            let filter = if midp_compatibility && row_index == 0 && row[0] > 4 {
                0
            } else {
                row[0]
            };
            let cur = &mut row[1..];
            unfilter_png_row(filter, cur, prior, filter_bpp)?;
            decode_png_row(
                cur,
                w as usize,
                ct,
                bit_depth,
                palette,
                alpha,
                transparent_gray,
                transparent_rgb,
                midp_compatibility,
                &mut pixels,
            )?;
            prior = cur;
        }
        pixels
    } else {
        decode_adam7(
            &mut raw,
            w,
            h,
            ct,
            bit_depth,
            palette,
            alpha,
            transparent_gray,
            transparent_rgb,
            midp_compatibility,
        )?
    };

    Ok(Image {
        width: w,
        height: h,
        pixels,
        mutable: false,
    })
}

fn png_transparency_sample(bytes: [u8; 2], bit_depth: u8) -> u16 {
    // tRNS stores 16-bit samples at every depth; unused high bits must be
    // masked before comparison, while all bits matter for a 16-bit image.
    u16::from_be_bytes(bytes) & (u16::MAX >> (16 - bit_depth))
}

#[allow(clippy::trivially_copy_pass_by_ref)]
pub(super) fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut h = Hasher::new();
    h.update(kind);
    h.update(data);
    out.extend_from_slice(&h.finalize().to_be_bytes());
}
pub(super) fn encode_png(image: &Image) -> Vec<u8> {
    let mut raw = Vec::with_capacity((image.width as usize * 4 + 1) * image.height as usize);
    for row in image.pixels.chunks_exact(image.width as usize) {
        raw.push(0);
        for p in row {
            raw.extend_from_slice(&[
                ((p >> 16) & 255) as u8,
                ((p >> 8) & 255) as u8,
                (p & 255) as u8,
                (p >> 24) as u8,
            ]);
        }
    }
    let data = deterministic_zlib_store(&raw);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&image.width.to_be_bytes());
    ihdr.extend_from_slice(&image.height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &data);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Emits zlib-wrapped DEFLATE stored blocks. The result is intentionally not
/// host-compressor-dependent; PNG snapshots must be byte-identical even when
/// Cargo feature unification selects a different decoder backend elsewhere.
pub(super) fn deterministic_zlib_store(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 65_535 * 5 + 11);
    out.extend_from_slice(&[0x78, 0x01]);
    if bytes.is_empty() {
        out.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    } else {
        let chunks = bytes.chunks(65_535);
        let count = chunks.len();
        for (index, block) in chunks.enumerate() {
            out.push(u8::from(index + 1 == count));
            let length = block.len() as u16;
            out.extend_from_slice(&length.to_le_bytes());
            out.extend_from_slice(&(!length).to_le_bytes());
            out.extend_from_slice(block);
        }
    }
    let mut a = 1u32;
    let mut b = 0u32;
    // With reduced initial sums, 5,552 bytes of 0xff still fit both sums in
    // u32. Reduce once per block instead of twice for every input byte.
    for block in bytes.chunks(5_552) {
        for &byte in block {
            a += u32::from(byte);
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    out.extend_from_slice(&(b << 16 | a).to_be_bytes());
    out
}

#[cfg(test)]
#[allow(clippy::trivially_copy_pass_by_ref, clippy::unreadable_literal)]
#[path = "../../../tests/unit/graphics/png.rs"]
mod tests;
