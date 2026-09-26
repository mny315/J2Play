use super::*;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::hint::black_box;
use std::time::Instant;

#[test]
fn branch_targets_must_point_to_instruction_boundaries() {
    // goto is at byte 2. The only instruction boundaries are 0, 2 and 5;
    // the operands of bipush and goto must never become jump destinations.
    for (delta, expected) in [
        (i16::MIN, u16::MAX),
        (-3, u16::MAX),
        (-2, 0),
        (-1, u16::MAX),
        (0, 1),
        (1, u16::MAX),
        (2, u16::MAX),
        (3, 2),
        (4, u16::MAX),
        (i16::MAX, u16::MAX),
    ] {
        for opcode in (0x99..=0xa7).chain(0xc6..=0xc7) {
            let [high, low] = delta.to_be_bytes();
            let code = [0x10, 7, opcode, high, low, 0xb1];
            let instructions = decode(&code).unwrap();
            let index = build_instruction_index(code.len(), &instructions);
            let metadata = build_runtime_instructions(&instructions, &index).unwrap();
            assert_eq!(metadata[1].branch_index, expected, "{opcode:02x}, {delta}");
            assert_eq!(metadata[0].branch_index, u16::MAX);
            assert_eq!(metadata[2].branch_index, u16::MAX);
        }
    }
}

#[test]
fn branch_indices_keep_the_reserved_and_unrepresentable_sentinel() {
    // This exceeds the classfile method limit but exercises the metadata
    // builder's own representation boundary without executing guest code.
    for index in [65_534, 65_535, 65_536] {
        let mut code = vec![0x00; index];
        code.extend_from_slice(&[0xa7, 0, 0]);
        let instructions = decode(&code).unwrap();
        let instruction_index = build_instruction_index(code.len(), &instructions);
        let metadata = build_runtime_instructions(&instructions, &instruction_index).unwrap();
        assert_eq!(
            metadata[index].branch_index,
            u16::try_from(index).unwrap_or(u16::MAX)
        );
    }
}

#[test]
#[ignore = "manual release benchmark for VM instruction preparation"]
fn instruction_metadata_throughput() {
    for (label, block, repeats, iterations) in [
        ("small-branches", &[0xa7, 0, 3][..], 16, 65_536),
        ("medium-branches", &[0xa7, 0, 3][..], 1024, 1024),
        ("large-branches", &[0xa7, 0, 3][..], 8192, 128),
        ("no-branches", &[0x10, 7, 0x57][..], 512, 1024),
    ] {
        let mut code = block.repeat(repeats);
        code.push(0xb1);
        let instructions = decode(&code).unwrap();
        let instruction_index = build_instruction_index(code.len(), &instructions);
        let expected = build_runtime_instructions(&instructions, &instruction_index).unwrap();
        let started = Instant::now();
        for _ in 0..iterations {
            black_box(
                build_runtime_instructions(black_box(&instructions), black_box(&instruction_index))
                    .unwrap(),
            );
        }
        let elapsed = started.elapsed();
        let actual = build_runtime_instructions(&instructions, &instruction_index).unwrap();
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        let mut signature = DefaultHasher::new();
        format!("{actual:?}").hash(&mut signature);
        eprintln!(
            "instruction-metadata-{label} elapsed={elapsed:?} signature={:016x}",
            signature.finish()
        );
    }
}
