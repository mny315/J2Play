use super::*;

#[test]
fn bounded_text_preserves_utf8_and_respects_even_tiny_byte_budgets() {
    assert_eq!(bounded_text("abc", 2), "ab");
    assert_eq!(bounded_text("🙂abc", 6), "…");
    assert_eq!(bounded_text("🙂abcd", 7), "🙂…");
    for text in ["", "ascii", "é界🙂mixed", "\0\ntext"] {
        for maximum in 0..=text.len() + 1 {
            let bounded = bounded_text(text, maximum);
            assert!(bounded.len() <= maximum);
            if text.len() <= maximum {
                assert_eq!(bounded, text);
            } else if maximum >= "…".len() {
                let prefix = bounded.strip_suffix('…').unwrap();
                assert!(text.starts_with(prefix));
            } else {
                assert!(text.starts_with(&bounded));
            }
        }
    }
}

#[test]
fn error_has_stable_display() {
    let error = EmuError::new(Category::Jar, "invalid", "broken archive");
    assert_eq!(error.to_string(), "jar[invalid]: broken archive");
}
