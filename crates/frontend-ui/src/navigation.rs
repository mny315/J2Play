//! Host Back and close requests, including overlay and settings draft dismissal.

use super::{FrontendApp, HostDecision, ImportFlow, Screen, SettingsTarget, egui};

impl FrontendApp {
    pub(super) fn handle_back(&mut self, ctx: &egui::Context, fresh_input: bool) {
        if self.exit_requested {
            return;
        }
        let platform_requested = self.platform.poll_navigation_back();
        let (close_requested, browser_back_pressed) = ctx.input(|input| {
            (
                input.viewport().close_requested(),
                input.key_pressed(egui::Key::BrowserBack)
                    && !self.platform.uses_platform_navigation_back()
                    && !input
                        .raw
                        .events
                        .contains(&egui::Event::WindowFocused(false)),
            )
        });
        if close_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if !platform_requested && !(fresh_input && (close_requested || browser_back_pressed)) {
            return;
        }
        if !super::focus_navigation::close_popup(ctx) {
            self.navigate_back();
        }
    }

    pub(super) fn navigate_back(&mut self) {
        if self.exit_confirmation {
            self.exit_confirmation = false;
            return;
        }
        if self.display_error.take().is_some() {
            return;
        }
        if self.data_action.take().is_some() {
            return;
        }
        if self.library_folders.dialog.take().is_some() {
            self.library_folders.finish_editing();
            return;
        }
        if let Some(request) = self.host_request.as_ref() {
            self.resolve_host_request(request.request_id, HostDecision::Deny);
            return;
        }
        if self.heap_recovery.take().is_some() {
            let _ = self.session.discard_failed();
            self.finish_stopped_session();
            return;
        }
        if self.platform.document_browser().is_some() {
            self.platform.cancel_document_browser();
            return;
        }
        if self.import_flow.take().is_some() {
            self.discard_import_draft();
            return;
        }
        if self.fullscreen_help_visible() {
            self.acknowledge_fullscreen_help();
            return;
        }
        if self.cancel_control_editor_preparation() {
            return;
        }
        if self.physical_editor.is_some() {
            if let Some(editor) = &mut self.physical_editor
                && editor.capturing()
            {
                editor.cancel_capture();
                return;
            }
            self.close_physical_editor(false);
            return;
        }
        if let Some(editor) = self.screen.control_editor_mut()
            && editor.landscape_requested
        {
            editor.landscape_requested = false;
            return;
        }
        if let Some(editor) = self.screen.control_editor_mut()
            && editor.navigation.moving.take().is_some()
        {
            return;
        }
        if matches!(&self.screen, Screen::Gameplay(gameplay) if gameplay.fullscreen) {
            if let Err(error) = self.set_gameplay_fullscreen(false) {
                self.show_error("Could not exit fullscreen mode", &error);
            }
            return;
        }
        if matches!(self.screen, Screen::Library) && self.library_selection.active {
            self.library_selection.clear();
            return;
        }
        if matches!(self.screen, Screen::Library) && self.library_search.open {
            self.library_search.close();
            return;
        }
        if matches!(self.screen, Screen::Library)
            && self.library_search.folder != super::library_folders::FolderFilter::All
        {
            self.select_library_folder(super::library_folders::FolderFilter::All);
            return;
        }
        match std::mem::replace(&mut self.screen, Screen::Library) {
            Screen::Library => self.exit_confirmation = true,
            Screen::AppSettings(mut settings) => {
                if settings.control_editor.take().is_some() {
                    self.screen = Screen::AppSettings(settings);
                }
            }
            Screen::Settings(mut settings) => {
                if settings.control_editor.is_some() {
                    settings.discard_control_editor();
                    self.screen = Screen::Settings(settings);
                } else if let SettingsTarget::PendingImport(prepared) = settings.target {
                    self.import_flow = Some(ImportFlow::ConfirmProfile(prepared));
                }
            }
            Screen::Gameplay(gameplay) => {
                self.screen = Screen::Gameplay(gameplay);
                self.release_all_input();
                self.exit_confirmation = true;
            }
        }
    }
}
