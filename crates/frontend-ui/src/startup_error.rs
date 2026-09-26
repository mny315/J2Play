use crate::{egui, i18n};
use frontend_core::Language;

/// Shared fallback when the main frontend cannot initialize.
pub struct StartupErrorApp {
    message: String,
    language: Language,
    text_scale: crate::PlatformTextScale,
}

impl StartupErrorApp {
    pub fn set_text_scale(&mut self, scale: crate::PlatformTextScale) {
        self.text_scale = scale;
    }

    /// Update the system language when platform initialization completes.
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    #[must_use]
    pub fn new(message: &str, language: Language) -> Self {
        Self {
            message: diagnostics::bounded_text(message, 2048),
            language,
            text_scale: crate::PlatformTextScale::default(),
        }
    }
}

impl eframe::App for StartupErrorApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_zoom_factor(crate::BASE_UI_ZOOM);
        i18n::install_scaled(ctx, self.language, self.text_scale);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let tr = i18n::Translator(self.language);
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(tr.text("J2Play could not start"));
            ui.add_space(12.0);
            ui.label(tr.error(&self.message));
            crate::scrolling::vertical()
                .max_height((ui.available_height() - 64.0).max(0.0))
                .show(ui, |ui| {
                    ui.collapsing(tr.text("Technical details"), |ui| {
                        ui.add(egui::Label::new(&self.message).wrap());
                    });
                });
            ui.add_space(12.0);
            if ui.button(tr.text("Exit")).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}

/// Localized title and explanation for a shell that cannot initialize graphics.
#[must_use]
pub fn startup_error_text(language: Language, message: &str) -> (String, String) {
    let tr = i18n::Translator(language);
    (
        tr.text("J2Play could not start"),
        format!(
            "{}\n\n{}\n{}",
            tr.error(message),
            tr.text("Technical details"),
            diagnostics::bounded_text(message, 2048)
        ),
    )
}
