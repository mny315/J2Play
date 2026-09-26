use super::{EmuError, vm_error};

pub(super) fn nokia_direct_transform(manipulation: i32) -> Option<i32> {
    const FLIP_HORIZONTAL: i32 = 0x2000;
    const FLIP_VERTICAL: i32 = 0x4000;
    const FLIP_MASK: i32 = FLIP_HORIZONTAL | FLIP_VERTICAL;
    let rotation = match manipulation & !FLIP_MASK {
        0 => 0,
        90 => 1,
        180 => 2,
        270 => 3,
        _ => return None,
    };
    let flips = usize::from(manipulation & FLIP_HORIZONTAL != 0)
        | (usize::from(manipulation & FLIP_VERTICAL != 0) << 1);
    // Nokia rotates counter-clockwise first, then flips the rotated result.
    // Values are MIDP Sprite transforms consumed by graphics_draw_region.
    Some([[0, 2, 1, 3], [6, 7, 4, 5], [3, 1, 2, 0], [5, 4, 7, 6]][rotation][flips])
}

pub(super) fn graphics_clip_rectangle(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    translate_x: i32,
    translate_y: i32,
    target_width: i32,
    target_height: i32,
) -> [i32; 4] {
    fn axis(origin: i32, extent: i32, translation: i32, limit: i32) -> (i32, i32) {
        let limit = i64::from(limit.max(0));
        let start = i64::from(origin) + i64::from(translation);
        let clipped_start = start.clamp(0, limit);
        if extent <= 0 {
            return (clipped_start as i32, 0);
        }
        let clipped_end = (start + i64::from(extent)).clamp(0, limit);
        (
            clipped_start as i32,
            (clipped_end - clipped_start).max(0) as i32,
        )
    }

    let (clip_x, clip_w) = axis(x, width, translate_x, target_width);
    let (clip_y, clip_h) = axis(y, height, translate_y, target_height);
    [clip_x, clip_y, clip_w, clip_h]
}

pub(super) fn image_axis_origin(
    position: i32,
    extent: i32,
    anchor: i32,
    horizontal: bool,
) -> Result<i32, EmuError> {
    let axis_mask = if horizontal { 0x0d } else { 0x32 };
    let axis_anchor = anchor & axis_mask;
    if (axis_anchor != 0 && axis_anchor & (axis_anchor - 1) != 0) || anchor & !0x3f != 0 {
        return Err(vm_error(
            "illegal-argument",
            "invalid Graphics image anchor",
        ));
    }
    let offset = if (horizontal && anchor & 0x08 != 0) || (!horizontal && anchor & 0x20 != 0) {
        extent
    } else if (horizontal && anchor & 0x01 != 0) || (!horizontal && anchor & 0x02 != 0) {
        extent / 2
    } else {
        0
    };
    Ok(position.saturating_add(offset.wrapping_neg()))
}

pub(super) fn image_origin(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    anchor: i32,
) -> Result<(i32, i32), EmuError> {
    Ok((
        image_axis_origin(x, width, anchor, true)?,
        image_axis_origin(y, height, anchor, false)?,
    ))
}

pub(super) fn unicode_text_origin(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    anchor: i32,
) -> Result<(i32, i32), EmuError> {
    const HCENTER: i32 = 1;
    const LEFT: i32 = 4;
    const RIGHT: i32 = 8;
    const TOP: i32 = 16;
    const BOTTOM: i32 = 32;
    const BASELINE: i32 = 64;
    const ALLOWED: i32 = HCENTER | LEFT | RIGHT | TOP | BOTTOM | BASELINE;
    let horizontal = anchor & (HCENTER | LEFT | RIGHT);
    let vertical = anchor & (TOP | BOTTOM | BASELINE);
    if anchor & !ALLOWED != 0
        || (horizontal != 0 && horizontal & (horizontal - 1) != 0)
        || (vertical != 0 && vertical & (vertical - 1) != 0)
    {
        return Err(vm_error("illegal-argument", "invalid Graphics text anchor"));
    }
    let origin_x = if anchor & RIGHT != 0 {
        x.wrapping_sub(width)
    } else if anchor & HCENTER != 0 {
        x.wrapping_sub(width / 2)
    } else {
        x
    };
    let origin_y = if anchor & BOTTOM != 0 {
        y.wrapping_sub(height)
    } else if anchor & BASELINE != 0 {
        y.wrapping_sub(height - 1)
    } else {
        y
    };
    Ok((origin_x, origin_y))
}
