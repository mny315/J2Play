use super::*;

fn fixture(replaced: usize, encoded: &[u8], references: bool) -> Vec<u8> {
    let mut pool = Vec::new();
    for index in 1..=14 {
        let text: Option<&[u8]> = match index {
            1 => Some(b"Fixture"),
            3 => Some(b"java/lang/Object"),
            5 => Some(b"value"),
            6 => Some(b"I"),
            7 => Some(b"run"),
            8 => Some(b"()V"),
            13 => Some(b"\xed\xa0\x80x\xed\xb0\x80"),
            _ => None,
        };
        if let Some(text) = text {
            let text = if index == replaced { encoded } else { text };
            pool.push(1);
            pool.extend_from_slice(&u16::try_from(text.len()).unwrap().to_be_bytes());
            pool.extend_from_slice(text);
        } else {
            pool.extend_from_slice(match index {
                2 => &[7, 0, 1],
                4 => &[7, 0, 3],
                9 if references => &[12, 0, 5, 0, 6],
                10 if references => &[9, 0, 2, 0, 9],
                11 if references => &[12, 0, 7, 0, 8],
                12 if references => &[10, 0, 2, 0, 11],
                14 => &[8, 0, 13],
                _ => &[3, 0, 0, 0, 0],
            });
        }
    }
    let mut bytes = class_with_pool(15, &pool);
    for value in [0x0021_u16, 2, 4, 0, 1, 1, 5, 6, 0, 1, 0x0109, 7, 8, 0, 0] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes
}

fn symbolic_text(index: usize, name: &[u8]) -> Vec<u8> {
    match index {
        6 => [b"L", name, b";"].concat(),
        8 => [b"()L", name, b";"].concat(),
        _ => name.to_vec(),
    }
}

#[test]
fn linkage_names_reject_lossy_surrogate_spellings() {
    for name in [b"\xed\xa0\x80", b"\xed\xa0\x81", b"\xed\xb0\x80"] {
        for index in [1, 5, 6, 7, 8] {
            for references in [false, true] {
                let bytes = fixture(index, &symbolic_text(index, name), references);
                assert_eq!(
                    parse(&bytes).unwrap_err().code(),
                    "unsupported-symbol-encoding"
                );
            }
        }
    }
}

#[test]
fn linkage_names_preserve_replacement_character_and_paired_surrogates() {
    for name in [&b"\xef\xbf\xbd"[..], &b"\xed\xa0\xbd\xed\xb8\x80"[..]] {
        for index in [1, 5, 6, 7, 8] {
            for references in [false, true] {
                parse(&fixture(index, &symbolic_text(index, name), references)).unwrap();
            }
        }
    }
}

#[test]
fn string_literals_keep_isolated_surrogates_after_linkage_validation() {
    let class = parse(&fixture(0, &[], true)).unwrap();
    assert_eq!(
        class.constant_pool[13]
            .as_ref()
            .unwrap()
            .utf16_units()
            .unwrap()
            .as_ref(),
        &[0xd800, 120, 0xdc00]
    );
}
