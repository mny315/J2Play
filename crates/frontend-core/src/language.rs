use serde::{Deserialize, Serialize};

/// Supported product languages. This never changes guest Java ME properties.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "ru")]
    Russian,
    #[serde(rename = "uk")]
    Ukrainian,
    #[serde(rename = "pl")]
    Polish,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "pt-BR")]
    Portuguese,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "it")]
    Italian,
    #[serde(rename = "tr")]
    Turkish,
    #[serde(rename = "id")]
    Indonesian,
    #[serde(rename = "vi")]
    Vietnamese,
    #[serde(rename = "th")]
    Thai,
    #[serde(rename = "zh-Hans")]
    Chinese,
    #[serde(rename = "hi")]
    Hindi,
    #[serde(rename = "ar")]
    Arabic,
}

impl Language {
    pub const ALL: [Self; 16] = [
        Self::English,
        Self::Russian,
        Self::Ukrainian,
        Self::Polish,
        Self::Spanish,
        Self::Portuguese,
        Self::German,
        Self::French,
        Self::Italian,
        Self::Turkish,
        Self::Indonesian,
        Self::Vietnamese,
        Self::Thai,
        Self::Chinese,
        Self::Hindi,
        Self::Arabic,
    ];

    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Russian => "ru",
            Self::Ukrainian => "uk",
            Self::Polish => "pl",
            Self::Spanish => "es",
            Self::Portuguese => "pt-BR",
            Self::German => "de",
            Self::French => "fr",
            Self::Italian => "it",
            Self::Turkish => "tr",
            Self::Indonesian => "id",
            Self::Vietnamese => "vi",
            Self::Thai => "th",
            Self::Chinese => "zh-Hans",
            Self::Hindi => "hi",
            Self::Arabic => "ar",
        }
    }

    #[must_use]
    pub const fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Russian => "Русский",
            Self::Ukrainian => "Українська",
            Self::Polish => "Polski",
            Self::Spanish => "Español",
            Self::Portuguese => "Português (Brasil)",
            Self::German => "Deutsch",
            Self::French => "Français",
            Self::Italian => "Italiano",
            Self::Turkish => "Türkçe",
            Self::Indonesian => "Bahasa Indonesia",
            Self::Vietnamese => "Tiếng Việt",
            Self::Thai => "ไทย",
            Self::Chinese => "简体中文",
            Self::Hindi => "हिन्दी",
            Self::Arabic => "العربية",
        }
    }

    /// Resolve BCP-47 and POSIX locale names; unsupported languages use English.
    #[must_use]
    pub fn from_locale(locale: &str) -> Self {
        let primary = locale.split(['-', '_', '.', '@']).next().unwrap_or("");
        match primary.to_ascii_lowercase().as_str() {
            "ru" => Self::Russian,
            "uk" => Self::Ukrainian,
            "pl" => Self::Polish,
            "es" => Self::Spanish,
            "pt" => Self::Portuguese,
            "de" => Self::German,
            "fr" => Self::French,
            "it" => Self::Italian,
            "tr" => Self::Turkish,
            "id" | "in" => Self::Indonesian,
            "vi" => Self::Vietnamese,
            "th" => Self::Thai,
            "zh" => Self::Chinese,
            "hi" => Self::Hindi,
            "ar" => Self::Arabic,
            _ => Self::English,
        }
    }
}
