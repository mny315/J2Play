//! A small manual chooser over the complete, unchanged launch catalog.

use super::{ProfileChoice, ProfileOption, egui};
use device_profile::DeviceProfile;

// Common choices for each screen/input/platform combination. These are useful
// defaults, not claims that historical guest contracts are interchangeable.
// In particular, keep S40/S60/Asha separate. Exact variants remain selectable
// and Automatic continues to resolve against the complete launch catalog.
const BASIC_PROFILES: &[&str] = &[
    "benq-siemens-featurephone",
    "nokia-featurephone",
    "nokia-s40-v1-keypad",
    "nokia-s40-v2-keypad",
    "nokia-s40-touch",
    "nokia-asha-touch",
    "nokia-s60-v2-keypad",
    "nokia-s60-keypad",
    "nokia-s60-touch",
    "samsung-featurephone",
    "samsung-midp1-keypad",
    "samsung-midp2-cldc10-keypad",
    "samsung-f480-touch",
    "samsung-touch",
    "samsung-midp20-m3g-touch",
    "samsung-wvga-touch",
    "siemens-midp1-basic-keypad",
    "siemens-midp1-color-keypad",
    "siemens-featurephone",
    "siemens-sxg75-keypad",
    "se-jp6-no-bluetooth-keypad",
    "se-jp7-media-keypad",
    "se-jp8-late-keypad",
    "se-jp6-touch",
    "se-jp8-touch",
];

pub(super) fn basic_profile_name(profile: &DeviceProfile) -> Option<String> {
    if !BASIC_PROFILES.contains(&profile.profile_id()) {
        return None;
    }
    let family = match profile.composition().persona().lineage() {
        "nokia-s40-keypad" | "nokia-s40-touch" => " · Series 40",
        "nokia-s60-keypad" | "nokia-s60-touch" => " · S60",
        "nokia-asha-touch" => " · Asha",
        _ => "",
    };
    let (width, height) = profile.canvas_dimensions()?;
    Some(format!(
        "{}{family} · {width}×{height} · {}",
        profile.device().manufacturer(),
        input_label(profile),
    ))
}

pub(super) fn input_label(profile: &DeviceProfile) -> &'static str {
    if profile
        .input()
        .pointer()
        .and_then(|pointer| pointer.events().value())
        .copied()
        .unwrap_or(false)
    {
        "Touch phone"
    } else {
        "Keypad phone"
    }
}

impl ProfileOption {
    pub(super) fn menu_name(&self, show_all: bool) -> &str {
        if show_all {
            &self.display_name
        } else {
            self.basic_name.as_deref().unwrap_or(&self.display_name)
        }
    }

    pub(super) fn visible_in_menu(&self, show_all: bool, choice: &ProfileChoice) -> bool {
        show_all
            || self.basic_name.is_some()
            || matches!(choice, ProfileChoice::Manual { profile_id } if profile_id == &self.profile_id)
    }
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    options: &[ProfileOption],
    choice: &mut ProfileChoice,
    show_all: bool,
) -> egui::Response {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let selected = match &*choice {
        ProfileChoice::Automatic => "Automatic",
        ProfileChoice::Manual { profile_id } => options
            .iter()
            .find(|option| &option.profile_id == profile_id)
            .map_or("Unavailable device profile", |option| {
                option.menu_name(show_all)
            }),
    };
    let combo_id = ui.make_persistent_id(egui::IdSalt::new("device-profile"));
    // Freeze visibility for the duration of this menu pass: choosing a common
    // profile must not remove the preceding saved historical option mid-loop.
    let previous = choice.clone();
    egui::ComboBox::from_id_salt("device-profile")
        .selected_text(tr.profile_name(selected))
        .width(ui.available_width())
        .height(super::PROFILE_MENU_MAX_HEIGHT)
        .wrap()
        .show_ui(ui, |ui| {
            super::apply_settings_style(ui);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            let mut menu = crate::focus_navigation::ComboMenu::begin(ui, combo_id);
            menu.selectable_value(ui, choice, ProfileChoice::Automatic, &tr.text("Automatic"));
            for option in options
                .iter()
                .filter(|option| option.visible_in_menu(show_all, &previous))
            {
                menu.selectable_value(
                    ui,
                    choice,
                    ProfileChoice::Manual {
                        profile_id: option.profile_id.clone(),
                    },
                    &tr.profile_name(option.menu_name(show_all)),
                );
            }
            menu.finish(ui);
        })
        .response
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/profile_picker.rs"]
mod tests;
