use super::settings_actions::SettingsActions;
use super::{
    Duration, FrontendApp, Screen, SessionState, apply_material_theme, egui, gameplay_panel_margin,
    inset_content_rect, material_bottom_bar_frame, paint_material_scrim,
};

pub(super) fn repaint_for_runtime_event(ctx: &egui::Context) {
    // egui's zero-delay request schedules two paints for layout settling.
    // Runtime events already carry complete state; a nonzero deadline requests
    // one paint. egui subtracts its predicted frame time from this deadline.
    ctx.request_repaint_after(Duration::from_millis(1));
}

// eframe can repeat logic during layout passes, and its hidden-window logic
// retains the previous frame's input. Neither is a new physical input batch.
pub(super) fn take_native_input(ctx: &egui::Context, last_time: &mut Option<u64>) -> bool {
    if ctx.current_pass_index() != 0 {
        return false;
    }
    let time = ctx.input(|input| input.time.to_bits());
    if *last_time == Some(time) {
        return false;
    }
    *last_time = Some(time);
    true
}

fn content_builder(app: &FrontendApp) -> egui::UiBuilder {
    let content = egui::UiBuilder::new().max_rect(app.safe_content_rect);
    if app
        .physical_editor
        .as_ref()
        .is_some_and(super::physical_controls::PhysicalEditor::capturing)
    {
        // Drop background IDs from egui's directional-focus cache while the
        // assignment picker is open, so arrows stay inside its modal layer.
        content.id_salt("physical-picker-background").disabled()
    } else {
        content
    }
}

