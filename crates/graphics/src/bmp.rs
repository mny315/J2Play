use super::{Category, EmuError, Image, checked_pixels, error};

pub(super) fn decode_bmp(bytes: &[u8]) -> Result<Image, EmuError> {
    if bytes.len() < 54 || !bytes.starts_with(b"BM") {
        return Err(error("bmp-structure", "truncated BMP header"));
    }
    let declared_size = bmp_u32(bytes, 2)? as usize;
    if declared_size != 0 && declared_size > bytes.len() {
        return Err(error("bmp-structure", "BMP file size exceeds input"));
    }
    let pixel_offset = bmp_u32(bytes, 10)? as usize;
    let dib_size = bmp_u32(bytes, 14)? as usize;
    if dib_size < 40 {
        return Err(error("bmp-format", "unsupported BMP DIB header"));
    }
    let dib_end = 14_usize
        .checked_add(dib_size)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| error("bmp-structure", "truncated BMP DIB header"))?;
    if pixel_offset < dib_end {
        return Err(error(
            "bmp-structure",
            "BMP pixel data overlaps the DIB header",
        ));
    }
    let width = bmp_i32(bytes, 18)?;
    let signed_height = bmp_i32(bytes, 22)?;
    if width <= 0 || signed_height == 0 || signed_height == i32::MIN {
        return Err(error("bmp-size", "invalid BMP dimensions"));
    }
    if bmp_u16(bytes, 26)? != 1 {
        return Err(error("bmp-format", "BMP must contain one color plane"));
    }
    let bits_per_pixel = bmp_u16(bytes, 28)?;
    if !matches!(bits_per_pixel, 1 | 4 | 8 | 16 | 24 | 32) {
        return Err(EmuError::new(
            Category::Api,
            "bmp-format",
            format!("unsupported BMP bit depth {bits_per_pixel}"),
        ));
    }
    let compression = bmp_u32(bytes, 30)?;
    if compression != 0 {
        return Err(EmuError::new(
            Category::Api,
            "bmp-format",
            format!("unsupported BMP compression {compression}"),
        ));
    }

    let width = width as u32;
    let height = signed_height.unsigned_abs();
    let pixel_count = checked_pixels(width, height)?;
    let palette = if bits_per_pixel <= 8 {
        let declared_colors = bmp_u32(bytes, 46)? as usize;
        let colors = if declared_colors == 0 {
            1_usize << bits_per_pixel
        } else {
            declared_colors
        };
        if colors > 1_usize << bits_per_pixel {
            return Err(error("bmp-palette", "invalid BMP palette size"));
        }
        let palette_end = dib_end
            .checked_add(
                colors
                    .checked_mul(4)
                    .ok_or_else(|| error("bmp-palette", "BMP palette size overflow"))?,
            )
            .filter(|end| *end <= pixel_offset && *end <= bytes.len())
            .ok_or_else(|| error("bmp-palette", "truncated BMP palette"))?;
        bytes[dib_end..palette_end].as_chunks::<4>().0
    } else {
        &[]
    };

    let row_bits = (width as usize)
        .checked_mul(usize::from(bits_per_pixel))
        .ok_or_else(|| error("bmp-data", "BMP row size overflow"))?;
    let row_bytes = row_bits
        .checked_add(31)
        .map(|bits| bits / 32 * 4)
        .ok_or_else(|| error("bmp-data", "BMP row size overflow"))?;
    let data_length = row_bytes
        .checked_mul(height as usize)
        .ok_or_else(|| error("bmp-data", "BMP pixel data size overflow"))?;
    let data_end = pixel_offset
        .checked_add(data_length)
        .filter(|end| *end <= bytes.len() && (declared_size == 0 || *end <= declared_size))
        .ok_or_else(|| error("bmp-data", "truncated BMP pixel data"))?;
    let data = &bytes[pixel_offset..data_end];
    let mut pixels = vec![0_u32; pixel_count];
    for encoded_y in 0..height as usize {
        let target_y = if signed_height < 0 {
            encoded_y
        } else {
            height as usize - 1 - encoded_y
        };
        let row = &data[encoded_y * row_bytes..(encoded_y + 1) * row_bytes];
        let start = target_y * width as usize;
        let output = &mut pixels[start..start + width as usize];
        for (x, pixel) in output.iter_mut().enumerate() {
            *pixel = match bits_per_pixel {
                1 => bmp_palette_pixel(palette, usize::from(row[x / 8] >> (7 - x % 8) & 1))?,
                4 => {
                    let byte = row[x / 2];
                    let index = if x.is_multiple_of(2) {
                        byte >> 4
                    } else {
                        byte & 0x0f
                    };
                    bmp_palette_pixel(palette, usize::from(index))?
                }
                8 => bmp_palette_pixel(palette, usize::from(row[x]))?,
                16 => {
                    let offset = x * 2;
                    let value = u16::from_le_bytes([row[offset], row[offset + 1]]);
                    let red = u32::from(value >> 10 & 0x1f) * 255 / 31;
                    let green = u32::from(value >> 5 & 0x1f) * 255 / 31;
                    let blue = u32::from(value & 0x1f) * 255 / 31;
                    0xff00_0000 | red << 16 | green << 8 | blue
                }
                24 | 32 => {
                    let offset = x * usize::from(bits_per_pixel / 8);
                    0xff00_0000
                        | u32::from(row[offset + 2]) << 16
                        | u32::from(row[offset + 1]) << 8
                        | u32::from(row[offset])
                }
                _ => unreachable!(),
            };
        }
    }
    Ok(Image {
        width,
        height,
        pixels,
        mutable: false,
    })
}

fn bmp_u16(bytes: &[u8], offset: usize) -> Result<u16, EmuError> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or_else(|| error("bmp-structure", "truncated BMP header"))
}

fn bmp_u32(bytes: &[u8], offset: usize) -> Result<u32, EmuError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| error("bmp-structure", "truncated BMP header"))
}

fn bmp_i32(bytes: &[u8], offset: usize) -> Result<i32, EmuError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(i32::from_le_bytes)
        .ok_or_else(|| error("bmp-structure", "truncated BMP header"))
}

fn bmp_palette_pixel(palette: &[[u8; 4]], index: usize) -> Result<u32, EmuError> {
    palette
        .get(index)
        .map(|entry| u32::from_le_bytes(*entry) | 0xff00_0000)
        .ok_or_else(|| error("bmp-palette", "BMP pixel index exceeds palette"))
}

#[cfg(test)]
#[allow(clippy::trivially_copy_pass_by_ref, clippy::unreadable_literal)]
#[path = "../../../tests/unit/graphics/bmp.rs"]
mod tests;
