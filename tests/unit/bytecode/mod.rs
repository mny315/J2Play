use super::*;

fn table_switch(low: i32, high: i32, padding: [u8; 3]) -> Vec<u8> {
    let mut code = vec![0xaa];
    code.extend_from_slice(&padding);
    code.extend_from_slice(&0_i32.to_be_bytes());
    code.extend_from_slice(&low.to_be_bytes());
    code.extend_from_slice(&high.to_be_bytes());
    if low <= high {
        for _ in low..=high {
            code.extend_from_slice(&0_i32.to_be_bytes());
        }
    }
    code
}

fn lookup_switch(keys: &[i32], padding: [u8; 3]) -> Vec<u8> {
    let mut code = vec![0xab];
    code.extend_from_slice(&padding);
    code.extend_from_slice(&0_i32.to_be_bytes());
    code.extend_from_slice(&i32::try_from(keys.len()).unwrap().to_be_bytes());
    for key in keys {
        code.extend_from_slice(&key.to_be_bytes());
        code.extend_from_slice(&0_i32.to_be_bytes());
    }
    code
}

#[test]
fn decodes_fixed_and_wide_instructions() {
    let decoded = decode(&[0x10, 0x7f, 0x84, 0, 1, 0xc4, 0x84, 0, 2, 0, 3, 0xb1]).unwrap();
    assert_eq!(decoded.len(), 4);
    assert_eq!(decoded[2].mnemonic(), "wide");
    assert_eq!(decoded[3].offset, 11);
}

#[test]
fn decodes_tableswitch_alignment() {
    let code = [
        0xaa, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 4, 0, 0, 0, 8,
    ];
    assert_eq!(decode(&code).unwrap()[0].operands.len(), 23);
}

#[test]
fn rejects_malformed_variable_instructions() {
    assert_eq!(decode(&[0xc4, 0xb1]).unwrap_err().code(), "invalid-wide");
    assert_eq!(decode(&[0xaa]).unwrap_err().code(), "truncated-instruction");
    assert_eq!(
        decode(&[0xab, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff])
            .unwrap_err()
            .code(),
        "invalid-lookupswitch"
    );
}

#[test]
fn rejects_reversed_tableswitch_range() {
    assert_eq!(
        decode(&table_switch(2, 1, [0; 3])).unwrap_err().code(),
        "invalid-tableswitch"
    );
}

#[test]
fn switch_padding_values_do_not_change_instruction_boundaries_at_any_alignment() {
    for switch in [table_switch(1, 1, [0; 3]), lookup_switch(&[-1, 2], [0; 3])] {
        for prefix in 0..4 {
            for byte in [0, 1, 0xab, 0xff] {
                let mut code = vec![0; prefix];
                code.push(switch[0]);
                code.extend(std::iter::repeat_n(byte, 3 - prefix));
                code.extend_from_slice(&switch[4..]);
                let end = code.len();
                code.push(0xb1);
                let instructions = decode(&code).unwrap();
                assert_eq!(instructions.len(), prefix + 2);
                assert_eq!(instructions[prefix].offset, prefix);
                assert_eq!(instructions[prefix].opcode, switch[0]);
                assert_eq!(instructions.last().unwrap().offset, end);
                assert_eq!(instructions.last().unwrap().opcode, 0xb1);
                assert_eq!(
                    decode(&code[..end - 1]).unwrap_err().code(),
                    "truncated-instruction"
                );
            }
        }
    }
}

#[test]
fn lookupswitch_keys_must_be_strictly_increasing() {
    assert_eq!(
        decode(&lookup_switch(&[-1, 0, 2], [0; 3])).unwrap().len(),
        1
    );
    assert_eq!(
        decode(&lookup_switch(&[2, 1], [0; 3])).unwrap_err().code(),
        "invalid-lookupswitch"
    );
    assert_eq!(
        decode(&lookup_switch(&[1, 1], [0; 3])).unwrap_err().code(),
        "invalid-lookupswitch"
    );
}

#[test]
fn every_single_byte_input_is_controlled() {
    for byte in u8::MIN..=u8::MAX {
        let _ = decode(&[byte]);
    }
}
