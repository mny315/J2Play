use super::{
    FrontendApp, HostDecision, RichText, Screen, Vec2, egui, material_primary_button,
    material_text_button, material_tonal_button,
};
use frontend_core::ResumeBehavior;

impl FrontendApp {
    pub(super) fn draw_resume_request(
        &mut self,
        ctx: &egui::Context,
        request_id: u64,
        title: &str,
        can_resume: bool,
        detail: &str,
    ) {
        let tr = crate::i18n::Translator::from_context(ctx);
        let remember_id = egui::Id::new(("resume-choice", self.session.active_ids(), request_id));
        let mut remember =
            ctx.data_mut(|data| data.get_temp::<bool>(remember_id).unwrap_or_default());
        let mut choice = None;
        let mut cancel = false;
        let style = ctx.global_style();
        let margin = egui::Frame::window(&style).total_margin().sum().x;
        let content_width = (self.safe_content_rect.width() - 48.0).clamp(80.0, 420.0);
        super::dialogs::dialog_window_with_width(
            ctx,
            if can_resume {
                tr.text("Resume game?")
            } else {
                tr.text("Automatic save unavailable")
            },
            content_width + margin,
        )
        .id(egui::Id::new("resume-game"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .constrain_to(self.safe_content_rect)
        .show(ctx, |ui| {
            super::apply_settings_style(ui);
            ui.set_width(content_width);
            super::scrolling::vertical()
                .max_height((self.safe_content_rect.height() - 220.0).clamp(80.0, 400.0))
                .show(ui, |ui| {
                    ui.label(RichText::new(title).strong());
                    ui.add_space(8.0);
                    ui.label(tr.resume_detail(detail));
                    ui.add_space(8.0);
                    let remember =
                        ui.checkbox(&mut remember, tr.text("Remember my choice for this game"));
                    super::focus_ring::track(ui, &remember, 12.0);
                });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if can_resume
                    && ui
                        .add(material_primary_button(
                            &self.material_theme,
                            tr.text("Continue"),
                        ))
                        .clicked()
                {
                    choice = Some(true);
                }
                if ui
                    .add(material_tonal_button(
                        &self.material_theme,
                        tr.text("Start normally"),
                    ))
                    .clicked()
                {
                    choice = Some(false);
                }
                cancel = ui
                    .add(material_text_button(
                        &self.material_theme,
                        tr.text("Cancel"),
                    ))
                    .clicked();
            });
        });
        ctx.data_mut(|data| data.insert_temp(remember_id, remember));
        if cancel {
            ctx.data_mut(|data| data.remove::<bool>(remember_id));
            self.resolve_host_request(request_id, HostDecision::Deny);
        } else if let Some(continue_game) = choice {
            self.resolve_resume_request(request_id, continue_game, remember);
            if self.host_request.is_none() {
                ctx.data_mut(|data| data.remove::<bool>(remember_id));
            }
        }
    }

    pub(super) fn resolve_resume_request(
        &mut self,
        request_id: u64,
        continue_game: bool,
        remember: bool,
    ) {
        if !self.host_request.as_ref().is_some_and(|request| request.request_id == request_id
            && matches!(request.kind, super::HostRequestKind::ResumeGame { can_resume, .. } if !continue_game || can_resume)) {
            return;
        }
        if remember {
            let Screen::Gameplay(gameplay) = &self.screen else {
                return;
            };
            let Some(entry) = self
                .entries
                .iter_mut()
                .find(|entry| entry.id() == gameplay.entry_id)
            else {
                return;
            };
            let mut settings = entry.settings().clone();
            settings.resume_behavior = if continue_game {
                ResumeBehavior::Always
            } else {
                ResumeBehavior::Never
            };
            if let Err(error) = self.repository.save_settings(entry, settings) {
                self.show_error("Could not save resume preference", &error);
                return;
            }
        }
        self.resolve_host_request(
            request_id,
            HostDecision::ResumeGame {
                continue_game,
                remember,
            },
        );
    }
}
