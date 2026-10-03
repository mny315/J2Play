use super::{
    Arc, Color32, DisplayError, Frame, FrontendApp, HeapRecovery, Instant,
    MAX_UI_TECHNICAL_ERROR_BYTES, Screen, SessionEventKind, SessionState, bounded_ui_text, egui,
};

impl FrontendApp {
    pub(super) fn process_runtime_events(&mut self, ctx: &egui::Context) {
        let events = match self.runtime.poll_events() {
            Ok(events) => events,
            Err(error) => {
                if let Some((session_id, attempt_id)) = self.session.active_ids() {
                    self.session
                        .apply_event(&super::SessionEvent::terminal_error(
                            session_id,
                            attempt_id,
                            "The emulator worker stopped.",
                            error.message(),
                        ));
                    let _ = self.session.discard_failed();
                    self.finish_stopped_session();
                }
                self.show_error("Emulator communication failed", &error);
                return;
            }
        };
        for event in events {
            self.process_runtime_event(ctx, event);
        }
    }

    pub(super) fn process_runtime_event(
        &mut self,
        ctx: &egui::Context,
        event: super::SessionEvent,
    ) {
        if !self.session.apply_event(&event) {
            return;
        }
        match event.kind {
            SessionEventKind::LaunchPrepared(plan) => self.apply_launch_preparation(&plan),
            SessionEventKind::Started => ctx.request_repaint(),
            SessionEventKind::Stopped => self.finish_stopped_session(),
            SessionEventKind::FrameAvailable => match self.runtime.frames().take_latest() {
                Ok(Some(frame)) => self.accept_frame(ctx, frame),
                Ok(None) => {}
                Err(error) => self.show_error("Could not present game", &error),
            },
            SessionEventKind::TextInputActive { active } => {
                self.set_platform_text_input(active);
            }
            SessionEventKind::HostRequest(request) => {
                self.release_all_input();
                self.host_request = Some(request);
            }
            SessionEventKind::HostRequestClosed { request_id } => {
                if self
                    .host_request
                    .as_ref()
                    .is_some_and(|request| request.request_id == request_id)
                {
                    self.host_request = None;
                }
            }
            SessionEventKind::PlatformRequestCancelled => {
                self.external_after_stop = None;
                self.platform.cancel_guest_operations();
            }
            SessionEventKind::CheckpointSaveFailed { detail } => {
                self.release_all_input();
                self.display_error = Some(DisplayError::runtime(
                    "Could not save the game state before stopping. Existing save files are kept.",
                    &detail,
                ));
            }
            SessionEventKind::Diagnostic {
                code,
                detail,
                repeated,
            } => {
                if self.runtime_diagnostics.len() == 8 {
                    self.runtime_diagnostics.pop_front();
                }
                self.runtime_diagnostics.push_back(if repeated > 1 {
                    format!("{code} ({repeated} times): {detail}")
                } else {
                    format!("{code}: {detail}")
                });
            }
            SessionEventKind::ManagedHeapLimit {
                detail,
                candidate_profile_ids,
            } => {
                self.release_all_input();
                self.heap_recovery = Some(HeapRecovery {
                    detail,
                    candidate_profile_ids,
                });
            }
            SessionEventKind::TerminalError {
                user_message,
                technical_details,
            } => {
                self.display_error = Some(DisplayError::runtime(&user_message, &technical_details));
                let _ = self.session.discard_failed();
                self.finish_stopped_session();
            }
        }
    }

    pub(super) fn process_runtime_telemetry(&mut self) {
        if !self.runtime_debug.enabled {
            return;
        }
        match self.runtime.telemetry().take_latest() {
            Ok(Some(telemetry))
                if self.session.active_ids()
                    == Some((telemetry.session_id, telemetry.attempt_id)) =>
            {
                self.runtime_debug.note_vm(Instant::now(), telemetry.stats);
            }
            Ok(Some(_) | None) => {}
            Err(error) => self.show_error("Could not read debug telemetry", &error),
        }
    }

