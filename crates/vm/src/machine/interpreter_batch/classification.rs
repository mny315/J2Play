//! Batch opcode decoding and side-effect classification during method preparation.

use crate::machine::Instruction;

// A dense internal dispatch code keeps the hot jump table compact. The
// original JVM opcode remains available for operand families and diagnostics.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub(in crate::machine) enum Opcode {
    Nop,
    Null,
    IntConstant(i8),
    LongConstant(u8),
    FloatConstant(u8),
    DoubleConstant(u8),
    ShortConstant,
    Constant,
    LoadInt(u8),
    StringScan(u8),
    LocalIntBinary(u8),
    LoadLong(u8),
    LoadFloat(u8),
    LoadDouble(u8),
    LoadReference(u8),
    LoadArray,
    StoreInt(u8),
    StoreLong(u8),
    StoreFloat(u8),
    StoreDouble(u8),
    StoreReference(u8),
    StoreArray,
    Pop,
    Duplicate,
    TwoSlots,
    Swap,
    IntAdd,
    IntSubtract,
    IntMultiply,
    IntDivide,
    IntRemainder,
    IntShiftLeft,
    IntShiftRight,
    IntUnsignedShiftRight,
    IntAnd,
    IntOr,
    IntXor,
    LongBinary,
    LongShift,
    LongNegate,
    IntToLong,
    LongToInt,
    IntUnary,
    Increment,
    IfZero(u8),
    IfIntPair(u8),
    IfReferencePair(bool),
    IfNull(bool),
    Goto,
    StaticField,
    InstanceField,
    ArrayLength,
    VirtualCall,
    Return(u8),
    FloatingBinary,
    FloatingUnary,
    Fallback,
}

impl Opcode {
    pub(in crate::machine) fn followed_by(self, opcode: u8) -> Self {
        if let Self::LoadInt(local) = self
            && matches!(
                opcode,
                0x60 | 0x64 | 0x68 | 0x78 | 0x7a | 0x7c | 0x7e | 0x80 | 0x82
            )
        {
            Self::LocalIntBinary(local)
        } else {
            self
        }
    }

