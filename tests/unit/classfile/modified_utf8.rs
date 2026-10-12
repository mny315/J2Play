use super::*;

#[test]
fn utf8_constants_preserve_every_java_code_unit() {
    for start in (0..=u16::MAX).step_by(256) {
        let units: Vec<_> = (start..=start + 255).collect();
        let mut encoded = Vec::new();
        for &unit in &units {
            match unit {
                1..=0x7f => encoded.push(u8::try_from(unit).unwrap()),
                0..=0x7ff => {
                    encoded.push(0xc0 | u8::try_from(unit >> 6).unwrap());
                    encoded.push(0x80 | u8::try_from(unit & 0x3f).unwrap());
                }
                _ => {
                    encoded.push(0xe0 | u8::try_from(unit >> 12).unwrap());
                    encoded.push(0x80 | u8::try_from((unit >> 6) & 0x3f).unwrap());
                    encoded.push(0x80 | u8::try_from(unit & 0x3f).unwrap());
                }
            }
        }
        let constant = parse_utf8_constant(&encoded, 20).unwrap();
        assert_eq!(constant.utf16_units().unwrap().as_ref(), units);
    }
}

#[test]
fn mixed_modified_and_standard_utf8_preserves_supplementary_characters() {
    for (character, pair) in [
        ('\u{10000}', [0xd800, 0xdc00]),
        ('\u{103ff}', [0xd800, 0xdfff]),
        ('\u{10400}', [0xd801, 0xdc00]),
        ('\u{1f600}', [0xd83d, 0xde00]),
        ('\u{10ffff}', [0xdbff, 0xdfff]),
    ] {
        let mut encoded = vec![0xc0, 0x80];
        encoded.extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes());
        encoded.extend_from_slice(&[0xed, 0xa0, 0x80]);
        let constant = parse_utf8_constant(&encoded, 20).unwrap();
        assert_eq!(
            constant.utf16_units().unwrap().as_ref(),
            [0, pair[0], pair[1], 0xd800]
        );
    }
}

#[test]
fn utf8_fast_path_preserves_modified_encoding_errors_and_offsets() {
    for encoded in [
        &b"prefix\0"[..],
        &b"prefix\xc0\x81"[..],
        &b"prefix\xe0\x80\x80"[..],
        &b"prefix\xf0\x80\x80\x80"[..],
        &b"prefix\xf4\x90\x80\x80"[..],
        &b"prefix\xed\xa0"[..],
        &b"prefix\xff"[..],
    ] {
        let error = parse_utf8_constant(encoded, 20).unwrap_err();
        assert_eq!(error.code(), "invalid-modified-utf8");
        assert!(error.message().contains("offset 26"));
    }
}
