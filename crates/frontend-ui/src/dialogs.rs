use super::{
    DataAction, DataActionKind, DisplayError, EmuError, FULLSCREEN_HELP_DASH_GAP,
    FULLSCREEN_HELP_DASH_LENGTH, FULLSCREEN_HELP_INSET, FrontendApp, HostDecision, HostRequestKind,
    RichText, Screen, SessionState, Vec2, egui, fullscreen_exit_gesture_rect, material_card_frame,
    material_danger_button, material_outlined_button, material_primary_button,
    material_supporting_text, material_text_button, material_tonal_button, profile_display_name,
};

/// Keep translated titles complete; egui's default title bar truncates them.
pub(super) fn dialog_window(
    ctx: &egui::Context,
    title: String,
    bounds: egui::Rect,
    maximum_width: f32,
) -> egui::Window<'static> {
    let width = (bounds.width() - 24.0).clamp(80.0, maximum_width);
    dialog_window_with_width(ctx, title, width)
}

/// Use the same outer width for title layout and the window's size limits.
/// Content widths exclude the window frame's margins and stroke.
pub(super) fn dialog_window_with_width(
    ctx: &egui::Context,
    title: String,
    width: f32,
) -> egui::Window<'static> {
    let style = ctx.global_style();
    let margin = egui::Frame::window(&style).total_margin().sum().x;
    let title_width = (width - margin - 16.0).max(32.0);
    let title = ctx.fonts_mut(|fonts| {
        fonts.layout(
            title,
            egui::TextStyle::Heading.resolve(&style),
            egui::Color32::PLACEHOLDER,
            title_width,
        )
    });
    egui::Window::new(egui::WidgetText::from(title))
        .default_width(width)
        .max_width(width)
}

impl FrontendApp {
    pub(super) fn show_error(&mut self, title: &str, error: &EmuError) {
        self.release_all_input();
        self.display_error = Some(DisplayError::from_emu(title, error));
    }

