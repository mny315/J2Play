//! Structural bytecode recognition for bounded array-loop acceleration.

use super::{ArrayFillLoop, CountedArrayFill};
use crate::machine::{
    ArrayAccessKind, Constant, Instruction, RuntimeInstructionMeta, resolve_field,
};

pub(in crate::machine) fn link_counted_array_fills(
    instructions: &[Instruction],
    constants: &[Option<Constant>],
    runtime: &mut [RuntimeInstructionMeta],
    first_global_index: usize,
) -> Vec<CountedArrayFill> {
    let mut fills = Vec::new();
    for (start, meta) in runtime.iter_mut().enumerate().take(instructions.len()) {
        let Some(fill) = instructions
            .get(start..start + 12)
            .and_then(|window| recognize_counted_array_fill(window, constants))
            .or_else(|| {
                instructions
                    .get(start..start + 10)
                    .and_then(|window| recognize_length_array_fill(window, constants))
            })
            .or_else(|| {
                instructions
                    .get(start..start + 13)
                    .and_then(|window| recognize_reverse_array_copy(window, constants))
            })
            .or_else(|| {
                instructions
                    .get(start..start + 9)
                    .and_then(|window| recognize_limit_array_fill(window, constants))
            })
        else {
            continue;
        };
        let global_index = first_global_index.saturating_add(fills.len());
        let Ok(encoded) = u16::try_from(global_index.saturating_add(1)) else {
            break;
        };
        meta.counted_array_fill = encoded;
        meta.batch_opcode = crate::machine::interpreter_batch::Opcode::Fallback;
        fills.push(fill);
    }
    fills
}

fn recognize_reverse_array_copy(
    instructions: &[Instruction],
    constants: &[Option<Constant>],
) -> Option<CountedArrayFill> {
    let [
        counter,
        limit,
        exit,
        destination,
        destination_index,
        increment_destination,
        source,
        decrement_source,
        source_index,
        read,
        write,
        increment_counter,
        back,
    ] = instructions
    else {
        return None;
    };
    let counter_local = int_load_local(counter)?;
    let limit_local = int_load_local(limit)?;
    let index_local = int_load_local(destination_index)?;
    let source_index_local = int_load_local(source_index)?;
    let source_local = match source.opcode {
        0x19 => u16::from(*source.operands.first()?),
        0x2a..=0x2d => u16::from(source.opcode - 0x2a),
        _ => return None,
    };
    let locals = [
        counter_local,
        limit_local,
        index_local,
        source_index_local,
        source_local,
    ];
    if locals
        .iter()
        .enumerate()
        .any(|(index, local)| locals[..index].contains(local))
    {
        return None;
    }
    let increment = |instruction: &Instruction, local: u16, delta: u8| {
        instruction.opcode == 0x84
            && u8::try_from(local)
                .ok()
                .is_some_and(|local| instruction.operands.as_ref() == [local, delta])
    };
    if exit.opcode != 0xa2
        || destination.opcode != 0xb2
        || read.opcode != 0x2e
        || write.opcode != 0x4f
        || back.opcode != 0xa7
        || branch_target(back)? != counter.offset
        || !increment(increment_destination, index_local, 1)
        || !increment(decrement_source, source_index_local, 255)
        || !increment(increment_counter, counter_local, 1)
    {
        return None;
    }
    let field_index = u16::from_be_bytes(destination.operands.get(..2)?.try_into().ok()?);
    if resolve_field(constants, field_index).ok()?.descriptor != "[I" {
        return None;
    }
    Some(CountedArrayFill {
        field_index,
        index_local,
        kind: ArrayFillLoop::ReverseCopy {
            counter_local,
            limit_local,
            source_local,
            source_index_local,
        },
        start_pc: u32::try_from(counter.offset).ok()?,
        exit_pc: u32::try_from(branch_target(exit)?).ok()?,
        access: ArrayAccessKind::Int,
    })
}

fn recognize_counted_array_fill(
    instructions: &[Instruction],
    constants: &[Option<Constant>],
) -> Option<CountedArrayFill> {
    let [
        load_count,
        duplicate,
        one,
        subtract,
        store_count,
        exit_branch,
        load_array,
        load_index,
        increment_index,
        load_value,
        store,
        back_branch,
    ] = instructions
    else {
        return None;
    };
    let count_local = int_load_local(load_count)?;
    if duplicate.opcode != 0x59
        || one.opcode != 0x04
        || subtract.opcode != 0x64
        || int_store_local(store_count)? != count_local
        || exit_branch.opcode != 0x9b
        || load_array.opcode != 0xb2
        || increment_index.opcode != 0x84
        || back_branch.opcode != 0xa7
    {
        return None;
    }
    let index_local = int_load_local(load_index)?;
    let value_local = int_load_local(load_value)?;
    if count_local == index_local || count_local == value_local || index_local == value_local {
        return None;
    }
    let [incremented_local, increment]: [u8; 2] =
        increment_index.operands.get(..2)?.try_into().ok()?;
    if u16::from(incremented_local) != index_local || increment != 1 {
        return None;
    }
    let [field_high, field_low]: [u8; 2] = load_array.operands.get(..2)?.try_into().ok()?;
    let field_index = u16::from_be_bytes([field_high, field_low]);
    let field = resolve_field(constants, field_index).ok()?;
    let access = array_store_access(store.opcode, &field.descriptor)?;
    let start_pc = u32::try_from(load_count.offset).ok()?;
    if branch_target(back_branch)? != load_count.offset {
        return None;
    }
    let exit_offset = branch_target(exit_branch)?;
    Some(CountedArrayFill {
        field_index,
        index_local,
        kind: ArrayFillLoop::CountDown {
            count_local,
            value_local,
        },
        start_pc,
        exit_pc: u32::try_from(exit_offset).ok()?,
        access,
    })
}

