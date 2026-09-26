use crate::archive_name::separator_tokens;
use device_profile::DeviceProfile;
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub(super) struct ModelAliasIndex {
    pub(super) manufacturer: HashSet<String>,
    pub(super) samsung: HashSet<String>,
    pub(super) ambiguous: HashSet<String>,
    pub(super) nokia_code: HashSet<String>,
    pub(super) nokia_numeric: HashSet<String>,
    pub(super) dimension_compacts: HashSet<String>,
}

impl ModelAliasIndex {
    pub(super) fn new(profiles: &[DeviceProfile]) -> Self {
        let mut manufacturers = BTreeMap::<String, BTreeSet<String>>::new();
        let mut manufacturer_counts = BTreeMap::<String, usize>::new();
        let mut samsung_owners = BTreeMap::<String, BTreeSet<(&str, u32, u32)>>::new();
        let mut nokia_code_counts = BTreeMap::<String, usize>::new();
        let mut nokia_numeric_counts = BTreeMap::<String, usize>::new();
        for profile in profiles {
            let manufacturer = profile.device().manufacturer();
            let normalized_manufacturer = manufacturer.to_ascii_lowercase();
            let manufacturer_tokens = separator_tokens(manufacturer);
            for hint in profile.device().archive_name_hints() {
                let canvas = hint.fullscreen_canvas();
                let owner = (profile.profile_id(), canvas.width(), canvas.height());
                for model in hint.model_names() {
                    let tokens = separator_tokens(model);
                    if let Some(suffix) = tokens.strip_prefix(manufacturer_tokens.as_slice())
                        && !suffix.is_empty()
                    {
                        *manufacturer_counts.entry(suffix.concat()).or_default() += 1;
                    }
                    let samsung = samsung_model_alias(&tokens, manufacturer);
                    let samsung_short = samsung_short_model_alias(&tokens, manufacturer);
                    let nokia_code = nokia_code_alias(&tokens);
                    let nokia_numeric = nokia_numeric_code(&tokens);
                    for alias in [
                        sony_ericsson_model_tokens(&tokens).map(<[String]>::concat),
                        siemens_model_alias(&tokens, manufacturer),
                        nokia_compact_model_alias(&tokens),
                    ]
                    .iter()
                    .chain([&samsung, &samsung_short, &nokia_code, &nokia_numeric])
                    .flatten()
                    {
                        manufacturers
                            .entry(alias.clone())
                            .or_default()
                            .insert(normalized_manufacturer.clone());
                    }
                    for alias in [samsung, samsung_short].into_iter().flatten() {
                        samsung_owners.entry(alias).or_default().insert(owner);
                    }
                    if let Some(alias) = nokia_code {
                        *nokia_code_counts.entry(alias).or_default() += 1;
                    }
                    if let Some(alias) = nokia_numeric {
                        *nokia_numeric_counts.entry(alias).or_default() += 1;
                    }
                }
            }
        }
        let ambiguous = manufacturers
            .into_iter()
            .filter_map(|(alias, manufacturers)| (manufacturers.len() > 1).then_some(alias))
            .collect::<HashSet<_>>();
        Self {
            manufacturer: unique_aliases(manufacturer_counts, &ambiguous),
            samsung: samsung_owners
                .into_iter()
                .filter_map(|(alias, owners)| {
                    (owners.len() == 1 && !ambiguous.contains(&alias)).then_some(alias)
                })
                .collect(),
            nokia_code: unique_aliases(nokia_code_counts, &ambiguous),
            nokia_numeric: unique_aliases(nokia_numeric_counts, &ambiguous),
            dimension_compacts: declared_dimension_compacts(profiles),
            ambiguous,
        }
    }
}

fn unique_aliases(counts: BTreeMap<String, usize>, ambiguous: &HashSet<String>) -> HashSet<String> {
    counts
        .into_iter()
        .filter_map(|(alias, count)| (count == 1 && !ambiguous.contains(&alias)).then_some(alias))
        .collect()
}