    pub(super) fn draw_error(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::Translator::from_context(ctx);
        let Some(error) = self.display_error.as_ref() else {
            return;
        };
        let mut close = false;
        dialog_window(
            ctx,
            tr.error_title(&error.title),
            self.safe_content_rect,
            440.0,
        )
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .constrain_to(self.safe_content_rect)
        .show(ctx, |ui| {
            ui.set_max_width(440.0_f32.min(ui.available_width()));
            super::scrolling::vertical()
                .max_height((self.safe_content_rect.height() - 160.0).clamp(0.0, 320.0))
                .show(ui, |ui| {
                    ui.label(tr.error(&error.user_message));
                    ui.collapsing(tr.text("Technical details"), |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&error.technical_details).monospace())
                                .wrap(),
                        );
                    });
                });
            ui.add_space(8.0);
            close = ui
                .add(material_primary_button(
                    &self.material_theme,
                    tr.text("Close"),
                ))
                .clicked();
        });
        if close {
            self.display_error = None;
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn draw_host_request(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::Translator::from_context(ctx);
        let Some(request) = self.host_request.clone() else {
            return;
        };
        let (title, prompt, value) = match &request.kind {
            HostRequestKind::Network { url } => (
                tr.text("Network request"),
                "Allow this game to connect to this address?",
                url.as_str(),
            ),
            HostRequestKind::FileConnection { url } => (
                tr.text("File access request"),
                "Allow this game to access its private file area?",
                url.as_str(),
            ),
            HostRequestKind::PlatformRequest { url } => (
                tr.text("Open external link"),
                "This game wants to open an external address.",
                url.as_str(),
            ),
            HostRequestKind::ResumeGame {
                title,
                can_resume,
                detail,
            } => {
                self.draw_resume_request(ctx, request.request_id, title, *can_resume, detail);
                return;
            }
        };
        let mut decision = None;
        let mut open_now = false;
        let mut open_after_stop = false;
        dialog_window(ctx, title, self.safe_content_rect, 460.0)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .constrain_to(self.safe_content_rect)
            .show(ctx, |ui| {
                ui.set_max_width(460.0_f32.min(ui.available_width()));
                ui.label(tr.text(prompt));
                material_card_frame(&self.material_theme).show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(value).monospace()).wrap());
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(material_text_button(&self.material_theme, tr.text("Deny")))
                        .clicked()
                    {
                        decision = Some(HostDecision::Deny);
                    }
                    if matches!(request.kind, HostRequestKind::PlatformRequest { .. }) {
                        open_now = ui
                            .add(material_primary_button(
                                &self.material_theme,
                                tr.text("Open now"),
                            ))
                            .clicked();
                        open_after_stop = ui
                            .add(material_tonal_button(
                                &self.material_theme,
                                tr.text("Open after stopping game"),
                            ))
                            .clicked();
                    } else {
                        if ui
                            .add(material_primary_button(
                                &self.material_theme,
                                tr.text("Allow once"),
                            ))
                            .clicked()
                        {
                            decision = Some(HostDecision::AllowOnce);
                        }
                        if ui
                            .add(material_tonal_button(
                                &self.material_theme,
                                tr.text("Allow for this session"),
                            ))
                            .clicked()
                        {
                            decision = Some(HostDecision::AllowForSession);
                        }
                    }
                });
                ui.label(material_supporting_text(
                    &self.material_theme,
                    tr.text("The request is denied automatically if it times out."),
                ));
            });
        if open_now {
            let HostRequestKind::PlatformRequest { url } = request.kind else {
                return;
            };
            match self.platform.open_external_url(&url) {
                Ok(()) => decision = Some(HostDecision::AllowOnce),
                Err(error) => {
                    decision = Some(HostDecision::Deny);
                    self.show_error("Could not open link", &error);
                }
            }
        } else if open_after_stop {
            let HostRequestKind::PlatformRequest { url } = request.kind else {
                return;
            };
            self.external_after_stop = Some(url);
            decision = Some(HostDecision::AllowAfterExit);
        }
        if let Some(decision) = decision {
            self.resolve_host_request(request.request_id, decision);
            if decision == HostDecision::AllowAfterExit {
                self.stop_active_session();
            }
        }
    }

    pub(super) fn resolve_host_request(&mut self, request_id: u64, decision: HostDecision) {
        let command = match self.session.resolve_host_request(request_id, decision) {
            Ok(command) => command,
            Err(error) => {
                self.show_error("Could not answer request", &error);
                return;
            }
        };
        if let Err(error) = self.runtime.submit(command) {
            self.show_error("Could not answer request", &error);
        } else if self
            .host_request
            .as_ref()
            .is_some_and(|request| request.request_id == request_id)
        {
            self.host_request = None;
        }
    }

    pub(super) fn draw_heap_recovery(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::Translator::from_context(ctx);
        let Some(recovery) = self.heap_recovery.as_ref() else {
            return;
        };
        let mut selected = None;
        let mut settings = false;
        let mut cancel = false;
        dialog_window(ctx, tr.text("Game needs more memory"), self.safe_content_rect, 460.0)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .constrain_to(self.safe_content_rect)
            .show(ctx, |ui| {
                ui.set_max_width(460.0_f32.min(ui.available_width()));
                super::scrolling::vertical()
                    .max_height((self.safe_content_rect.height() - 210.0).clamp(0.0, 320.0))
                    .show(ui, |ui| {
                        ui.label(tr.text("The automatic profile reached its managed-heap limit. You can retry with an exact compatible profile."));
                        ui.collapsing(tr.text("Technical details"), |ui| {
                            ui.add(egui::Label::new(RichText::new(&recovery.detail).monospace()).wrap());
                        });
                        if recovery.candidate_profile_ids.is_empty() {
                            ui.label(tr.text("No higher-capacity profile with the same Canvas was found."));
                        } else {
                            for profile_id in &recovery.candidate_profile_ids {
                                let profile_name = profile_display_name(
                                    &self.profile_options,
                                    profile_id,
                                )
                                .unwrap_or("Unavailable device profile");
                                if ui
                                    .add_sized(
                                        [ui.available_width(), 48.0],
                                        material_outlined_button(
                                            &self.material_theme,
                                            tr.format("Retry with {profile}", &[("profile", &tr.profile_name(profile_name))]),
                                        ),
                                    )
                                    .clicked()
                                {
                                    selected = Some(profile_id.clone());
                                }
                            }
                        }
                    });
                settings = ui
                    .add(material_outlined_button(
                        &self.material_theme,
                        tr.text("Game settings"),
                    ))
                    .clicked();
                cancel = ui
                    .add(material_text_button(
                        &self.material_theme,
                        tr.text("Return to library"),
                    ))
                    .clicked();
            });
        if let Some(profile_id) = selected {
            self.retry_with_profile(&profile_id);
        } else if settings {
            let entry_id = match &self.screen {
                Screen::Gameplay(gameplay) => Some(gameplay.entry_id.clone()),
                _ => None,
            };
            let _ = self.session.discard_failed();
            self.finish_stopped_session();
            if let Some(entry_id) = entry_id {
                self.open_settings(&entry_id, true);
            }
        } else if cancel {
            let _ = self.session.discard_failed();
            self.finish_stopped_session();
        }
    }

    pub(super) fn draw_data_action_confirmation(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::Translator::from_context(ctx);
        let Some(action) = self.data_action.clone() else {
            return;
        };
        let (title, explanation, confirm_label) = match action.kind {
            DataActionKind::EntryRecord => (
                tr.text("Remove library entry?"),
                "This removes only the library listing and its settings. The private JAR/JAD and RMS/saves remain on this device.",
                "Remove entry",
            ),
            DataActionKind::PrivateArchives => (
                tr.text("Delete private JAR/JAD copy?"),
                "This removes only the imported archive copy. The library entry and RMS/saves remain, but the game cannot start until it is imported again.",
                "Delete JAR/JAD",
            ),
            DataActionKind::RuntimeData => (
                tr.text("Delete RMS and saves?"),
                "This permanently removes this game's RMS, FileConnection data, and automatic resume save. The library entry and private JAR/JAD remain.",
                "Delete RMS and saves",
            ),
            DataActionKind::ResumeSave => (
                tr.text("Clear save state?"),
                "This removes the saved game state used to resume the game. In-game saves and settings will be kept.",
                "Clear save state",
            ),
            DataActionKind::Game => (
                tr.text("Delete selected games?"),
                "This permanently deletes the selected games from the library, their imported JAR/JAD copies, settings, all in-game saves and save states. Original files outside the library are kept.",
                "Delete games",
            ),
        };
        let mut confirm = false;
        let mut cancel = false;
        dialog_window(ctx, title, self.safe_content_rect, 440.0)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .constrain_to(self.safe_content_rect)
            .show(ctx, |ui| {
                ui.set_max_width(440.0_f32.min(ui.available_width()));
                if action.entry_ids.len() == 1 {
                    if let Some(entry) = self
                        .entries
                        .iter()
                        .find(|entry| entry.id() == action.entry_ids[0])
                    {
                        ui.label(tr.format("Game: {title}", &[("title", entry.title())]));
                    }
                } else {
                    ui.label(tr.format(
                        "Selected: {count}",
                        &[("count", &action.entry_ids.len().to_string())],
                    ));
                }
                ui.label(tr.text(explanation));
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    confirm = ui
                        .add(material_danger_button(
                            &self.material_theme,
                            tr.text(confirm_label),
                        ))
                        .clicked();
                    cancel = ui
                        .add(material_text_button(
                            &self.material_theme,
                            tr.text("Cancel"),
                        ))
                        .clicked();
                });
            });
        if cancel {
            self.data_action = None;
        } else if confirm {
            self.execute_data_action(&action);
        }
    }

    pub(super) fn execute_data_action(&mut self, action: &DataAction) {
        if self.session.state() != SessionState::Idle {
            return;
        }
        self.data_action = None;
        for id in &action.entry_ids {
            if let Err(error) = self.delete_entry_data(id, action.kind) {
                self.show_error("Could not delete app data", &error);
                return;
            }
        }
        if action.kind == DataActionKind::Game {
            self.library_selection.clear();
        }
    }

    fn delete_entry_data(&mut self, id: &str, kind: DataActionKind) -> Result<(), EmuError> {
        let Some(index) = self.entries.iter().position(|entry| entry.id() == id) else {
            self.library_selection.ids.remove(id);
            return Ok(());
        };
        let entry = &self.entries[index];
        match kind {
            DataActionKind::EntryRecord => self.repository.delete_entry_record(entry),
            DataActionKind::PrivateArchives => self.repository.delete_private_archives(entry),
            DataActionKind::RuntimeData => self.repository.delete_runtime_data(entry),
            DataActionKind::ResumeSave => self.repository.delete_resume_save(entry),
            DataActionKind::Game => self.repository.delete_game(entry),
        }?;
        if matches!(kind, DataActionKind::EntryRecord | DataActionKind::Game) {
            self.icons.remove(id);
            self.library_assets.invalidate();
            self.entries.remove(index);
            self.library_selection.ids.remove(id);
            self.library_motion.invalidate_entries();
            self.library_search.invalidate();
            self.screen = Screen::Library;
        } else if kind == DataActionKind::PrivateArchives {
            self.icons.remove(id);
            self.library_assets.invalidate();
        }
        Ok(())
    }

    pub(super) fn draw_fullscreen_help(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::Translator::from_context(ctx);
        if !self.fullscreen_help_visible() {
            return;
        }
        let gesture_rect = fullscreen_exit_gesture_rect(self.safe_content_rect);
        let outline = gesture_rect.shrink(FULLSCREEN_HELP_INSET);
        let mut help_rect = self.safe_content_rect.shrink(FULLSCREEN_HELP_INSET);
        help_rect.min.y = (gesture_rect.bottom() + FULLSCREEN_HELP_INSET).min(help_rect.bottom());
        let path = [
            outline.left_top(),
            outline.right_top(),
            outline.right_bottom(),
            outline.left_bottom(),
            outline.left_top(),
        ];
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("fullscreen-exit-gesture-outline"),
        ));
        painter.extend(egui::Shape::dashed_line(
            &path,
            egui::Stroke::new(2.0, self.material_theme.primary),
            FULLSCREEN_HELP_DASH_LENGTH,
            FULLSCREEN_HELP_DASH_GAP,
        ));

        let mut acknowledged = false;
        dialog_window(ctx, tr.text("Fullscreen controls"), help_rect, 320.0)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .anchor(egui::Align2::CENTER_TOP, Vec2::ZERO)
            .constrain_to(help_rect)
            .show(ctx, |ui| {
                ui.set_max_width(320.0_f32.min(ui.available_width()));
                ui.label(
                    tr.text("To exit fullscreen, double-tap inside the dashed area at the top of the screen."),
                );
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    acknowledged = ui
                        .add(
                            material_primary_button(&self.material_theme, tr.text("OK"))
                                .min_size(Vec2::new(88.0, 48.0)),
                        )
                        .clicked();
                });
            });
        if acknowledged {
            self.acknowledge_fullscreen_help();
        }
    }

    pub(super) fn draw_exit_confirmation(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::Translator::from_context(ctx);
        if !self.exit_confirmation {
            return;
        }
        let mut exit = false;
        dialog_window(ctx, tr.text("Exit J2Play?"), self.safe_content_rect, 400.0)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .constrain_to(self.safe_content_rect)
            .show(ctx, |ui| {
                ui.set_max_width(400.0_f32.min(ui.available_width()));
                ui.label(tr.text("Any running game will be stopped safely."));
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(material_danger_button(
                            &self.material_theme,
                            tr.text("Exit"),
                        ))
                        .clicked()
                    {
                        exit = true;
                    }
                    if ui
                        .add(material_text_button(
                            &self.material_theme,
                            tr.text("Cancel"),
                        ))
                        .clicked()
                    {
                        self.exit_confirmation = false;
                    }
                });
            });
        if exit {
            self.confirm_exit(ctx);
        }
    }

    pub(super) fn confirm_exit(&mut self, ctx: &egui::Context) {
        self.platform.cancel_pending_operations();
        self.clear_gameplay_input_geometry();
        self.host_request = None;
        // Worker shutdown tears down the guest without the explicit Stop save.
        match self.runtime.shutdown().and(self.platform.shutdown()) {
            Ok(()) => {
                self.exit_requested = true;
                self.exit_confirmation = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(error) => {
                self.exit_confirmation = false;
                self.show_error("Could not close J2Play", &error);
            }
        }
    }
}
