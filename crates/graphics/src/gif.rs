use super::{EmuError, Image, checked_pixels, error};

struct GifReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> GifReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 6 }
    }

    fn byte(&mut self) -> Result<u8, EmuError> {
        let value = *self
            .bytes
            .get(self.position)
            .ok_or_else(|| error("gif-structure", "truncated GIF"))?;
        self.position += 1;
        Ok(value)
    }

    fn little_u16(&mut self) -> Result<u16, EmuError> {
        let low = self.byte()?;
        let high = self.byte()?;
        Ok(u16::from_le_bytes([low, high]))
    }

    fn slice(&mut self, length: usize) -> Result<&'a [u8], EmuError> {
        let end = self
            .position
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| error("gif-structure", "truncated GIF block"))?;
        let value = &self.bytes[self.position..end];
        self.position = end;
        Ok(value)
    }

    fn sub_blocks(&mut self) -> Result<Vec<u8>, EmuError> {
        let mut output = Vec::new();
        loop {
            let length = usize::from(self.byte()?);
            if length == 0 {
                return Ok(output);
            }
            output.extend_from_slice(self.slice(length)?);
        }
    }

    fn skip_sub_blocks(&mut self) -> Result<(), EmuError> {
        loop {
            let length = usize::from(self.byte()?);
            if length == 0 {
                return Ok(());
            }
            let _ = self.slice(length)?;
        }
    }
}

pub(super) fn decode_gif(bytes: &[u8]) -> Result<Image, EmuError> {
    let mut reader = GifReader::new(bytes);
    let screen_width = u32::from(reader.little_u16()?);
    let screen_height = u32::from(reader.little_u16()?);
    let screen_pixels = checked_pixels(screen_width, screen_height)?;
    let packed = reader.byte()?;
    let background_index = reader.byte()?;
    let _aspect_ratio = reader.byte()?;
    let global_palette = if packed & 0x80 != 0 {
        Some(read_gif_palette(
            &mut reader,
            1_usize << (usize::from(packed & 0x07) + 1),
        )?)
    } else {
        None
    };
    let mut transparent_index = None;

    loop {
        match reader.byte()? {
            0x21 => {
                let label = reader.byte()?;
                if label == 0xf9 {
                    let size = reader.byte()?;
                    if size != 4 {
                        return Err(error(
                            "gif-structure",
                            "invalid GIF graphic control extension",
                        ));
                    }
                    let control = reader.byte()?;
                    let _delay = reader.little_u16()?;
                    let index = reader.byte()?;
                    if reader.byte()? != 0 {
                        return Err(error(
                            "gif-structure",
                            "unterminated GIF graphic control extension",
                        ));
                    }
                    transparent_index = (control & 1 != 0).then_some(index);
                } else {
                    reader.skip_sub_blocks()?;
                }
            }
            0x2c => {
                let left = u32::from(reader.little_u16()?);
                let top = u32::from(reader.little_u16()?);
                let width = u32::from(reader.little_u16()?);
                let height = u32::from(reader.little_u16()?);
                let image_pixels = checked_pixels(width, height)?;
                if left
                    .checked_add(width)
                    .is_none_or(|right| right > screen_width)
                    || top
                        .checked_add(height)
                        .is_none_or(|bottom| bottom > screen_height)
                {
                    return Err(error("gif-structure", "GIF image exceeds logical screen"));
                }
                let image_packed = reader.byte()?;
                let local_palette = if image_packed & 0x80 != 0 {
                    Some(read_gif_palette(
                        &mut reader,
                        1_usize << (usize::from(image_packed & 0x07) + 1),
                    )?)
                } else {
                    None
                };
                let palette = local_palette
                    .as_ref()
                    .or(global_palette.as_ref())
                    .ok_or_else(|| error("gif-palette", "GIF image has no color table"))?;
                let minimum_code_size = reader.byte()?;
                let compressed = reader.sub_blocks()?;
                let indices = decode_gif_lzw(&compressed, minimum_code_size, image_pixels)?;
                let rows = gif_row_order(height, image_packed & 0x40 != 0);
                // Opaque GIFs use the logical screen's global background,
                // even when the image itself supplies a different local table.
                let background = if let (Some(palette), None) = (&global_palette, transparent_index)
                {
                    *palette.get(usize::from(background_index)).ok_or_else(|| {
                        error("gif-palette", "GIF background index exceeds color table")
                    })?
                } else {
                    0
                };
                let mut pixels = vec![background; screen_pixels];
                // Both rectangles and the decoded index count are validated
                // above, so each row fits within the logical screen.
                for (source, target_row) in indices.chunks_exact(width as usize).zip(rows) {
                    let start = ((top + target_row) * screen_width + left) as usize;
                    let target = &mut pixels[start..start + width as usize];
                    for (&palette_index, pixel) in source.iter().zip(target) {
                        *pixel = if transparent_index == Some(palette_index) {
                            0
                        } else {
                            *palette.get(usize::from(palette_index)).ok_or_else(|| {
                                error("gif-palette", "GIF pixel index exceeds color table")
                            })?
                        };
                    }
                }
                return Ok(Image {
                    width: screen_width,
                    height: screen_height,
                    pixels,
                    mutable: false,
                });
            }
            0x3b => return Err(error("gif-data", "GIF contains no image frame")),
            _ => return Err(error("gif-structure", "unknown GIF block marker")),
        }
    }
}

