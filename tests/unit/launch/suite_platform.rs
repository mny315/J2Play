use super::*;

#[test]
fn platform_patterns_are_anchored_case_sensitive_ascii_globs() {
    for (identity, pattern, expected) in [
        ("Nokia6233", "Nokia6233", true),
        ("Nokia6233", "Nokia623*", true),
        ("Nokia6233", "Nokia62??", true),
        ("Nokia6233", "*kia*3", true),
        ("Nokia6233", "Nokia6300", false),
        ("Nokia6233", "nokia6233", false),
        ("Nokia6233", "Nokia623", false),
        ("Nokia6233", "Nokia6233?", false),
        ("Nokia", "Nokia*", true),
        ("Nokia", "*", true),
        ("", "*", true),
    ] {
        assert_eq!(
            platform_matches(identity, pattern),
            expected,
            "{identity}: {pattern}"
        );
    }
    assert!(!platform_matches(
        &"a".repeat(MAX_IDENTITY_BYTES),
        &format!("{}b", "*a".repeat(60))
    ));
}

#[test]
fn malformed_or_oversized_platform_lists_are_not_partially_used() {
    assert_eq!(
        platform_patterns(" Nokia6233 @ Nokia6300 "),
        Some(vec!["Nokia6233", "Nokia6300"])
    );
    for value in [
        String::new(),
        "Nokia6233@".to_owned(),
        "Nokia6233@@Nokia6300".to_owned(),
        "Nokiä6233".to_owned(),
        "a".repeat(MAX_IDENTITY_BYTES + 1),
        vec!["Nokia*"; MAX_PATTERNS + 1].join("@"),
        "a".repeat(MAX_DECLARATION_BYTES + 1),
    ] {
        assert!(platform_patterns(&value).is_none());
    }
}
