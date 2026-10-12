use super::*;

fn encode(units: &[u16], name: &str) -> Option<Vec<u8>> {
    Some(CharacterEncoding::for_name(name)?.encode(units))
}

#[test]
fn aliases_resolve_to_the_same_encoding() {
    for (expected, names) in [
        (
            CharacterEncoding::Utf8,
            &["UTF-8", "utf8", "uTf_8", " UTF 8 "][..],
        ),
        (
            CharacterEncoding::Latin1,
            &[
                "ISO-8859-1",
                "ISO8859_1",
                "ISO88591",
                "ISO8859",
                "latin-1",
                "LATIN1",
            ],
        ),
        (CharacterEncoding::Ascii, &["US-ASCII", "us_ascii", "ascii"]),
        (
            CharacterEncoding::Utf16Be,
            &["UTF-16BE", "utf16be", "UTF_16_BE"],
        ),
        (
            CharacterEncoding::Utf16Le,
            &["UTF-16LE", "utf16le", "UTF_16_LE"],
        ),
        (CharacterEncoding::Utf16, &["UTF-16", "utf16", "utf_16"]),
    ] {
        for name in names {
            assert_eq!(CharacterEncoding::for_name(name), Some(expected), "{name}");
        }
    }
    for name in [
        "",
        "---",
        "UTF-32",
        "UTF-8junk",
        "ISO88591extra",
        "UTF\08",
        "\tUTF8",
        "é",
    ] {
        assert_eq!(CharacterEncoding::for_name(name), None, "{name:?}");
    }
    assert_eq!(CharacterEncoding::for_name(&"X".repeat(65_536)), None);
}

#[test]
fn string_decoders_cover_single_byte_and_utf16_forms() {
    for (encoding, bytes, expected) in [
        (
            CharacterEncoding::Utf8,
            &b"A\xc3\xa9\xf0\x9f\x98\x80"[..],
            &[65, 0xe9, 0xd83d, 0xde00][..],
        ),
        (
            CharacterEncoding::Latin1,
            &[65, 0x80, 0xff],
            &[65, 0x80, 0xff],
        ),
        (
            CharacterEncoding::Ascii,
            &[65, 0x80, 0xff],
            &[65, 0xfffd, 0xfffd],
        ),
        (
            CharacterEncoding::Utf16Be,
            &[0, 65, 0x20, 0xac],
            &[65, 0x20ac],
        ),
        (
            CharacterEncoding::Utf16Le,
            &[65, 0, 0xac, 0x20],
            &[65, 0x20ac],
        ),
        (CharacterEncoding::Utf16, &[0xfe, 0xff, 0, 65], &[65]),
        (CharacterEncoding::Utf16, &[0xff, 0xfe, 65, 0], &[65]),
        (CharacterEncoding::Utf16, &[0, 65, 0xff], &[65, 0xfffd]),
        (CharacterEncoding::Utf16, &[0xff], &[0xfffd]),
        (CharacterEncoding::Utf16, &[0xfe, 0xff], &[]),
        (
            CharacterEncoding::Utf16Be,
            &[0xfe, 0xff, 0, 65],
            &[0xfeff, 65],
        ),
        (
            CharacterEncoding::Utf16Le,
            &[0xff, 0xfe, 65, 0],
            &[0xfeff, 65],
        ),
    ] {
        assert_eq!(encoding.decode(bytes), expected, "{encoding:?} {bytes:?}");
        assert!(encoding.decode(&[]).is_empty());
    }
}

#[test]
fn unicode_encodings_roundtrip_java_text() {
    let units = [0, 65, 0xe9, 0x20ac, 0xd83d, 0xde00];
    for encoding in [
        CharacterEncoding::Utf8,
        CharacterEncoding::Utf16Be,
        CharacterEncoding::Utf16Le,
        CharacterEncoding::Utf16,
    ] {
        assert_eq!(encoding.decode(&encoding.encode(&units)), units);
    }
}

#[test]
fn named_string_encodings_keep_existing_output_bytes() {
    let value = [u16::from(b'A'), 0x00e9, 0x20ac];
    assert_eq!(encode(&value, "UTF-8").unwrap(), "Aé€".as_bytes());
    assert_eq!(encode(&value, "ISO_8859-1").unwrap(), [b'A', 0xe9, b'?']);
    assert_eq!(encode(&value, "US-ASCII").unwrap(), [b'A', b'?', b'?']);
    assert_eq!(encode(&value[..2], "UTF-16BE").unwrap(), [0, b'A', 0, 0xe9]);
    assert_eq!(
        encode(&value[..1], "UTF-16").unwrap(),
        [0xfe, 0xff, 0, b'A']
    );
    assert!(encode(&value, "x-unknown").is_none());
}

#[test]
fn utf8_decoding_preserves_replacement_groups_and_adjacent_valid_text() {
    let check = |bytes: &[u8]| {
        assert_eq!(
            CharacterEncoding::Utf8.decode(bytes),
            String::from_utf8_lossy(bytes)
                .encode_utf16()
                .collect::<Vec<_>>(),
            "{bytes:?}"
        );
    };
    for first in 0..=u8::MAX {
        for second in 0..=u8::MAX {
            check(&[b'A', first, second, b'Z']);
        }
    }
    for bytes in [
        &[0xe0, 0xa0, 0x80][..], // Lowest three-byte character.
        &[0xed, 0xa0, 0x80],     // Surrogate code point.
        &[0xe0, 0x80, 0x80],     // Overlong form.
        &[0xf0, 0x90, 0x80, 0x80],
        &[0xf4, 0x8f, 0xbf, 0xbf],
        &[0xf4, 0x90, 0x80, 0x80], // Beyond the Unicode range.
        &[0xf0, 0x9f, b'!', 0x80], // Interrupted sequence.
    ] {
        for end in 0..=bytes.len() {
            check(&bytes[..end]);
            let mut surrounded = "é".as_bytes().to_vec();
            surrounded.extend_from_slice(&bytes[..end]);
            surrounded.extend_from_slice("🙂".as_bytes());
            check(&surrounded);
        }
    }
}
