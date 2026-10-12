//! Fuse only the side-effect-free miss path of a String array search. A hit,
//! unresolved access or malformed value remains at the original bytecode PC.
use super::{
    Allocation, ArrayKind, ClassState, Heap, HeapValue, Method, RuntimeInstructionMeta, Value,
    interpreter_batch::Opcode,
};

const MISS_INSTRUCTIONS: u64 = 12;

pub(super) fn link(code: &mut [RuntimeInstructionMeta]) {
    for head in 0..code.len().saturating_sub(9) {
        let Some(index) = int_local(&code[head]) else {
            continue;
        };
        let Opcode::LoadReference(_) = code[head + 4].batch_opcode else {
            continue;
        };
        let ops = [
            0xb2,
            0xbe,
            0xa2,
            code[head + 4].opcode,
            0xb2,
            code[head + 6].opcode,
            0x32,
            0xb6,
            0x99,
        ];
        if code[head + 1..head + 10]
            .iter()
            .zip(ops)
            .any(|(ins, op)| ins.opcode != op)
            || int_local(&code[head + 6]) != Some(index)
            || constant(&code[head + 1]) != constant(&code[head + 5])
        {
            continue;
        }
        let miss = usize::from(code[head + 9].branch_index);
        let Some(increment) = code.get(miss) else {
            continue;
        };
        let Some(backedge) = code.get(miss + 1) else {
            continue;
        };
        if increment.opcode == 0x84
            && increment.operands[..2] == [index, 1]
            && backedge.opcode == 0xa7
            && usize::from(backedge.branch_index) == head
        {
            code[head].batch_opcode = Opcode::StringScan(index);
        }
    }
}

fn int_local(ins: &RuntimeInstructionMeta) -> Option<u8> {
    match ins.opcode {
        0x15 => Some(ins.operands[0]),
        0x1a..=0x1d => Some(ins.opcode - 0x1a),
        _ => None,
    }
}

fn constant(ins: &RuntimeInstructionMeta) -> u16 {
    u16::from_be_bytes([ins.operands[0], ins.operands[1]])
}

pub(super) fn skip_misses(
    method: &Method,
    head: usize,
    index_local: u8,
    locals: &mut [Option<Value>],
    heap: &Heap,
    classes: &ClassState,
    strings: &super::StringValues,
    budget: u64,
) -> Option<(usize, u64)> {
    if budget < MISS_INSTRUCTIONS {
        return None;
    }
    let Value::Int(start) = locals.get(usize::from(index_local))?.as_ref()? else {
        return None;
    };
    let start = usize::try_from(*start).ok()?;
    let Opcode::LoadReference(query_local) =
        method.runtime_instructions.get(head + 4)?.batch_opcode
    else {
        return None;
    };
    let Value::Reference(Some(query)) = locals.get(usize::from(query_local))?.as_ref()? else {
        return None;
    };
    let code = &method.runtime_instructions;
    let cached = classes
        .constant_pool_cache(method)?
        .virtual_method_targets
        .get(constant(&code[head + 8]))?;
    if !cached.string_equals_ignore_case {
        return None;
    }
    let Allocation::Object { class, .. } = heap.get(*query).ok()? else {
        return None;
    };
    if class.as_ref() != cached.receiver_class {
        return None;
    }
    let query = strings.get(query)?;
    if query.len() > 256 {
        return None;
    }
    let access = classes.static_field_fast_access(method, constant(&code[head + 1]))?;
    if !classes
        .initialized
        .contains_at(access.slots.declaring_class?)
    {
        return None;
    }
    let Value::Reference(Some(array)) = classes
        .static_fields
        .get_linked(access.slots.static_field?)?
    else {
        return None;
    };
    let Allocation::Array {
        kind: ArrayKind::Reference(_),
        elements,
    } = heap.get(*array).ok()?
    else {
        return None;
    };
    let candidates = elements.get(start..)?;
    let mut skipped = 0_usize;
    for value in candidates
        .iter()
        .take(usize::try_from(budget / MISS_INSTRUCTIONS).ok()?)
    {
        let HeapValue::Reference(candidate) = value else {
            break;
        };
        if let Some(candidate) = candidate {
            let Some(units) = strings.get(candidate) else {
                break;
            };
            if units.len() > 256
                || super::compatibility_strings::utf16_equals_ignore_case(query, units)
            {
                break;
            }
        }
        skipped += 1;
    }
    if skipped == 0 {
        return None;
    }
    let next = i32::try_from(start.checked_add(skipped)?).ok()?;
    let miss = usize::from(code.get(head + 9)?.branch_index);
    let backedge_pc = code.get(miss)?.next_pc as usize;
    locals[usize::from(index_local)] = Some(Value::Int(next));
    Some((backedge_pc, skipped as u64 * MISS_INSTRUCTIONS))
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/string_array_scan/mod.rs"]
mod tests;