fn recognize_length_array_fill(
    instructions: &[Instruction],
    constants: &[Option<Constant>],
) -> Option<CountedArrayFill> {
    let [
        load_index,
        length_array,
        length,
        exit,
        store_array,
        store_index,
        value,
        store,
        increment,
        back,
    ] = instructions
    else {
        return None;
    };
    let index_local = int_load_local(load_index)?;
    if length_array.opcode != 0xb2
        || length.opcode != 0xbe
        || exit.opcode != 0xa2
        || store_array.opcode != 0xb2
        || store_array.operands != length_array.operands
        || int_load_local(store_index)? != index_local
        || increment.opcode != 0x84
        || increment.operands.as_ref() != [u8::try_from(index_local).ok()?, 1]
        || back.opcode != 0xa7
        || branch_target(back)? != load_index.offset
    {
        return None;
    }
    let value = match value.opcode {
        0x02..=0x08 => i32::from(value.opcode) - 3,
        0x10 => i32::from(*value.operands.first()? as i8),
        0x11 => i32::from(i16::from_be_bytes(
            value.operands.get(..2)?.try_into().ok()?,
        )),
        _ => return None,
    };
    let field_index = u16::from_be_bytes(length_array.operands.get(..2)?.try_into().ok()?);
    let field = resolve_field(constants, field_index).ok()?;
    let access = array_store_access(store.opcode, &field.descriptor)?;
    Some(CountedArrayFill {
        field_index,
        index_local,
        kind: ArrayFillLoop::ToLength { value },
        start_pc: u32::try_from(load_index.offset).ok()?,
        exit_pc: u32::try_from(branch_target(exit)?).ok()?,
        access,
    })
}

fn recognize_limit_array_fill(
    instructions: &[Instruction],
    constants: &[Option<Constant>],
) -> Option<CountedArrayFill> {
    let [
        index,
        limit,
        exit,
        array,
        store_index,
        value,
        store,
        increment,
        back,
    ] = instructions
    else {
        return None;
    };
    let index_local = int_load_local(index)?;
    let limit_local = int_load_local(limit)?;
    let value_local = int_load_local(value)?;
    if index_local == limit_local
        || index_local == value_local
        || exit.opcode != 0xa2
        || array.opcode != 0xb2
        || int_load_local(store_index)? != index_local
        || increment.opcode != 0x84
        || increment.operands.as_ref() != [u8::try_from(index_local).ok()?, 1]
        || back.opcode != 0xa7
        || branch_target(back)? != index.offset
    {
        return None;
    }
    let field_index = u16::from_be_bytes(array.operands.get(..2)?.try_into().ok()?);
    let field = resolve_field(constants, field_index).ok()?;
    let access = array_store_access(store.opcode, &field.descriptor)?;
    Some(CountedArrayFill {
        field_index,
        index_local,
        access,
        kind: ArrayFillLoop::ToLimit {
            limit_local,
            value_local,
        },
        start_pc: u32::try_from(index.offset).ok()?,
        exit_pc: u32::try_from(branch_target(exit)?).ok()?,
    })
}

fn array_store_access(opcode: u8, descriptor: &str) -> Option<ArrayAccessKind> {
    Some(match (opcode, descriptor) {
        (0x4f, "[I") => ArrayAccessKind::Int,
        (0x54, "[Z" | "[B") => ArrayAccessKind::ByteOrBoolean,
        (0x55, "[C") => ArrayAccessKind::Char,
        (0x56, "[S") => ArrayAccessKind::Short,
        _ => return None,
    })
}

fn int_load_local(instruction: &Instruction) -> Option<u16> {
    match instruction.opcode {
        0x15 => instruction.operands.first().copied().map(u16::from),
        0x1a..=0x1d => Some(u16::from(instruction.opcode - 0x1a)),
        _ => None,
    }
}

fn int_store_local(instruction: &Instruction) -> Option<u16> {
    match instruction.opcode {
        0x36 => instruction.operands.first().copied().map(u16::from),
        0x3b..=0x3e => Some(u16::from(instruction.opcode - 0x3b)),
        _ => None,
    }
}

fn branch_target(instruction: &Instruction) -> Option<usize> {
    let [high, low]: [u8; 2] = instruction.operands.get(..2)?.try_into().ok()?;
    instruction
        .offset
        .checked_add_signed(isize::from(i16::from_be_bytes([high, low])))
}