fn read_gif_palette(reader: &mut GifReader<'_>, entries: usize) -> Result<Vec<u32>, EmuError> {
    Ok(reader
        .slice(
            entries
                .checked_mul(3)
                .ok_or_else(|| error("gif-palette", "GIF color table size overflow"))?,
        )?
        .chunks_exact(3)
        .map(|rgb| {
            0xff00_0000 | u32::from(rgb[0]) << 16 | u32::from(rgb[1]) << 8 | u32::from(rgb[2])
        })
        .collect())
}

fn gif_row_order(height: u32, interlaced: bool) -> impl Iterator<Item = u32> {
    let passes: &[(u32, usize)] = if interlaced {
        &[(0, 8), (4, 8), (2, 4), (1, 2)]
    } else {
        &[(0, 1)]
    };
    passes
        .iter()
        .flat_map(move |&(start, step)| (start..height).step_by(step))
}

fn decode_gif_lzw(
    compressed: &[u8],
    minimum_code_size: u8,
    expected_pixels: usize,
) -> Result<Vec<u8>, EmuError> {
    if !(2..=8).contains(&minimum_code_size) {
        return Err(error("gif-lzw", "invalid GIF LZW minimum code size"));
    }
    let clear = 1_u16 << minimum_code_size;
    let end = clear + 1;
    let mut prefix = [0_u16; 4096];
    let mut suffix = [0_u8; 4096];
    for code in 0..clear {
        suffix[usize::from(code)] = code as u8;
    }
    let mut stack = [0_u8; 4096];
    let mut output = Vec::with_capacity(expected_pixels);
    let mut bit_position = 0_usize;
    let mut code_size = minimum_code_size + 1;
    let mut next_code = end + 1;
    let mut previous = None;
    let mut first = 0_u8;

    while output.len() < expected_pixels {
        let Some(mut code) = read_gif_code(compressed, &mut bit_position, code_size) else {
            break;
        };
        if code == clear {
            code_size = minimum_code_size + 1;
            next_code = end + 1;
            previous = None;
            continue;
        }
        if code == end {
            break;
        }
        if code >= 4096 || code > next_code {
            return Err(error("gif-lzw", "invalid GIF LZW code"));
        }
        let input_code = code;
        let mut stack_length = 0_usize;
        if code == next_code {
            let Some(previous_code) = previous else {
                return Err(error("gif-lzw", "invalid first GIF LZW code"));
            };
            stack[stack_length] = first;
            stack_length += 1;
            code = previous_code;
        }
        while code >= clear {
            if code >= next_code || stack_length >= stack.len() {
                return Err(error("gif-lzw", "invalid GIF LZW dictionary chain"));
            }
            stack[stack_length] = suffix[usize::from(code)];
            stack_length += 1;
            code = prefix[usize::from(code)];
        }
        first = suffix[usize::from(code)];
        stack[stack_length] = first;
        stack_length += 1;
        while stack_length != 0 && output.len() < expected_pixels {
            stack_length -= 1;
            output.push(stack[stack_length]);
        }
        if let Some(previous_code) = previous
            && next_code < 4096
        {
            prefix[usize::from(next_code)] = previous_code;
            suffix[usize::from(next_code)] = first;
            next_code += 1;
            if next_code == 1_u16 << code_size && code_size < 12 {
                code_size += 1;
            }
        }
        previous = Some(input_code);
    }
    if output.len() != expected_pixels {
        return Err(error("gif-data", "GIF pixel stream has an invalid length"));
    }
    Ok(output)
}

fn read_gif_code(bytes: &[u8], bit_position: &mut usize, size: u8) -> Option<u16> {
    let end = bit_position.checked_add(usize::from(size))?;
    if end > bytes.len().checked_mul(8)? {
        return None;
    }
    // LZW codes are at most 12 bits, so even an unaligned code fits in three bytes.
    let source = &bytes[*bit_position / 8..];
    let word = u32::from(source.first().copied().unwrap_or(0))
        | u32::from(source.get(1).copied().unwrap_or(0)) << 8
        | u32::from(source.get(2).copied().unwrap_or(0)) << 16;
    let value = (word >> (*bit_position % 8)) & ((1_u32 << size) - 1);
    *bit_position = end;
    Some(value as u16)
}

#[cfg(test)]
#[allow(clippy::trivially_copy_pass_by_ref, clippy::unreadable_literal)]
#[path = "../../../tests/unit/graphics/gif.rs"]
mod tests;
