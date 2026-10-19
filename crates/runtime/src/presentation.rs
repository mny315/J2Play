use super::{Category, EmuError};

pub const LCDUI_SYSTEM_CHROME: u32 = 0xff10_1418;

// DeviceProfile Canvas validation permits at most this many pixels. This API
// also accepts raw dimensions, so enforce the bound before allocating storage.
const MAX_COMPOSED_FRAME_PIXELS: usize = 4 * 1024 * 1024;

/// Borrows a complete Canvas frame, or composes its surrounding system chrome.
/// Dimensions and pixel counts have the same bounds on both paths.
pub fn lcdui_frame_view<'a>(
    display: (u32, u32),
    frame: (u32, u32),
    pixels: &'a [u32],
    scratch: &'a mut Vec<u32>,
) -> Result<(&'a [u32], platform::LogicalRect), EmuError> {
    if display == frame {
        validate_lcdui_frame(display, frame, pixels.len())?;
        return Ok((
            pixels,
            platform::LogicalRect {
                x: 0,
                y: 0,
                width: frame.0,
                height: frame.1,
            },
        ));
    }
    let region = compose_unscaled_lcdui_frame(display, frame, pixels, scratch)?;
    Ok((scratch.as_slice(), region))
}

pub fn compose_unscaled_lcdui_frame(
    (display_width, display_height): (u32, u32),
    (frame_width, frame_height): (u32, u32),
    pixels: &[u32],
    output: &mut Vec<u32>,
) -> Result<platform::LogicalRect, EmuError> {
    let display_len = validate_lcdui_frame(
        (display_width, display_height),
        (frame_width, frame_height),
        pixels.len(),
    )?;
    output
        .try_reserve_exact(display_len.saturating_sub(output.len()))
        .map_err(|error| {
            EmuError::with_source(
                Category::Api,
                "framebuffer-allocation",
                "cannot allocate LCDUI framebuffer",
                error,
            )
        })?;
    let horizontal_offset = (display_width - frame_width) / 2;
    let vertical_offset = (display_height - frame_height) / 2;
    let offset_x = usize::try_from(horizontal_offset).unwrap_or(0);
    let offset_y = usize::try_from(vertical_offset).unwrap_or(0);
    let display_width_usize = usize::try_from(display_width).unwrap_or(usize::MAX);
    let frame_width_usize = usize::try_from(frame_width).unwrap_or(usize::MAX);
    // The validated frame fits inside the bounded display. Append each row
    // once, filling only its chrome rather than clearing pixels we overwrite.
    output.clear();
    output.resize(offset_y * display_width_usize, LCDUI_SYSTEM_CHROME);
    for source in pixels.chunks_exact(frame_width_usize) {
        let row_start = output.len();
        output.resize(row_start + offset_x, LCDUI_SYSTEM_CHROME);
        output.extend_from_slice(source);
        output.resize(row_start + display_width_usize, LCDUI_SYSTEM_CHROME);
    }
    output.resize(display_len, LCDUI_SYSTEM_CHROME);
    Ok(platform::LogicalRect {
        x: horizontal_offset,
        y: vertical_offset,
        width: frame_width,
        height: frame_height,
    })
}

fn validate_lcdui_frame(
    (display_width, display_height): (u32, u32),
    (frame_width, frame_height): (u32, u32),
    pixel_count: usize,
) -> Result<usize, EmuError> {
    if frame_width == 0 || frame_height == 0 || display_width == 0 || display_height == 0 {
        return Err(EmuError::new(
            Category::Api,
            "framebuffer-size",
            "LCDUI dimensions must be positive",
        ));
    }
    usize::try_from(frame_width)
        .unwrap_or(usize::MAX)
        .checked_mul(usize::try_from(frame_height).unwrap_or(usize::MAX))
        .filter(|length| *length == pixel_count)
        .ok_or_else(|| {
            EmuError::new(
                Category::Api,
                "framebuffer-size",
                "LCDUI frame does not match its dimensions",
            )
        })?;
    if frame_width > display_width || frame_height > display_height {
        return Err(EmuError::new(
            Category::Api,
            "framebuffer-size",
            "ordinary Canvas does not fit inside the physical display",
        ));
    }
    let display_len = usize::try_from(display_width)
        .unwrap_or(usize::MAX)
        .checked_mul(usize::try_from(display_height).unwrap_or(usize::MAX))
        .filter(|length| *length <= MAX_COMPOSED_FRAME_PIXELS)
        .ok_or_else(|| {
            EmuError::new(
                Category::Api,
                "framebuffer-size",
                "display exceeds framebuffer limits",
            )
        })?;
    Ok(display_len)
}
