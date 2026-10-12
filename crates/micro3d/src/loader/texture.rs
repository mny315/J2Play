use super::{
    Arc, Cursor, EmuError, LoaderLimits, bounded_file, checked_mul, loader_error,
    validate_retained_bytes,
};

/// Decoded 8-bit paletted BMP texture in straight-alpha ARGB form.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<[u32]>,
    pub for_model: bool,
    pub color_key: u32,
}

impl TextureData {
    pub fn parse(bytes: &[u8], for_model: bool, limits: LoaderLimits) -> Result<Self, EmuError> {
        bounded_file(bytes, limits)?;
        let mut cursor = Cursor::new(bytes);
        if cursor.take(2)? != b"BM" {
            return Err(loader_error(
                "texture-magic",
                "Texture resource is not a BMP",
            ));
        }
        let declared_size = usize::try_from(cursor.u32()?)
            .map_err(|_| loader_error("texture-size", "BMP size does not fit the host"))?;
        cursor.skip(4)?;
        let declared_pixel_offset = usize::try_from(cursor.u32()?).map_err(|_| {
            loader_error("texture-offset", "BMP pixel offset does not fit the host")
        })?;
        let dib_size = usize::try_from(cursor.u32()?)
            .map_err(|_| loader_error("texture-header", "BMP header size does not fit the host"))?;
        let palette_start = 14_usize
            .checked_add(dib_size)
            .filter(|start| dib_size >= 40 && declared_pixel_offset >= *start)
            .ok_or_else(|| loader_error("texture-header", "BMP header/size is invalid"))?;
        let width = cursor.i32()?;
        let height = cursor.i32()?;
        if width <= 0 || height == 0 || height == i32::MIN {
            return Err(loader_error(
                "texture-dimensions",
                "BMP dimensions are invalid",
            ));
        }
        if cursor.u16()? != 1 || cursor.u16()? != 8 || cursor.u32()? != 0 {
            return Err(loader_error(
                "texture-format",
                "Micro3D textures must be uncompressed 8-bit paletted BMP",
            ));
        }
        cursor.skip(12)?;
        let colors_used = usize::try_from(cursor.u32()?).map_err(|_| {
            loader_error("texture-palette", "BMP palette count does not fit the host")
        })?;
        cursor.skip(4)?;
        let width = u32::try_from(width)
            .map_err(|_| loader_error("texture-width", "negative BMP width"))?;
        let top_down = height < 0;
        let height = height.unsigned_abs();
        let pixel_count = usize::try_from(width)
            .ok()
            .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
            .ok_or_else(|| loader_error("texture-overflow", "BMP pixel count overflow"))?;
        if pixel_count == 0 || pixel_count > limits.texture_pixels {
            return Err(loader_error(
                "texture-budget",
                "BMP exceeds the texture pixel budget",
            ));
        }
        validate_retained_bytes(
            checked_mul(pixel_count, size_of::<u32>(), "Texture decoded pixels")?,
            limits,
            "Texture",
        )?;
        let row_stride = usize::try_from(width)
            .ok()
            .and_then(|w| w.checked_add(3))
            .map(|w| w & !3)
            .ok_or_else(|| loader_error("texture-overflow", "BMP row stride overflow"))?;
        let image_bytes = row_stride
            .checked_mul(usize::try_from(height).unwrap_or(usize::MAX))
            .ok_or_else(|| loader_error("texture-overflow", "BMP image byte count overflow"))?;
        // Some shipped Micro3D games synthesize an indexed BMP in memory but
        // leave bfOffBits at the 54-byte header boundary. The handset engine
        // still consumes the declared palette before the pixels. Accept only
        // that unambiguous, fully bounded legacy layout; normal BMPs continue
        // to honor their declared pixel offset.
        let unpaletted_size = palette_start.checked_add(image_bytes);
        let appended_palette_offset = (declared_pixel_offset == palette_start
            && (1..=256).contains(&colors_used)
            && Some(declared_size) == unpaletted_size)
            .then(|| {
                palette_start
                    .checked_add(colors_used.checked_mul(4)?)?
                    .checked_add(image_bytes)
                    .filter(|end| *end <= bytes.len())
                    .map(|_| palette_start + colors_used * 4)
            })
            .flatten();
        let pixel_offset = appended_palette_offset.unwrap_or(declared_pixel_offset);
        let available_palette = pixel_offset
            .checked_sub(palette_start)
            .ok_or_else(|| loader_error("texture-palette", "BMP palette overlaps its header"))?
            / 4;
        let palette_count = if colors_used == 0 {
            available_palette.min(256)
        } else {
            colors_used
        };
        if palette_count == 0 || palette_count > 256 || palette_count > available_palette {
            return Err(loader_error(
                "texture-palette",
                format!(
                    "BMP palette is invalid: colors_used={colors_used} palette_count={palette_count} available={available_palette} pixel_offset={pixel_offset} dib_size={dib_size} declared_size={declared_size} bytes={} dimensions={width}x{height} image_bytes={image_bytes}",
                    bytes.len(),
                ),
            ));
        }
        let mut palette = Vec::with_capacity(palette_count);
        for index in 0..palette_count {
            let start = palette_start + index * 4;
            let entry = bytes
                .get(start..start + 4)
                .ok_or_else(|| loader_error("texture-truncated", "BMP palette is truncated"))?;
            palette.push(
                0xff00_0000
                    | (u32::from(entry[2]) << 16)
                    | (u32::from(entry[1]) << 8)
                    | u32::from(entry[0]),
            );
        }
        let color_key = palette[0] & 0x00ff_ffff;
        let end = pixel_offset
            .checked_add(image_bytes)
            .ok_or_else(|| loader_error("texture-overflow", "BMP image end overflow"))?;
        if end > bytes.len() || (appended_palette_offset.is_none() && end > declared_size) {
            return Err(loader_error(
                "texture-truncated",
                "BMP pixel data is truncated",
            ));
        }
        let mut pixels = vec![0_u32; pixel_count];
        let width_usize = usize::try_from(width).unwrap_or(0);
        let height_usize = usize::try_from(height).unwrap_or(0);
        for output_row in 0..height_usize {
            let source_row = if top_down {
                output_row
            } else {
                height_usize - 1 - output_row
            };
            let start = pixel_offset + source_row * row_stride;
            for column in 0..width_usize {
                let index = usize::from(bytes[start + column]);
                pixels[output_row * width_usize + column] =
                    *palette.get(index).ok_or_else(|| {
                        loader_error(
                            "texture-index",
                            "BMP pixel references a missing palette entry",
                        )
                    })?;
            }
        }
        Ok(Self {
            width,
            height,
            pixels: pixels.into(),
            for_model,
            color_key,
        })
    }

    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        self.pixels.len().saturating_mul(size_of::<u32>())
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/micro3d/loader/texture.rs"]
mod tests;
