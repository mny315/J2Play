//! Bounded Java bytecode decoding and disassembly.

use diagnostics::{Category, EmuError};
use std::fmt::Write;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Instruction {
    pub offset: usize,
    pub opcode: u8,
    pub operands: Box<[u8]>,
}

impl Instruction {
    #[must_use]
    pub fn mnemonic(&self) -> &'static str {
        mnemonic(self.opcode).unwrap_or("<unknown>")
    }
}

/// Decodes one complete JVM bytecode array.
///
/// # Errors
///
/// Returns a class-loading diagnostic with bytecode offset for unknown or
/// truncated instructions and malformed switch/wide encodings.
pub fn decode(code: &[u8]) -> Result<Vec<Instruction>, EmuError> {
    let mut instructions = Vec::new();
    let mut offset = 0;
    while offset < code.len() {
        let start = offset;
        let opcode = code[offset];
        offset += 1;
        let mnemonic = mnemonic(opcode).ok_or_else(|| {
            error(
                "unknown-opcode",
                start,
                format!("unknown opcode 0x{opcode:02x}"),
            )
        })?;
        let operand_end = match opcode {
            0xaa => table_switch_end(code, start, offset)?,
            0xab => lookup_switch_end(code, start, offset)?,
            0xc4 => wide_end(code, start, offset)?,
            _ => offset
                .checked_add(fixed_operand_width(opcode))
                .ok_or_else(|| error("truncated-instruction", start, "operand length overflows"))?,
        };
        let operands = code.get(offset..operand_end).ok_or_else(|| {
            error(
                "truncated-instruction",
                start,
                format!("truncated {mnemonic} operands"),
            )
        })?;
        instructions.push(Instruction {
            offset: start,
            opcode,
            operands: operands.into(),
        });
        offset = operand_end;
    }
    Ok(instructions)
}

#[must_use]
pub fn disassemble(instructions: &[Instruction]) -> String {
    let mut output = String::new();
    for instruction in instructions {
        let _ = write!(
            output,
            "{:04}: {}",
            instruction.offset,
            instruction.mnemonic()
        );
        for operand in &instruction.operands {
            let _ = write!(output, " {operand:02x}");
        }
        output.push('\n');
    }
    output
}

fn fixed_operand_width(opcode: u8) -> usize {
    match opcode {
        0x10 | 0x12 | 0x15..=0x19 | 0x36..=0x3a | 0xa9 | 0xbc => 1,
        0x11
        | 0x13
        | 0x14
        | 0x84
        | 0x99..=0xa8
        | 0xb2..=0xb8
        | 0xbb
        | 0xbd
        | 0xc0
        | 0xc1
        | 0xc6
        | 0xc7 => 2,
        0xc5 => 3,
        0xb9 | 0xba | 0xc8 | 0xc9 => 4,
        _ => 0,
    }
}

fn table_switch_end(code: &[u8], start: usize, after_opcode: usize) -> Result<usize, EmuError> {
    let aligned = switch_aligned(code, start, after_opcode)?;
    let header_end = aligned
        .checked_add(12)
        .ok_or_else(|| error("invalid-tableswitch", start, "header length overflows"))?;
    let header = code.get(aligned..header_end).ok_or_else(|| {
        error(
            "truncated-instruction",
            start,
            "truncated tableswitch header",
        )
    })?;
    let low = i32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    let high = i32::from_be_bytes([header[8], header[9], header[10], header[11]]);
    if low > high {
        return Err(error(
            "invalid-tableswitch",
            start,
            format!("low value {low} exceeds high value {high}"),
        ));
    }
    let count = i64::from(high) - i64::from(low) + 1;
    let count = usize::try_from(count).map_err(|_| {
        error(
            "invalid-tableswitch",
            start,
            "table length is not representable",
        )
    })?;
    header_end
        .checked_add(
            count
                .checked_mul(4)
                .ok_or_else(|| error("invalid-tableswitch", start, "table length overflows"))?,
        )
        .filter(|end| *end <= code.len())
        .ok_or_else(|| {
            error(
                "truncated-instruction",
                start,
                "truncated tableswitch offsets",
            )
        })
}

