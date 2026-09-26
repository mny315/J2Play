use super::*;

fn lookup_code(pc: usize, keys: &[i32]) -> Vec<u8> {
    let mut code = vec![0; pc];
    code.push(0xab);
    code.resize((pc + 4) & !3, 0);
    code.extend_from_slice(&100_i32.to_be_bytes());
    code.extend_from_slice(&i32::try_from(keys.len()).unwrap().to_be_bytes());
    for (index, key) in keys.iter().enumerate() {
        code.extend_from_slice(&key.to_be_bytes());
        code.extend_from_slice(&i32::try_from(index + 1).unwrap().to_be_bytes());
    }
    code
}

#[test]
fn lookup_selects_signed_keys_and_defaults_at_every_alignment() {
    let keys = [i32::MIN, -100, -1, 0, 1, 100, i32::MAX];
    for pc in 0..4 {
        let code = lookup_code(pc, &keys);
        bytecode::decode(&code).unwrap();
        for (index, key) in keys.into_iter().enumerate() {
            assert_eq!(switch_lookup(pc, &code, key).unwrap(), pc + index + 1);
        }
        for key in [i32::MIN + 1, -101, -99, 2, 99, 101, i32::MAX - 1] {
            assert_eq!(switch_lookup(pc, &code, key).unwrap(), pc + 100);
        }
        assert_eq!(
            switch_lookup(pc, &lookup_code(pc, &[]), 0).unwrap(),
            pc + 100
        );
    }
}

#[test]
fn lookup_bounds_its_table_before_searching() {
    let code = lookup_code(0, &[-1, 1]);
    for length in 0..code.len() {
        for key in [-1, 0, 1] {
            assert_eq!(
                switch_lookup(0, &code[..length], key).unwrap_err().code(),
                "invalid-switch"
            );
        }
    }
    for count in [-1_i32, i32::MAX] {
        let mut code = code.clone();
        code[8..12].copy_from_slice(&count.to_be_bytes());
        assert_eq!(
            switch_lookup(0, &code, 0).unwrap_err().code(),
            "invalid-switch"
        );
    }
}

#[test]
fn switch_read_rejects_out_of_range_offsets_without_overflow() {
    for offset in [1, 4, usize::MAX - 1, usize::MAX] {
        assert_eq!(
            read_i32(&[0; 4], offset).unwrap_err().code(),
            "invalid-switch"
        );
    }
}
