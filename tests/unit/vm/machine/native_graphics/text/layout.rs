use super::*;
use crate::machine::DefaultNativeContext;

#[path = "../../../../../support/text_languages.rs"]
mod languages;

fn layout(text: &str) -> Vec<u16> {
    visual_text(
        &text.encode_utf16().collect::<Vec<_>>(),
        &DefaultNativeContext,
    )
    .unwrap()
    .into_owned()
}

#[test]
fn unicode_visual_order_preserves_marks_numbers_mirroring_and_isolates() {
    for (logical, expected) in [
        ("שלום", "םולש"),
        ("שָׁלוֹם", "םוֹלשָׁ"),
        ("שלום 123", "123 םולש"),
        ("ABC שלום 123", "ABC 123 םולש"),
        ("שלום (ABC)", "(ABC) םולש"),
        ("(שלום)", "(םולש)"),
        ("שלום [123]", "[123] םולש"),
        ("שלום\nעולם", "םולש\nםלוע"),
        (
            "العربية",
            "\u{fe94}\u{fef4}\u{fe91}\u{feae}\u{fecc}\u{fedf}\u{fe8d}",
        ),
        ("فارسی", "\u{fbfd}\u{feb3}\u{fead}\u{fe8e}\u{fed3}"),
        ("اردو", "\u{feed}\u{fea9}\u{fead}\u{fe8d}"),
        ("بَت", "\u{fe96}\u{fe91}\u{064e}"),
        ("ب\u{200c}ت", "\u{fe95}\u{200c}\u{fe8f}"),
        ("ب\u{200d}ت", "\u{fe96}\u{200d}\u{fe91}"),
    ] {
        assert_eq!(
            layout(logical),
            expected.encode_utf16().collect::<Vec<_>>(),
            "{logical:?}"
        );
    }
    assert_eq!(
        layout("abc \u{2067}אב 123\u{2069} xyz")
            .into_iter()
            .filter(|unit| !is_invisible(*unit))
            .collect::<Vec<_>>(),
        "abc 123 בא xyz".encode_utf16().collect::<Vec<_>>()
    );
    assert_eq!(
        layout("\u{202e}ABC\u{202c}")
            .into_iter()
            .filter(|unit| !is_invisible(*unit))
            .collect::<Vec<_>>(),
        "CBA".encode_utf16().collect::<Vec<_>>()
    );
}

#[test]
fn utf16_reordering_keeps_surrogate_pairs_and_accepts_lone_surrogates() {
    let visual = layout("אב𐐀ג");
    assert_eq!(String::from_utf16(&visual).unwrap(), "ג𐐀בא");
    let malformed = [0x05d0, 0xd800, 0x05d1, 0xdc00];
    let visual = visual_text(&malformed, &DefaultNativeContext).unwrap();
    assert_eq!(visual.len(), malformed.len());
    assert_eq!(visual.iter().filter(|&&unit| unit == 0xd800).count(), 1);
    assert_eq!(visual.iter().filter(|&&unit| unit == 0xdc00).count(), 1);
}

#[test]
fn text_layout_bounds_scratch_space_and_borrows_ltr_payloads() {
    let ltr = vec![u16::from(b'A'); MAX_BIDI_UNITS + 1];
    assert!(matches!(
        visual_text(&ltr, &DefaultNativeContext).unwrap(),
        Cow::Borrowed(_)
    ));
    let rtl = vec![0x05d0; MAX_BIDI_UNITS + 1];
    assert_eq!(
        visual_text(&rtl, &DefaultNativeContext).unwrap_err().code(),
        "graphics-text-limit"
    );
    let separated = "א\n".repeat(MAX_BIDI_UNITS / 2);
    assert_eq!(layout(&separated).len(), MAX_BIDI_UNITS);
}

#[test]
fn every_guest_language_preset_keeps_its_script_direction_and_bmp_glyphs() {
    for &(locale, logical, expected) in languages::LANGUAGE_SAMPLES {
        let visual = layout(logical);
        assert_eq!(
            visual,
            expected.encode_utf16().collect::<Vec<_>>(),
            "{locale}"
        );
        for unit in visual {
            assert!(
                crate::machine::font::unifont_bmp_glyph(unit).is_some(),
                "{locale}: {unit:04x}"
            );
        }
    }
}
