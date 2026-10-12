//! Filename normalization, identifier boundaries and orientation tokens.

use crate::decision::CanvasOrientation;
use std::collections::{BTreeSet, HashSet};

#[derive(Debug)]
pub(crate) struct NormalizedArchiveName {
    pub(super) normalized: String,
    tokens: Vec<String>,
    compact: String,
    token_boundaries: HashSet<usize>,
    digit_runs: HashSet<String>,
}

impl NormalizedArchiveName {
    pub(crate) fn new(value: &str) -> Self {
        let normalized = value.to_ascii_lowercase();
        let tokens = separator_tokens(&normalized);
        let compact = tokens.concat();
        let mut token_boundaries = HashSet::from([0]);
        let mut compact_offset = 0;
        for token in &tokens {
            compact_offset += token.len();
            token_boundaries.insert(compact_offset);
        }
        let digit_runs = tokens
            .iter()
            .flat_map(|token| {
                token
                    .split(|character: char| !character.is_ascii_digit())
                    .filter(|digits| !digits.is_empty())
                    .map(str::to_owned)
            })
            .collect();
        Self {
            normalized,
            tokens,
            compact,
            token_boundaries,
            digit_runs,
        }
    }

    pub(super) fn contains_token_sequence(&self, needle: &[String]) -> bool {
        contains_token_sequence(&self.tokens, needle)
    }

    fn contains_compact(&self, needle: &str) -> bool {
        !needle.is_empty() && self.compact.contains(needle)
    }

    pub(super) fn contains_compact_identifier(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }
        self.tokens
            .iter()
            .any(|token| compact_identifier_in(token, needle, None))
            || compact_identifier_in(&self.compact, needle, Some(&self.token_boundaries))
    }

    pub(crate) fn contains_profile_signal(&self, signal: &str) -> bool {
        if signal.bytes().all(|byte| byte.is_ascii_digit()) {
            self.digit_runs.contains(signal)
        } else if signal == "nokia" {
            self.contains_nokia_signal()
        } else if signal == "sie" {
            self.tokens.iter().any(|token| token == "sie")
        } else {
            self.contains_compact_identifier(signal)
        }
    }

    fn contains_nokia_signal(&self) -> bool {
        if self.contains_compact("nokia") {
            return true;
        }
        self.tokens.iter().any(|token| {
            token == "nok"
                || token.strip_prefix("nok").is_some_and(|suffix| {
                    suffix.starts_with("s40")
                        || suffix.starts_with("s60")
                        || suffix.starts_with("asha")
                        || suffix.as_bytes().first().is_some_and(u8::is_ascii_digit)
                        || (suffix.len() >= 2
                            && matches!(suffix.as_bytes()[0], b'c' | b'e' | b'n' | b'x')
                            && suffix.as_bytes()[1].is_ascii_digit())
                })
                || token.strip_suffix("nok").is_some_and(|prefix| {
                    !prefix.is_empty() && prefix.bytes().any(|byte| byte.is_ascii_digit())
                })
        })
    }

    pub(super) fn axis_hints(&self) -> HashSet<u32> {
        self.digit_runs
            .iter()
            .filter_map(|digits| {
                (2..=3)
                    .contains(&digits.len())
                    .then(|| digits.parse().ok())
                    .flatten()
            })
            .filter(|axis| *axis >= 64)
            .collect()
    }

    pub(super) fn contains_digit_run_with_dimension_affix(
        &self,
        digits: &str,
        dimension_compacts: &HashSet<String>,
    ) -> bool {
        self.digit_runs.iter().any(|run| {
            run == digits
                || run
                    .strip_prefix(digits)
                    .is_some_and(|suffix| dimension_compacts.contains(suffix))
                || run
                    .strip_suffix(digits)
                    .is_some_and(|prefix| dimension_compacts.contains(prefix))
        })
    }

    pub(super) fn profile_hint_matches(&self, signals: &BTreeSet<String>) -> usize {
        signals
            .iter()
            .filter(|signal| self.contains_profile_signal(signal))
            .count()
    }

    pub(super) fn contains_contextual_dimensions(
        &self,
        dimensions: (u32, u32),
        profile_signals: &BTreeSet<String>,
    ) -> bool {
        let dimensions = format!("{}{}", dimensions.0, dimensions.1);
        self.digit_runs.contains(&dimensions)
            || profile_signals.iter().any(|signal| {
                !signal.bytes().all(|byte| byte.is_ascii_digit())
                    && (self.contains_compact(&format!("{signal}{dimensions}"))
                        || self.contains_compact(&format!("{dimensions}{signal}")))
            })
    }
}

fn compact_identifier_in(
    haystack: &str,
    needle: &str,
    token_boundaries: Option<&HashSet<usize>>,
) -> bool {
    let boundary = |index| {
        index == 0
            || index == haystack.len()
            || token_boundaries.is_some_and(|boundaries| boundaries.contains(&index))
    };
    let starts_with_digit = needle.as_bytes()[0].is_ascii_digit();
    let ends_with_digit = needle.as_bytes()[needle.len() - 1].is_ascii_digit();
    haystack.match_indices(needle).any(|(start, matched)| {
        let end = start + matched.len();
        (!starts_with_digit || boundary(start) || !haystack.as_bytes()[start - 1].is_ascii_digit())
            && (!ends_with_digit || boundary(end) || !haystack.as_bytes()[end].is_ascii_digit())
    })
}

pub(super) fn archive_orientation_hint(name: &NormalizedArchiveName) -> Option<CanvasOrientation> {
    let tokens = &name.tokens;
    let mut orientation = None;
    for (index, token) in tokens.iter().enumerate() {
        let candidate = match token.as_str() {
            "portrait" | "vertical" => CanvasOrientation::Portrait,
            "landscape" | "horizontal" => CanvasOrientation::Landscape,
            "p" if is_language_orientation_suffix(tokens, index) => CanvasOrientation::Portrait,
            "l" if is_language_orientation_suffix(tokens, index) => CanvasOrientation::Landscape,
            _ => continue,
        };
        match orientation {
            Some(previous) if previous != candidate => return None,
            Some(_) => {}
            None => orientation = Some(candidate),
        }
    }
    orientation
}

fn is_language_orientation_suffix(tokens: &[String], index: usize) -> bool {
    // A following one-letter token makes this an initialism such as `L.A.`,
    // not the deployed `<language>_L` / `<language>_P` suffix convention.
    index != 0
        && is_two_letter_language_tag(&tokens[index - 1])
        && !tokens.get(index + 1).is_some_and(|next| {
            next.len() == 1 && next.bytes().all(|byte| byte.is_ascii_alphabetic())
        })
}

fn is_two_letter_language_tag(token: &str) -> bool {
    token.len() == 2 && token.bytes().all(|byte| byte.is_ascii_alphabetic())
}

pub(crate) fn separator_tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn contains_token_sequence(haystack: &[String], needle: &[String]) -> bool {
    !needle.is_empty()
        && needle.len() <= haystack.len()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
