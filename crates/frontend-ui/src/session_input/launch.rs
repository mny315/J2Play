use crate::{FrontendApp, GameplayScreen, Screen, SessionEvent, SessionState};
use frontend_core::{LaunchPlan, ProfileChoice};

impl FrontendApp {
    pub(crate) fn open_game(&mut self, entry_id: &str) {
        self.open_game_with_orientation(entry_id, None);
    }

    pub(crate) fn open_game_with_orientation(
        &mut self,
        entry_id: &str,
        orientation: Option<launch::CanvasOrientation>,
    ) {
        let Some(entry) = self.entries.iter().find(|entry| entry.id() == entry_id) else {
            return;
        };
        // A display hint lays out the loading screen. Only the worker's fresh
        // launch plan enables guest pointer input and establishes its Canvas.
        let mut dimensions = entry
            .automatic_canvas_dimensions(&self.catalog_fingerprint)
            .or_else(|| {
                let id = entry.effective_profile_id(&self.catalog_fingerprint)?;
                self.profile_options
                    .iter()
                    .find(|option| option.profile_id == id)?
                    .canvas_dimensions
            })
            .unwrap_or((240, 320));
        if matches!(
            (orientation, dimensions.0.cmp(&dimensions.1)),
            (
                Some(launch::CanvasOrientation::Portrait),
                std::cmp::Ordering::Greater
            ) | (
                Some(launch::CanvasOrientation::Landscape),
                std::cmp::Ordering::Less
            )
        ) {
            dimensions = (dimensions.1, dimensions.0);
        }
        let mut effective_settings = entry.settings().clone();
        self.app_settings
            .apply_control_defaults(&mut effective_settings);
        let gameplay = GameplayScreen {
            entry_id: entry_id.to_owned(),
            title: entry.title().to_owned(),
            canvas_dimensions: dimensions,
            pointer_events: false,
            game_scale: effective_settings.game_scale,
            portrait_frame_percent: effective_settings.portrait_frame_percent,
            control_layout: effective_settings.control_layout,
            landscape_control_layout: effective_settings.landscape_control_layout,
            vibration: effective_settings.vibration,
            orientation,
            fast_forward: false,
            fullscreen: false,
        };
        let command = match self.session.start_with_orientation(entry_id, orientation) {
            Ok(command) => command,
            Err(error) => {
                self.show_error("Could not start game", &error);
                return;
            }
        };
        if let Err(error) = self.runtime.submit(command) {
            if let Some((session_id, attempt_id)) = self.session.active_ids() {
                let failed = SessionEvent::terminal_error(
                    session_id,
                    attempt_id,
                    "The emulator session could not be started.",
                    error.message(),
                );
                let _ = self.session.apply_event(&failed);
                let _ = self.session.discard_failed();
            }
            self.show_error("Could not start game", &error);
            return;
        }
        self.release_all_input();
        self.physical_bindings = effective_settings
            .physical_bindings
            .as_ref()
            .unwrap_or(&self.app_settings.physical_bindings)
            .clone();
        self.latest_frame = None;
        self.game_texture = None;
        self.canvas_rect = None;
        self.control_regions.clear();
        self.stick_region = None;
        self.touch_owners.clear();
        self.gameplay_transition = crate::gameplay_transition::GameplayTransition::default();
        self.host_request = None;
        self.heap_recovery = None;
        self.runtime_diagnostics.clear();
        self.runtime_debug.reset_session();
        self.fullscreen_policy = Some(crate::fullscreen::GameplayFullscreenPolicy::new(
            self.app_settings
                .effective_fullscreen(effective_settings.fullscreen),
        ));
        self.screen = Screen::Gameplay(Box::new(gameplay));
    }

    pub(crate) fn apply_launch_preparation(&mut self, plan: &LaunchPlan) {
        if matches!(
            self.session.state(),
            SessionState::Stopping | SessionState::Failed
        ) {
            return;
        }
        let Screen::Gameplay(gameplay) = &mut self.screen else {
            return;
        };
        gameplay.canvas_dimensions = plan.decision.selection().canvas_dimensions();
        gameplay.pointer_events = plan.pointer_events;
        if gameplay.orientation.is_none()
            && let Some(entry) = self
                .entries
                .iter_mut()
                .find(|entry| entry.id() == gameplay.entry_id)
            && let Err(error) = self.repository.cache_launch_resolution(entry, plan)
        {
            let warning = format!(
                "{}: could not refresh profile cache: {}",
                entry.id(),
                error.message(),
            );
            self.push_library_warning(&warning);
        }
        self.clear_gameplay_input_geometry();
    }

    pub(crate) fn retry_with_profile(&mut self, profile_id: &str) {
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
        settings.device_profile = ProfileChoice::Manual {
            profile_id: profile_id.to_owned(),
        };
        if let Err(error) = self.repository.save_settings(entry, settings) {
            self.show_error("Could not save recovery profile", &error);
            return;
        }
        let command = match self.session.retry() {
            Ok(command) => command,
            Err(error) => {
                self.show_error("Could not retry game", &error);
                return;
            }
        };
        if let Err(error) = self.runtime.submit(command) {
            if let Some((session_id, attempt_id)) = self.session.active_ids() {
                let failed = SessionEvent::terminal_error(
                    session_id,
                    attempt_id,
                    "The emulator retry could not be started.",
                    error.message(),
                );
                let _ = self.session.apply_event(&failed);
            }
            self.show_error("Could not retry game", &error);
            return;
        }
        if let Screen::Gameplay(gameplay) = &mut self.screen {
            gameplay.pointer_events = false;
            gameplay.fast_forward = false;
        }
        self.clear_gameplay_input_geometry();
        self.latest_frame = None;
        self.game_texture = None;
        self.heap_recovery = None;
        self.runtime_diagnostics.clear();
        self.runtime_debug.reset_session();
    }
}