impl eframe::App for FrontendApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.exit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.platform.close_exits_app() && ctx.input(|input| input.viewport().close_requested())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_exit(ctx);
            return;
        }
        if !self.runtime_waker_bound {
            let repaint_context = ctx.clone();
            let result = self
                .runtime
                .set_event_waker(move || repaint_for_runtime_event(&repaint_context));
            self.runtime_waker_bound = true;
            let input_context = ctx.clone();
            self.platform
                .set_physical_input_waker(std::sync::Arc::new(move || {
                    repaint_for_runtime_event(&input_context);
                }));
            if let Err(error) = result {
                self.show_error("Emulator communication failed", &error);
            }
        }
        crate::i18n::install_scaled(
            ctx,
            self.app_settings
                .language
                .unwrap_or_else(|| self.platform.system_language()),
            self.platform.system_text_scale(),
        );
        self.process_platform_theme(ctx);
        ctx.set_zoom_factor(
            super::BASE_UI_ZOOM * f32::from(self.app_settings.ui_scale_percent) / 100.0,
        );
        apply_material_theme(ctx, &self.material_theme);
        if self
            .startup_splash
            .as_mut()
            .is_some_and(|splash| splash.update(ctx))
        {
            return;
        }
        self.startup_splash = None;
        if let Err(error) = self.platform.pump_runtime_effects() {
            self.show_error("Platform output failed", &error);
        }
        self.process_platform_insets();
        self.process_lifecycle_events();
        self.process_runtime_events(ctx);
        self.process_runtime_telemetry();
        self.process_platform_events();
        self.process_control_editor_preparation(ctx);
        self.process_platform_text_input(ctx);
        self.process_platform_fullscreen();
        self.sync_gameplay_fullscreen_policy(ctx.viewport_rect());
        let fresh_input = take_native_input(ctx, &mut self.last_native_input_time);
        let content = inset_content_rect(
            ctx.content_rect(),
            self.platform_insets,
            ctx.pixels_per_point(),
        );
        let geometry_changed = self.invalidate_gameplay_geometry(content);
        let allow_game =
            !(geometry_changed || fresh_input && self.process_fullscreen_exit_gesture(ctx));
        if fresh_input && allow_game {
            self.process_game_input(ctx);
        }
        self.handle_back(ctx, fresh_input);
        self.process_physical_inputs(ctx, fresh_input, allow_game);
        self.configure_physical_input();
        if !self.platform_suspended {
            if self.gameplay_transition.busy() {
                ctx.request_repaint();
            }
            match self.session.state() {
                SessionState::Starting | SessionState::Stopping => {
                    ctx.request_repaint();
                }
                SessionState::Running if self.platform.needs_periodic_poll() => {
                    // Runtime events wake egui immediately. This slower poll is
                    // only a fallback for platform adapters whose asynchronous
                    // signals do not enter the native window event loop.
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
                SessionState::Idle | SessionState::Paused { .. } | SessionState::Failed
                    if self.picker_pending || self.platform.needs_periodic_poll() =>
                {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
                SessionState::Running
                | SessionState::Idle
                | SessionState::Paused { .. }
                | SessionState::Failed => {}
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        crate::i18n::warm_language_menu(ui.ctx());
        if let Some(splash) = &mut self.startup_splash {
            splash.paint(ui, self.material_theme.background);
            return;
        }
        self.safe_content_rect = inset_content_rect(
            ui.max_rect(),
            self.platform_insets,
            ui.ctx().pixels_per_point(),
        );
        let opacity = self.sync_ui_motion(ui.ctx());
        let content_enabled = !self.overlay_active();
        ui.scope_builder(content_builder(self), |ui| {
            ui.multiply_opacity(opacity);
            match &self.screen {
                Screen::Library => {
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.add_enabled_ui(content_enabled, |ui| {
                            // The scrim owns dimming. Blocking the folder background must
                            // not abruptly halve its opacity on top of that animation.
                            if self.library_folders.dialog.is_some() {
                                ui.set_opacity(opacity);
                            }
                            self.draw_library(ui);
                        });
                    });
                }
                Screen::AppSettings(_) | Screen::Settings(_) => {
                    self.draw_settings_page(ui, content_enabled);
                }
                Screen::Gameplay(gameplay) => {
                    let fullscreen = gameplay.fullscreen;
                    let frame = egui::Frame::central_panel(ui.style())
                        .inner_margin(gameplay_panel_margin(fullscreen));
                    egui::CentralPanel::default().frame(frame).show(ui, |ui| {
                        ui.add_enabled_ui(content_enabled, |ui| self.draw_gameplay(ui));
                    });
                }
            }
        });
        paint_material_scrim(ui, self.safe_content_rect, self.overlay_active());
        if self.exit_confirmation {
            self.draw_exit_confirmation(ui.ctx());
        } else if self.display_error.is_some() {
            self.draw_error(ui.ctx());
        } else if self.data_action.is_some() {
            self.draw_data_action_confirmation(ui.ctx());
        } else if self.library_folders.dialog.is_some() {
            self.draw_folder_dialog(ui.ctx());
        } else if self.host_request.is_some() {
            self.draw_host_request(ui.ctx());
        } else if self.heap_recovery.is_some() {
            self.draw_heap_recovery(ui.ctx());
        } else if self.platform.document_browser().is_some() {
            self.draw_document_browser(ui.ctx());
        } else if self.import_flow.is_some() {
            self.draw_import_flow(ui.ctx());
        } else if self.fullscreen_help_visible() {
            self.draw_fullscreen_help(ui.ctx());
        } else if self.control_editor_preparing() {
            self.draw_control_editor_preparation(ui.ctx());
        }
        self.repaint_changed_page(ui.ctx());
        self.draw_physical_capture(ui.ctx());
        if !self.platform_suspended
            && (!matches!(self.screen, Screen::Gameplay(_)) || self.overlay_active())
        {
            self.focus_ring.restore_controller_focus(ui.ctx());
        }
        self.focus_ring.paint(
            ui.ctx(),
            &self.material_theme,
            !self.platform_suspended,
            self.safe_content_rect,
        );
        self.configure_physical_input();
        if self.platform.has_native_text_input() {
            self.library_folders.editing = self.folder_dialog_editable()
                && ui.memory(egui::Memory::focused)
                    == Some(super::library_folders::name_field_id());
            self.library_search.editing = self.library_search.open
                && matches!(self.screen, Screen::Library)
                && !self.overlay_active()
                && ui.memory(egui::Memory::focused) == Some(super::library_search::field_id());
            // The shell's Rust IME adapter owns visibility; winit's generic
            // NativeActivity keyboard must not compete with that editor.
            ui.ctx().output_mut(|output| output.ime = None);
            self.sync_platform_text_input();
        }
    }

    fn on_exit(&mut self) {
        self.platform.cancel_pending_operations();
        self.release_all_input();
        if let Err(error) = self.restore_system_orientation() {
            eprintln!("j2play: platform[orientation-cleanup]: {error}");
        }
        if let Err(error) = self.runtime.shutdown() {
            eprintln!("j2play: platform[shutdown-on-exit]: {error}");
        }
        if let Err(error) = self.library_assets.shutdown() {
            eprintln!("j2play: platform[library-assets-shutdown]: {error}");
        }
        if let Err(error) = self.editor_preparation.shutdown() {
            eprintln!("j2play: platform[control-editor-shutdown]: {error}");
        }
        if let Err(error) = self.platform.shutdown() {
            eprintln!("j2play: platform[shutdown-on-exit]: {error}");
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.material_theme.background.to_normalized_gamma_f32()
    }
}

impl FrontendApp {
    fn draw_settings_page(&mut self, ui: &mut egui::Ui, content_enabled: bool) {
        let app_settings = matches!(self.screen, Screen::AppSettings(_));
        let editing_controls = self.screen.control_editor_mut().is_some();
        let editing_physical = self.physical_editor.is_some();
        let page = self.motion_page();
        let frame = material_bottom_bar_frame(&self.material_theme);
        let margin = frame.total_margin().sum();
        let actions = SettingsActions::new(
            ui,
            ui.available_width() - margin.x,
            editing_controls || editing_physical,
        );
        let mut action = None;
        egui::Panel::bottom(if app_settings {
            "app-settings-actions"
        } else {
            "game-settings-actions"
        })
        .exact_size(actions.height() + margin.y)
        .frame(frame)
        .show(ui, |ui| {
            ui.add_enabled_ui(content_enabled, |ui| {
                action = self.draw_settings_actions(ui, &actions);
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            super::scrolling::vertical()
                .id_salt(page)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_enabled_ui(content_enabled, |ui| {
                        if editing_physical {
                            self.draw_physical_editor(ui);
                        } else if editing_controls {
                            self.draw_control_editor(ui);
                        } else if app_settings {
                            self.draw_app_settings(ui);
                        } else {
                            self.draw_settings(ui);
                        }
                    });
                });
        });
        // Finish painting the page that received the click before navigation.
        // The destination starts its fade on its first complete paint.
        if action.is_some() {
            ui.ctx().request_repaint();
        }
        self.apply_settings_action(action);
    }

    pub(super) fn invalidate_gameplay_geometry(&mut self, content: egui::Rect) -> bool {
        if content == self.safe_content_rect {
            return false;
        }
        // Insets, resizing and zoom can move targets without rotating the window.
        // Cancel before handling End, which can otherwise persist an obsolete drag.
        if (content.width() > content.height())
            != (self.safe_content_rect.width() > self.safe_content_rect.height())
        {
            self.gameplay_transition.request();
        }
        self.clear_gameplay_input_geometry();
        true
    }
}
