use super::{Category, EmuError};

pub(super) fn friendly_message(error: &EmuError) -> String {
    match error.category() {
        Category::Jar | Category::ClassLoading => {
            "This file is not a supported or valid Java ME game archive.".to_owned()
        }
        Category::Api if error.code().starts_with("device-selection") => {
            "No compatible built-in device profile could be selected.".to_owned()
        }
        Category::Vm => "The game stopped because the emulator reported an error.".to_owned(),
        Category::Api | Category::M3g | Category::Micro3d | Category::Platform => {
            error.message().to_owned()
        }
    }
}
