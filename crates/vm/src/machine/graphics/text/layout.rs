use super::super::{EmuError, check_raster_cancellation, vm_error};
use std::borrow::Cow;
use unicode_bidi::{BidiClass, bidi_class, utf16::ParagraphBidiInfo};

mod data;

// Bound the scratch allocations and uninterrupted Unicode algorithm work.
// Plain LTR text borrows its original payload and has no scratch allocation.
pub(super) const MAX_BIDI_UNITS: usize = 16_384;

pub(super) fn is_nonspacing(unit: u16) -> bool {
    if unit < 0x0300 {
        return false;
    }
    let index = data::NONSPACING.partition_point(|&(_, end)| end < unit);
    data::NONSPACING
        .get(index)
        .is_some_and(|&(start, _)| start <= unit)
}

pub(super) const fn is_invisible(unit: u16) -> bool {
    matches!(unit, 0x00ad | 0x034f | 0x061c | 0x180e | 0x200b..=0x200f
        | 0x202a..=0x202e | 0x2060..=0x206f | 0xfeff)
}

fn class(unit: u16) -> BidiClass {
    char::from_u32(u32::from(unit)).map_or(BidiClass::L, bidi_class)
}

pub(super) fn needs_bidi(text: &[u16], index: usize) -> bool {
    let unit = text[index];
    // Skip common LTR blocks without a Unicode property lookup. The BMP RTL
    // scripts live in these ranges; decode supplementary scripts as scalars.
    let value = if (0xd800..=0xdbff).contains(&unit) {
        let Some(&low) = text
            .get(index + 1)
            .filter(|&&low| (0xdc00..=0xdfff).contains(&low))
        else {
            return false;
        };
        char::from_u32(0x1_0000 + ((u32::from(unit) - 0xd800) << 10) + u32::from(low) - 0xdc00)
            .map_or(BidiClass::L, bidi_class)
    } else if matches!(unit, 0x0590..=0x08ff | 0x200e..=0x206f | 0xfb1d..=0xfdff | 0xfe70..=0xfeff)
    {
        class(unit)
    } else {
        return false;
    };
    matches!(
        value,
        BidiClass::R
            | BidiClass::AL
            | BidiClass::RLE
            | BidiClass::RLO
            | BidiClass::LRE
            | BidiClass::LRO
            | BidiClass::PDF
            | BidiClass::RLI
            | BidiClass::LRI
            | BidiClass::FSI
            | BidiClass::PDI
    )
}

fn joining_type(unit: u16) -> u8 {
    let index = data::JOINING.partition_point(|&(_, end, _)| end < unit);
    data::JOINING
        .get(index)
        .filter(|&&(start, _, _)| start <= unit)
        .map_or(0, |&(_, _, value)| value)
}

fn arabic_forms(text: &[u16], host: &dyn natives::HostServices) -> Result<Vec<u16>, EmuError> {
    let mut connections = vec![0_u8; text.len()];
    let mut previous = None;
    for (index, &unit) in text.iter().enumerate() {
        check_raster_cancellation(host, index)?;
        let joining = joining_type(unit);
        if joining == 4 {
            continue;
        }
        if let Some((previous_index, previous_joining)) = previous
            && previous_joining & 1 != 0
            && joining & 2 != 0
        {
            connections[previous_index] |= 2;
            connections[index] |= 1;
        }
        previous = Some((index, joining));
    }
    text.iter()
        .zip(connections)
        .enumerate()
        .map(|(index, (&unit, connections))| {
            check_raster_cancellation(host, index)?;
            Ok(data::ARABIC_FORMS
                .binary_search_by_key(&unit, |&(base, _)| base)
                .map_or(unit, |index| {
                    data::ARABIC_FORMS[index].1[usize::from(connections)]
                }))
        })
        .collect()
}

fn mirror(unit: u16) -> u16 {
    data::MIRRORS
        .binary_search_by_key(&unit, |&(source, _)| source)
        .map_or(unit, |index| data::MIRRORS[index].1)
}

pub(super) fn visual_text<'a>(
    text: &'a [u16],
    host: &dyn natives::HostServices,
) -> Result<Cow<'a, [u16]>, EmuError> {
    let mut bidi_required = false;
    for (chunk_index, chunk) in text.chunks(1_024).enumerate() {
        check_raster_cancellation(host, chunk_index * 1_024)?;
        for offset in 0..chunk.len() {
            if needs_bidi(text, chunk_index * 1_024 + offset) {
                bidi_required = true;
                break;
            }
        }
        if bidi_required {
            break;
        }
    }
    visual_text_with_hint(text, host, bidi_required)
}

pub(super) fn visual_text_with_hint<'a>(
    text: &'a [u16],
    host: &dyn natives::HostServices,
    bidi_required: bool,
) -> Result<Cow<'a, [u16]>, EmuError> {
    if !bidi_required {
        return Ok(Cow::Borrowed(text));
    }
    if text.len() > MAX_BIDI_UNITS {
        return Err(vm_error(
            "graphics-text-limit",
            "bidirectional text exceeds the layout limit",
        ));
    }
    let mut output = Vec::with_capacity(text.len());
    // Resolve independent paragraphs individually, avoiding a whole-text
    // level-vector clone for each separator in an untrusted string.
    for paragraph in text.split_inclusive(|&unit| class(unit) == BidiClass::B) {
        check_raster_cancellation(host, 0)?;
        let (body, separator) = if paragraph
            .last()
            .is_some_and(|&unit| class(unit) == BidiClass::B)
        {
            paragraph.split_at(paragraph.len() - 1)
        } else {
            (paragraph, &[][..])
        };
        if body.is_empty() {
            output.extend_from_slice(separator);
            continue;
        }
        let bidi = ParagraphBidiInfo::new(body, None);
        let levels = bidi.reordered_levels(0..body.len());
        let shaped = arabic_forms(body, host)?;
        let mut clusters: Vec<std::ops::Range<usize>> = Vec::new();
        for (index, &unit) in body.iter().enumerate() {
            check_raster_cancellation(host, index)?;
            let low_surrogate = (0xdc00..=0xdfff).contains(&unit)
                && index > 0
                && (0xd800..=0xdbff).contains(&body[index - 1]);
            if (is_nonspacing(unit) || low_surrogate)
                && let Some(cluster) = clusters.last_mut()
            {
                cluster.end = index + 1;
            } else {
                clusters.push(index..index + 1);
            }
        }
        let cluster_levels = clusters
            .iter()
            .map(|cluster| levels[cluster.start])
            .collect::<Vec<_>>();
        let order = ParagraphBidiInfo::reorder_visual(&cluster_levels);
        for (work, index) in order.into_iter().enumerate() {
            check_raster_cancellation(host, work)?;
            for offset in clusters[index].clone() {
                let unit = shaped[offset];
                output.push(if levels[offset].is_rtl() {
                    mirror(unit)
                } else {
                    unit
                });
            }
        }
        output.extend_from_slice(separator);
    }
    Ok(Cow::Owned(output))
}

#[cfg(test)]
#[path = "../../../../../../tests/unit/vm/machine/native_graphics/text/layout.rs"]
mod tests;
