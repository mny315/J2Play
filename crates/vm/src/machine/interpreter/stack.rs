//! JVM stack permutations over indivisible one- and two-slot values.

use super::super::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Underflow,
    TypeMismatch,
    Overflow,
    UnsupportedOpcode,
}

/// Checks the complete permutation before changing the stack or allocating.
pub(super) fn rearrange(
    opcode: u8,
    stack: &mut Vec<Value>,
    slots: usize,
    limit: usize,
) -> Result<usize, Error> {
    let (top_slots, below_slots) = match opcode {
        0x57 | 0x59 => (1, 0), // pop, dup
        0x58 | 0x5c => (2, 0), // pop2, dup2
        0x5a | 0x5f => (1, 1), // dup_x1, swap
        0x5b => (1, 2),        // dup_x2
        0x5d => (2, 1),        // dup2_x1
        0x5e => (2, 2),        // dup2_x2
        _ => return Err(Error::UnsupportedOpcode),
    };
    // An insertion group requires at least two values. Report a missing
    // operand before checking the categories of those values.
    if below_slots != 0 && stack.len() < 2 {
        return Err(Error::Underflow);
    }
    let end = stack.len();
    let top = group_start(stack, end, top_slots)?;
    if matches!(opcode, 0x57 | 0x58) {
        stack.truncate(top);
        return Ok(slots - top_slots);
    }
    let below = group_start(stack, top, below_slots)?;
    let duplicated_slots = if opcode == 0x5f { 0 } else { top_slots };
    let next_slots = slots
        .checked_add(duplicated_slots)
        .filter(|slots| *slots <= limit)
        .ok_or(Error::Overflow)?;
    if duplicated_slots != 0 {
        stack.extend_from_within(top..end);
    }
    // [below, top] becomes [top, below], leaving any appended copy at the end.
    if below != top {
        stack[below..end].rotate_right(end - top);
    }
    Ok(next_slots)
}

fn group_start(stack: &[Value], mut end: usize, mut slots: usize) -> Result<usize, Error> {
    while slots != 0 {
        end = end.checked_sub(1).ok_or(Error::Underflow)?;
        slots = slots
            .checked_sub(stack[end].slots())
            .ok_or(Error::TypeMismatch)?;
    }
    Ok(end)
}

#[cfg(test)]
#[path = "../../../../../tests/unit/vm/machine/interpreter/stack.rs"]
mod tests;
