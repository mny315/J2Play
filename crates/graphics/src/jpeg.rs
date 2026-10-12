//! Bounded JPEG decoding into the shared opaque ARGB image representation.

use super::{Image, MAX_IMAGE_PIXELS, checked_pixels, error};
use diagnostics::{Category, EmuError};
use jpeg_decoder::{Decoder, PixelFormat};
use std::io::Cursor;

pub(super) fn decode_jpeg(bytes: &[u8]) -> Result<Image, EmuError> {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_max_decoding_buffer_size(MAX_IMAGE_PIXELS.saturating_mul(4));
    let decode_error = |source| {
        EmuError::new(
            Category::Api,
            "jpeg-decode",
            format!("invalid JPEG data: {source}"),
        )
    };
    decoder.read_info().map_err(decode_error)?;
    let info = decoder
        .info()
        .ok_or_else(|| error("jpeg-structure", "JPEG has no image metadata"))?;
    let width = u32::from(info.width);
    let height = u32::from(info.height);
    let pixel_count = checked_pixels(width, height)?;
    let decoded_bytes = decoder.decode().map_err(decode_error)?;
    let pixels = match info.pixel_format {
        PixelFormat::L8 => {
            if decoded_bytes.len() != pixel_count {
                return Err(error("jpeg-data", "JPEG luminance size mismatch"));
            }
            decoded_bytes
                .into_iter()
                .map(|gray| {
                    0xff00_0000 | u32::from(gray) << 16 | u32::from(gray) << 8 | u32::from(gray)
                })
                .collect()
        }
        PixelFormat::L16 => {
            if decoded_bytes.len() != pixel_count.saturating_mul(2) {
                return Err(error("jpeg-data", "JPEG luminance size mismatch"));
            }
            let precision = lossless_sample_precision(bytes)
                .filter(|precision| (9..=16).contains(precision))
                .ok_or_else(|| error("jpeg-data", "invalid JPEG sample precision"))?;
            decoded_bytes
                .chunks_exact(2)
                .map(|sample| {
                    // L16 describes storage, not precision: jpeg-decoder leaves
                    // 9..16-bit lossless samples unscaled in native byte order.
                    let sample = u16::from_ne_bytes([sample[0], sample[1]]) >> (precision - 8);
                    let gray = u32::from(u8::try_from(sample).map_err(|_| {
                        error("jpeg-data", "JPEG luminance exceeds sample precision")
                    })?);
                    Ok(0xff00_0000 | gray << 16 | gray << 8 | gray)
                })
                .collect::<Result<_, EmuError>>()?
        }
        PixelFormat::RGB24 => {
            if decoded_bytes.len() != pixel_count.saturating_mul(3) {
                return Err(error("jpeg-data", "JPEG RGB size mismatch"));
            }
            decoded_bytes
                .chunks_exact(3)
                .map(|rgb| {
                    0xff00_0000
                        | u32::from(rgb[0]) << 16
                        | u32::from(rgb[1]) << 8
                        | u32::from(rgb[2])
                })
                .collect()
        }
        PixelFormat::CMYK32 => {
            if decoded_bytes.len() != pixel_count.saturating_mul(4) {
                return Err(error("jpeg-data", "JPEG CMYK size mismatch"));
            }
            decoded_bytes
                .chunks_exact(4)
                .map(|cmyk| {
                    // The decoder already normalizes Adobe's inverted CMYK.
                    // Black scales the remaining light; adding ink amounts
                    // instead incorrectly clips mixed midtones to zero.
                    let light = 255 - u16::from(cmyk[3]);
                    let component = |value: u8| u32::from((255 - u16::from(value)) * light / 255);
                    0xff00_0000
                        | component(cmyk[0]) << 16
                        | component(cmyk[1]) << 8
                        | component(cmyk[2])
                })
                .collect()
        }
    };
    Ok(Image {
        width,
        height,
        pixels,
        mutable: false,
    })
}

// jpeg-decoder validates SOF3 but does not expose its precision in ImageInfo.
// Walk the length-delimited header segments (T.81 B.2.2), never marker-like
// bytes inside a comment/application payload or the entropy-coded scan.
fn lossless_sample_precision(mut bytes: &[u8]) -> Option<u8> {
    loop {
        let start = bytes.iter().position(|byte| *byte == 0xff)?;
        bytes = &bytes[start + 1..];
        let marker_start = bytes.iter().position(|byte| *byte != 0xff)?;
        let (&marker, remaining) = bytes[marker_start..].split_first()?;
        bytes = remaining;
        match marker {
            0x00 | 0x01 | 0xd0..=0xd8 => continue,
            0xd9 | 0xda => return None,
            _ => {}
        }
        let length = usize::from(u16::from_be_bytes(bytes.get(..2)?.try_into().ok()?));
        let segment = bytes.get(2..length)?;
        if marker == 0xc3 {
            return segment.first().copied();
        }
        bytes = bytes.get(length..)?;
    }
}

#[cfg(test)]
#[allow(clippy::trivially_copy_pass_by_ref, clippy::unreadable_literal)]
#[path = "../../../tests/unit/graphics/jpeg.rs"]
mod tests;
