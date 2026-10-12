//! Product strings stay logical Unicode; guest content and technical diagnostics are untouched.
use crate::egui;
use frontend_core::Language;
use std::collections::BTreeMap;
use std::sync::OnceLock;

mod fonts;
pub(crate) use fonts::warm_language_menu;

const CONTEXT_LANGUAGE: &str = "product-language";

#[derive(Clone, Copy)]
pub(crate) struct Translator(pub Language);

impl Translator {
    pub(crate) fn from_context(ctx: &egui::Context) -> Self {
        Self(
            ctx.data(|data| data.get_temp(egui::Id::new(CONTEXT_LANGUAGE)))
                .unwrap_or_default(),
        )
    }

    pub(crate) fn text(self, source: impl AsRef<str>) -> String {
        self.lookup(source.as_ref()).to_owned()
    }

    pub(crate) fn lookup(self, source: &str) -> &str {
        catalog(self.0).get(source).map_or(source, String::as_str)
    }

    /// Substitute named values once, so archive titles cannot introduce placeholders.
    pub(crate) fn format(self, source: &str, values: &[(&str, &str)]) -> String {
        let mut remaining = self.lookup(source);
        let mut output = String::with_capacity(remaining.len());
        while let Some(start) = remaining.find('{') {
            output.push_str(&remaining[..start]);
            remaining = &remaining[start..];
            let Some(end) = remaining.find('}') else {
                break;
            };
            let key = &remaining[1..end];
            output.push_str(
                values
                    .iter()
                    .find(|(name, _)| *name == key)
                    .map_or(&remaining[..=end], |(_, value)| *value),
            );
            remaining = &remaining[end + 1..];
        }
        output.push_str(remaining);
        output
    }

    /// Only for labels assembled by our controls editor, never archive/user text.
    pub(crate) fn control(self, source: &str) -> String {
        if let Some(translated) = catalog(self.0).get(source) {
            return translated.clone();
        }
        if let Some(key) = source.strip_suffix(" key") {
            let key = match key {
                "Star" => "*",
                "Pound" => "#",
                other => other,
            };
            return self.format("{key} key", &[("key", key)]);
        }
        for (prefix, template) in [
            ("Hardware key ", "Hardware key {number}"),
            ("Extra button ", "Extra button {number}"),
            ("Extra ", "Extra button {number}"),
        ] {
            if let Some(number) = source.strip_prefix(prefix) {
                return self.format(template, &[("number", number)]);
            }
        }
        if let Some(key) = source.strip_prefix("Keyboard ") {
            return format!("{}: {key}", self.text("Keyboard"));
        }
        for prefix in ["Left stick", "Right stick", "D-pad"] {
            if let Some(direction) = source
                .strip_prefix(prefix)
                .and_then(|s| s.strip_prefix(' '))
            {
                let direction = match direction {
                    "left" => "Left",
                    "right" => "Right",
                    "up" => "Up",
                    "down" => "Down",
                    other => other,
                };
                return format!("{}: {}", self.text(prefix), self.text(direction));
            }
        }
        self.translate_parts(source, " / ")
    }

    /// Manufacturer/model tokens are preserved; only product-owned descriptors change.
    pub(crate) fn profile_name(self, source: &str) -> String {
        self.translate_parts(source, " · ")
    }

    fn translate_parts(self, source: &str, separator: &str) -> String {
        let mut output = String::with_capacity(source.len());
        for (index, part) in source.split(separator).enumerate() {
            if index != 0 {
                output.push_str(separator);
            }
            output.push_str(self.lookup(part));
        }
        output
    }

    pub(crate) fn profile_reason(self, source: &str) -> String {
        if let Some(api) = source
            .strip_prefix("Supports the game's Java ME APIs (JSR-")
            .and_then(|text| text.strip_suffix(")."))
        {
            self.format(
                "Supports the game's Java ME APIs (JSR-{api}).",
                &[("api", api)],
            )
        } else {
            self.text(source)
        }
    }

    pub(crate) fn resume_detail(self, source: &str) -> String {
        if catalog(self.0).contains_key(source) || self.0 == Language::English {
            self.text(source)
        } else {
            self.text("The automatic save cannot be used. You can start normally.")
        }
    }

    pub(crate) fn error_title(self, source: &str) -> String {
        if self.0 == Language::English || catalog(self.0).contains_key(source) {
            self.text(source)
        } else {
            self.text("Error")
        }
    }

    pub(crate) fn error(self, source: &str) -> String {
        if self.0 == Language::English || catalog(self.0).contains_key(source) {
            self.text(source)
        } else {
            self.text("The operation failed. See technical details.")
        }
    }
}

#[cfg(test)]
pub(crate) fn install(ctx: &egui::Context, language: Language) {
    install_scaled(ctx, language, crate::PlatformTextScale::default());
}

pub(crate) fn install_scaled(
    ctx: &egui::Context,
    language: Language,
    scale: crate::PlatformTextScale,
) {
    fonts::install(ctx, scale);
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(CONTEXT_LANGUAGE), language));
}

fn catalog(language: Language) -> &'static BTreeMap<String, String> {
    macro_rules! catalogs {
        ($($variant:ident => $tag:literal),+ $(,)?) => {
            match language { $(Language::$variant => {
                static STRINGS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
                STRINGS.get_or_init(|| serde_json::from_str(include_str!(concat!("../locales/", $tag, ".json")))
                    .expect("validated built-in translation catalog"))
            }),+ }
        };
    }
    catalogs! {
        English => "en", Russian => "ru", Ukrainian => "uk", Polish => "pl",
        Spanish => "es", Portuguese => "pt-BR", German => "de", French => "fr",
        Italian => "it", Turkish => "tr", Indonesian => "id", Vietnamese => "vi",
        Thai => "th", Chinese => "zh-Hans", Hindi => "hi", Arabic => "ar",
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/i18n.rs"]
mod tests;