fn lookup_switch_end(code: &[u8], start: usize, after_opcode: usize) -> Result<usize, EmuError> {
    let aligned = switch_aligned(code, start, after_opcode)?;
    let header_end = aligned
        .checked_add(8)
        .ok_or_else(|| error("invalid-lookupswitch", start, "header length overflows"))?;
    let header = code.get(aligned..header_end).ok_or_else(|| {
        error(
            "truncated-instruction",
            start,
            "truncated lookupswitch header",
        )
    })?;
    let pairs = i32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    let pairs = usize::try_from(pairs).map_err(|_| {
        error(
            "invalid-lookupswitch",
            start,
            "negative lookupswitch pair count",
        )
    })?;
    let end =
        header_end
            .checked_add(pairs.checked_mul(8).ok_or_else(|| {
                error("invalid-lookupswitch", start, "pair table length overflows")
            })?)
            .filter(|end| *end <= code.len())
            .ok_or_else(|| {
                error(
                    "truncated-instruction",
                    start,
                    "truncated lookupswitch pairs",
                )
            })?;
    let pair_bytes = &code[header_end..end];
    let mut previous = None;
    for pair in pair_bytes.chunks_exact(8) {
        let key = i32::from_be_bytes([pair[0], pair[1], pair[2], pair[3]]);
        if previous.is_some_and(|previous| key <= previous) {
            return Err(error(
                "invalid-lookupswitch",
                start,
                "match keys are not strictly increasing",
            ));
        }
        previous = Some(key);
    }
    Ok(end)
}

fn align_switch(after_opcode: usize) -> Result<usize, EmuError> {
    after_opcode
        .checked_add((4 - (after_opcode % 4)) % 4)
        .ok_or_else(|| {
            error(
                "truncated-instruction",
                after_opcode,
                "switch alignment overflows",
            )
        })
}

fn switch_aligned(code: &[u8], start: usize, after_opcode: usize) -> Result<usize, EmuError> {
    let aligned = align_switch(after_opcode)?;
    // Padding aligns the switch operands; its values have no execution meaning.
    // JVMS 7 §6.5 tableswitch/lookupswitch no longer require zero-filled padding.
    // Retain the extent check without rejecting otherwise readable older classes.
    code.get(after_opcode..aligned)
        .ok_or_else(|| error("truncated-instruction", start, "truncated switch padding"))?;
    Ok(aligned)
}

fn wide_end(code: &[u8], start: usize, after_opcode: usize) -> Result<usize, EmuError> {
    let modified = *code.get(after_opcode).ok_or_else(|| {
        error(
            "truncated-instruction",
            start,
            "wide has no modified opcode",
        )
    })?;
    let width = match modified {
        0x15..=0x19 | 0x36..=0x3a | 0xa9 => 3,
        0x84 => 5,
        _ => {
            return Err(error(
                "invalid-wide",
                start,
                format!("opcode 0x{modified:02x} cannot be widened"),
            ));
        }
    };
    after_opcode
        .checked_add(width)
        .filter(|end| *end <= code.len())
        .ok_or_else(|| error("truncated-instruction", start, "truncated wide instruction"))
}

fn error(code: &'static str, offset: usize, message: impl std::fmt::Display) -> EmuError {
    EmuError::new(
        Category::ClassLoading,
        code,
        format!("at bytecode offset {offset}: {message}"),
    )
}