    pub(in crate::machine) fn decode(opcode: u8, operands: [u8; 5]) -> Self {
        match opcode {
            0x00 => Self::Nop,
            0x01 => Self::Null,
            0x02..=0x08 => Self::IntConstant(opcode as i8 - 3),
            0x09..=0x0a => Self::LongConstant(opcode - 0x09),
            0x0b..=0x0d => Self::FloatConstant(opcode - 0x0b),
            0x0e..=0x0f => Self::DoubleConstant(opcode - 0x0e),
            0x10 => Self::IntConstant(operands[0] as i8),
            0x11 => Self::ShortConstant,
            0x12..=0x14 => Self::Constant,
            0x15 => Self::LoadInt(operands[0]),
            0x1a..=0x1d => Self::LoadInt(opcode - 0x1a),
            0x16 => Self::LoadLong(operands[0]),
            0x1e..=0x21 => Self::LoadLong(opcode - 0x1e),
            0x17 => Self::LoadFloat(operands[0]),
            0x22..=0x25 => Self::LoadFloat(opcode - 0x22),
            0x18 => Self::LoadDouble(operands[0]),
            0x26..=0x29 => Self::LoadDouble(opcode - 0x26),
            0x19 => Self::LoadReference(operands[0]),
            0x2a..=0x2d => Self::LoadReference(opcode - 0x2a),
            0x2e..=0x35 => Self::LoadArray,
            0x36 => Self::StoreInt(operands[0]),
            0x3b..=0x3e => Self::StoreInt(opcode - 0x3b),
            0x37 => Self::StoreLong(operands[0]),
            0x3f..=0x42 => Self::StoreLong(opcode - 0x3f),
            0x38 => Self::StoreFloat(operands[0]),
            0x43..=0x46 => Self::StoreFloat(opcode - 0x43),
            0x39 => Self::StoreDouble(operands[0]),
            0x47..=0x4a => Self::StoreDouble(opcode - 0x47),
            0x3a => Self::StoreReference(operands[0]),
            0x4b..=0x4e => Self::StoreReference(opcode - 0x4b),
            0x4f..=0x52 | 0x54..=0x56 => Self::StoreArray,
            0x57 => Self::Pop,
            0x59 => Self::Duplicate,
            0x58 | 0x5c => Self::TwoSlots,
            0x5a | 0x5f => Self::Swap,
            0x60 => Self::IntAdd,
            0x64 => Self::IntSubtract,
            0x68 => Self::IntMultiply,
            0x6c => Self::IntDivide,
            0x70 => Self::IntRemainder,
            0x78 => Self::IntShiftLeft,
            0x7a => Self::IntShiftRight,
            0x7c => Self::IntUnsignedShiftRight,
            0x7e => Self::IntAnd,
            0x80 => Self::IntOr,
            0x82 => Self::IntXor,
            0x61 | 0x65 | 0x69 | 0x6d | 0x71 | 0x7f | 0x81 | 0x83 | 0x94 => Self::LongBinary,
            0x62..=0x63 | 0x66..=0x67 | 0x6a..=0x6b | 0x6e..=0x6f | 0x72..=0x73 | 0x95..=0x98 => {
                Self::FloatingBinary
            }
            0x76..=0x77 | 0x86..=0x87 | 0x89..=0x90 => Self::FloatingUnary,
            0x79 | 0x7b | 0x7d => Self::LongShift,
            0x75 => Self::LongNegate,
            0x85 => Self::IntToLong,
            0x88 => Self::LongToInt,
            0x74 | 0x91..=0x93 => Self::IntUnary,
            0x84 => Self::Increment,
            0x99..=0x9e => Self::IfZero(opcode),
            0x9f..=0xa4 => Self::IfIntPair(opcode),
            0xa5..=0xa6 => Self::IfReferencePair(opcode == 0xa5),
            0xa7 => Self::Goto,
            0xc6..=0xc7 => Self::IfNull(opcode == 0xc6),
            0xb2..=0xb3 => Self::StaticField,
            0xb4..=0xb5 => Self::InstanceField,
            0xbe => Self::ArrayLength,
            0xb6 => Self::VirtualCall,
            0xac..=0xb1 => Self::Return(opcode),
            _ => Self::Fallback,
        }
    }
}

pub(in crate::machine) fn is_readonly_leaf(instructions: &[Instruction]) -> bool {
    is_readonly(instructions, false)
}

pub(in crate::machine) fn is_readonly_call_tree(instructions: &[Instruction]) -> bool {
    instructions.iter().any(|ins| {
        ins.opcode == 0xb8
            || matches!(ins.opcode, 0x09..=0x0a | 0x14 | 0x85)
            || backward_branch(ins)
    }) && is_readonly(instructions, true)
}

fn is_readonly(instructions: &[Instruction], allow_calls: bool) -> bool {
    !instructions.is_empty()
        && instructions.len() <= 512
        && instructions
            .iter()
            .any(|ins| matches!(ins.opcode, 0xac..=0xb1))
        && instructions.iter().all(|ins| {
            // Direct leaf replay retains its static bound. Memo evaluation
            // also admits loops, with the existing one-quantum runtime budget.
            if !allow_calls && backward_branch(ins) {
                return false;
            }
            (allow_calls && ins.opcode == 0xb8)
                || matches!(ins.opcode,
            0x00..=0x35 | 0x36..=0x4e | 0x57..=0x5a | 0x5c | 0x5f |
            0x60..=0x61 | 0x64..=0x65 | 0x68..=0x69 | 0x6c..=0x6d |
            0x70..=0x71 | 0x74..=0x75 | 0x78..=0x85 | 0x88 | 0x91..=0x94 |
            0x99..=0xa7 | 0xac..=0xb1 | 0xb2 | 0xb4 | 0xbe | 0xc6..=0xc7)
        })
}

fn backward_branch(ins: &Instruction) -> bool {
    matches!(ins.opcode, 0x99..=0xa7 | 0xc6..=0xc7)
        && ins
            .operands
            .get(..2)
            .is_none_or(|bytes| i16::from_be_bytes([bytes[0], bytes[1]]) <= 0)
}
