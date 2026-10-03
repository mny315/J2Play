use super::*;

#[test]
fn simple_mapping_handles_expansions_without_losing_the_one_character_contract() {
    for (lower, upper) in [
        ('i', 'I'),
        ('ä', 'Ä'),
        ('σ', 'Σ'),
        ('\u{1fb3}', '\u{1fbc}'),
        ('\u{1fc3}', '\u{1fcc}'),
        ('\u{1ff3}', '\u{1ffc}'),
    ] {
        assert_eq!(simple_case_mapping(lower, true), upper);
        assert_eq!(simple_case_mapping(upper, false), lower);
    }
    assert_eq!(simple_case_mapping('İ', false), 'i');
    assert_eq!(simple_case_mapping('ı', true), 'I');
    for base in [0x1f80, 0x1f90, 0x1fa0] {
        for offset in 0..8 {
            let lower = char::from_u32(base + offset).unwrap();
            let upper = char::from_u32(base + offset + 8).unwrap();
            assert_eq!(simple_case_mapping(lower, true), upper);
            assert_eq!(simple_case_mapping(upper, false), lower);
            assert_eq!(simple_case_mapping(upper, true), upper);
        }
    }
    for character in ['ß', 'ﬀ', '\u{0149}', '\u{0390}', '\u{1f50}'] {
        assert_eq!(simple_case_mapping(character, true), character);
    }
}