#[allow(clippy::too_many_lines)]
fn mnemonic(opcode: u8) -> Option<&'static str> {
    const NAMES: [&str; 202] = [
        "nop",
        "aconst_null",
        "iconst_m1",
        "iconst_0",
        "iconst_1",
        "iconst_2",
        "iconst_3",
        "iconst_4",
        "iconst_5",
        "lconst_0",
        "lconst_1",
        "fconst_0",
        "fconst_1",
        "fconst_2",
        "dconst_0",
        "dconst_1",
        "bipush",
        "sipush",
        "ldc",
        "ldc_w",
        "ldc2_w",
        "iload",
        "lload",
        "fload",
        "dload",
        "aload",
        "iload_0",
        "iload_1",
        "iload_2",
        "iload_3",
        "lload_0",
        "lload_1",
        "lload_2",
        "lload_3",
        "fload_0",
        "fload_1",
        "fload_2",
        "fload_3",
        "dload_0",
        "dload_1",
        "dload_2",
        "dload_3",
        "aload_0",
        "aload_1",
        "aload_2",
        "aload_3",
        "iaload",
        "laload",
        "faload",
        "daload",
        "aaload",
        "baload",
        "caload",
        "saload",
        "istore",
        "lstore",
        "fstore",
        "dstore",
        "astore",
        "istore_0",
        "istore_1",
        "istore_2",
        "istore_3",
        "lstore_0",
        "lstore_1",
        "lstore_2",
        "lstore_3",
        "fstore_0",
        "fstore_1",
        "fstore_2",
        "fstore_3",
        "dstore_0",
        "dstore_1",
        "dstore_2",
        "dstore_3",
        "astore_0",
        "astore_1",
        "astore_2",
        "astore_3",
        "iastore",
        "lastore",
        "fastore",
        "dastore",
        "aastore",
        "bastore",
        "castore",
        "sastore",
        "pop",
        "pop2",
        "dup",
        "dup_x1",
        "dup_x2",
        "dup2",
        "dup2_x1",
        "dup2_x2",
        "swap",
        "iadd",
        "ladd",
        "fadd",
        "dadd",
        "isub",
        "lsub",
        "fsub",
        "dsub",
        "imul",
        "lmul",
        "fmul",
        "dmul",
        "idiv",
        "ldiv",
        "fdiv",
        "ddiv",
        "irem",
        "lrem",
        "frem",
        "drem",
        "ineg",
        "lneg",
        "fneg",
        "dneg",
        "ishl",
        "lshl",
        "ishr",
        "lshr",
        "iushr",
        "lushr",
        "iand",
        "land",
        "ior",
        "lor",
        "ixor",
        "lxor",
        "iinc",
        "i2l",
        "i2f",
        "i2d",
        "l2i",
        "l2f",
        "l2d",
        "f2i",
        "f2l",
        "f2d",
        "d2i",
        "d2l",
        "d2f",
        "i2b",
        "i2c",
        "i2s",
        "lcmp",
        "fcmpl",
        "fcmpg",
        "dcmpl",
        "dcmpg",
        "ifeq",
        "ifne",
        "iflt",
        "ifge",
        "ifgt",
        "ifle",
        "if_icmpeq",
        "if_icmpne",
        "if_icmplt",
        "if_icmpge",
        "if_icmpgt",
        "if_icmple",
        "if_acmpeq",
        "if_acmpne",
        "goto",
        "jsr",
        "ret",
        "tableswitch",
        "lookupswitch",
        "ireturn",
        "lreturn",
        "freturn",
        "dreturn",
        "areturn",
        "return",
        "getstatic",
        "putstatic",
        "getfield",
        "putfield",
        "invokevirtual",
        "invokespecial",
        "invokestatic",
        "invokeinterface",
        "invokedynamic",
        "new",
        "newarray",
        "anewarray",
        "arraylength",
        "athrow",
        "checkcast",
        "instanceof",
        "monitorenter",
        "monitorexit",
        "wide",
        "multianewarray",
        "ifnull",
        "ifnonnull",
        "goto_w",
        "jsr_w",
    ];
    NAMES.get(usize::from(opcode)).copied()
}

#[cfg(test)]
#[path = "../../../tests/unit/bytecode/mod.rs"]
mod tests;
