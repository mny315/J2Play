use super::*;

#[test]
fn midp_locale_accepts_language_country_and_bounded_variants() {
    for locale in [
        "ru",
        "ru-RU",
        "he-IL",
        "iw-IL",
        "in-ID",
        "zh-CN-Hans",
        "pt-BR",
    ] {
        validate_guest_locale(locale).unwrap();
    }
    for locale in [
        "",
        "r",
        "rus",
        "RU",
        "ru_RU",
        "ru-Rus",
        "he-il",
        "en-",
        "en-US-",
        "en-US-123456789",
        "en-US\n",
        "éé",
        "en-US-💣",
    ] {
        assert_eq!(
            validate_guest_locale(locale).unwrap_err().code(),
            "guest-locale",
            "{locale:?}"
        );
    }
    assert!(validate_guest_locale(&format!("en-US{}", "-abcdefgh".repeat(8))).is_err());
}
