use std::collections::{BTreeSet, HashMap, VecDeque};
use vm::acceleration::IntegerMethodSpec;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Op {
    pub opcode: u8,
    pub immediate: i32,
    pub target: usize,
}

#[derive(Debug)]
pub(crate) struct Block {
    pub start: usize,
    pub end: usize,
    pub depth: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct Verified {
    pub ops: Vec<Op>,
    pub blocks: Vec<Block>,
    pub block_at: HashMap<usize, usize>,
}

#[allow(clippy::too_many_lines)]
pub(crate) fn verify(spec: &IntegerMethodSpec) -> Option<Verified> {
    if spec.code.len() > 2_048
        || spec.max_locals > 32
        || spec.max_stack > 32
        || spec.parameter_slots.len() > 8
        || spec.integer_constants.len() > 512
        || spec.static_integer_parameters.len() > 8
    {
        return None;
    }
    let mut static_fields = BTreeSet::new();
    let mut static_slots = BTreeSet::new();
    for &(field, slot) in &spec.static_integer_parameters {
        if field == 0
            || !static_fields.insert(field)
            || !static_slots.insert(slot)
            || !spec.parameter_slots.contains(&slot)
        {
            return None;
        }
    }
    let instructions = bytecode::decode(&spec.code).ok()?;
    if instructions.is_empty() || instructions.len() > 512 {
        return None;
    }
    let offsets: HashMap<_, _> = instructions
        .iter()
        .enumerate()
        .map(|(index, instruction)| (instruction.offset, index))
        .collect();
    let mut ops = Vec::with_capacity(instructions.len());
    let mut leaders = BTreeSet::from([0]);
    for (index, instruction) in instructions.iter().enumerate() {
        let opcode = instruction.opcode;
        let operands = &instruction.operands;
        let mut op = Op {
            opcode,
            immediate: 0,
            target: 0,
        };
        op.immediate = match opcode {
            0x00
            | 0x57..=0x60
            | 0x64
            | 0x68
            | 0x6c
            | 0x70
            | 0x74
            | 0x78
            | 0x7a
            | 0x7c
            | 0x7e
            | 0x80
            | 0x82
            | 0x91..=0x93
            | 0xac => 0,
            0x02..=0x08 => i32::from(opcode) - 3,
            0x10 => i32::from(i8::from_be_bytes([*operands.first()?])),
            0x11 => i32::from(i16::from_be_bytes(operands.get(..2)?.try_into().ok()?)),
            0x12 | 0x13 => {
                let constant = if opcode == 0x12 {
                    u16::from(*operands.first()?)
                } else {
                    u16::from_be_bytes(operands.get(..2)?.try_into().ok()?)
                };
                spec.integer_constants
                    .iter()
                    .find(|(key, _)| *key == constant)?
                    .1
            }
            0x15 | 0x36 => i32::from(*operands.first()?),
            0xb2 => {
                let field = u16::from_be_bytes(operands.get(..2)?.try_into().ok()?);
                let slot = spec
                    .static_integer_parameters
                    .iter()
                    .find(|&&(index, _)| index == field)?
                    .1;
                op.opcode = 0x15;
                i32::from(slot)
            }
            0x1a..=0x1d => i32::from(opcode - 0x1a),
            0x3b..=0x3e => i32::from(opcode - 0x3b),
            0x84 => {
                op.target = usize::from(*operands.first()?);
                i32::from(i8::from_be_bytes([*operands.get(1)?]))
            }
            0x99..=0xa4 | 0xa7 => {
                let displacement = i16::from_be_bytes(operands.get(..2)?.try_into().ok()?);
                let target = instruction
                    .offset
                    .checked_add_signed(isize::from(displacement))?;
                op.target = *offsets.get(&target)?;
                leaders.insert(op.target);
                0
            }
            _ => return None,
        };
        // Snapshot arguments occupy reserved locals outside the guest's local
        // array. Guest loads/stores must not accidentally access those slots.
        let local = match opcode {
            0x15 | 0x36 | 0x1a..=0x1d | 0x3b..=0x3e => u8::try_from(op.immediate).ok(),
            0x84 => u8::try_from(op.target).ok(),
            _ => None,
        };
        if local.is_some_and(|slot| static_slots.contains(&slot)) {
            return None;
        }
        if matches!(opcode, 0x99..=0xa4 | 0xa7 | 0xac) && index + 1 < instructions.len() {
            leaders.insert(index + 1);
        }
        ops.push(op);
    }
    let mut assigned = 0_u32;
    for &slot in &spec.parameter_slots {
        if slot >= spec.max_locals || assigned & (1 << slot) != 0 {
            return None;
        }
        assigned |= 1 << slot;
    }
    let mut states = vec![None; ops.len()];
    states[0] = Some((0_usize, assigned));
    let mut pending = VecDeque::from([0]);
    let mut work = 0;
    let mut has_return = false;
    while let Some(index) = pending.pop_front() {
        work += 1;
        if work > 20_000 {
            return None;
        }
        let (mut depth, mut initialized) = states[index]?;
        let op = ops[index];
        let (required, produced) = match op.opcode {
            0x00 | 0xa7 => (0, 0),
            0x02..=0x08 | 0x10..=0x13 => (0, 1),
            0x15 | 0x1a..=0x1d => {
                let slot = u32::try_from(op.immediate).ok()?;
                if slot >= u32::from(spec.max_locals) || initialized & (1 << slot) == 0 {
                    return None;
                }
                (0, 1)
            }
            0x36 | 0x3b..=0x3e => {
                let slot = u32::try_from(op.immediate).ok()?;
                if slot >= u32::from(spec.max_locals) {
                    return None;
                }
                initialized |= 1 << slot;
                (1, 0)
            }
            0x84 => {
                if op.target >= usize::from(spec.max_locals) || initialized & (1 << op.target) == 0
                {
                    return None;
                }
                (0, 0)
            }
            0x57 | 0x99..=0x9e => (1, 0),
            0x58 | 0x9f..=0xa4 => (2, 0),
            0x59 => (1, 2),
            0x5a => (2, 3),
            0x5b => (3, 4),
            0x5c => (2, 4),
            0x5d => (3, 5),
            0x5e => (4, 6),
            0x5f => (2, 2),
            0x60 | 0x64 | 0x68 | 0x6c | 0x70 | 0x78 | 0x7a | 0x7c | 0x7e | 0x80 | 0x82 => (2, 1),
            0x74 | 0x91..=0x93 => (1, 1),
            0xac => {
                if depth != 1 {
                    return None;
                }
                has_return = true;
                (1, 0)
            }
            _ => return None,
        };
        if depth < required {
            return None;
        }
        depth = depth - required + produced;
        if depth > usize::from(spec.max_stack) {
            return None;
        }
        let successors = match op.opcode {
            0xac => [None, None],
            0xa7 => [Some(op.target), None],
            0x99..=0xa4 => [Some(op.target), Some(index + 1)],
            _ => [Some(index + 1), None],
        };
        for successor in successors.into_iter().flatten() {
            let state = states.get_mut(successor)?;
            match *state {
                None => {
                    *state = Some((depth, initialized));
                    pending.push_back(successor);
                }
                Some((old_depth, old_initialized)) => {
                    if old_depth != depth {
                        return None;
                    }
                    let intersection = initialized & old_initialized;
                    if intersection != old_initialized {
                        *state = Some((depth, intersection));
                        pending.push_back(successor);
                    }
                }
            }
        }
    }
    if !has_return {
        return None;
    }
    let leaders: Vec<_> = leaders.into_iter().collect();
    let blocks = leaders
        .iter()
        .enumerate()
        .map(|(index, &start)| Block {
            start,
            end: leaders.get(index + 1).copied().unwrap_or(ops.len()),
            depth: states[start].map(|(depth, _)| depth),
        })
        .collect();
    let block_at = leaders
        .into_iter()
        .enumerate()
        .map(|(index, start)| (start, index))
        .collect();
    Some(Verified {
        ops,
        blocks,
        block_at,
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/native-code/verify/mod.rs"]
mod tests;