    pub(super) fn accept_frame(&mut self, ctx: &egui::Context, frame: Arc<Frame>) {
        if self.session.active_ids() != Some((frame.session_id, frame.attempt_id))
            || matches!(
                self.session.state(),
                SessionState::Stopping | SessionState::Failed
            )
        {
            return;
        }
        if self.game_texture.is_some() && self.latest_frame.as_deref() == Some(frame.as_ref()) {
            // The same frame still counts as a delivery, but it does not
            // require another color conversion or GPU texture upload.
            self.runtime_debug.note_frame(Instant::now());
            return;
        }
        if self.latest_frame.as_ref().is_none_or(|previous| {
            (previous.width, previous.height, previous.canvas_region)
                != (frame.width, frame.height, frame.canvas_region)
        }) {
            // Guest fullscreen/chrome changes can remap Canvas coordinates and
            // resize the keypad without a host window or inset change.
            self.clear_gameplay_input_geometry();
        }
        let dimensions = [frame.width as usize, frame.height as usize];
        let pixels = frame_colors(&frame.pixels);
        let image = egui::ColorImage::new(dimensions, pixels);
        if let Some(texture) = &mut self.game_texture {
            texture.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.game_texture =
                Some(ctx.load_texture("j2play-game-frame", image, egui::TextureOptions::NEAREST));
        }
        self.latest_frame = Some(frame);
        self.runtime_debug.note_frame(Instant::now());
        // The runtime event woke this logic pass; its texture is ready for
        // the ensuing paint. Requesting another paint here redraws it twice.
    }

    pub(super) fn finish_stopped_session(&mut self) {
        self.fullscreen_policy = None;
        self.gameplay_transition = super::gameplay_transition::GameplayTransition::default();
        let fullscreen_cleanup_error = self.set_gameplay_fullscreen(false).err();
        let orientation_cleanup_error = self.restore_system_orientation().err();
        self.set_platform_text_input(false);
        self.latest_frame = None;
        self.game_texture = None;
        self.canvas_rect = None;
        self.control_regions.clear();
        self.stick_region = None;
        self.touch_owners.clear();
        self.host_request = None;
        self.heap_recovery = None;
        self.screen = Screen::Library;
        for (title, code, error) in [
            (
                "Could not exit fullscreen mode",
                "fullscreen-cleanup",
                fullscreen_cleanup_error,
            ),
            (
                "Could not restore screen rotation",
                "orientation-cleanup",
                orientation_cleanup_error,
            ),
        ] {
            let Some(error) = error else { continue };
            if let Some(display_error) = &mut self.display_error {
                display_error.technical_details = bounded_ui_text(
                    &format!(
                        "{}\nplatform[{code}]: {}",
                        display_error.technical_details,
                        error.message()
                    ),
                    MAX_UI_TECHNICAL_ERROR_BYTES,
                );
            } else {
                self.show_error(title, &error);
            }
        }
        if let Some(url) = self.external_after_stop.take()
            && let Err(error) = self.platform.open_external_url(&url)
        {
            self.show_error("Could not open link", &error);
        }
    }
}

fn frame_colors(pixels: &[u32]) -> Vec<Color32> {
    // Convert the opaque prefix without per-pixel alpha handling. Keep that
    // work even when a transparent pixel occurs near the end of the frame.
    let opaque_count = pixels
        .iter()
        .position(|pixel| pixel >> 24 != 255)
        .unwrap_or(pixels.len());
    let (opaque, remaining) = pixels.split_at(opaque_count);
    let mut output = Vec::with_capacity(pixels.len());
    output.extend(opaque.iter().map(|pixel| {
        let [_, red, green, blue] = pixel.to_be_bytes();
        Color32::from_rgb(red, green, blue)
    }));
    output.extend(remaining.iter().map(|pixel| {
        let [alpha, red, green, blue] = pixel.to_be_bytes();
        Color32::from_rgba_unmultiplied(red, green, blue, alpha)
    }));
    output
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/runtime_events/frame_colors.rs"]
mod frame_color_tests;
