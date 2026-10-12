use super::EmuError;

fn maximal_utf16_suffix(
    units: &[u16],
    order_greater: bool,
    tick: &mut impl FnMut() -> Result<(), EmuError>,
) -> Result<(usize, usize), EmuError> {
    let mut left = 0usize;
    let mut right = 1usize;
    let mut offset = 0usize;
    let mut period = 1usize;
    while let Some(&candidate) = units.get(right + offset) {
        tick()?;
        let current = units[left + offset];
        if (candidate < current && !order_greater) || (candidate > current && order_greater) {
            right += offset + 1;
            offset = 0;
            period = right - left;
        } else if candidate == current {
            if offset + 1 == period {
                right += offset + 1;
                offset = 0;
            } else {
                offset += 1;
            }
        } else {
            left = right;
            right += 1;
            offset = 0;
            period = 1;
        }
    }
    Ok((left, period))
}
pub(super) fn two_way_utf16_index(
    haystack: &[u16],
    needle: &[u16],
    mut poll: impl FnMut() -> Result<(), EmuError>,
) -> Result<Option<usize>, EmuError> {
    if needle.is_empty() {
        return Ok(Some(0));
    }
    if needle.len() > haystack.len() {
        return Ok(None);
    }
    poll()?;
    // Count work in both preprocessing and matching, including long prefixes.
    let mut work = 0usize;
    let mut tick = || {
        work = work.wrapping_add(1);
        if work.is_multiple_of(1_024) {
            poll()?;
        }
        Ok(())
    };
    let first = maximal_utf16_suffix(needle, false, &mut tick)?;
    let second = maximal_utf16_suffix(needle, true, &mut tick)?;
    let (critical, candidate_period) = if first.0 > second.0 { first } else { second };
    let short_period = if let Some(prefix) = candidate_period
        .checked_add(critical)
        .and_then(|end| needle.get(candidate_period..end))
    {
        let mut matches = true;
        for (&left, &right) in prefix.iter().zip(&needle[..critical]) {
            tick()?;
            if left != right {
                matches = false;
                break;
            }
        }
        matches
    } else {
        false
    };
    let period = if short_period {
        candidate_period
    } else {
        critical.max(needle.len() - critical) + 1
    };
    let mut position = 0usize;
    let mut memory = 0usize;
    while position <= haystack.len() - needle.len() {
        tick()?;
        let right_start = if short_period {
            critical.max(memory)
        } else {
            critical
        };
        let mut index = right_start;
        while index < needle.len() && needle[index] == haystack[position + index] {
            tick()?;
            index += 1;
        }
        if index != needle.len() {
            position += index - critical + 1;
            memory = 0;
            continue;
        }

        let left_end = if short_period { memory } else { 0 };
        index = critical;
        while index > left_end && needle[index - 1] == haystack[position + index - 1] {
            tick()?;
            index -= 1;
        }
        if index <= left_end {
            return Ok(Some(position));
        }
        position += period;
        if short_period {
            memory = needle.len() - period;
        }
    }
    Ok(None)
}
