use crate::{MaterialTheme, RichText, egui, material_card_frame};

// Autonyms stay independent of both the UI translation catalog and guest locale.
// Use romanized autonyms for scripts outside the bundled font coverage.
// Regions distinguish resource sets commonly shipped together in MIDlet suites.
const LOCALES: &[(&str, &str)] = &[
    ("af-ZA", "Afrikaans"),
    ("sq-AL", "Shqip"),
    ("ar-SA", "العربية"),
    ("az-AZ", "Azərbaycanca"),
    ("eu-ES", "Euskara"),
    ("be-BY", "Беларуская"),
    ("bn-BD", "Bangla"),
    ("bs-BA", "Bosanski"),
    ("bg-BG", "Български"),
    ("ca-ES", "Català"),
    ("zh-CN", "简体中文"),
    ("zh-TW", "中文"),
    ("zh-HK", "中文"),
    ("hr-HR", "Hrvatski"),
    ("cs-CZ", "Čeština"),
    ("da-DK", "Dansk"),
    ("nl-NL", "Nederlands"),
    ("en-US", "English"),
    ("en-GB", "English"),
    ("et-EE", "Eesti"),
    ("fi-FI", "Suomi"),
    ("fr-FR", "Français"),
    ("fr-CA", "Français"),
    ("de-DE", "Deutsch"),
    ("el-GR", "Ελληνικά"),
    ("he-IL", "Ivrit"),
    ("hi-IN", "हिन्दी"),
    ("hu-HU", "Magyar"),
    ("is-IS", "Íslenska"),
    ("id-ID", "Bahasa Indonesia"),
    ("ga-IE", "Gaeilge"),
    ("it-IT", "Italiano"),
    ("ja-JP", "Nihongo"),
    ("kk-KZ", "Қазақша"),
    ("ko-KR", "Hangugeo"),
    ("lv-LV", "Latviešu"),
    ("lt-LT", "Lietuvių"),
    ("mk-MK", "Македонски"),
    ("ms-MY", "Bahasa Melayu"),
    ("no-NO", "Norsk"),
    ("fa-IR", "فارسی"),
    ("pl-PL", "Polski"),
    ("pt-BR", "Português"),
    ("pt-PT", "Português"),
    ("ro-RO", "Română"),
    ("ru-RU", "Русский"),
    ("sr-RS", "Српски"),
    ("sk-SK", "Slovenčina"),
    ("sl-SI", "Slovenščina"),
    ("es-ES", "Español"),
    ("es-MX", "Español"),
    ("sw-KE", "Kiswahili"),
    ("sv-SE", "Svenska"),
    ("tl-PH", "Tagalog"),
    ("ta-IN", "Tamil"),
    ("th-TH", "ไทย"),
    ("tr-TR", "Türkçe"),
    ("uk-UA", "Українська"),
    ("ur-PK", "اردو"),
    ("uz-UZ", "Oʻzbekcha"),
    ("vi-VN", "Tiếng Việt"),
];

pub(super) fn draw(ui: &mut egui::Ui, theme: &MaterialTheme, locale: &mut Option<String>) {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    material_card_frame(theme).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let title = ui.label(RichText::new(tr.text("Game language")).size(18.0).strong());
        ui.add_space(4.0);
        let selected = locale.as_deref().map_or_else(
            || tr.text("Device profile"),
            |code| {
                LOCALES.iter().find(|(tag, _)| *tag == code).map_or_else(
                    || tr.text("Custom language"),
                    |(_, name)| format!("{name} · {code}"),
                )
            },
        );
        let custom = locale
            .as_ref()
            .filter(|code| !LOCALES.iter().any(|(tag, _)| tag == code));
        let custom_value = Some(custom.cloned().unwrap_or_default());
        let id = ui.make_persistent_id(egui::IdSalt::new("guest-language"));
        let response = egui::ComboBox::from_id_salt("guest-language")
            .selected_text(selected)
            .width(ui.available_width())
            .height(crate::PROFILE_MENU_MAX_HEIGHT)
            .wrap()
            .show_ui(ui, |ui| {
                crate::apply_settings_style(ui);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                let mut menu = crate::focus_navigation::ComboMenu::begin(ui, id);
                menu.selectable_value(ui, locale, None, &tr.text("Device profile"));
                for &(code, name) in LOCALES {
                    menu.selectable_value(
                        ui,
                        locale,
                        Some(code.into()),
                        &format!("{name} · {code}"),
                    );
                }
                menu.selectable_value(ui, locale, custom_value, &tr.text("Custom language"));
                menu.finish(ui);
            })
            .response
            .labelled_by(title.id);
        crate::focus_ring::track(ui, &response, 12.0);
        if let Some(code) = locale {
            let label = ui.label(tr.text("Locale code"));
            let response = ui
                .add_sized(
                    [ui.available_width(), crate::SETTINGS_TOUCH_TARGET_HEIGHT],
                    egui::TextEdit::singleline(code)
                        .id_salt("guest-locale-code")
                        .char_limit(device_profile::MAX_GUEST_LOCALE_BYTES)
                        .vertical_align(egui::Align::Center),
                )
                .labelled_by(label.id);
            crate::focus_ring::track(ui, &response, 4.0);
        }
    });
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/guest_language.rs"]
mod tests;