fn declared_dimension_compacts(profiles: &[DeviceProfile]) -> HashSet<String> {
    let mut dimensions = HashSet::new();
    for profile in profiles {
        if profile.display().screen_modes().is_empty() {
            if let Some((width, height)) = profile.canvas_dimensions() {
                dimensions.insert(format!("{width}{height}"));
                dimensions.insert(format!("{height}{width}"));
            }
            continue;
        }
        for mode in profile.display().screen_modes() {
            let canvas = mode.fullscreen_canvas();
            dimensions.insert(format!("{}{}", canvas.width(), canvas.height()));
            dimensions.insert(format!("{}{}", canvas.height(), canvas.width()));
        }
    }
    dimensions
}

pub(super) fn nokia_code_alias(model_tokens: &[String]) -> Option<String> {
    let (manufacturer, model) = model_tokens.split_first()?;
    if manufacturer != "nokia" {
        return None;
    }
    let code_start = model
        .iter()
        .position(|token| token.bytes().any(|byte| byte.is_ascii_digit()))?;
    Some(format!("nokia{}", model[code_start..].concat()))
}

pub(super) fn nokia_numeric_code(model_tokens: &[String]) -> Option<String> {
    let (manufacturer, model) = model_tokens.split_first()?;
    if manufacturer != "nokia" {
        return None;
    }
    model
        .iter()
        .flat_map(|token| {
            token
                .split(|character: char| !character.is_ascii_digit())
                .filter(|digits| !digits.is_empty())
        })
        .find(|digits| digits.len() == 4)
        .map(str::to_owned)
}

pub(super) fn sony_ericsson_model_tokens(model_tokens: &[String]) -> Option<&[String]> {
    match model_tokens {
        [sony, ericsson, model @ ..] if sony == "sony" && ericsson == "ericsson" => {
            (!model.is_empty()).then_some(model)
        }
        _ => None,
    }
}

pub(super) fn siemens_model_alias(model_tokens: &[String], manufacturer: &str) -> Option<String> {
    let manufacturer_tokens = separator_tokens(manufacturer);
    if !manufacturer_tokens.iter().any(|token| token == "siemens") {
        return None;
    }
    let model = model_tokens.strip_prefix(manufacturer_tokens.as_slice())?;
    let alias = model.concat();
    (!alias.is_empty() && alias.bytes().any(|byte| byte.is_ascii_alphabetic())).then_some(alias)
}

pub(super) fn samsung_model_alias(model_tokens: &[String], manufacturer: &str) -> Option<String> {
    if !manufacturer.eq_ignore_ascii_case("Samsung") {
        return None;
    }
    let manufacturer_tokens = separator_tokens(manufacturer);
    let model = model_tokens.strip_prefix(manufacturer_tokens.as_slice())?;
    let alias = model.concat();
    (!alias.is_empty()
        && alias.bytes().any(|byte| byte.is_ascii_alphabetic())
        && alias.bytes().any(|byte| byte.is_ascii_digit()))
    .then_some(alias)
}

pub(super) fn samsung_short_model_alias(
    model_tokens: &[String],
    manufacturer: &str,
) -> Option<String> {
    if !manufacturer.eq_ignore_ascii_case("Samsung") {
        return None;
    }
    let manufacturer_tokens = separator_tokens(manufacturer);
    let model = model_tokens.strip_prefix(manufacturer_tokens.as_slice())?;
    let model = match model {
        [prefix, rest @ ..] if matches!(prefix.as_str(), "sgh" | "gt" | "sch" | "sph") => rest,
        model => model,
    };
    let alias = model.concat();
    (!alias.is_empty()
        && alias.bytes().any(|byte| byte.is_ascii_alphabetic())
        && alias.bytes().any(|byte| byte.is_ascii_digit()))
    .then_some(alias)
}

pub(super) fn nokia_compact_model_alias(model_tokens: &[String]) -> Option<String> {
    let (manufacturer, model) = model_tokens.split_first()?;
    if manufacturer != "nokia" || model.is_empty() {
        return None;
    }
    let mut alias = model.concat();
    if !alias.starts_with('n') {
        alias.insert(0, 'n');
    }
    Some(alias)
}

#[cfg(test)]
#[path = "../../../tests/unit/launch/model_aliases/mod.rs"]
mod tests;
